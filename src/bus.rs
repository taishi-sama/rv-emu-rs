use crate::{
    clint::Clint,
    mmdevice::MMdevice,
    primitive_audio::{self, PrimitiveAudioProducer, PrimitiveAudioReceiver},
    ram::RAM,
    syscon::Syscon,
    traps::{Trap, TrapType},
    uart::UART,
};

//Physical memory map. Devices only know their region size; the bus owns the addresses.
pub const RAM_ADDRESS: u32 = 0x80000000;
pub const UART_ADDRESS: u32 = 0x10000000;
pub const PRIMITIVE_AUDIO_ADDRESS: u32 = 0x10000200;
pub const CLINT_ADDRESS: u32 = 0x11000000;
pub const SYSCON_ADDRESS: u32 = 0x11100000;

//Range patterns need a single constant bound, so each device's exclusive region end is named here.
const RAM_END: u32 = RAM_ADDRESS + RAM::MEMORY_REGION_SIZE;
const UART_END: u32 = UART_ADDRESS + UART::MEMORY_REGION_SIZE;
const AUDIO_END: u32 = PRIMITIVE_AUDIO_ADDRESS + PrimitiveAudioReceiver::MEMORY_REGION_SIZE;
const CLINT_END: u32 = CLINT_ADDRESS + Clint::MEMORY_REGION_SIZE;
const SYSCON_END: u32 = SYSCON_ADDRESS + Syscon::MEMORY_REGION_SIZE;

pub const RAM_ADDRESS_END: u32 = RAM_END - 1;

//Selects the fault type an atomic access reports: LR.W faults as a load, SC.W and AMOs as stores.
#[derive(Clone, Copy)]
pub enum AtomicAccess {
    Load,
    Store,
}

#[inline(always)]
fn with_address<T>(result: Result<T, TrapType>, address: u32) -> Result<T, Trap> {
    result.map_err(|tcause| Trap {
        tcause,
        tval: address,
    })
}

//The physical address space: RAM, the CLINT and the memory mapped devices. Everything here takes
//physical addresses; translating them from the addresses issued by the CPU is the MMU's job.
pub struct MemoryBus {
    ram: RAM,
    pub uart: UART,
    pub audio: PrimitiveAudioReceiver,
    pub clint: Clint,
    pub syscon: Syscon,
}

