use crate::{mmdevice::MMdevice, traps::TrapType};

const POWEROFF_VALUE: u32 = 0x5555;
const REBOOT_VALUE: u32 = 0x7777;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SysconEvent {
    PowerOff,
    Reboot,
}

//Register bank used by syscon-poweroff and syscon-reboot. Writing the corresponding value to
//offset 0 asks the host to power off or reboot the machine; other values are stored but ignored.
pub struct Syscon {
    value: u32,
    event: Option<SysconEvent>,
}

impl Syscon {
    pub(crate) fn new() -> Self {
        Syscon {
            value: 0,
            event: None,
        }
    }
    pub fn take_event(&mut self) -> Option<SysconEvent> {
        self.event.take()
    }
}

impl MMdevice for Syscon {
    const MEMORY_REGION_SIZE: u32 = 0x1000;

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
        match offset {
            0 => Ok(self.value),
            _ => Err(TrapType::LoadAccessFault),
        }
    }
    fn write_word(&mut self, offset: u32, value: u32) -> Result<(), TrapType> {
        match offset {
            0 => {
                self.value = value;
                let event = match value {
                    POWEROFF_VALUE => SysconEvent::PowerOff,
                    REBOOT_VALUE => SysconEvent::Reboot,
                    _ => return Ok(()),
                };
                //The first untaken request wins.
                self.event.get_or_insert(event);
                Ok(())
            }
            _ => Err(TrapType::StoreAccessFault),
        }
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
