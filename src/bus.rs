use crate::{
    clint::Clint,
    goldfish_events::{self, GoldfishEvents},
    mmdevice::MMdevice,
    plic::{InterruptSource, Plic, PlicContext},
    primitive_audio::{self, PrimitiveAudioProducer, PrimitiveAudioReceiver},
    ram::RAM,
    simple_framebuffer::SimpleScreenbuffer,
    syscon::Syscon,
    traps::{Trap, TrapType},
    uart::{self, UART},
};

//Physical memory map. Devices only know their region size; the bus owns the addresses.
pub const RAM_ADDRESS: u32 = 0x80000000;
pub const PLIC_ADDRESS: u32 = 0x0c000000;
pub const UART_ADDRESS: u32 = 0x10000000;
pub const PRIMITIVE_AUDIO_ADDRESS: u32 = 0x10000200;
pub const GOLDFISH_EVENTS_ADDRESS: u32 = 0x10001000;
pub const CLINT_ADDRESS: u32 = 0x11000000;
pub const SYSCON_ADDRESS: u32 = 0x11100000;
pub const FRAMEBUFFER_ADDRESS: u32 = 0x30000000;

//The PLIC has 32 device sources plus the reserved source 0, matching riscv,ndev = <0x20> in
//the device tree. Source 10 is the UART and source 11 is the goldfish input device.
const PLIC_SOURCES: u32 = 33;

//Range patterns need a single constant bound, so each device's exclusive region end is named here.
const RAM_END: u32 = RAM_ADDRESS + RAM::MEMORY_REGION_SIZE;
const PLIC_END: u32 = PLIC_ADDRESS + Plic::MEMORY_REGION_SIZE;
const UART_END: u32 = UART_ADDRESS + UART::MEMORY_REGION_SIZE;
const AUDIO_END: u32 = PRIMITIVE_AUDIO_ADDRESS + PrimitiveAudioReceiver::MEMORY_REGION_SIZE;
const GOLDFISH_EVENTS_END: u32 = GOLDFISH_EVENTS_ADDRESS + GoldfishEvents::MEMORY_REGION_SIZE;
const CLINT_END: u32 = CLINT_ADDRESS + Clint::MEMORY_REGION_SIZE;
const SYSCON_END: u32 = SYSCON_ADDRESS + Syscon::MEMORY_REGION_SIZE;
const FRAMEBUFFER_END: u32 = FRAMEBUFFER_ADDRESS + SimpleScreenbuffer::MEMORY_REGION_SIZE;

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

//A word access needs four bytes inside a single memory-mapped region.
fn word_in_region(address: u32, start: u32, end: u32) -> bool {
    address >= start && address < end && end - address >= 4
}

//The physical address space: RAM, the CLINT and the memory mapped devices. Everything here takes
//physical addresses; translating them from the addresses issued by the CPU is the MMU's job.
pub struct MemoryBus {
    ram: RAM,
    pub plic: Plic,
    pub uart: UART,
    pub audio: PrimitiveAudioReceiver,
    pub input: GoldfishEvents,
    pub framebuffer: SimpleScreenbuffer,
    pub clint: Clint,
    pub syscon: Syscon,
}

