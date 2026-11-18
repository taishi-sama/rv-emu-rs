use std::{
    fs,
    io::{self, Read, Write},
    process,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use bus::MemoryBus;
use clap::Parser;
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

//The kernel image needs Sv32 and 128 MiB of RAM; the initramfs does not fit in less.
const DEFAULT_DTB: &str = "rv-emu-rs-sv32.dtb";
//Booted when no image path is passed on the command line.
const DEFAULT_IMAGE: &str = "./bad_apple/target/riscv32imac-unknown-none-elf/release/bad_apple";

//Command line. Help, version, unknown-option diagnostics and the `--` separator come from
//clap; the boot mode is picked from the flag, the number of paths and the image contents.
#[derive(Parser)]
#[command(
    name = "rv-emu-rs",
    version,
    about = "RISC-V RV32GC emulator",
    after_help = "\
Boot modes:
  <image>                  an ELF executable or a flat kernel Image; the device tree
                           defaults to rv-emu-rs-sv32.dtb
  <image> <dtb>            flat kernel Image with an explicit device tree
  <fw> <Image> <dtb>       OpenSBI fw_dynamic.bin firmware, kernel Image and device tree
  --payload <blob> [dtb]   flat OpenSBI fw_payload.bin blob

Without an image the bad_apple demo build is used."
)]
struct Args {
    /// Treat <image> as a flat OpenSBI fw_payload.bin blob
    #[arg(long)]
    payload: bool,

    /// Run headless without the framebuffer window
    #[arg(long)]
    no_display: bool,

    /// Image to boot: an ELF, a flat kernel Image, or (with --payload) a fw_payload.bin
    /// blob; the firmware boot takes all three paths <fw_dynamic.bin> <Image> <dtb>.
    /// Defaults to the bad_apple demo build.
    #[arg(value_name = "IMAGE [DTB]", num_args = 0..=3)]
    paths: Vec<String>,
}

fn exit_with(message: impl std::fmt::Display) -> ! {
    eprintln!("error: {message}");
    process::exit(1);
}

fn read_or_exit(path: &str, what: &str) -> Vec<u8> {
    fs::read(path).unwrap_or_else(|err| {
        eprintln!("error: could not read the {what} \"{path}\": {err}");
        process::exit(1);
    })
}

fn main() {
    let args = Args::parse();
    //The payload boot has no firmware-triple form.
    if args.payload && !(1..=2).contains(&args.paths.len()) {
        exit_with(format!(
            "--payload takes <fw_payload.bin> [dtb], got {} argument(s)",
            args.paths.len()
        ));
    }
    let path = args
        .paths
        .first()
        .cloned()
        .unwrap_or_else(|| DEFAULT_IMAGE.to_string());
    //A host audio device is optional: without one the emulator keeps running and the audio
    //ring simply fills up and drops samples.
    let audio_output = match OutputStream::try_default() {
        Ok((stream, stream_handle)) => Some((stream, stream_handle)),
        Err(err) => {
            eprintln!("audio: no output stream available ({err}); continuing without sound");
            None
        }
    };
    let contents = read_or_exit(&path, "image");

    let (bus, audio) = MemoryBus::new();
    //Keep a host-side handle after the bus is moved into the MMU.
    let screen_buffer = bus.framebuffer.shared();
    if let Some((_stream, stream_handle)) = &audio_output {
        if let Err(err) = stream_handle.play_raw(audio.convert_samples::<f32>()) {
            eprintln!("audio: could not start playback ({err}); continuing without sound");
        }
    }
    //Four boot paths: --payload <fw_payload.bin> [dtb] (OpenSBI and the kernel in one blob),
    //<fw_dynamic.bin> <Image> <dtb> (M-mode firmware, boot info record in a2), a flat kernel
    //Image booted in M-mode, or an ELF. The DTB defaults to rv-emu-rs-sv32.dtb when omitted.
    let emu = if args.payload {
        if contents.starts_with(b"\x7fELF") {
            exit_with("the payload must be a flat binary (fw_payload.bin), not an ELF");
        }
        let dtb_path = args.paths.get(1).map(String::as_str).unwrap_or(DEFAULT_DTB);
        let dtb = read_or_exit(dtb_path, "device tree");
        println!("Loading payload \"{path}\" with dtb \"{dtb_path}\"");
        emulator::Emulator::from_payload(contents, dtb, MMU::new(bus))
    } else if args.paths.len() == 3 {
        let kernel_path = &args.paths[1];
        let dtb_path = &args.paths[2];
        if contents.starts_with(b"\x7fELF") {
            exit_with("the firmware must be a flat binary (fw_dynamic.bin), not an ELF");
        }
        let kernel = read_or_exit(kernel_path, "kernel image");
        let dtb = read_or_exit(dtb_path, "device tree");
        println!("Loading firmware \"{path}\", kernel \"{kernel_path}\", dtb \"{dtb_path}\"");
        emulator::Emulator::from_firmware(contents, kernel, dtb, MMU::new(bus))
    } else if contents.starts_with(b"\x7fELF") {
        println!("Loading elf \"{path}\"");
        emulator::Emulator::from_elf(contents, MMU::new(bus))
    } else {
        //A flat kernel Image; the device tree defaults to the one built from rv-emu-rs-sv32.dts.
        let dtb_path = args
            .paths
            .get(1)
            .cloned()
            .unwrap_or_else(|| DEFAULT_DTB.to_string());
        let dtb = read_or_exit(&dtb_path, "device tree");
        println!("Loading image \"{path}\" with dtb \"{dtb_path}\"");
        emulator::Emulator::from_image(contents, dtb, MMU::new(bus))
    };
    let emu = match emu {
        Ok(emu) => emu,
        Err(err) => {
            eprintln!("error: could not boot \"{path}\": {err}");
            process::exit(1);
        }
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

    if args.no_display {
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
    println!("Start executing...");
    //Feed stdin into the guest's UART receive buffer.
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
    //Draining in batches keeps stdout locked once per batch and keeps multi-byte UTF-8 written
    //by the guest intact, instead of printing each byte as a separate char.
    let mut out = Vec::new();
    //Execute batch of instructions
    const DEVICE_UPDATE_INTERVAL: usize = 1024;
    let step_batch = DEVICE_UPDATE_INTERVAL;
    loop {
        if stop.load(Ordering::Acquire) {
            break;
        }
        while let Ok(byte) = rx.try_recv() {
            emu.cpu.mmu.bus.uart.host_push(byte);
        }
        match emu.cpu.handle_interrupt() {
            Err(err) => {
                println!("Encountered error! {err:?}");
                break;
            }
            // If CPU did not process any interrupts, sleep for in practice minimally possible time to not burn CPU cycles.
            Ok(false) if emu.cpu.state.wfi => thread::sleep(Duration::from_micros(10)),
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
        out.clear();
        while let Some(byte) = emu.cpu.mmu.bus.uart.try_get_byte() {
            out.push(byte);
        }
        if !out.is_empty() {
            let _ = io::stdout().write_all(&out);
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
    //Drain whatever the guest wrote after the last poll so its final output is not lost.
    let tail: Vec<u8> = std::iter::from_fn(|| emu.cpu.mmu.bus.uart.try_get_byte()).collect();
    if !tail.is_empty() {
        let _ = io::stdout().write_all(&tail);
        let _ = io::stdout().flush();
    }
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
        let mut emu = emulator::Emulator::from_elf(elf_contents, MMU::new(bus)).unwrap();
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
