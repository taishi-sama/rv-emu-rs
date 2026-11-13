use crate::{mmdevice::MMdevice, traps::TrapType};

pub const RAM_SIZE: usize = 64 * 1024 * 1024;

pub struct RAM {
    data: Box<[u8]>,
}

impl RAM {
    pub fn new() -> Self {
        RAM {
            data: vec![0; RAM_SIZE].into_boxed_slice(),
        }
    }
    //Raw access used by the ELF/Image loaders and the debugger.
    pub fn read_raw(&self, offset: u32) -> Option<u8> {
        self.data.get(offset as usize).copied()
    }
}

impl MMdevice for RAM {
    const MEMORY_REGION_SIZE: u32 = RAM_SIZE as u32;

    fn fetch_halfword(&mut self, offset: u32) -> Result<u16, TrapType> {
        let start = offset as usize;
        let bytes = self
            .data
            .get(start..start + 2)
            .ok_or(TrapType::InstructionAccessFault)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
    }
    fn read_word_atomically(&mut self, offset: u32) -> Result<u32, TrapType> {
        if offset & 0b11 != 0 {
            return Err(TrapType::LoadAddressMisaligned);
        }
        let start = offset as usize;
        let bytes = self
            .data
            .get(start..start + 4)
            .ok_or(TrapType::LoadAccessFault)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }
    fn write_word_atomically(&mut self, offset: u32, word: u32) -> Result<(), TrapType> {
        if offset & 0b11 != 0 {
            return Err(TrapType::StoreAddressMisaligned);
        }
        let start = offset as usize;
        let bytes = self
            .data
            .get_mut(start..start + 4)
            .ok_or(TrapType::StoreAccessFault)?;
        bytes.copy_from_slice(&word.to_le_bytes());
        Ok(())
    }
    fn read_word(&mut self, offset: u32) -> Result<u32, TrapType> {
        let start = offset as usize;
        let bytes = self
            .data
            .get(start..start + 4)
            .ok_or(TrapType::LoadAccessFault)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }
    fn write_word(&mut self, offset: u32, word: u32) -> Result<(), TrapType> {
        let start = offset as usize;
        let bytes = self
            .data
            .get_mut(start..start + 4)
            .ok_or(TrapType::StoreAccessFault)?;
        bytes.copy_from_slice(&word.to_le_bytes());
        Ok(())
    }
    fn read_halfword(&mut self, offset: u32) -> Result<u16, TrapType> {
        let start = offset as usize;
        let bytes = self
            .data
            .get(start..start + 2)
            .ok_or(TrapType::LoadAccessFault)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
    }
    fn write_halfword(&mut self, offset: u32, halfword: u16) -> Result<(), TrapType> {
        let start = offset as usize;
        let bytes = self
            .data
            .get_mut(start..start + 2)
            .ok_or(TrapType::StoreAccessFault)?;
        bytes.copy_from_slice(&halfword.to_le_bytes());
        Ok(())
    }
    fn read_byte(&mut self, offset: u32) -> Result<u8, TrapType> {
        self.data
            .get(offset as usize)
            .copied()
            .ok_or(TrapType::LoadAccessFault)
    }
    fn write_byte(&mut self, offset: u32, byte: u8) -> Result<(), TrapType> {
        *self
            .data
            .get_mut(offset as usize)
            .ok_or(TrapType::StoreAccessFault)? = byte;
        Ok(())
    }
}
