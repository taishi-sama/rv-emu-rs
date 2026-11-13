use std::time::Instant;

use crate::{mmdevice::MMdevice, traps::TrapType};

const MSIP_OFFSET: u32 = 0x0000;
const MTIMECMP_OFFSET: u32 = 0x4000;
const MTIMECMP_HIGH_OFFSET: u32 = 0x4004;
const MTIME_OFFSET: u32 = 0xbff8;
const MTIME_HIGH_OFFSET: u32 = 0xbffc;

//One hart's CLINT registers. The emulator runs a single core today, but the CLINT is a shared
//device with one of these banks per hart, so adding cores means adding entries to `Clint::banks`
//and selecting the bank from the hart id in the MSIP/MTIMECMP decode.
struct ClintBank {
    //Bit 0 of MSIP: the software interrupt pending flag.
    msip: bool,
    mtimecmp: u64,
}

//Core Local Interruptor: one device shared by all harts, holding a per-hart MSIP and MTIMECMP
//bank plus a single global mtime. mtime counts host microseconds since construction.
pub struct Clint {
    //Indexed by hart id; hart 0 is the only core for now.
    banks: [ClintBank; 1],
    time_start: Instant,
}

impl Clint {
    pub(crate) fn new() -> Self {
        Clint {
            banks: [ClintBank {
                msip: false,
                //all ones keeps the timer interrupt from firing until software programs it
                mtimecmp: u64::MAX,
            }],
            time_start: Instant::now(),
        }
    }
    pub fn mtime(&self) -> u64 {
        self.time_start.elapsed().as_micros() as u64
    }
    pub fn msip(&self) -> bool {
        self.banks[0].msip
    }
    pub fn mtip(&self) -> bool {
        self.mtime() >= self.banks[0].mtimecmp
    }
}

impl MMdevice for Clint {
    const MEMORY_REGION_SIZE: u32 = 0x10000;

    fn fetch_halfword(&mut self, _offset: u32) -> Result<u16, TrapType> {
        Err(TrapType::InstructionAccessFault)
    }
    fn read_word_atomically(&mut self, _offset: u32) -> Result<u32, TrapType> {
        Err(TrapType::LoadAccessFault)
    }
    fn write_word_atomically(&mut self, _offset: u32, _word: u32) -> Result<(), TrapType> {
        Err(TrapType::StoreAccessFault)
    }
    fn read_word(&mut self, offset: u32) -> Result<u32, TrapType> {
        Ok(match offset {
            MSIP_OFFSET => self.banks[0].msip as u32,
            MTIMECMP_OFFSET => self.banks[0].mtimecmp as u32,
            MTIMECMP_HIGH_OFFSET => (self.banks[0].mtimecmp >> 32) as u32,
            MTIME_OFFSET => self.mtime() as u32,
            MTIME_HIGH_OFFSET => (self.mtime() >> 32) as u32,
            _ => return Err(TrapType::LoadAccessFault),
        })
    }
    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), TrapType> {
        match offset {
            MSIP_OFFSET => self.banks[0].msip = value & 1 != 0,
            MTIMECMP_OFFSET => {
                self.banks[0].mtimecmp = (self.banks[0].mtimecmp & !0xffff_ffff) | value as u64
            }
            MTIMECMP_HIGH_OFFSET => {
                self.banks[0].mtimecmp =
                    (self.banks[0].mtimecmp & 0xffff_ffff) | (value as u64) << 32
            }
            //mtime is derived from the host clock, so guest writes cannot change it
            MTIME_OFFSET | MTIME_HIGH_OFFSET => {}
            _ => return Err(TrapType::StoreAccessFault),
        }
        Ok(())
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