impl MemoryBus {
    pub fn new() -> (Self, PrimitiveAudioProducer) {
        let (audio_res, audio_prod) = primitive_audio::create_primitive_audio_pair();
        (
            MemoryBus {
                ram: RAM::new(),
                uart: UART::new(),
                audio: audio_res,
                clint: Clint::new(),
                syscon: Syscon::new(),
            },
            audio_prod,
        )
    }
    //TODO: implement PLIC and query it  
    pub fn external_interrupt_pending(&self) -> bool {
        false
    }
    // Need to implement only fetch 16 bits of instructions, CPU will fetch upper 16 bits for normal instructions after it decodes length.
    pub fn fetch_halfword(&mut self, address: u32) -> Result<u16, Trap> {
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.fetch_halfword(address - RAM_ADDRESS),
            UART_ADDRESS..UART_END => self.uart.fetch_halfword(address - UART_ADDRESS),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => {
                self.audio.fetch_halfword(address - PRIMITIVE_AUDIO_ADDRESS)
            }
            CLINT_ADDRESS..CLINT_END => self.clint.fetch_halfword(address - CLINT_ADDRESS),
            SYSCON_ADDRESS..SYSCON_END => self.syscon.fetch_halfword(address - SYSCON_ADDRESS),
            _ => Err(TrapType::InstructionAccessFault),
        };
        with_address(result, address)
    }
    pub fn read_word_atomically(
        &mut self,
        address: u32,
        access: AtomicAccess,
    ) -> Result<u32, Trap> {
        if address & 0b11 != 0 {
            return Err(Trap {
                tcause: match access {
                    AtomicAccess::Load => TrapType::LoadAddressMisaligned,
                    AtomicAccess::Store => TrapType::StoreAddressMisaligned,
                },
                tval: address,
            });
        }
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.read_word_atomically(address - RAM_ADDRESS),
            UART_ADDRESS..UART_END => self.uart.read_word_atomically(address - UART_ADDRESS),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => self
                .audio
                .read_word_atomically(address - PRIMITIVE_AUDIO_ADDRESS),
            CLINT_ADDRESS..CLINT_END => self.clint.read_word_atomically(address - CLINT_ADDRESS),
            SYSCON_ADDRESS..SYSCON_END => {
                self.syscon.read_word_atomically(address - SYSCON_ADDRESS)
            }
            _ => Err(match access {
                AtomicAccess::Load => TrapType::LoadAccessFault,
                AtomicAccess::Store => TrapType::StoreAccessFault,
            }),
        };
        result.map_err(|tcause| Trap {
            tcause: match access {
                AtomicAccess::Load => tcause,
                AtomicAccess::Store => match tcause {
                    TrapType::LoadAccessFault => TrapType::StoreAccessFault,
                    TrapType::LoadAddressMisaligned => TrapType::StoreAddressMisaligned,
                    _ => tcause,
                },
            },
            tval: address,
        })
    }
    pub fn write_word_atomically(&mut self, address: u32, word: u32) -> Result<(), Trap> {
        if address & 0b11 != 0 {
            return Err(Trap {
                tcause: TrapType::StoreAddressMisaligned,
                tval: address,
            });
        }
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.write_word_atomically(address - RAM_ADDRESS, word),
            UART_ADDRESS..UART_END => self
                .uart
                .write_word_atomically(address - UART_ADDRESS, word),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => self
                .audio
                .write_word_atomically(address - PRIMITIVE_AUDIO_ADDRESS, word),
            CLINT_ADDRESS..CLINT_END => self
                .clint
                .write_word_atomically(address - CLINT_ADDRESS, word),
            SYSCON_ADDRESS..SYSCON_END => self
                .syscon
                .write_word_atomically(address - SYSCON_ADDRESS, word),
            _ => Err(TrapType::StoreAccessFault),
        };
        with_address(result, address)
    }
    pub fn read_word(&mut self, address: u32) -> Result<u32, Trap> {
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.read_word(address - RAM_ADDRESS),
            UART_ADDRESS..UART_END => self.uart.read_word(address - UART_ADDRESS),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => {
                self.audio.read_word(address - PRIMITIVE_AUDIO_ADDRESS)
            }
            CLINT_ADDRESS..CLINT_END => self.clint.read_word(address - CLINT_ADDRESS),
            SYSCON_ADDRESS..SYSCON_END => self.syscon.read_word(address - SYSCON_ADDRESS),
            _ => Err(TrapType::LoadAccessFault),
        };
        with_address(result, address)
    }
    pub fn read_halfword(&mut self, address: u32) -> Result<u16, Trap> {
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.read_halfword(address - RAM_ADDRESS),
            UART_ADDRESS..UART_END => self.uart.read_halfword(address - UART_ADDRESS),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => {
                self.audio.read_halfword(address - PRIMITIVE_AUDIO_ADDRESS)
            }
            CLINT_ADDRESS..CLINT_END => self.clint.read_halfword(address - CLINT_ADDRESS),
            SYSCON_ADDRESS..SYSCON_END => self.syscon.read_halfword(address - SYSCON_ADDRESS),
            _ => Err(TrapType::LoadAccessFault),
        };
        with_address(result, address)
    }
    pub fn read_byte(&mut self, address: u32) -> Result<u8, Trap> {
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.read_byte(address - RAM_ADDRESS),
            UART_ADDRESS..UART_END => self.uart.read_byte(address - UART_ADDRESS),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => {
                self.audio.read_byte(address - PRIMITIVE_AUDIO_ADDRESS)
            }
            CLINT_ADDRESS..CLINT_END => self.clint.read_byte(address - CLINT_ADDRESS),
            SYSCON_ADDRESS..SYSCON_END => self.syscon.read_byte(address - SYSCON_ADDRESS),
            _ => Err(TrapType::LoadAccessFault),
        };
        with_address(result, address)
    }
    pub fn write_word(&mut self, address: u32, word: u32) -> Result<(), Trap> {
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.write_word(address - RAM_ADDRESS, word),
            UART_ADDRESS..UART_END => self.uart.write_word(address - UART_ADDRESS, word),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => self
                .audio
                .write_word(address - PRIMITIVE_AUDIO_ADDRESS, word),
            CLINT_ADDRESS..CLINT_END => self.clint.write_word(address - CLINT_ADDRESS, word),
            SYSCON_ADDRESS..SYSCON_END => self.syscon.write_word(address - SYSCON_ADDRESS, word),
            _ => Err(TrapType::StoreAccessFault),
        };
        with_address(result, address)
    }
    pub fn write_halfword(&mut self, address: u32, halfword: u16) -> Result<(), Trap> {
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.write_halfword(address - RAM_ADDRESS, halfword),
            UART_ADDRESS..UART_END => self.uart.write_halfword(address - UART_ADDRESS, halfword),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => self
                .audio
                .write_halfword(address - PRIMITIVE_AUDIO_ADDRESS, halfword),
            CLINT_ADDRESS..CLINT_END => {
                self.clint.write_halfword(address - CLINT_ADDRESS, halfword)
            }
            SYSCON_ADDRESS..SYSCON_END => self
                .syscon
                .write_halfword(address - SYSCON_ADDRESS, halfword),
            _ => Err(TrapType::StoreAccessFault),
        };
        with_address(result, address)
    }
    pub fn write_byte(&mut self, address: u32, byte: u8) -> Result<(), Trap> {
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.write_byte(address - RAM_ADDRESS, byte),
            UART_ADDRESS..UART_END => self.uart.write_byte(address - UART_ADDRESS, byte),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => self
                .audio
                .write_byte(address - PRIMITIVE_AUDIO_ADDRESS, byte),
            CLINT_ADDRESS..CLINT_END => self.clint.write_byte(address - CLINT_ADDRESS, byte),
            SYSCON_ADDRESS..SYSCON_END => self.syscon.write_byte(address - SYSCON_ADDRESS, byte),
            _ => Err(TrapType::StoreAccessFault),
        };
        with_address(result, address)
    }
    //Loads an ELF image without going through address translation.
    pub fn write_raw_to_ram(&mut self, address: u32, byte: u8) -> bool {
        let offset = address.wrapping_sub(RAM_ADDRESS);
        offset < RAM::MEMORY_REGION_SIZE && self.ram.write_byte(offset, byte).is_ok()
    }
    pub fn read_raw_from_ram(&self, address: u32) -> Option<u8> {
        let offset = address.wrapping_sub(RAM_ADDRESS);
        if offset < RAM::MEMORY_REGION_SIZE {
            self.ram.read_raw(offset)
        } else {
            None
        }
    }
}
