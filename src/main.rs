use std::{
    env,
    fs::File,
    io::{self, Read, Write},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use bus::MemoryBus;
use mmu::MMU;
use rodio::{OutputStream, Source};
use syscon::SysconEvent;

pub mod bus;
pub mod clint;
pub mod cpu;
pub mod display;
pub mod elf_analyzer;
pub mod emulator;
pub mod errors;
#[cfg(feature = "plus_FD")]
mod fpu;
pub mod manual_debugger;
pub mod mmdevice;
pub mod mmu;
pub mod ops_decode;
pub mod primitive_audio;
pub mod ram;
pub mod simple_framebuffer;
pub mod syscon;
pub mod traps;
pub mod uart;
//pub mod gdb;

fn main() {
    //let mut m = mmu::MMU::default();
    let mut path = "./bad_apple/target/riscv32imac-unknown-none-elf/release/bad_apple".to_string();
    //let mut path = "./bad_apple/target/riscv32i-unknown-none-elf/debug/bad_apple".to_string();

    //let path = "./test_asm/target/testtraps.s.elf";
    //let mut path: String = "./test_asm/target/testadd.s.elf".to_string();
    let mut args: Vec<String> = env::args().skip(1).collect();
    //The kernel image needs Sv32 and 128 MiB of RAM; the initramfs does not fit in less.
    const DEFAULT_DTB: &str = "rv-emu-rs-sv32.dtb";
    let no_display = args.iter().any(|arg| arg == "--no-display");
    args.retain(|arg| arg != "--no-display");
    let payload = args.iter().any(|arg| arg == "--payload");
    args.retain(|arg| arg != "--payload");
    //Validate the payload invocation before opening anything, so a missing or extra argument
    //reports the real problem instead of an unrelated file error.
    if payload {
        assert!(
            (1..=2).contains(&args.len()),
            "--payload takes <fw_payload.bin> [dtb], got {args:?}"
        );
    }
    if let Some(p) = args.first() {
        path = p.clone()
    }
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let mut image_file = File::open(&path).unwrap();
    let mut contents = vec![];
    image_file.read_to_end(&mut contents).unwrap();

    let (bus, audio) = MemoryBus::new();
    //Keep a host-side handle after the bus is moved into the MMU.
    let screen_buffer = bus.framebuffer.shared();
    stream_handle.play_raw(audio.convert_samples()).unwrap();
    //Four boot paths: --payload <fw_payload.bin> [dtb] (OpenSBI and the kernel in one blob),
    //<fw_dynamic.bin> <Image> <dtb> (M-mode firmware, boot info record in a2), a flat kernel
    //Image booted in M-mode, or an ELF. The DTB defaults to rv-emu-rs-sv32.dtb when omitted.
    let emu = if payload {
        assert!(
            !contents.starts_with(b"\x7fELF"),
            "the payload must be a flat binary (fw_payload.bin), not an ELF"
        );
        let dtb_path = args.get(1).map(String::as_str).unwrap_or(DEFAULT_DTB);
        let dtb = std::fs::read(dtb_path)
            .unwrap_or_else(|err| panic!("could not read the device tree \"{dtb_path}\": {err}"));
        println!("Loading payload \"{path}\" with dtb \"{dtb_path}\"");
        emulator::Emulator::from_payload(contents, dtb, MMU::new(bus))
    } else if args.len() >= 3 {
        let kernel_path = &args[1];
        let dtb_path = &args[2];
        assert!(
            !contents.starts_with(b"\x7fELF"),
            "the firmware must be a flat binary (fw_dynamic.bin), not an ELF"
        );
        let kernel = std::fs::read(kernel_path).unwrap_or_else(|err| {
            panic!("could not read the kernel image \"{kernel_path}\": {err}")
        });
        let dtb = std::fs::read(dtb_path)
            .unwrap_or_else(|err| panic!("could not read the device tree \"{dtb_path}\": {err}"));
        println!("Loading firmware \"{path}\", kernel \"{kernel_path}\", dtb \"{dtb_path}\"");
        emulator::Emulator::from_firmware(contents, kernel, dtb, MMU::new(bus))
    } else if contents.starts_with(b"\x7fELF") {
        println!("Loading elf \"{path}\"");
        emulator::Emulator::from_elf(contents, MMU::new(bus))
    } else {
        //A flat kernel Image; the device tree defaults to the one built from rv-emu-rs-sv32.dts.
        let dtb_path = args
            .get(1)
            .cloned()
            .unwrap_or_else(|| DEFAULT_DTB.to_string());
        println!("Loading image \"{path}\" with dtb \"{dtb_path}\"");
        let dtb = std::fs::read(&dtb_path)
            .unwrap_or_else(|err| panic!("could not read the device tree \"{dtb_path}\": {err}"));
        emulator::Emulator::from_image(contents, dtb, MMU::new(bus))
    };
    let stop_emulator = Arc::new(AtomicBool::new(false));
    let emulator_done = Arc::new(AtomicBool::new(false));
    let instruction_counter = Arc::new(AtomicU32::new(0));
    let emulator_thread = {
        let stop_emulator = Arc::clone(&stop_emulator);
        let emulator_done = Arc::clone(&emulator_done);
        let instruction_counter = Arc::clone(&instruction_counter);
        thread::Builder::new()
            .name("emulator".to_string())
            .spawn(move || {
                let _done = EmulatorDoneGuard(emulator_done);
                run_emulator(emu, stop_emulator, instruction_counter);
            })
            .expect("could not spawn emulator thread")
    };

    if no_display {
        println!("Display disabled (--no-display)");
        let _ = emulator_thread.join();
        return;
    }

    match display::run(screen_buffer, emulator_done, instruction_counter) {
        display::DisplayExit::WindowClosed => {
            stop_emulator.store(true, Ordering::Release);
        }
        display::DisplayExit::EmulatorFinished | display::DisplayExit::InitFailed => {}
    }
    let _ = emulator_thread.join();
}

//Set the emulator-done flag on every exit path, including a panic in the emulator thread.
struct EmulatorDoneGuard(Arc<AtomicBool>);

impl Drop for EmulatorDoneGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn run_emulator(
    mut emu: emulator::Emulator,
    stop: Arc<AtomicBool>,
    instruction_counter: Arc<AtomicU32>,
) {
    let mut stdin = io::stdin();
    println!("Start executing...");
    let display = false;
    //Feed stdin into the guest's UART receive buffer while running normally. In display
    //(single-step) mode stdin is used for stepping instead.
    let uart_input = if display {
        None
    } else {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut stdin = io::stdin();
            let mut byte = [0u8; 1];
            loop {
                match stdin.read(&mut byte) {
                    Ok(0) => break,
                    Ok(_) => {
                        if tx.send(byte[0]).is_err() {
                            break;
                        }
                    }
                    Err(err) if err.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        });
        Some(rx)
    };
    //Execute batch of instructions
    const DEVICE_UPDATE_INTERVAL: usize = 1024;
    let step_batch = if display { 1 } else { DEVICE_UPDATE_INTERVAL };
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        //let display = true;
        let pc = emu.cpu.state.pc;
        if display {
            println!("pc: {:0>8x}", pc);
            let _ = stdin.read(&mut [0u8]).unwrap();
        }
        if let Some(input) = &uart_input {
            while let Ok(byte) = input.try_recv() {
                emu.cpu.mmu.bus.uart.host_push(byte);
            }
        }
        match emu.cpu.handle_interrupt() {
            Err(err) => {
                println!("Encountered error! {err:?}");
                break;
            }
            Ok(false) if emu.cpu.state.wfi => {
                thread::sleep(Duration::from_micros(10));
            } // If CPU did not process any interrupts, sleep for in practice minimally possible time to not burn CPU cycles.
            Ok(_) => {}
        }
        let mut failed = false;
        let mut executed = 0u32;
        for _ in 0..step_batch {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            //WFI means the hart retires nothing; go back to handle_interrupt immediately instead of
            //burning the rest of the batch on no-op steps.
            if emu.cpu.state.wfi {
                break;
            }
            if let Err(err) = emu.cpu.step() {
                println!("Encountered error! {err:?}");
                failed = true;
                break;
            }
            executed += 1;
        }
        instruction_counter.fetch_add(executed, Ordering::Relaxed);
        let mut wrote = false;
        while let Some(x) = emu.cpu.mmu.bus.uart.try_get_byte() {
            print!("{}", x as char);
            wrote = true;
        }
        if wrote {
            let _ = io::stdout().flush();
        }
        //Drain before acting on a fatal error or syscon request so the last output is not lost.
        if failed {
            break;
        }
        if let Some(event) = emu.take_syscon_event() {
            match event {
                SysconEvent::PowerOff => println!("Poweroff requested"),
                //TODO: restart from the loaded ELF instead of exiting.
                SysconEvent::Reboot => println!("Reboot requested"),
            }
            break;
        }
    }
    println!("UART output buffer:");
    let mut v = vec![];
    while let Some(x) = emu.cpu.mmu.bus.uart.try_get_byte() {
        //print!("{}", x as char)
        v.push(x)
    }

    for i in v.chunks_exact(2) {
        println!("Test {}: {}", i[1], i[0] as char)
    }
    println!();
}

