use crate::{primitive_audio::{self, PrimitiveAudioProducer, PrimitiveAudioReciever}, traps::Trap, uart::UART};

pub const RAM_SIZE: usize = 64 * 1024 * 1024;

pub const RAM_ADDRESS: u32 = 0x80000000;
pub const RAM_ADDRESS_END: u32 = RAM_ADDRESS + RAM_SIZE as u32 - 1;

pub const UART_REGION_SIZE: usize = 0x100;
pub const UART_ADDRESS: u32 = 0x10000000;
pub const UART_ADDRESS_END: u32 = 0x10000000 + UART_REGION_SIZE as u32 - 1;
pub const PRIMITIVE_AUDIO_ADDRESS: u32 = 0x10000200;


pub struct MMU {
    memory: Box<[u8]>,
    pub uart: UART,
    pub audio: PrimitiveAudioReciever
}


impl MMU {
    pub fn new() -> (Self, PrimitiveAudioProducer) {
        let v = vec![0; RAM_SIZE];
        let (audio_res, audio_prod) = primitive_audio::create_primitive_audio_pair();
        (MMU {
            memory: v.into(),
            uart: UART::new(),
            audio: audio_res,
        }, audio_prod)
    }
    pub fn fetch_word(&self, address: u32) -> Result<u32, Trap> {
        //if address % 2 != 0 {
        //    return Err(Trap {
        //        trap_type: crate::traps::TrapType::InstructionAddressMisaligned,
        //        value: address,
        //    });
        //}
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let mem_adr = (address - RAM_ADDRESS) as usize;
                Ok(u32::from_le_bytes(
                    self.memory[mem_adr..mem_adr + 4].try_into().unwrap(),
                ))
            }
            UART_ADDRESS..=UART_ADDRESS_END => {
                todo!()
            }
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!()
            }
            _ => Err(Trap {
                tcause: crate::traps::TrapType::InstructionAccessFault,
                tval: address,
            }),
        }
    }
    pub fn read_word(&self, address: u32) -> Result<u32, Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let mem_adr = (address - RAM_ADDRESS) as usize;
                Ok(u32::from_le_bytes(
                    self.memory[mem_adr..mem_adr + 4].try_into().unwrap(),
                ))
            }
            UART_ADDRESS..=UART_ADDRESS_END => {
                todo!()
            }
            PRIMITIVE_AUDIO_ADDRESS => {
                Ok(self.audio.get_size())
            }
            _ => Err(Trap {
                tcause: crate::traps::TrapType::LoadAccessFault,
                tval: address,
            }),
        }
    }
    pub fn read_halfword(&self, address: u32) -> Result<u16, Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let mem_adr = (address - RAM_ADDRESS) as usize;
                Ok(u16::from_le_bytes(
                    self.memory[mem_adr..mem_adr + 2].try_into().unwrap(),
                ))
            }
            UART_ADDRESS..=UART_ADDRESS_END => {
                todo!()
            }
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!()
            }
            _ => Err(Trap {
                tcause: crate::traps::TrapType::LoadAccessFault,
                tval: address,
            }),
        }
    }
    pub fn read_byte(&self, address: u32) -> Result<u8, Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => Ok(self.memory[(address - RAM_ADDRESS) as usize]),
            UART_ADDRESS..=UART_ADDRESS_END => {
                todo!()
            }
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!()
            }
            _ => Err(Trap {
                tcause: crate::traps::TrapType::LoadAccessFault,
                tval: address,
            }),
        }
    }
    pub fn write_word(&mut self, address: u32, word: u32) -> Result<(), Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let word = word.to_le_bytes();
                let mem_adr = (address - RAM_ADDRESS) as usize;
                self.memory[mem_adr..mem_adr + 4].copy_from_slice(&word);
                Ok(())
            }
            UART_ADDRESS..=UART_ADDRESS_END => {
                todo!()
            }
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!()
            }
            _ => Err(Trap {
                tcause: crate::traps::TrapType::StoreAccessFault,
                tval: address,
            }),
        }
    }
    pub fn write_halfword(&mut self, address: u32, halfword: u16) -> Result<(), Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let word = halfword.to_le_bytes();
                let mem_adr = (address - RAM_ADDRESS) as usize;
                self.memory[mem_adr..mem_adr + 2].copy_from_slice(&word);
                Ok(())
            }
            UART_ADDRESS..=UART_ADDRESS_END => {
                todo!()
            }
            PRIMITIVE_AUDIO_ADDRESS => {
                Ok(self.audio.write(halfword as i16))
            }
            _ => Err(Trap {
                tcause: crate::traps::TrapType::StoreAccessFault,
                tval: address,
            }),
        }
    }
    pub fn write_byte(&mut self, address: u32, byte: u8) -> Result<(), Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                self.memory[(address - RAM_ADDRESS) as usize] = byte;
                Ok(())
            }
            UART_ADDRESS => {
                //println!("Writing 0x{:02x} to UART", byte);
                self.uart.emu_push(byte);
                Ok(())
            }
            UART_ADDRESS..=UART_ADDRESS_END => {
                todo!()
            }
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!()
            }
            _ => Err(Trap {
                tcause: crate::traps::TrapType::StoreAccessFault,
                tval: address,
            }),
        }
    }
    pub fn write_raw_to_ram(&mut self, address: u32, byte: u8) -> bool {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                self.memory[(address - RAM_ADDRESS) as usize] = byte;
                true
            }
            _ => false,
        }
    }
    pub fn read_raw_from_ram(&self, address: u32) -> Option<u8> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => Some(self.memory[(address - RAM_ADDRESS) as usize]),
            _ => None,
        }
    }
}
