//! Goldfish events input device.
//!
//! Android's goldfish virtual input device is a single event FIFO with three
//! capability pages. Linux's `goldfish_events` driver (`google,goldfish-events-keypad`)
//! reads the pages during probe to build an ordinary `input_dev`, then reads one
//! `type, code, value` triple per interrupt and forwards it to the input core.
//!
//! Layout used here (matching the driver in `drivers/input/keyboard/goldfish_events.c`):
//!
//! * `0x00` read  - pop the next event word
//! * `0x00` write - select a capability page
//! * `0x04` read  - length of the selected page in bytes
//! * `0x08` + i   - byte `i` of the selected page (word reads for ABS data)
//!
//! Pages are `PAGE_NAME` (device name), `PAGE_EVBITS | type` (the capability
//! bitmap for one event type) and `PAGE_ABSDATA | EV_ABS` (four u32 per axis:
//! minimum, maximum, fuzz and flat).
//!
//! The keyboard and pointer share one `input_dev`, exactly like the emulated
//! hardware device. Linux's VT keyboard handler therefore also sees the mouse
//! buttons, so in mediumraw mode a button appears as an extended scancode with
//! a few spurious key releases; Doom ignores them. A split keyboard/pointer
//! pair would need two MMIO instances. The pointer is absolute: `ABS_X`/`ABS_Y` are framebuffer
//! pixel coordinates, and Linux's `mousedev` turns them into the relative
//! packets expected by `/dev/input/mice`, so SDL 1.2 sees a normal mouse.

use std::collections::VecDeque;

use crate::{
    mmdevice::MMdevice,
    plic::{InterruptSource, IrqLine, Plic},
    simple_framebuffer::{FRAMEBUFFER_HEIGHT, FRAMEBUFFER_WIDTH},
    traps::TrapType,
};

/// PLIC source used by the input device. Keep this in sync with the device tree.
pub const IRQ: u32 = 11;

const REGION_SIZE: u32 = 0x1000;

const REG_READ: u32 = 0x00;
const REG_SET_PAGE: u32 = 0x00;
const REG_LEN: u32 = 0x04;
const REG_DATA: u32 = 0x08;

const PAGE_NAME: u32 = 0x00000;
const PAGE_EVBITS: u32 = 0x10000;
const PAGE_ABSDATA: u32 = 0x20000 | EV_ABS;

// Event types, as defined by the Linux input protocol.
const EV_SYN: u32 = 0x00;
const EV_KEY: u32 = 0x01;
const EV_ABS: u32 = 0x03;

const ABS_X: u32 = 0x00;
const ABS_Y: u32 = 0x01;

pub const BTN_LEFT: u16 = 0x110;
pub const BTN_RIGHT: u16 = 0x111;
pub const BTN_MIDDLE: u16 = 0x112;
pub const BTN_SIDE: u16 = 0x113;
pub const BTN_EXTRA: u16 = 0x114;

const NAME: &[u8] = b"rv-emu-rs input";

// `EV_KEY`/`EV_ABS` tell the input core which capability pages matter. Bit 0 is
// `EV_SYN`, bit 1 `EV_KEY` and bit 3 `EV_ABS`.
const EV_BITS: &[u8] = &[0b0000_1011];
// `ABS_X` and `ABS_Y`; there are no relative axes. The absence of
// `BTN_TOUCH`/`BTN_TOOL_FINGER` keeps mousedev from treating the device as a
// tablet or touchpad, so it takes the absolute-mouse path.
const ABS_BITS: &[u8] = &[0b0000_0011];

// KEY_MAX is 0x2ff, so 96 bytes cover every keyboard code Linux defines.
const KEY_BITMAP_BYTES: usize = 96;
// The driver caps the ABS page at ABS_MAX + 1 axes and reads 4 u32 per axis.
const ABS_MAX: u32 = 0x3f;
const ABSDATA_BYTES: u32 = (ABS_MAX + 1) * 4 * 4;

