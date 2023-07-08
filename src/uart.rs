use std::collections::VecDeque;

pub struct UART {
    from_emu_buffer: VecDeque<u8>,

    to_emu_buffer: VecDeque<u8>,
}
impl UART {
    pub fn new() -> Self {
        UART {
            from_emu_buffer: Default::default(),
            to_emu_buffer: Default::default(),
        }
    }
    pub fn try_get_byte(&mut self) -> Option<u8> {
        self.from_emu_buffer.pop_front()
    }
    pub fn emu_push(&mut self, byte: u8) {
        self.from_emu_buffer.push_back(byte)
    }
}
