use std::{
    env, fs::File, io::{self, Read}
};

pub mod cpu;
pub mod elf_analyzer;
pub mod emulator;
pub mod mmu;
pub mod ops_decode;
pub mod traps;
pub mod uart;
pub mod errors;
pub mod manual_debugger;
//pub mod gdb;

fn main() {
    //let mut m = mmu::MMU::default();
    //let mut path = "./bad_apple/target/riscv32i-unknown-none-elf/release/bad_apple".to_string();
    let mut path = "./bad_apple/target/riscv32i-unknown-none-elf/release/bad_apple".to_string();
    
    //let path = "./test_asm/target/testtraps.s.elf";
    //let mut path: String = "./test_asm/target/testadd.s.elf".to_string();
    if let Some(p) = env::args().skip(1).next() {
        path = p
    }
    println!("Loading elf \"{}\"", path);
    let mut elf_file = File::open(path).unwrap();
    let mut elf_contents = vec![];
    elf_file.read_to_end(&mut elf_contents).unwrap();

    let mut emu = emulator::Emulator::from_elf(elf_contents);
    let mut stdin = io::stdin();
    println!("Start executing...");
    let reg_names = [
        "zo", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4",
        "a5", "a6", "a7", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4",
        "t5", "t6",
    ];

    let display = false;
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
                    println!("Executed instruction 0x{:0>8x} on address {:x}", instr, pc);
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
    use std::{fs::File, io::Read, path::Path};

    use crate::emulator;

    pub fn run_arch_tests(path: &Path) {
        let mut elf_file = File::open(path).unwrap();
        let mut elf_contents = vec![];
        elf_file.read_to_end(&mut elf_contents).unwrap();
        let mut emu = emulator::Emulator::from_elf(elf_contents);
        loop {
            let res = emu.cpu.execute_instruction();
            match res {
                Ok(_) => (),
                Err(_) => break,
            }
        }
        let mut v = vec![];
        while let Some(x) = emu.cpu.mmu.uart.try_get_byte() {
            //print!("{}", x as char)
            v.push(x)
        }
        for i in v.chunks_exact(2) {
            assert_eq!(
                i[0],
                b'y',
                "Test {} failed in {}",
                i[1],
                path.to_str().unwrap()
            )
            //println!("Test {}: {}", i[1], i[0] as char)
        }
    }
    #[test]
    pub fn test_add() {
        let path = "./test_asm/target/testadd.s.elf";
        run_arch_tests(Path::new(path));
    }
    #[test]
    pub fn test_addi() {
        let path = "./test_asm/target/testaddi.s.elf";
        run_arch_tests(Path::new(path));
    }
    #[test]
    pub fn test_sll() {
        let path = "./test_asm/target/testsll.s.elf";
        run_arch_tests(Path::new(path));
    }
    #[test]
    pub fn test_bltu() {
        let path = "./test_asm/target/testbltu.s.elf";
        run_arch_tests(Path::new(path));
    }
}
