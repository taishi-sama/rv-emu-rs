use crate::{
    bus::{RAM_ADDRESS, RAM_SIZE},
    cpu::CPU,
    elf_analyzer::elf_setup_bus,
    mmu::MMU,
    syscon::SysconEvent,
};

//The device tree is copied into the top 16 KiB of RAM. Linux reserves the blob itself, so the
//device tree needs no reserved-memory node. Linux is entered with a1 pointing here.
pub const DTB_SIZE: u32 = 0x4000;
pub const DTB_ADDRESS: u32 = RAM_ADDRESS + RAM_SIZE as u32 - DTB_SIZE;

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
    //Boots a flat kernel Image following the RISC-V Linux boot protocol: the first instruction is
    //at the base of RAM, a0 holds the hart id and a1 points at the device tree.
    pub fn from_image(image: Vec<u8>, dtb: Vec<u8>, mut mmu: MMU) -> Self {
        assert!(
            dtb.len() <= DTB_SIZE as usize,
            "device tree does not fit in the top 16 KiB of RAM"
        );
        assert!(
            dtb.starts_with(&[0xd0, 0x0d, 0xfe, 0xed]),
            "not a flattened device tree blob"
        );
        assert!(
            image.len() as u32 <= DTB_ADDRESS - RAM_ADDRESS,
            "kernel image overlaps the device tree"
        );
        for (offset, byte) in image.iter().enumerate() {
            assert!(
                mmu.bus.write_raw_to_ram(RAM_ADDRESS + offset as u32, *byte),
                "kernel image does not fit in RAM"
            );
        }
        for (offset, byte) in dtb.iter().enumerate() {
            assert!(
                mmu.bus.write_raw_to_ram(DTB_ADDRESS + offset as u32, *byte),
                "device tree does not fit in the top 16 KiB of RAM"
            );
        }
        let mut cpu = CPU::new(mmu);
        cpu.state.pc = RAM_ADDRESS;
        cpu.state.set_x(10, 0);
        cpu.state.set_x(11, DTB_ADDRESS);
        Emulator { cpu }
    }
    pub fn take_syscon_event(&mut self) -> Option<SysconEvent> {
        self.cpu.mmu.bus.syscon.take_event()
    }
}