// Advertised absolute ranges. They match the framebuffer, so the mousedev
// coordinate conversion is 1:1 with guest pixels.
const ABS_X_MAX: u32 = FRAMEBUFFER_WIDTH;
const ABS_Y_MAX: u32 = FRAMEBUFFER_HEIGHT;

// Keep the FIFO bounded while the guest has not probed the driver yet. One
// event is three words; dropping whole events keeps the FIFO aligned.
const MAX_EVENTS: usize = 256;

/// An input event from the host window, already translated to Linux input codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostInput {
    /// `EV_KEY` with a Linux `KEY_*` or `BTN_*` code.
    Key { code: u16, pressed: bool },
    /// Absolute pointer position in framebuffer pixels.
    Pointer { x: u16, y: u16 },
}

pub struct GoldfishEvents {
    fifo: VecDeque<u32>,
    page: u32,
    irq: IrqLine,
    key_bitmap: [u8; KEY_BITMAP_BYTES],
    // Events are only queued after the driver has selected the name page during
    // probe. Before that the guest cannot consume them, and boot-time motion
    // would just fill the FIFO.
    armed: bool,
}

impl GoldfishEvents {
    pub fn new(irq: IrqLine) -> Self {
        let mut key_bitmap = [0u8; KEY_BITMAP_BYTES];
        // Advertise every ordinary keyboard code. This keeps the capability
        // bitmap in sync with the host mapping without duplicating the list.
        for code in 1..=u8::MAX as usize {
            key_bitmap[code / 8] |= 1 << (code % 8);
        }
        for code in [BTN_LEFT, BTN_RIGHT, BTN_MIDDLE, BTN_SIDE, BTN_EXTRA] {
            key_bitmap[code as usize / 8] |= 1 << (code as usize % 8);
        }
        GoldfishEvents {
            fifo: VecDeque::new(),
            page: PAGE_NAME,
            irq,
            key_bitmap,
            armed: false,
        }
    }

    /// Queues one `EV_KEY` event, for both keyboard keys and pointer buttons.
    pub fn push_key(&mut self, code: u16, pressed: bool) {
        self.push_event(EV_KEY, u32::from(code), u32::from(pressed));
    }

    /// Queues one absolute pointer position as separate `ABS_X` and `ABS_Y`
    /// events. The guest driver syncs after each triple, so mousedev sees the
    /// axes in two reports; X is first because Doom turns with X and ignores Y.
    pub fn push_pointer(&mut self, x: u16, y: u16) {
        // Check the whole pair up front: a half-queued X/Y pair would leave
        // mousedev with a stale axis.
        if !self.has_room(6) {
            return;
        }
        let x = x.min(ABS_X_MAX as u16 - 1);
        let y = y.min(ABS_Y_MAX as u16 - 1);
        self.push_event(EV_ABS, ABS_X, u32::from(x));
        self.push_event(EV_ABS, ABS_Y, u32::from(y));
    }

    fn has_room(&self, words: usize) -> bool {
        self.armed && self.fifo.len() + words <= MAX_EVENTS * 3
    }

    fn push_event(&mut self, event_type: u32, code: u32, value: u32) {
        if !self.has_room(3) {
            return;
        }
        self.fifo.extend([event_type, code, value]);
    }

    fn event_bits(&self, event_type: u32) -> Option<&[u8]> {
        match event_type {
            EV_SYN => Some(EV_BITS),
            EV_KEY => Some(&self.key_bitmap),
            EV_ABS => Some(ABS_BITS),
            _ => None,
        }
    }

    fn page_len(&self) -> u32 {
        if self.page == PAGE_NAME {
            NAME.len() as u32
        } else if self.page & 0xffff_0000 == PAGE_EVBITS {
            self.event_bits(self.page & 0xffff)
                .map_or(0, |bits| bits.len() as u32)
        } else if self.page == PAGE_ABSDATA {
            ABSDATA_BYTES
        } else {
            0
        }
    }

    fn abs_word(&self, index: u32) -> u32 {
        match (index / 4, index % 4) {
            (0, 0) | (1, 0) => 0,
            (0, 1) => ABS_X_MAX,
            (1, 1) => ABS_Y_MAX,
            _ => 0,
        }
    }

