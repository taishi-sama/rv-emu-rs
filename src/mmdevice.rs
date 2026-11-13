use crate::traps::TrapType;

// Shared interface of memory mapped devices.
#[allow(non_camel_case_types)]
pub trait MMdevice {
    const MEMORY_REGION_SIZE: u32;

    //Instruction fetch only ever reads 16 bits; the CPU fetches the upper half itself.
    fn fetch_halfword(&mut self, offset: u32) -> Result<u16, TrapType>;
    //Atomic accesses are only meaningful for RAM; other devices report an access fault.
    fn read_word_atomically(&mut self, offset: u32) -> Result<u32, TrapType>;
    fn write_word_atomically(&mut self, offset: u32, word: u32) -> Result<(), TrapType>;
    fn read_word(&mut self, offset: u32) -> Result<u32, TrapType>;
    fn write_word(&mut self, offset: u32, word: u32) -> Result<(), TrapType>;
    fn read_halfword(&mut self, offset: u32) -> Result<u16, TrapType>;
    fn write_halfword(&mut self, offset: u32, halfword: u16) -> Result<(), TrapType>;
    fn read_byte(&mut self, offset: u32) -> Result<u8, TrapType>;
    fn write_byte(&mut self, offset: u32, byte: u8) -> Result<(), TrapType>;
}
