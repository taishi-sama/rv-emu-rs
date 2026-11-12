use std::{
    env,
    fs::File,
    io::{self, Read},
};

use mmu::MMU;
use rodio::{OutputStream, Source};

pub mod cpu;
pub mod elf_analyzer;
pub mod emulator;
pub mod errors;
pub mod manual_debugger;
pub mod mmu;
pub mod ops_decode;
pub mod primitive_audio;
pub mod traps;
pub mod uart;
//pub mod gdb;

fn main() {
    //let mut m = mmu::MMU::default();
    let mut path = "./bad_apple/target/riscv32i-unknown-none-elf/release/bad_apple".to_string();
    //let mut path = "./bad_apple/target/riscv32i-unknown-none-elf/debug/bad_apple".to_string();

    //let path = "./test_asm/target/testtraps.s.elf";
    //let mut path: String = "./test_asm/target/testadd.s.elf".to_string();
    if let Some(p) = env::args().skip(1).next() {
        path = p
    }
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    println!("Loading elf \"{}\"", path);
    let mut elf_file = File::open(path).unwrap();
    let mut elf_contents = vec![];
    elf_file.read_to_end(&mut elf_contents).unwrap();

    let (mmu, audio) = MMU::new();
    stream_handle.play_raw(audio.convert_samples()).unwrap();
    let mut emu = emulator::Emulator::from_elf(elf_contents, mmu);
    let mut stdin = io::stdin();
    println!("Start executing...");
    let reg_names = [
        "zo", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
        "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
        "t5", "t6",
    ];

    let display = false;
    let mut counter: u128 = 1;
    loop {
        //let display = true;
        let pc = emu.cpu.pc;
        if display {
            println!("pc: {:0>8x}", pc);
            let _ = stdin.read(&mut [0u8]).unwrap();
        }
        let res = emu.cpu.step();
        while let Some(x) = emu.cpu.mmu.uart.try_get_byte() {
            print!("{}", x as char)
        }
        match res {
            Ok(instr) => {
                if display {
                    println!(
                        "Executed instruction 0x{:0>8x} on address {:x}; instruction no {}",
                        instr, pc, counter
                    );
                    for (ind, x) in emu.cpu.get_registers().into_iter().enumerate() {
                        let reg_name = reg_names[ind];
                        print!("{reg_name:<3}: {:^10x}; ", x);
                        if ind % 8 == 7 && ind != 0 {
                            println!()
                        }
                    }
                    println!()
                }
            }
            Err(trap) => {
                //println!("Achieved trap: {} on address {:x}", trap, emu.cpu.pc);
                println!("Encountered error! {trap:?}");
                break;
            }
        }

        // if counter%100000000 == 0 {
        //    emu.cpu.process_trap(traps::Trap { tcause: traps::TrapType::EnvironmentCallFromMMode, tval: 0 }).unwrap();
        // }

        counter += 1;
    }
    println!("UART output buffer:");
    let mut v = vec![];
    while let Some(x) = emu.cpu.mmu.uart.try_get_byte() {
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

    use crate::{emulator, mmu::MMU};

    const MAX_STEPS: usize = 10_000_000;
    const END_MARKER: [u8; 2] = [b'z', 0];

    fn test_asm_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_asm")
    }

    fn build_arch_tests() {
        static BUILD: OnceLock<Result<(), String>> = OnceLock::new();
        let result = BUILD.get_or_init(|| {
            let output = Command::new("sh")
                .arg("build.sh")
                .current_dir(test_asm_dir())
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
                panic!("cannot open {}: {err}; run test_asm/build.sh", path.display())
            })
            .read_to_end(&mut elf_contents)
            .unwrap();

        let (mmu, _audio) = MMU::new();
        let mut emu = emulator::Emulator::from_elf(elf_contents, mmu);
        let mut steps = 0;
        while steps < MAX_STEPS && emu.cpu.step().is_ok() {
            steps += 1;
        }
        assert!(
            steps < MAX_STEPS,
            "{} did not stop after {MAX_STEPS} instructions",
            path.display()
        );

        let output: Vec<u8> = std::iter::from_fn(|| emu.cpu.mmu.uart.try_get_byte()).collect();
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
        test_and: ("testand.s.elf", &[2, 3, 4, 5, 6, 7, 8]),
        test_andi: ("testandi.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_auipc: ("testauipc.s.elf", &[2, 3, 4, 5, 6]),
        test_beq: ("testbeq.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_bge: ("testbge.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
        test_bgeu: ("testbgeu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
        test_blt: ("testblt.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_bltu: ("testbltu.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_bne: ("testbne.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_csr: (
            "testcsr.s.elf",
            &[2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29],
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
        test_traps: ("testtraps.s.elf", &[2, 3, 4, 5, 6, 7]),
        test_xor: ("testxor.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9]),
        test_xori: ("testxori.s.elf", &[2, 3, 4, 5, 6, 7, 8, 9, 10]),
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
                REGISTERED_ELVES.contains(&elf.as_str()),
                "{file_name} has no arch test registered"
            );
        }
    }

    #[test]
    fn every_registered_arch_test_has_a_source() {
        for elf in REGISTERED_ELVES {
            let source = elf.trim_end_matches(".elf");
            assert!(
                test_asm_dir().join("tests").join(source).is_file(),
                "{elf} has no test_asm/tests/{source}"
            );
        }
    }
}