    fn page_byte(&self, offset: u32) -> u8 {
        if offset >= self.page_len() {
            return 0;
        }
        if self.page == PAGE_NAME {
            return NAME[offset as usize];
        }
        if self.page == PAGE_ABSDATA {
            return (self.abs_word(offset / 4) >> (8 * (offset % 4))) as u8;
        }
        if self.page & 0xffff_0000 == PAGE_EVBITS {
            if let Some(bits) = self.event_bits(self.page & 0xffff) {
                return bits[offset as usize];
            }
        }
        0
    }

    fn page_word(&self, offset: u32) -> u32 {
        (0..4).fold(0, |word, byte| {
            word | u32::from(self.page_byte(offset + byte)) << (8 * byte)
        })
    }
}

impl MMdevice for GoldfishEvents {
    const MEMORY_REGION_SIZE: u32 = REGION_SIZE;

    fn read_word(&mut self, offset: u32) -> Result<u32, TrapType> {
        match offset {
            REG_READ => Ok(self.fifo.pop_front().unwrap_or(0)),
            REG_LEN => Ok(self.page_len()),
            REG_DATA.. => Ok(self.page_word(offset - REG_DATA)),
            _ => Err(TrapType::LoadAccessFault),
        }
    }

    fn read_byte(&mut self, offset: u32) -> Result<u8, TrapType> {
        match offset {
            //The event FIFO is a word stream; a byte read would desynchronise
            //the type/code/value triples without consuming a whole event.
            REG_READ => Err(TrapType::LoadAccessFault),
            REG_LEN => Ok(self.page_len() as u8),
            REG_DATA.. => Ok(self.page_byte(offset - REG_DATA)),
            _ => Err(TrapType::LoadAccessFault),
        }
    }

    fn write_word(&mut self, offset: u32, word: u32) -> Result<(), TrapType> {
        match offset {
            REG_SET_PAGE => {
                self.page = word;
                if word == PAGE_NAME {
                    self.armed = true;
                }
                Ok(())
            }
            _ => Err(TrapType::StoreAccessFault),
        }
    }

    fn write_byte(&mut self, offset: u32, byte: u8) -> Result<(), TrapType> {
        match offset {
            REG_SET_PAGE => {
                self.page = u32::from(byte);
                if self.page == PAGE_NAME {
                    self.armed = true;
                }
                Ok(())
            }
            _ => Err(TrapType::StoreAccessFault),
        }
    }
}