impl MemoryBus {
    pub fn new() -> (Self, PrimitiveAudioProducer) {
        let (audio_res, audio_prod) = primitive_audio::create_primitive_audio_pair();
        let mut plic = Plic::new(PLIC_SOURCES, crate::plic::CONTEXTS);
        let uart_irq = plic
            .register_source(uart::IRQ)
            .expect("UART PLIC source must be available");
        let input_irq = plic
            .register_source(goldfish_events::IRQ)
            .expect("input PLIC source must be available");
        (
            MemoryBus {
                ram: RAM::new(),
                plic,
                uart: UART::new(uart_irq),
                audio: audio_res,
                input: GoldfishEvents::new(input_irq),
                framebuffer: SimpleScreenbuffer::new(),
                clint: Clint::new(),
                syscon: Syscon::new(),
            },
            audio_prod,
        )
    }
    //Push the current device line levels into the PLIC. The CPU calls this before sampling its
    //external-interrupt lines; devices never claim or complete interrupts themselves.
    #[inline]
    pub fn poll_irq_lines(&mut self) {
        let MemoryBus {
            plic, uart, input, ..
        } = self;
        uart.poll_irqs(plic);
        input.poll_irqs(plic);
    }
    #[inline]
    pub fn external_interrupt_pending(&self, context: PlicContext) -> bool {
        self.plic.pending_for(context)
    }
    // Need to implement only fetch 16 bits of instructions, CPU will fetch upper 16 bits for normal instructions after it decodes length.
    pub fn fetch_halfword(&mut self, address: u32) -> Result<u16, Trap> {
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.fetch_halfword(address - RAM_ADDRESS),
            UART_ADDRESS..UART_END => self.uart.fetch_halfword(address - UART_ADDRESS),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => {
                self.audio.fetch_halfword(address - PRIMITIVE_AUDIO_ADDRESS)
            }
            FRAMEBUFFER_ADDRESS..FRAMEBUFFER_END => self
                .framebuffer
                .fetch_halfword(address - FRAMEBUFFER_ADDRESS),
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
            FRAMEBUFFER_ADDRESS..FRAMEBUFFER_END => self
                .framebuffer
                .read_word_atomically(address - FRAMEBUFFER_ADDRESS),
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
            FRAMEBUFFER_ADDRESS..FRAMEBUFFER_END => self
                .framebuffer
                .write_word_atomically(address - FRAMEBUFFER_ADDRESS, word),
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
            PLIC_ADDRESS..PLIC_END => self.plic.read_word(address - PLIC_ADDRESS),
            UART_ADDRESS..UART_END => self.uart.read_word(address - UART_ADDRESS),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => {
                self.audio.read_word(address - PRIMITIVE_AUDIO_ADDRESS)
            }
            GOLDFISH_EVENTS_ADDRESS..GOLDFISH_EVENTS_END => {
                self.input.read_word(address - GOLDFISH_EVENTS_ADDRESS)
            }
            FRAMEBUFFER_ADDRESS..FRAMEBUFFER_END => {
                self.framebuffer.read_word(address - FRAMEBUFFER_ADDRESS)
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
            FRAMEBUFFER_ADDRESS..FRAMEBUFFER_END => self
                .framebuffer
                .read_halfword(address - FRAMEBUFFER_ADDRESS),
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
            GOLDFISH_EVENTS_ADDRESS..GOLDFISH_EVENTS_END => {
                self.input.read_byte(address - GOLDFISH_EVENTS_ADDRESS)
            }
            FRAMEBUFFER_ADDRESS..FRAMEBUFFER_END => {
                self.framebuffer.read_byte(address - FRAMEBUFFER_ADDRESS)
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
            PLIC_ADDRESS..PLIC_END => self.plic.write_word(address - PLIC_ADDRESS, word),
            UART_ADDRESS..UART_END => self.uart.write_word(address - UART_ADDRESS, word),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => self
                .audio
                .write_word(address - PRIMITIVE_AUDIO_ADDRESS, word),
            GOLDFISH_EVENTS_ADDRESS..GOLDFISH_EVENTS_END => self
                .input
                .write_word(address - GOLDFISH_EVENTS_ADDRESS, word),
            FRAMEBUFFER_ADDRESS..FRAMEBUFFER_END => self
                .framebuffer
                .write_word(address - FRAMEBUFFER_ADDRESS, word),
            CLINT_ADDRESS..CLINT_END => self.clint.write_word(address - CLINT_ADDRESS, word),
            SYSCON_ADDRESS..SYSCON_END => self.syscon.write_word(address - SYSCON_ADDRESS, word),
            _ => Err(TrapType::StoreAccessFault),
        };
        with_address(result, address)
    }
    //64-bit accesses are used by FLD/FSD. They are decomposed into two word accesses, but both
    //words are range-checked first so a bad second word cannot leave a partial store behind.
    pub fn read_dword(&mut self, address: u32) -> Result<u64, Trap> {
        //The PLIC and the input FIFO are 32-bit register windows; a 64-bit read
        //would trigger a claim or pop two event words on the second access.
        if (PLIC_ADDRESS..PLIC_END).contains(&address)
            || (GOLDFISH_EVENTS_ADDRESS..GOLDFISH_EVENTS_END).contains(&address)
        {
            return Err(Trap {
                tcause: TrapType::LoadAccessFault,
                tval: address,
            });
        }
        let low = self.read_word(address)?;
        let high = self.read_word(address.wrapping_add(4))?;
        Ok(low as u64 | ((high as u64) << 32))
    }
    pub fn check_write_dword(&self, address: u32) -> Result<(), Trap> {
        //The input region is deliberately absent: a 64-bit store would split
        //into a page-select write followed by a fault, so reject it before the
        //first word takes effect.
        let regions = [
            (RAM_ADDRESS, RAM_END),
            (UART_ADDRESS, UART_END),
            (PRIMITIVE_AUDIO_ADDRESS, AUDIO_END),
            (FRAMEBUFFER_ADDRESS, FRAMEBUFFER_END),
            (CLINT_ADDRESS, CLINT_END),
            (SYSCON_ADDRESS, SYSCON_END),
        ];
        let writable = |addr: u32| {
            regions
                .iter()
                .any(|&(start, end)| word_in_region(addr, start, end))
        };
        if !writable(address) {
            return Err(Trap {
                tcause: TrapType::StoreAccessFault,
                tval: address,
            });
        }
        let high = address.wrapping_add(4);
        if !writable(high) {
            return Err(Trap {
                tcause: TrapType::StoreAccessFault,
                tval: high,
            });
        }
        Ok(())
    }
    pub fn write_dword(&mut self, address: u32, value: u64) -> Result<(), Trap> {
        self.check_write_dword(address)?;
        self.write_word(address, value as u32)?;
        self.write_word(address.wrapping_add(4), (value >> 32) as u32)
    }
    pub fn write_halfword(&mut self, address: u32, halfword: u16) -> Result<(), Trap> {
        let result = match address {
            RAM_ADDRESS..RAM_END => self.ram.write_halfword(address - RAM_ADDRESS, halfword),
            UART_ADDRESS..UART_END => self.uart.write_halfword(address - UART_ADDRESS, halfword),
            PRIMITIVE_AUDIO_ADDRESS..AUDIO_END => self
                .audio
                .write_halfword(address - PRIMITIVE_AUDIO_ADDRESS, halfword),
            FRAMEBUFFER_ADDRESS..FRAMEBUFFER_END => self
                .framebuffer
                .write_halfword(address - FRAMEBUFFER_ADDRESS, halfword),
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
            GOLDFISH_EVENTS_ADDRESS..GOLDFISH_EVENTS_END => self
                .input
                .write_byte(address - GOLDFISH_EVENTS_ADDRESS, byte),
            FRAMEBUFFER_ADDRESS..FRAMEBUFFER_END => self
                .framebuffer
                .write_byte(address - FRAMEBUFFER_ADDRESS, byte),
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
