use std::collections::VecDeque;

use crate::{
    mmdevice::MMdevice,
    plic::{InterruptSource, IrqLine, Plic},
    traps::TrapType,
};

pub const MAX_BUFFER: usize = 1 << 16;
/// PLIC source used by the UART. Keep this in sync with the device tree.
pub const IRQ: u32 = 10;

//16550 register offsets.
const RBR_THR: u32 = 0;
const IER_DLM: u32 = 1;
const IIR_FCR: u32 = 2;
const LCR: u32 = 3;
const MCR: u32 = 4;
const LSR: u32 = 5;
const MSR: u32 = 6;
const SCR: u32 = 7;

const IER_RX: u8 = 0x01;
const IER_THRE: u8 = 0x02;

const LSR_DR: u8 = 0x01;
const LSR_THRE: u8 = 0x20;
const LSR_TEMT: u8 = 0x40;

//16550/8250-compatible UART with byte registers at offsets 0-7 (loopback and modem-status
//inputs are not modelled, MSR reads 0).
//TX is bounded by MAX_BUFFER: THRE reports free space, TEMT reports an empty queue, and a write
//while THRE is low is dropped, so a guest that ignores THRE loses output. RX is unbounded.
//
//The THRE interrupt is edge-like in the 16550: it is raised when the transmitter becomes empty,
//and cleared by reading IIR or writing THR. Keeping a separate latch prevents a fast-draining
//emulated transmitter from holding the PLIC line high forever.
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
    irq: IrqLine,
    thre_pending: bool,
}

impl UART {
    pub fn new(irq: IrqLine) -> Self {
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
            irq,
            thre_pending: false,
        }
    }
    //Guest -> host.
    pub fn try_get_byte(&mut self) -> Option<u8> {
        let byte = self.from_emu_buffer.pop_front();
        //The transmitter has just become empty; this is when a real THRE interrupt is raised.
        if byte.is_some() && self.from_emu_buffer.is_empty() && self.ier & IER_THRE != 0 {
            self.thre_pending = true;
        }
        byte
    }
    //Host -> guest.
    pub fn host_push(&mut self, byte: u8) {
        self.to_emu_buffer.push_back(byte)
    }
    fn irq_pending(&self) -> bool {
        (self.ier & IER_RX != 0 && !self.to_emu_buffer.is_empty())
            || (self.ier & IER_THRE != 0 && self.thre_pending)
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
    fn iir(&mut self) -> u8 {
        let fifo = if self.fcr & 0x01 != 0 { 0xc0 } else { 0 };
        if self.ier & IER_RX != 0 && !self.to_emu_buffer.is_empty() {
            fifo | 0x04 //received data available
        } else if self.ier & IER_THRE != 0 && self.thre_pending {
            //Reading IIR acknowledges the THRE interrupt.
            self.thre_pending = false;
            fifo | 0x02 //transmitter holding register empty
        } else {
            fifo | 0x01 //no interrupt pending
        }
    }
}

impl InterruptSource for UART {
    #[inline]
    fn poll_irqs(&mut self, plic: &mut Plic) {
        plic.set_level(self.irq, self.irq_pending());
    }
}

impl MMdevice for UART {
    const MEMORY_REGION_SIZE: u32 = 0x100;

    fn read_byte(&mut self, offset: u32) -> Result<u8, TrapType> {
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
            _ => Err(TrapType::LoadAccessFault),
        }
    }
    fn write_byte(&mut self, offset: u32, byte: u8) -> Result<(), TrapType> {
        let dlab = self.lcr & 0x80 != 0;
        match offset {
            RBR_THR if dlab => self.dll = byte,
            RBR_THR => {
                //A THR write acknowledges a pending THRE interrupt.
                self.thre_pending = false;
                if self.from_emu_buffer.len() < MAX_BUFFER {
                    self.from_emu_buffer.push_back(byte);
                }
            }
            IER_DLM if dlab => self.dlm = byte,
            IER_DLM => {
                let was_thre_enabled = self.ier & IER_THRE != 0;
                self.ier = byte & 0x0f;
                if self.ier & IER_THRE == 0 {
                    self.thre_pending = false;
                } else if !was_thre_enabled && self.from_emu_buffer.is_empty() {
                    self.thre_pending = true;
                }
            }
            IIR_FCR => {
                //FCR: clear-RX/TX-FIFO are self-clearing.
                if byte & 0x02 != 0 {
                    self.to_emu_buffer.clear();
                }
                if byte & 0x04 != 0 {
                    self.from_emu_buffer.clear();
                    if self.ier & IER_THRE != 0 {
                        self.thre_pending = true;
                    }
                }
                self.fcr = byte & !0x06;
            }
            LCR => self.lcr = byte,
            MCR => self.mcr = byte & 0x1f,
            //LSR and MSR are read-only.
            LSR | MSR => {}
            SCR => self.scr = byte,
            _ => return Err(TrapType::StoreAccessFault),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plic::PlicContext;

    fn uart() -> (UART, Plic) {
        let mut plic = Plic::new(32, 2);
        let irq = plic.register_source(IRQ).unwrap();
        (UART::new(irq), plic)
    }

    #[test]
    fn rx_interrupt_follows_data_available() {
        let (mut uart, _) = uart();
        uart.write_byte(IER_DLM, IER_RX).unwrap();
        assert!(!uart.irq_pending());
        uart.host_push(b'x');
        assert!(uart.irq_pending());
        assert_eq!(uart.read_byte(RBR_THR).unwrap(), b'x');
        assert!(!uart.irq_pending());
    }

    #[test]
    fn thre_interrupt_is_acknowledged_by_iir_or_thr_write() {
        let (mut uart, _) = uart();
        uart.write_byte(IER_DLM, IER_THRE).unwrap();
        assert!(uart.irq_pending());
        assert_eq!(uart.read_byte(IIR_FCR).unwrap() & 0x0f, 0x02);
        assert!(!uart.irq_pending());

        uart.write_byte(RBR_THR, b'x').unwrap();
        assert!(!uart.irq_pending());
        assert_eq!(uart.try_get_byte(), Some(b'x'));
        assert!(uart.irq_pending());
    }

    #[test]
    fn rx_takes_priority_over_thre_without_losing_thre() {
        let (mut uart, _) = uart();
        uart.write_byte(IER_DLM, IER_RX | IER_THRE).unwrap();
        uart.host_push(b'x');
        assert_eq!(uart.read_byte(IIR_FCR).unwrap() & 0x0f, 0x04);
        assert!(uart.irq_pending());
        assert_eq!(uart.read_byte(RBR_THR).unwrap(), b'x');
        assert_eq!(uart.read_byte(IIR_FCR).unwrap() & 0x0f, 0x02);
        assert!(!uart.irq_pending());
    }

    #[test]
    fn polling_pushes_the_line_into_the_plic() {
        let (mut uart, mut plic) = uart();
        //Enable source 10 in the supervisor context and give it a non-zero priority.
        plic.write_word(10 * 4, 1).unwrap();
        plic.write_word(0x0000_2000 + 0x80, 1 << IRQ).unwrap();
        uart.write_byte(IER_DLM, IER_RX).unwrap();
        uart.host_push(b'x');
        uart.poll_irqs(&mut plic);
        assert!(plic.pending_for(PlicContext::Supervisor));
        assert_eq!(plic.claim(PlicContext::Supervisor), IRQ);
        uart.read_byte(RBR_THR).unwrap();
        uart.poll_irqs(&mut plic);
        assert!(!plic.pending_for(PlicContext::Supervisor));
        plic.complete(PlicContext::Supervisor, IRQ);
    }
}