impl InterruptSource for GoldfishEvents {
    fn poll_irqs(&mut self, plic: &mut Plic) {
        plic.set_level(self.irq, !self.fifo.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plic::{CONTEXTS, PlicContext};

    const PLIC_SOURCES: u32 = 33;

    fn device() -> (GoldfishEvents, Plic) {
        let mut plic = Plic::new(PLIC_SOURCES, CONTEXTS);
        let irq = plic.register_source(IRQ).unwrap();
        (GoldfishEvents::new(irq), plic)
    }

    #[test]
    fn name_page_matches_the_driver_probe() {
        let (mut device, _) = device();
        device.write_word(REG_SET_PAGE, PAGE_NAME).unwrap();
        assert_eq!(device.read_word(REG_LEN).unwrap(), NAME.len() as u32);
        let name: Vec<u8> = (0..NAME.len() as u32)
            .map(|offset| device.read_byte(REG_DATA + offset).unwrap())
            .collect();
        assert_eq!(name, NAME);
    }

    #[test]
    fn capability_pages_describe_a_keyboard_and_absolute_pointer() {
        let (mut device, _) = device();
        device.write_word(REG_SET_PAGE, PAGE_EVBITS).unwrap();
        assert_eq!(device.read_byte(REG_DATA).unwrap(), 0b0000_1011);

        device
            .write_word(REG_SET_PAGE, PAGE_EVBITS | EV_KEY)
            .unwrap();
        assert_eq!(device.read_word(REG_LEN).unwrap(), KEY_BITMAP_BYTES as u32);
        let left_button_bit = BTN_LEFT as u32;
        assert_ne!(
            device
                .read_byte(REG_DATA + left_button_bit / 8)
                .unwrap()
                & (1 << (left_button_bit % 8)),
            0
        );

        device
            .write_word(REG_SET_PAGE, PAGE_EVBITS | EV_ABS)
            .unwrap();
        assert_eq!(device.read_byte(REG_DATA).unwrap(), 0b0000_0011);
    }

    #[test]
    fn abs_page_reports_framebuffer_ranges() {
        let (mut device, _) = device();
        device.write_word(REG_SET_PAGE, PAGE_ABSDATA).unwrap();
        assert_eq!(device.read_word(REG_LEN).unwrap(), ABSDATA_BYTES);
        assert_eq!(device.read_word(REG_DATA).unwrap(), 0);
        assert_eq!(device.read_word(REG_DATA + 4).unwrap(), ABS_X_MAX);
        assert_eq!(device.read_word(REG_DATA + 16).unwrap(), 0);
        assert_eq!(device.read_word(REG_DATA + 20).unwrap(), ABS_Y_MAX);
    }

    #[test]
    fn events_are_dropped_until_the_driver_probes() {
        let (mut device, mut plic) = device();
        plic.write_word(IRQ * 4, 1).unwrap();
        plic.write_word(0x0000_2000 + 0x80, 1 << IRQ).unwrap();

        device.push_key(30, true);
        assert!(device.fifo.is_empty());
        device.poll_irqs(&mut plic);
        assert!(!plic.pending_for(PlicContext::Supervisor));

        device.write_word(REG_SET_PAGE, PAGE_NAME).unwrap();
        device.push_key(30, true);
        assert_eq!(device.fifo.len(), 3);
        device.poll_irqs(&mut plic);
        assert!(plic.pending_for(PlicContext::Supervisor));
    }

    #[test]
    fn byte_reads_of_the_event_fifo_fault() {
        let (mut device, _) = device();
        assert!(device.read_byte(REG_READ).is_err());
    }

    #[test]
    fn fifo_limit_does_not_split_a_pointer_pair() {
        let (mut device, _) = device();
        device.write_word(REG_SET_PAGE, PAGE_NAME).unwrap();
        while device.fifo.len() + 6 <= MAX_EVENTS * 3 {
            device.push_key(1, true);
        }
        let before = device.fifo.len();
        device.push_pointer(10, 20);
        assert_eq!(device.fifo.len(), before);

        for _ in 0..3 {
            device.read_word(REG_READ).unwrap();
        }
        device.push_pointer(10, 20);
        assert_eq!(device.fifo.len(), MAX_EVENTS * 3);
    }

    #[test]
    fn fifo_delivers_triples_and_drives_the_plic_line() {
        let (mut device, mut plic) = device();
        plic.write_word(IRQ * 4, 1).unwrap();
        plic.write_word(0x0000_2000 + 0x80, 1 << IRQ).unwrap();
        device.write_word(REG_SET_PAGE, PAGE_NAME).unwrap();

        device.push_key(30, true);
        device.poll_irqs(&mut plic);
        assert!(plic.pending_for(PlicContext::Supervisor));
        assert_eq!(device.read_word(REG_READ).unwrap(), EV_KEY);
        assert_eq!(device.read_word(REG_READ).unwrap(), 30);
        assert_eq!(device.read_word(REG_READ).unwrap(), 1);
        device.poll_irqs(&mut plic);
        assert!(!plic.pending_for(PlicContext::Supervisor));

        device.push_pointer(100, 200);
        assert_eq!(device.read_word(REG_READ).unwrap(), EV_ABS);
        assert_eq!(device.read_word(REG_READ).unwrap(), ABS_X);
        assert_eq!(device.read_word(REG_READ).unwrap(), 100);
        assert_eq!(device.read_word(REG_READ).unwrap(), EV_ABS);
        assert_eq!(device.read_word(REG_READ).unwrap(), ABS_Y);
        assert_eq!(device.read_word(REG_READ).unwrap(), 200);
    }
}
