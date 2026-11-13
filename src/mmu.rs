use crate::{
    bus::{AtomicAccess, MemoryBus},
    traps::Trap,
};

// Future place for properly implementing memory translation(Sv32), currently it is just flat map for all modes.
pub struct MMU {
    pub bus: MemoryBus,
}

impl MMU {
    pub fn new(bus: MemoryBus) -> Self {
        MMU { bus }
    }
    //The core's view of the CLINT, which is a shared device on the bus.
    pub fn mtime(&mut self) -> u64 {
        self.bus.clint.mtime()
    }
    pub fn msip(&mut self) -> bool {
        self.bus.clint.msip()
    }
    pub fn mtip(&mut self) -> bool {
        self.bus.clint.mtip()
    }
    pub fn fetch_halfword(&mut self, address: u32) -> Result<u16, Trap> {
        self.bus.fetch_halfword(address)
    }
    //Reads also take &mut self so that translation caches (e.g. a TLB) can be added later
    //without changing the callers.
    pub fn read_word_atomically(
        &mut self,
        address: u32,
        access: AtomicAccess,
    ) -> Result<u32, Trap> {
        self.bus.read_word_atomically(address, access)
    }
    pub fn write_word_atomically(&mut self, address: u32, word: u32) -> Result<(), Trap> {
        self.bus.write_word_atomically(address, word)
    }
    pub fn read_word(&mut self, address: u32) -> Result<u32, Trap> {
        self.bus.read_word(address)
    }
    pub fn read_halfword(&mut self, address: u32) -> Result<u16, Trap> {
        self.bus.read_halfword(address)
    }
    pub fn read_byte(&mut self, address: u32) -> Result<u8, Trap> {
        self.bus.read_byte(address)
    }
    pub fn write_word(&mut self, address: u32, word: u32) -> Result<(), Trap> {
        self.bus.write_word(address, word)
    }
    pub fn write_halfword(&mut self, address: u32, halfword: u16) -> Result<(), Trap> {
        self.bus.write_halfword(address, halfword)
    }
    pub fn write_byte(&mut self, address: u32, byte: u8) -> Result<(), Trap> {
        self.bus.write_byte(address, byte)
    }
}
