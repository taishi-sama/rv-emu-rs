use crate::{
    clint::{Clint, CLINT_ADDRESS, CLINT_ADDRESS_END},
    primitive_audio::{self, PrimitiveAudioProducer, PrimitiveAudioReceiver},
    syscon::{Syscon, SYSCON_ADDRESS, SYSCON_ADDRESS_END},
    traps::{Trap, TrapType},
    uart::{UART, UART_ADDRESS, UART_ADDRESS_END},
};

pub const RAM_SIZE: usize = 64 * 1024 * 1024;

pub const RAM_ADDRESS: u32 = 0x80000000;
pub const RAM_ADDRESS_END: u32 = RAM_ADDRESS + RAM_SIZE as u32 - 1;

pub const PRIMITIVE_AUDIO_ADDRESS: u32 = 0x10000200;

//Selects the fault type an atomic access reports: LR.W faults as a load, SC.W and AMOs as stores.
#[derive(Clone, Copy)]
pub enum AtomicAccess {
    Load,
    Store,
}

//The physical address space: RAM, the CLINT and the memory mapped devices. Everything here takes
//physical addresses; translating them from the addresses issued by the CPU is the MMU's job.
pub struct MemoryBus {
    ram: Box<[u8]>,
    pub uart: UART,
    pub audio: PrimitiveAudioReceiver,
    pub clint: Clint,
    pub syscon: Syscon,
}

