use crate::{
    bus::RAM_ADDRESS, cpu::CPU, elf_analyzer::elf_setup_bus, mmu::MMU, ram::RAM_SIZE,
    syscon::SysconEvent,
};

//The device tree is copied into the top 16 KiB of RAM. Linux reserves the blob itself, so the
//device tree needs no reserved-memory node. Linux is entered with a1 pointing here.
pub const DTB_SIZE: u32 = 0x4000;
pub const DTB_ADDRESS: u32 = RAM_ADDRESS + RAM_SIZE as u32 - DTB_SIZE;

//OpenSBI fw_dynamic receives a fw_dynamic_info record in a2 instead of having the jump address
//and FDT address compiled in. The record lives just below the device tree; it is only read
//during early firmware startup, so nothing needs to reserve it afterwards.
const FW_DYNAMIC_INFO_ADDRESS: u32 = DTB_ADDRESS - 0x1000;
const FW_DYNAMIC_INFO_SIZE: u32 = 24;
const FW_DYNAMIC_MAGIC: u32 = 0x4942_534f;
const FW_DYNAMIC_VERSION: u32 = 2;
const FW_DYNAMIC_NEXT_MODE_S: u32 = 1;

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
        Self::check_dtb(&dtb);
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
    //Boots a firmware (OpenSBI fw_dynamic.bin) in M-mode at the base of RAM. The kernel Image is
    //loaded at the offset given by its RISC-V boot header. A fw_dynamic_info record in a2 tells
    //the firmware to jump to the kernel in S-mode with a1 (the device tree) unchanged.
    pub fn from_firmware(firmware: Vec<u8>, kernel: Vec<u8>, dtb: Vec<u8>, mut mmu: MMU) -> Self {
        Self::check_dtb(&dtb);
        let text_offset = Self::image_text_offset(&kernel);
        assert!(
            text_offset < RAM_SIZE as u32,
            "kernel text offset is outside RAM"
        );
        let kernel_address = RAM_ADDRESS + text_offset;
        assert!(
            firmware.len() as u32 <= text_offset,
            "firmware overlaps the kernel image"
        );
        assert!(
            kernel_address as usize + kernel.len() <= FW_DYNAMIC_INFO_ADDRESS as usize,
            "kernel image overlaps the boot info record"
        );
        assert!(
            FW_DYNAMIC_INFO_ADDRESS + FW_DYNAMIC_INFO_SIZE <= DTB_ADDRESS,
            "boot info record overlaps the device tree"
        );
        for (offset, byte) in firmware.iter().enumerate() {
            assert!(
                mmu.bus.write_raw_to_ram(RAM_ADDRESS + offset as u32, *byte),
                "firmware does not fit in RAM"
            );
        }
        for (offset, byte) in kernel.iter().enumerate() {
            assert!(
                mmu.bus
                    .write_raw_to_ram(kernel_address + offset as u32, *byte),
                "kernel image does not fit in RAM"
            );
        }
        let boot_info = [
            FW_DYNAMIC_MAGIC,
            FW_DYNAMIC_VERSION,
            kernel_address,
            FW_DYNAMIC_NEXT_MODE_S,
            0, // options
            0, // boot hart
        ];
        for (index, word) in boot_info.iter().enumerate() {
            for (offset, byte) in word.to_le_bytes().iter().enumerate() {
                assert!(
                    mmu.bus.write_raw_to_ram(
                        FW_DYNAMIC_INFO_ADDRESS + index as u32 * 4 + offset as u32,
                        *byte
                    ),
                    "boot info record does not fit in RAM"
                );
            }
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
        cpu.state.set_x(12, FW_DYNAMIC_INFO_ADDRESS);
        Emulator { cpu }
    }
    fn check_dtb(dtb: &[u8]) {
        assert!(
            dtb.len() <= DTB_SIZE as usize,
            "device tree does not fit in the top 16 KiB of RAM"
        );
        assert!(
            dtb.starts_with(&[0xd0, 0x0d, 0xfe, 0xed]),
            "not a flattened device tree blob"
        );
    }
    //The RISC-V boot header (version 2) stores the image load offset from the start of RAM as a
    //little-endian u64 at offset 8 and the magic "RISCV" at offset 48.
    fn image_text_offset(image: &[u8]) -> u32 {
        assert!(
            image.len() >= 64,
            "kernel image is too small to contain a RISC-V boot header"
        );
        assert!(
            &image[48..56] == b"RISCV\0\0\0",
            "kernel image has no RISC-V boot header"
        );
        let offset = u64::from_le_bytes(image[8..16].try_into().unwrap());
        assert!(
            offset <= u32::MAX as u64 && offset & 0xfff == 0,
            "kernel text offset is not a 32-bit page-aligned value"
        );
        offset as u32
    }
    pub fn take_syscon_event(&mut self) -> Option<SysconEvent> {
        self.cpu.mmu.bus.syscon.take_event()
    }
}
