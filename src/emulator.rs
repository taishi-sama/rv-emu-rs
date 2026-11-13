use crate::{cpu::CPU, elf_analyzer::elf_setup_bus, mmu::MMU, syscon::SysconEvent};

pub struct Emulator {
    pub cpu: CPU,
}

impl Emulator {
    //TODO: make fallable interface
    pub fn from_elf(elf: Vec<u8>, mut mmu: MMU) -> Self {
        let pc = elf_setup_bus(elf, &mut mmu.bus);
        let mut cpu = CPU::new(mmu);
        cpu.state.pc = pc;
        Emulator { cpu }
    }
    pub fn take_syscon_event(&mut self) -> Option<SysconEvent> {
        self.cpu.mmu.bus.syscon.take_event()
    }
}
