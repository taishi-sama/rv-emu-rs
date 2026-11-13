use crate::traps::{Trap, TrapType};

pub const SYSCON_ADDRESS: u32 = 0x11100000;
pub const SYSCON_SIZE: usize = 0x1000;
pub const SYSCON_ADDRESS_END: u32 = SYSCON_ADDRESS + SYSCON_SIZE as u32 - 1;

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
    pub fn read_word(&self, address: u32) -> Result<u32, Trap> {
        debug_assert!(matches!(address, SYSCON_ADDRESS..=SYSCON_ADDRESS_END));
        match address.wrapping_sub(SYSCON_ADDRESS) {
            0 => Ok(self.value),
            _ => Err(Trap {
                tcause: TrapType::LoadAccessFault,
                tval: address,
            }),
        }
    }
    pub fn write_word(&mut self, address: u32, value: u32) -> Result<(), Trap> {
        debug_assert!(matches!(address, SYSCON_ADDRESS..=SYSCON_ADDRESS_END));
        match address.wrapping_sub(SYSCON_ADDRESS) {
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
            _ => Err(Trap {
                tcause: TrapType::StoreAccessFault,
                tval: address,
            }),
        }
    }
}