impl MemoryBus {
    pub fn new() -> (Self, PrimitiveAudioProducer) {
        let v = vec![0; RAM_SIZE];
        let (audio_res, audio_prod) = primitive_audio::create_primitive_audio_pair();
        (
            MemoryBus {
                ram: v.into(),
                uart: UART::new(),
                audio: audio_res,
                clint: Clint::new(),
                syscon: Syscon::new(),
            },
            audio_prod,
        )
    }
    // Need to implement only fetch 16 bits of instructions, CPU will fetch upper 16 bits for normal instructions after it decodes length.
    pub fn fetch_halfword(&self, address: u32) -> Result<u16, Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let mem_adr = (address - RAM_ADDRESS) as usize;
                let bytes = self.ram.get(mem_adr..mem_adr + 2).ok_or(Trap {
                    tcause: TrapType::InstructionAccessFault,
                    tval: address,
                })?;
                Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
            }
            UART_ADDRESS..=UART_ADDRESS_END => Err(Trap {
                tcause: TrapType::InstructionAccessFault,
                tval: address,
            }),
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!("Instruction fetch from the audio device is not implemented")
            }
            _ => Err(Trap {
                tcause: TrapType::InstructionAccessFault,
                tval: address,
            }),
        }
    }
    //Atomic accesses only exist for RAM words: MMIO devices are not atomically addressable.
    fn atomic_word_offset(address: u32, access: AtomicAccess) -> Result<usize, Trap> {
        if address & 0b11 != 0 {
            return Err(Trap {
                tcause: match access {
                    AtomicAccess::Load => TrapType::LoadAddressMisaligned,
                    AtomicAccess::Store => TrapType::StoreAddressMisaligned,
                },
                tval: address,
            });
        }
        if matches!(address, UART_ADDRESS..=UART_ADDRESS_END) {
            return Err(Trap {
                tcause: match access {
                    AtomicAccess::Load => TrapType::LoadAccessFault,
                    AtomicAccess::Store => TrapType::StoreAccessFault,
                },
                tval: address,
            });
        }
        if address == PRIMITIVE_AUDIO_ADDRESS {
            todo!("Atomic access to the audio device is not implemented");
        }
        if !matches!(address, RAM_ADDRESS..=RAM_ADDRESS_END) {
            return Err(Trap {
                tcause: match access {
                    AtomicAccess::Load => TrapType::LoadAccessFault,
                    AtomicAccess::Store => TrapType::StoreAccessFault,
                },
                tval: address,
            });
        }
        Ok((address - RAM_ADDRESS) as usize)
    }
    pub fn read_word_atomically(&self, address: u32, access: AtomicAccess) -> Result<u32, Trap> {
        let mem_adr = Self::atomic_word_offset(address, access)?;
        Ok(u32::from_le_bytes(
            self.ram[mem_adr..mem_adr + 4].try_into().unwrap(),
        ))
    }
    pub fn write_word_atomically(&mut self, address: u32, word: u32) -> Result<(), Trap> {
        let mem_adr = Self::atomic_word_offset(address, AtomicAccess::Store)?;
        self.ram[mem_adr..mem_adr + 4].copy_from_slice(&word.to_le_bytes());
        Ok(())
    }
    pub fn read_word(&self, address: u32) -> Result<u32, Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let mem_adr = (address - RAM_ADDRESS) as usize;
                let bytes = self.ram.get(mem_adr..mem_adr + 4).ok_or(Trap {
                    tcause: TrapType::LoadAccessFault,
                    tval: address,
                })?;
                Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
            }
            UART_ADDRESS..=UART_ADDRESS_END => Err(Trap {
                tcause: TrapType::LoadAccessFault,
                tval: address,
            }),
            PRIMITIVE_AUDIO_ADDRESS => Ok(self.audio.get_size()),
            CLINT_ADDRESS..=CLINT_ADDRESS_END => self.clint.read_word(address),
            SYSCON_ADDRESS..=SYSCON_ADDRESS_END => self.syscon.read_word(address),
            _ => Err(Trap {
                tcause: TrapType::LoadAccessFault,
                tval: address,
            }),
        }
    }
    pub fn read_halfword(&self, address: u32) -> Result<u16, Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let mem_adr = (address - RAM_ADDRESS) as usize;
                let bytes = self.ram.get(mem_adr..mem_adr + 2).ok_or(Trap {
                    tcause: TrapType::LoadAccessFault,
                    tval: address,
                })?;
                Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
            }
            UART_ADDRESS..=UART_ADDRESS_END => Err(Trap {
                tcause: TrapType::LoadAccessFault,
                tval: address,
            }),
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!("Audio halfword reads are not implemented")
            }
            _ => Err(Trap {
                tcause: TrapType::LoadAccessFault,
                tval: address,
            }),
        }
    }
    pub fn read_byte(&mut self, address: u32) -> Result<u8, Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => Ok(self.ram[(address - RAM_ADDRESS) as usize]),
            UART_ADDRESS..=UART_ADDRESS_END => self.uart.read_byte(address),
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!("Audio byte reads are not implemented")
            }
            _ => Err(Trap {
                tcause: TrapType::LoadAccessFault,
                tval: address,
            }),
        }
    }
    pub fn write_word(&mut self, address: u32, word: u32) -> Result<(), Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let mem_adr = (address - RAM_ADDRESS) as usize;
                let bytes = self.ram.get_mut(mem_adr..mem_adr + 4).ok_or(Trap {
                    tcause: TrapType::StoreAccessFault,
                    tval: address,
                })?;
                bytes.copy_from_slice(&word.to_le_bytes());
                Ok(())
            }
            UART_ADDRESS..=UART_ADDRESS_END => Err(Trap {
                tcause: TrapType::StoreAccessFault,
                tval: address,
            }),
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!("Audio word writes are not implemented")
            }
            CLINT_ADDRESS..=CLINT_ADDRESS_END => self.clint.write_word(address, word),
            SYSCON_ADDRESS..=SYSCON_ADDRESS_END => self.syscon.write_word(address, word),
            _ => Err(Trap {
                tcause: TrapType::StoreAccessFault,
                tval: address,
            }),
        }
    }
    pub fn write_halfword(&mut self, address: u32, halfword: u16) -> Result<(), Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                let mem_adr = (address - RAM_ADDRESS) as usize;
                let bytes = self.ram.get_mut(mem_adr..mem_adr + 2).ok_or(Trap {
                    tcause: TrapType::StoreAccessFault,
                    tval: address,
                })?;
                bytes.copy_from_slice(&halfword.to_le_bytes());
                Ok(())
            }
            UART_ADDRESS..=UART_ADDRESS_END => Err(Trap {
                tcause: TrapType::StoreAccessFault,
                tval: address,
            }),
            PRIMITIVE_AUDIO_ADDRESS => Ok(self.audio.write(halfword as i16)),
            _ => Err(Trap {
                tcause: TrapType::StoreAccessFault,
                tval: address,
            }),
        }
    }
    pub fn write_byte(&mut self, address: u32, byte: u8) -> Result<(), Trap> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                self.ram[(address - RAM_ADDRESS) as usize] = byte;
                Ok(())
            }
            UART_ADDRESS..=UART_ADDRESS_END => self.uart.write_byte(address, byte),
            PRIMITIVE_AUDIO_ADDRESS => {
                todo!("Audio byte writes are not implemented")
            }
            _ => Err(Trap {
                tcause: TrapType::StoreAccessFault,
                tval: address,
            }),
        }
    }
    //Loads an ELF image without going through address translation.
    pub fn write_raw_to_ram(&mut self, address: u32, byte: u8) -> bool {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => {
                self.ram[(address - RAM_ADDRESS) as usize] = byte;
                true
            }
            _ => false,
        }
    }
    pub fn read_raw_from_ram(&self, address: u32) -> Option<u8> {
        match address {
            RAM_ADDRESS..=RAM_ADDRESS_END => Some(self.ram[(address - RAM_ADDRESS) as usize]),
            _ => None,
        }
    }
}
