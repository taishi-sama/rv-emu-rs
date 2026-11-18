use crate::traps::TrapType;

// Shared interface of memory mapped devices.
//
// The defaults model an ordinary MMIO device: every access faults, and each device overrides
// only the widths it actually implements. Instruction fetch always faults (code is not
// executable MMIO) and atomics only exist for RAM.
#[allow(non_camel_case_types)]
pub trait MMdevice {
    const MEMORY_REGION_SIZE: u32;

    //Instruction fetch only ever reads 16 bits; the CPU fetches the upper half itself.
    fn fetch_halfword(&mut self, _offset: u32) -> Result<u16, TrapType> {
        Err(TrapType::InstructionAccessFault)
    }
    //Atomic accesses are only meaningful for RAM; other devices report an access fault.
    fn read_word_atomically(&mut self, _offset: u32) -> Result<u32, TrapType> {
        Err(TrapType::LoadAccessFault)
    }
    fn write_word_atomically(&mut self, _offset: u32, _word: u32) -> Result<(), TrapType> {
        Err(TrapType::StoreAccessFault)
    }
    fn read_word(&mut self, _offset: u32) -> Result<u32, TrapType> {
        Err(TrapType::LoadAccessFault)
    }
    fn write_word(&mut self, _offset: u32, _word: u32) -> Result<(), TrapType> {
        Err(TrapType::StoreAccessFault)
    }
    fn read_halfword(&mut self, _offset: u32) -> Result<u16, TrapType> {
        Err(TrapType::LoadAccessFault)
    }
    fn write_halfword(&mut self, _offset: u32, _halfword: u16) -> Result<(), TrapType> {
        Err(TrapType::StoreAccessFault)
    }
    fn read_byte(&mut self, _offset: u32) -> Result<u8, TrapType> {
        Err(TrapType::LoadAccessFault)
    }
    fn write_byte(&mut self, _offset: u32, _byte: u8) -> Result<(), TrapType> {
        Err(TrapType::StoreAccessFault)
    }
}