#[cfg(test)]
mod test {
    use std::{fs::File, io::Read, path::PathBuf, process::Command, sync::OnceLock};

    use crate::{bus::MemoryBus, emulator, mmu::MMU};

    const MAX_STEPS: usize = 10_000_000;
    const END_MARKER: [u8; 2] = [b'z', 0];

    fn test_asm_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_asm")
    }

    fn build_arch_tests() {
        static BUILD: OnceLock<Result<(), String>> = OnceLock::new();
        let result = BUILD.get_or_init(|| {
            let mut command = Command::new("sh");
            command.arg("build.sh").current_dir(test_asm_dir());
            //The FP test sources need an F/D-capable assembler, so skip them when the FPU is
            //compiled out.
            command.env(
                "RV_EMU_PLUS_FD",
                if cfg!(feature = "plus_FD") { "1" } else { "0" },
            );
            let output = command
                .output()
                .map_err(|err| format!("could not run test_asm/build.sh: {err}"))?;
            if output.status.success() {
                Ok(())
            } else {
                Err(format!(
                    "test_asm/build.sh failed with {}\n{}",
                    output.status,
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        });
        if let Err(err) = result {
            panic!("{err}");
        }
    }

    fn run_arch_test(elf_name: &str, expected: &[u8]) {
        build_arch_tests();

        let path = test_asm_dir().join("target").join(elf_name);
        let mut elf_contents = Vec::new();
        File::open(&path)
            .unwrap_or_else(|err| {
                panic!(
                    "cannot open {}: {err}; run test_asm/build.sh",
                    path.display()
                )
            })
            .read_to_end(&mut elf_contents)
            .unwrap();

        let (bus, _audio) = MemoryBus::new();
        let mut emu = emulator::Emulator::from_elf(elf_contents, MMU::new(bus));
        let mut steps = 0;
        while steps < MAX_STEPS {
            if emu.cpu.handle_interrupt().is_err() {
                break;
            }
            if emu.cpu.step().is_err() {
                break;
            }
            steps += 1;
            if emu.take_syscon_event().is_some() {
                break;
            }
        }
        assert!(
            steps < MAX_STEPS,
            "{} did not stop after {MAX_STEPS} instructions",
            path.display()
        );

        let output: Vec<u8> = std::iter::from_fn(|| emu.cpu.mmu.bus.uart.try_get_byte()).collect();
        let preview = &output[..output.len().min(64)];
        assert!(
            output.ends_with(&END_MARKER),
            "{} stopped before reaching stop_by_fault ({} byte(s): {preview:?})",
            path.display(),
            output.len()
        );

        let results = &output[..output.len() - END_MARKER.len()];
        assert_eq!(
            results.len() % 2,
            0,
            "{} produced a truncated test result",
            path.display()
        );

        let failed: Vec<String> = results
            .chunks_exact(2)
            .filter(|result| result[0] != b'y')
            .map(|result| format!("#{}: {}", result[1], result[0] as char))
            .collect();
        assert!(
            failed.is_empty(),
            "{} reported {} failed test(s): {}",
            path.display(),
            failed.len(),
            failed.join(", ")
        );

        let observed: Vec<u8> = results.chunks_exact(2).map(|result| result[1]).collect();
        assert_eq!(
            observed,
            expected,
            "{} reported test numbers other than the ones its source defines",
            path.display()
        );
        eprintln!("{}: {} test(s) passed", elf_name, observed.len());
    }

    macro_rules! arch_tests {
        ($($name:ident: ($elf:literal, $expected:expr $(,)?)),* $(,)?) => {
            const REGISTERED_ELVES: &[&str] = &[$($elf),*];

            $(
                #[test]
                fn $name() {
                    run_arch_test($elf, $expected);
                }
            )*
        };
    }

    arch_tests! {
        test_add: ("testadd.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_addi: ("testaddi.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_amoadd: ("testamoadd.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_amoand: ("testamoand.s.elf", &[2, 3, 4, 5]),
        test_amomax: ("testamomax.s.elf", &[2, 3, 4, 5, 6, 7]),
        test_amomaxu: ("testamomaxu.s.elf", &[2, 3, 4, 5, 6, 7]),
        test_amomin: ("testamomin.s.elf", &[2, 3, 4, 5, 6, 7]),
        test_amominu: ("testamominu.s.elf", &[2, 3, 4, 5, 6, 7]),
        test_amoor: ("testamoor.s.elf", &[2, 3, 4, 5]),
        test_amoswap: ("testamoswap.s.elf", &[2, 3, 4, 5]),
        test_amoxor: ("testamoxor.s.elf", &[2, 3, 4, 5]),
        test_and: ("testand.s.elf", &[2, 3, 4, 5, 6, 7, 8]),
        test_andi: ("testandi.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_auipc: ("testauipc.s.elf", &[2, 3, 4, 5, 6]),
        test_beq: ("testbeq.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_bge: ("testbge.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
        test_bgeu: ("testbgeu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
        test_blt: ("testblt.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_bltu: ("testbltu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_bne: ("testbne.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_cadd: ("testcadd.s.elf", &[2, 3]),
        test_caddi: ("testcaddi.s.elf", &[2, 3, 4, 5]),
        test_caddi16sp: ("testcaddi16sp.s.elf", &[2, 3]),
        test_caddi4spn: ("testcaddi4spn.s.elf", &[2, 3]),
        test_cand: ("testcand.s.elf", &[2, 3]),
        test_candi: ("testcandi.s.elf", &[2, 3, 4]),
        test_cbeqz: ("testcbeqz.s.elf", &[2, 3, 4]),
        test_cbnez: ("testcbnez.s.elf", &[2, 3, 4]),
        test_cebreak: ("testcebreak.s.elf", &[2, 3]),
        test_cj: ("testcj.s.elf", &[2, 3]),
        test_cjal: ("testcjal.s.elf", &[2]),
        test_cjalr: ("testcjalr.s.elf", &[2, 3, 4]),
        test_cjr: ("testcjr.s.elf", &[2, 3]),
        test_cli: ("testcli.s.elf", &[2, 3, 4]),
        test_clui: ("testclui.s.elf", &[2, 3, 4]),
        test_clw: ("testclw.s.elf", &[2, 3, 4]),
        test_clwsp: ("testclwsp.s.elf", &[2, 3]),
        test_cmv: ("testcmv.s.elf", &[2, 3]),
        test_cnop: ("testcnop.s.elf", &[2, 3]),
        test_cor: ("testcor.s.elf", &[2, 3]),
        test_cslli: ("testcslli.s.elf", &[2, 3, 4]),
        test_csrai: ("testcsrai.s.elf", &[2, 3]),
        test_csrli: ("testcsrli.s.elf", &[2, 3]),
        test_csub: ("testcsub.s.elf", &[2, 3]),
        test_csw: ("testcsw.s.elf", &[2, 3, 4]),
        test_cswsp: ("testcswsp.s.elf", &[2, 3]),
        test_cxor: ("testcxor.s.elf", &[2, 3]),
        test_mcsr: (
            "testmcsr.s.elf",
            &[
                2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
                25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37,
            ],
        ),
        test_div: ("testdiv.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10]),
        test_divu: ("testdivu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10]),
        test_fence: ("testfence.s.elf", &[2, 3, 4]),
        test_jal: ("testjal.s.elf", &[2, 3, 4, 5]),
        test_jalr: ("testjalr.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_lb: ("testlb.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_lbu: ("testlbu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_lh: ("testlh.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_lhu: ("testlhu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_lui: ("testlui.s.elf", &[2, 3, 4, 5, 6, 7]),
        test_lw: ("testlw.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_lrsc: (
            "testlrsc.s.elf",
            &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17],
        ),
        test_mul: ("testmul.s.elf", &[2, 3, 4, 5, 6, 7, 30, 31, 32, 33, 34, 35, 36, 37]),
        test_mulh: ("testmulh.s.elf", &[2, 3, 4, 5, 6, 7, 30, 31, 32, 33, 34, 35]),
        test_mulhsu: ("testmulhsu.s.elf", &[2, 3, 4, 5, 6, 7, 30, 31, 32, 33, 34, 35]),
        test_mulhu: ("testmulhu.s.elf", &[2, 3, 4, 5, 6, 7, 30, 31, 32, 33, 34, 35]),
        test_or: ("testor.s.elf", &[2, 3, 4, 5, 6, 7, 8]),
        test_ori: ("testori.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_rem: ("testrem.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10]),
        test_remu: ("testremu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10]),
        test_sb: ("testsb.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_sh: ("testsh.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_sll: (
            "testsll.s.elf",
            &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20],
        ),
        test_slli: ("testslli.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_slt: ("testslt.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_slti: ("testslti.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_sltiu: ("testsltiu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_sltu: ("testsltu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_sra: (
            "testsra.s.elf",
            &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
        ),
        test_srai: ("testsrai.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_srl: (
            "testsrl.s.elf",
            &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
        ),
        test_srli: ("testsrli.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_sub: ("testsub.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]),
        test_sw: ("testsw.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_traps: (
            "testtraps.s.elf",
            &[
                2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
                25, 26, 27, 28,
            ],
        ),
        test_xor: ("testxor.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_xori: ("testxori.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10]),
    }

    //F/D tests only run when the FPU is compiled in, but their sources are always registered so
    //the build and source-consistency checks stay identical for both feature sets.
    macro_rules! fp_arch_tests {
        ($($name:ident: ($elf:literal, $expected:expr $(,)?)),* $(,)?) => {
            const FP_ELVES: &[&str] = &[$($elf),*];

            $(
                #[cfg(feature = "plus_FD")]
                #[test]
                fn $name() {
                    run_arch_test($elf, $expected);
                }
            )*
        };
    }

    fp_arch_tests! {
        test_cfp: (
            "testcfp.s.elf",
            &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
                25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40],
        ),
        test_fpcsr: ("testfpcsr.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_fpadd_d: ("testfpadd_d.s.elf", &[2, 3, 4, 5, 6, 7, 8]),
        test_fpsub_d: ("testfpsub_d.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_fpmul_d: ("testfpmul_d.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10]),
        test_fpdiv_d: ("testfpdiv_d.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_fpsqrt_d: ("testfpsqrt_d.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
        test_fpsgnj_d: ("testfpsgnj_d.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]),
        test_fpminmax_d: (
            "testfpminmax_d.s.elf",
            &[
                2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38,
            ],
        ),
        test_fpcompare_d: ("testfpcompare_d.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]),
        test_fpcvt_d: (
            "testfpcvt_d.s.elf",
            &[
                2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42,
            ],
        ),
        test_fpclass: ("testfpclass.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21]),
        test_fpfma_d: ("testfpfma_d.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]),
        test_fpldst_d: ("testfpldst_d.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        test_fpnan: ("testfpnan.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13]),
        test_fpround: ("testfpround.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24]),
        test_fpexcept: ("testfpexcept.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25]),
        test_fptraps: (
            "testfptraps.s.elf",
            &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24,
                25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
                46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66,
                67, 68, 69, 70, 71, 72],
        ),
        test_cfpldst: ("testcfpldst.s.elf", &[2, 3, 4, 5, 6, 7]),
        test_cfptraps: (
            "testcfptraps.s.elf",
            &[
                2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
                24, 25, 26, 27, 28, 29, 30, 31,
            ],
        ),
    }

    #[test]
    fn every_test_asm_source_is_registered() {
        for entry in std::fs::read_dir(test_asm_dir().join("tests")).unwrap() {
            let file_name = entry.unwrap().file_name().into_string().unwrap();
            if !file_name.starts_with("test") || !file_name.ends_with(".s") {
                continue;
            }
            let elf = format!("{file_name}.elf");
            assert!(
                REGISTERED_ELVES.contains(&elf.as_str()) || FP_ELVES.contains(&elf.as_str()),
                "{file_name} has no arch test registered"
            );
        }
    }

    #[test]
    fn every_registered_arch_test_has_a_source() {
        for elf in REGISTERED_ELVES.iter().chain(FP_ELVES) {
            let source = elf.trim_end_matches(".elf");
            assert!(
                test_asm_dir().join("tests").join(source).is_file(),
                "{elf} has no test_asm/tests/{source}"
            );
        }
    }
}
