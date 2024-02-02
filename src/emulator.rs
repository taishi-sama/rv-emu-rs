


use crate::{cpu::CPU, elf_analyzer::elf_setup_mmu};

pub struct Emulator {
    pub cpu: CPU,
}

impl Emulator {
    //TODO: make fallable interface
    pub fn from_elf(elf: Vec<u8>) -> Self {
        let (mm, pc) = elf_setup_mmu(elf);
        let mut cpu = CPU::new(mm);
        cpu.pc = pc;
        Emulator { cpu }
    }
}
