use std::collections::VecDeque;

use crate::traps::{Trap, TrapType};

pub const UART_ADDRESS: u32 = 0x10000000;
const UART_REGION_SIZE: usize = 0x100;
pub const UART_ADDRESS_END: u32 = UART_ADDRESS + UART_REGION_SIZE as u32 - 1;

pub const MAX_BUFFER: usize = 1 << 16;

//16550 register offsets.
const RBR_THR: u32 = 0;
const IER_DLM: u32 = 1;
const IIR_FCR: u32 = 2;
const LCR: u32 = 3;
const MCR: u32 = 4;
const LSR: u32 = 5;
const MSR: u32 = 6;
const SCR: u32 = 7;

const LSR_DR: u8 = 0x01;
const LSR_THRE: u8 = 0x20;
const LSR_TEMT: u8 = 0x40;

//16550/8250-compatible UART with byte registers at offsets 0-7 (loopback and modem-status
//inputs are not modelled, MSR reads 0).
//TX is bounded by MAX_BUFFER: THRE reports free space, TEMT reports an empty queue, and a write
//while THRE is low is dropped, so a guest that ignores THRE loses output. RX is unbounded.
pub struct UART {
    from_emu_buffer: VecDeque<u8>,
    to_emu_buffer: VecDeque<u8>,
    ier: u8,
    fcr: u8,
    lcr: u8,
    mcr: u8,
    scr: u8,
    dll: u8,
    dlm: u8,
}
impl UART {
    pub fn new() -> Self {
        UART {
            from_emu_buffer: Default::default(),
            to_emu_buffer: Default::default(),
            ier: 0,
            fcr: 0,
            lcr: 0,
            mcr: 0,
            scr: 0,
            dll: 0,
            dlm: 0,
        }
    }
    //Guest -> host.
    pub fn try_get_byte(&mut self) -> Option<u8> {
        self.from_emu_buffer.pop_front()
    }
    //Host -> guest.
    pub fn host_push(&mut self, byte: u8) {
        self.to_emu_buffer.push_back(byte)
    }
    fn lsr(&self) -> u8 {
        let mut lsr = 0;
        if self.from_emu_buffer.len() < MAX_BUFFER {
            lsr |= LSR_THRE;
        }
        if self.from_emu_buffer.is_empty() {
            lsr |= LSR_TEMT;
        }
        if !self.to_emu_buffer.is_empty() {
            lsr |= LSR_DR;
        }
        lsr
    }
    //The UART interrupt line is not wired to the CPU (there is no PLIC), so IIR is informational.
    //TODO: model THRE-clear-on-IIR-read and per-source pending bits if an IRQ path is added.
    fn iir(&self) -> u8 {
        let fifo = if self.fcr & 0x01 != 0 { 0xc0 } else { 0 };
        if self.ier & 0x01 != 0 && !self.to_emu_buffer.is_empty() {
            fifo | 0x04 //received data available
        } else if self.ier & 0x02 != 0 {
            fifo | 0x02 //transmitter holding register empty
        } else {
            fifo | 0x01 //no interrupt pending
        }
    }
    pub fn read_byte(&mut self, address: u32) -> Result<u8, Trap> {
        let offset = address.wrapping_sub(UART_ADDRESS);
        let dlab = self.lcr & 0x80 != 0;
        match offset {
            RBR_THR if dlab => Ok(self.dll),
            RBR_THR => Ok(self.to_emu_buffer.pop_front().unwrap_or(0)),
            IER_DLM if dlab => Ok(self.dlm),
            IER_DLM => Ok(self.ier),
            IIR_FCR => Ok(self.iir()),
            LCR => Ok(self.lcr),
            MCR => Ok(self.mcr),
            LSR => Ok(self.lsr()),
            MSR => Ok(0),
            SCR => Ok(self.scr),
            _ => Err(Trap {
                tcause: TrapType::LoadAccessFault,
                tval: address,
            }),
        }
    }
    pub fn write_byte(&mut self, address: u32, byte: u8) -> Result<(), Trap> {
        let offset = address.wrapping_sub(UART_ADDRESS);
        let dlab = self.lcr & 0x80 != 0;
        match offset {
            RBR_THR if dlab => self.dll = byte,
            RBR_THR => {
                if self.from_emu_buffer.len() < MAX_BUFFER {
                    self.from_emu_buffer.push_back(byte);
                }
            }
            IER_DLM if dlab => self.dlm = byte,
            IER_DLM => self.ier = byte & 0x0f,
            IIR_FCR => {
                //FCR: clear-RX/TX-FIFO are self-clearing.
                if byte & 0x02 != 0 {
                    self.to_emu_buffer.clear();
                }
                if byte & 0x04 != 0 {
                    self.from_emu_buffer.clear();
                }
                self.fcr = byte & !0x06;
            }
            LCR => self.lcr = byte,
            MCR => self.mcr = byte & 0x1f,
            //LSR and MSR are read-only.
            LSR | MSR => {}
            SCR => self.scr = byte,
            _ => {
                return Err(Trap {
                    tcause: TrapType::StoreAccessFault,
                    tval: address,
                })
            }
        }
        Ok(())
    }
}
