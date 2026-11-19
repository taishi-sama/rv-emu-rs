use crate::{
    bus::{MemoryBus, RAM_ADDRESS},
    cpu::CPU,
    elf_analyzer::elf_setup_bus,
    mmu::MMU,
    ram::RAM_SIZE,
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

//The boot info record must never reach the device tree; this is a compile-time layout check.
const _: () = assert!(FW_DYNAMIC_INFO_ADDRESS + FW_DYNAMIC_INFO_SIZE <= DTB_ADDRESS);

//U-Boot's RISC-V QEMU S-mode build is linked at 0x80400000. It relocates itself
//to the top of RAM before booting the kernel, which frees this address again.
//The value mirrors CONFIG_TEXT_BASE from board/emulation/qemu-riscv/Kconfig in
//U-Boot's qemu-riscv32_smode_defconfig; a board link-address change must update it.
pub const UBOOT_LOAD_ADDRESS: u32 = RAM_ADDRESS + 0x0040_0000;

pub struct Emulator {
    pub cpu: CPU,
}

impl Emulator {
    pub fn from_elf(elf: Vec<u8>, mut mmu: MMU) -> Result<Self, String> {
        let pc = elf_setup_bus(elf, &mut mmu.bus)?;
        let mut cpu = CPU::new(mmu);
        cpu.state.pc = pc;
        Ok(Emulator { cpu })
    }
    //Boots a flat kernel Image following the RISC-V Linux boot protocol: the first instruction is
    //at the base of RAM, a0 holds the hart id and a1 points at the device tree.
    pub fn from_image(image: Vec<u8>, dtb: Vec<u8>, mut mmu: MMU) -> Result<Self, String> {
        Self::check_dtb(&dtb)?;
        if image.len() as u64 + RAM_ADDRESS as u64 > DTB_ADDRESS as u64 {
            return Err(format!(
                "kernel image is {} byte(s) and overlaps the device tree",
                image.len()
            ));
        }
        for (offset, byte) in image.iter().enumerate() {
            if !mmu.bus.write_raw_to_ram(RAM_ADDRESS + offset as u32, *byte) {
                return Err("kernel image does not fit in RAM".to_string());
            }
        }
        Self::write_dtb(&dtb, &mut mmu.bus)?;
        let mut cpu = CPU::new(mmu);
        cpu.state.pc = RAM_ADDRESS;
        cpu.state.set_x(10, 0);
        cpu.state.set_x(11, DTB_ADDRESS);
        Ok(Emulator { cpu })
    }
    //Boots a firmware (OpenSBI fw_dynamic.bin) in M-mode at the base of RAM. The kernel Image is
    //loaded at the offset given by its RISC-V boot header. A fw_dynamic_info record in a2 tells
    //the firmware to jump to the kernel in S-mode with a1 (the device tree) unchanged.
    pub fn from_firmware(
        firmware: Vec<u8>,
        kernel: Vec<u8>,
        dtb: Vec<u8>,
        mmu: MMU,
    ) -> Result<Self, String> {
        let text_offset = Self::image_text_offset(&kernel)?;
        if text_offset >= RAM_SIZE as u32 {
            return Err(format!(
                "kernel text offset 0x{text_offset:08x} is outside RAM"
            ));
        }
        Self::from_firmware_at(firmware, kernel, dtb, RAM_ADDRESS + text_offset, mmu)
    }
    //Boots OpenSBI fw_dynamic with a flat S-mode payload at U-Boot's link address. U-Boot has
    //no RISC-V Image header, so the address comes from its board configuration rather than from
    //the image itself.
    pub fn from_uboot(
        firmware: Vec<u8>,
        uboot: Vec<u8>,
        dtb: Vec<u8>,
        mmu: MMU,
    ) -> Result<Self, String> {
        Self::from_firmware_at(firmware, uboot, dtb, UBOOT_LOAD_ADDRESS, mmu)
    }
    //Loads OpenSBI at the base of RAM and a flat payload at an explicit address. OpenSBI enters
    //the payload in S-mode at that address with a0 = hart id and a1 = device tree.
    fn from_firmware_at(
        firmware: Vec<u8>,
        payload: Vec<u8>,
        dtb: Vec<u8>,
        payload_address: u32,
        mut mmu: MMU,
    ) -> Result<Self, String> {
        Self::check_dtb(&dtb)?;
        let payload_offset = payload_address.wrapping_sub(RAM_ADDRESS);
        if payload_address < RAM_ADDRESS || payload_offset >= RAM_SIZE as u32 {
            return Err(format!(
                "payload address 0x{payload_address:08x} is outside RAM"
            ));
        }
        //Only the loaded firmware image is checked here. OpenSBI's heap and scratch sit
        //above _fw_end, inside the <0x50000 runtime region it reserves in the FDT; the
        //supported payload addresses (0x80400000) leave several MiB of headroom.
        if firmware.len() as u64 > u64::from(payload_offset) {
            return Err("firmware overlaps the payload".to_string());
        }
        if payload_address as u64 + payload.len() as u64 > FW_DYNAMIC_INFO_ADDRESS as u64 {
            return Err("payload overlaps the boot info record".to_string());
        }
        for (offset, byte) in firmware.iter().enumerate() {
            if !mmu.bus.write_raw_to_ram(RAM_ADDRESS + offset as u32, *byte) {
                return Err("firmware does not fit in RAM".to_string());
            }
        }
        for (offset, byte) in payload.iter().enumerate() {
            if !mmu
                .bus
                .write_raw_to_ram(payload_address + offset as u32, *byte)
            {
                return Err("payload does not fit in RAM".to_string());
            }
        }
        let boot_info = [
            FW_DYNAMIC_MAGIC,
            FW_DYNAMIC_VERSION,
            payload_address,
            FW_DYNAMIC_NEXT_MODE_S,
            0, // options
            0, // boot hart
        ];
        for (index, word) in boot_info.iter().enumerate() {
            for (offset, byte) in word.to_le_bytes().iter().enumerate() {
                let address = FW_DYNAMIC_INFO_ADDRESS + index as u32 * 4 + offset as u32;
                if !mmu.bus.write_raw_to_ram(address, *byte) {
                    return Err("boot info record does not fit in RAM".to_string());
                }
            }
        }
        Self::write_dtb(&dtb, &mut mmu.bus)?;
        let mut cpu = CPU::new(mmu);
        cpu.state.pc = RAM_ADDRESS;
        cpu.state.set_x(10, 0);
        cpu.state.set_x(11, DTB_ADDRESS);
        cpu.state.set_x(12, FW_DYNAMIC_INFO_ADDRESS);
        Ok(Emulator { cpu })
    }
    //Boots an OpenSBI fw_payload image: the firmware and the Linux Image in a single blob, laid
    //out by FW_PAYLOAD_OFFSET (0x400000 for rv32), so the loader needs no offset arithmetic.
    //The firmware starts in M-mode at the base of RAM, receives the device tree in a1 and jumps
    //to the kernel it embeds; a2 is unused.
    pub fn from_payload(payload: Vec<u8>, dtb: Vec<u8>, mut mmu: MMU) -> Result<Self, String> {
        Self::check_dtb(&dtb)?;
        if payload.len() as u64 + RAM_ADDRESS as u64 > DTB_ADDRESS as u64 {
            return Err(format!(
                "payload is {} byte(s) and overlaps the device tree",
                payload.len()
            ));
        }
        for (offset, byte) in payload.iter().enumerate() {
            if !mmu.bus.write_raw_to_ram(RAM_ADDRESS + offset as u32, *byte) {
                return Err("payload does not fit in RAM".to_string());
            }
        }
        Self::write_dtb(&dtb, &mut mmu.bus)?;
        let mut cpu = CPU::new(mmu);
        cpu.state.pc = RAM_ADDRESS;
        cpu.state.set_x(10, 0);
        cpu.state.set_x(11, DTB_ADDRESS);
        Ok(Emulator { cpu })
    }
    fn check_dtb(dtb: &[u8]) -> Result<(), String> {
        if dtb.len() > DTB_SIZE as usize {
            return Err(format!(
                "device tree is {} byte(s), but only {DTB_SIZE} fit in RAM",
                dtb.len()
            ));
        }
        if !dtb.starts_with(&[0xd0, 0x0d, 0xfe, 0xed]) {
            return Err("not a flattened device tree blob".to_string());
        }
        Ok(())
    }
    fn write_dtb(dtb: &[u8], bus: &mut MemoryBus) -> Result<(), String> {
        for (offset, byte) in dtb.iter().enumerate() {
            if !bus.write_raw_to_ram(DTB_ADDRESS + offset as u32, *byte) {
                return Err("device tree does not fit in the top 16 KiB of RAM".to_string());
            }
        }
        Ok(())
    }
    //The RISC-V boot header (version 2) stores the image load offset from the start of RAM as a
    //little-endian u64 at offset 8 and the magic "RISCV" at offset 48.
    fn image_text_offset(image: &[u8]) -> Result<u32, String> {
        if image.len() < 64 {
            return Err("kernel image is too small to contain a RISC-V boot header".to_string());
        }
        if &image[48..56] != b"RISCV\0\0\0" {
            return Err("kernel image has no RISC-V boot header".to_string());
        }
        let offset = u64::from_le_bytes(image[8..16].try_into().unwrap());
        if offset > u32::MAX as u64 || offset & 0xfff != 0 {
            return Err(format!(
                "kernel text offset 0x{offset:x} is not a 32-bit page-aligned value"
            ));
        }
        Ok(offset as u32)
    }
    pub fn take_syscon_event(&mut self) -> Option<SysconEvent> {
        self.cpu.mmu.bus.syscon.take_event()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MemoryBus;

    fn test_dtb() -> Vec<u8> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/rv-emu-rs-sv32.dtb"))
            .expect("the repo device tree must exist")
    }

    fn boot_error(result: Result<Emulator, String>) -> String {
        match result {
            Ok(_) => panic!("boot should have been rejected"),
            Err(err) => err,
        }
    }

    fn boot_info_word(bus: &crate::bus::MemoryBus, index: u32) -> u32 {
        let mut bytes = [0u8; 4];
        for (offset, byte) in bytes.iter_mut().enumerate() {
            *byte = bus
                .read_raw_from_ram(FW_DYNAMIC_INFO_ADDRESS + index * 4 + offset as u32)
                .expect("boot info record must be in RAM");
        }
        u32::from_le_bytes(bytes)
    }

    #[test]
    fn uboot_loads_at_its_linked_address() {
        let (bus, _audio) = MemoryBus::new();
        let emu = Emulator::from_uboot(vec![0x11; 4], vec![0x22; 8], test_dtb(), MMU::new(bus))
            .expect("U-Boot boot must set up");
        assert_eq!(UBOOT_LOAD_ADDRESS, 0x8040_0000);
        assert_eq!(emu.cpu.state.pc, RAM_ADDRESS);
        assert_eq!(emu.cpu.state.get_x(10), 0);
        assert_eq!(emu.cpu.state.get_x(11), DTB_ADDRESS);
        assert_eq!(emu.cpu.state.get_x(12), FW_DYNAMIC_INFO_ADDRESS);
        for offset in 0..4 {
            assert_eq!(
                emu.cpu.mmu.bus.read_raw_from_ram(RAM_ADDRESS + offset),
                Some(0x11)
            );
        }
        for offset in 0..8 {
            assert_eq!(
                emu.cpu
                    .mmu
                    .bus
                    .read_raw_from_ram(UBOOT_LOAD_ADDRESS + offset),
                Some(0x22)
            );
        }
        let expected = [
            FW_DYNAMIC_MAGIC,
            FW_DYNAMIC_VERSION,
            UBOOT_LOAD_ADDRESS,
            FW_DYNAMIC_NEXT_MODE_S,
            0,
            0,
        ];
        for (index, word) in expected.iter().enumerate() {
            assert_eq!(boot_info_word(&emu.cpu.mmu.bus, index as u32), *word);
        }
    }

    #[test]
    fn firmware_boot_rejects_an_overlapping_payload() {
        let (bus, _audio) = MemoryBus::new();
        let err = boot_error(Emulator::from_firmware_at(
            vec![0x11; 8],
            vec![0x22; 4],
            test_dtb(),
            RAM_ADDRESS + 4,
            MMU::new(bus),
        ));
        assert_eq!(err, "firmware overlaps the payload");
    }

    #[test]
    fn firmware_boot_rejects_a_payload_outside_ram() {
        let (bus, _audio) = MemoryBus::new();
        let err = boot_error(Emulator::from_firmware_at(
            Vec::new(),
            vec![0x22; 4],
            test_dtb(),
            RAM_ADDRESS - 4,
            MMU::new(bus),
        ));
        assert_eq!(
            err,
            format!("payload address 0x{:08x} is outside RAM", RAM_ADDRESS - 4)
        );
    }

    #[test]
    fn firmware_boot_rejects_a_payload_over_the_boot_info() {
        let (bus, _audio) = MemoryBus::new();
        let err = boot_error(Emulator::from_firmware_at(
            Vec::new(),
            vec![0x22; 4],
            test_dtb(),
            FW_DYNAMIC_INFO_ADDRESS - 2,
            MMU::new(bus),
        ));
        assert_eq!(err, "payload overlaps the boot info record");
    }
}
