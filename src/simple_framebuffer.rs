//! Simple framebuffer device for the Linux `simple-framebuffer` preudo-driver.
//!
//! Represents 640x480 a8r8g8b8 surface internally divided into rows for granular change detection via atomic dirty flag.
//! Primary way to fetch row in general is through `ScreenBufferRow::content`, updated row is through `ScreenBufferRow::take_if_dirty`(automatically clears dirty flag) function

use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

use crate::{mmdevice::MMdevice, traps::TrapType};

pub const FRAMEBUFFER_WIDTH: u32 = 640;
pub const FRAMEBUFFER_HEIGHT: u32 = 480;
pub const FRAMEBUFFER_BYTES_PER_PIXEL: u32 = 4;
pub const FRAMEBUFFER_STRIDE: u32 = FRAMEBUFFER_WIDTH * FRAMEBUFFER_BYTES_PER_PIXEL;
pub const FRAMEBUFFER_SIZE: u32 = FRAMEBUFFER_STRIDE * FRAMEBUFFER_HEIGHT;

pub type Pixel = u32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelFormat {
    /// 32 bits per pixel, stored as 0xAARRGGBB in guest memory.
    A8R8G8B8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenBufferParams {
    pub width: u32,
    pub height: u32,
    /// Bytes per row, including any padding.
    pub stride: u32,
    pub format: PixelFormat,
}

impl ScreenBufferParams {
    pub const fn bytes_per_pixel(&self) -> u32 {
        FRAMEBUFFER_BYTES_PER_PIXEL
    }
    pub const fn size(&self) -> u32 {
        self.stride * self.height
    }
    pub const fn pixels_per_row(&self) -> usize {
        (self.stride / FRAMEBUFFER_BYTES_PER_PIXEL) as usize
    }
}

pub struct ScreenBufferRow {
    row_content: Mutex<Box<[Pixel]>>,
    is_dirty: AtomicBool,
}

impl ScreenBufferRow {
    fn new(pixel_count: usize) -> Self {
        ScreenBufferRow {
            row_content: Mutex::new(vec![0; pixel_count].into_boxed_slice()),
            //The row has never been presented yet.
            is_dirty: AtomicBool::new(true),
        }
    }
    pub fn content(&self) -> MutexGuard<'_, Box<[Pixel]>> {
        self.row_content
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    pub fn is_dirty(&self) -> bool {
        self.is_dirty.load(Ordering::Acquire)
    }
    /// Clears flag and returns wrapped buffer.
    pub fn take_if_dirty(&self) -> Option<MutexGuard<'_, Box<[Pixel]>>> {
        if self.take_dirty() {
            Some(self.content())
        } else {
            None
        }
    }
    // Clears the flag and reports whether the row changed since the previous call.
    fn take_dirty(&self) -> bool {
        self.is_dirty.swap(false, Ordering::AcqRel)
    }
    fn mark_dirty(&self) {
        self.is_dirty.store(true, Ordering::Release);
    }
}

pub struct SharedScreenBuffer {
    rows: Box<[ScreenBufferRow]>,
    params: ScreenBufferParams,
}

impl SharedScreenBuffer {
    pub fn new(params: ScreenBufferParams) -> Self {
        assert!(params.width > 0 && params.height > 0);
        assert!(params.stride >= params.width * FRAMEBUFFER_BYTES_PER_PIXEL);
        assert_eq!(params.stride % FRAMEBUFFER_BYTES_PER_PIXEL, 0);
        let pixels_per_row = params.pixels_per_row();
        let rows = (0..params.height)
            .map(|_| ScreenBufferRow::new(pixels_per_row))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        SharedScreenBuffer { rows, params }
    }
    pub fn params(&self) -> ScreenBufferParams {
        self.params
    }
    pub fn rows(&self) -> &[ScreenBufferRow] {
        &self.rows
    }
    pub fn row(&self, index: u32) -> Option<&ScreenBufferRow> {
        self.rows.get(index as usize)
    }
    fn read_bytes(&self, offset: u32, len: u32) -> Option<u32> {
        if offset.checked_add(len)? > self.params.size() {
            return None;
        }
        let row_index = offset / self.params.stride;
        let offset_in_row = offset % self.params.stride;
        let mut value = 0u32;
        if offset_in_row + len <= self.params.stride {
            //The whole access lives in one row, so take its lock once.
            let row = self.rows.get(row_index as usize)?;
            let content = row.content();
            for i in 0..len as usize {
                let byte_offset = offset_in_row as usize + i;
                let byte = (content.get(byte_offset / 4)? >> ((byte_offset % 4) * 8)) as u8;
                value |= (byte as u32) << (i * 8);
            }
        } else {
            //A rare access that crosses a row boundary; handle each byte separately.
            for i in 0..len {
                let address = offset + i;
                let row = self.rows.get((address / self.params.stride) as usize)?;
                let byte_offset = (address % self.params.stride) as usize;
                let content = row.content();
                let byte = (content.get(byte_offset / 4)? >> ((byte_offset % 4) * 8)) as u8;
                value |= (byte as u32) << (i * 8);
            }
        }
        Some(value)
    }
    fn write_bytes(&self, offset: u32, len: u32, value: u32) -> bool {
        let Some(end) = offset.checked_add(len) else {
            return false;
        };
        if end > self.params.size() {
            return false;
        }
        let row_index = offset / self.params.stride;
        let offset_in_row = offset % self.params.stride;
        if offset_in_row + len <= self.params.stride {
            let Some(row) = self.rows.get(row_index as usize) else {
                return false;
            };
            {
                let mut content = row.content();
                for i in 0..len as usize {
                    let byte_offset = offset_in_row as usize + i;
                    let Some(pixel) = content.get_mut(byte_offset / 4) else {
                        return false;
                    };
                    let shift = (byte_offset % 4) * 8;
                    *pixel = (*pixel & !(0xff << shift)) | (((value >> (i * 8)) & 0xff) << shift);
                }
            }
            row.mark_dirty();
        } else {
            //A rare access that crosses a row boundary; mark both rows dirty.
            for i in 0..len {
                let address = offset + i;
                let Some(row) = self.rows.get((address / self.params.stride) as usize) else {
                    return false;
                };
                let byte_offset = (address % self.params.stride) as usize;
                {
                    let mut content = row.content();
                    let Some(pixel) = content.get_mut(byte_offset / 4) else {
                        return false;
                    };
                    let shift = (byte_offset % 4) * 8;
                    *pixel = (*pixel & !(0xff << shift)) | (((value >> (i * 8)) & 0xff) << shift);
                }
                row.mark_dirty();
            }
        }
        true
    }
}

pub struct SimpleScreenbuffer {
    internal: Arc<SharedScreenBuffer>,
}

impl SimpleScreenbuffer {
    pub fn new() -> Self {
        SimpleScreenbuffer {
            internal: Arc::new(SharedScreenBuffer::new(ScreenBufferParams {
                width: FRAMEBUFFER_WIDTH,
                height: FRAMEBUFFER_HEIGHT,
                stride: FRAMEBUFFER_STRIDE,
                format: PixelFormat::A8R8G8B8,
            })),
        }
    }
    //The display thread keeps its own Arc and reads rows through it.
    pub fn shared(&self) -> Arc<SharedScreenBuffer> {
        Arc::clone(&self.internal)
    }
    pub fn params(&self) -> ScreenBufferParams {
        self.internal.params()
    }
    fn check_bounds(&self, offset: u32, len: u32, cause: TrapType) -> Result<(), TrapType> {
        match offset.checked_add(len) {
            Some(end) if end <= self.internal.params().size() => Ok(()),
            _ => Err(cause),
        }
    }
    fn read(&self, offset: u32, len: u32) -> Result<u32, TrapType> {
        self.check_bounds(offset, len, TrapType::LoadAccessFault)?;
        self.internal
            .read_bytes(offset, len)
            .ok_or(TrapType::LoadAccessFault)
    }
    fn write(&self, offset: u32, len: u32, value: u32) -> Result<(), TrapType> {
        self.check_bounds(offset, len, TrapType::StoreAccessFault)?;
        if self.internal.write_bytes(offset, len, value) {
            Ok(())
        } else {
            Err(TrapType::StoreAccessFault)
        }
    }
}

impl Default for SimpleScreenbuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl MMdevice for SimpleScreenbuffer {
    const MEMORY_REGION_SIZE: u32 = FRAMEBUFFER_SIZE;

    fn read_word(&mut self, offset: u32) -> Result<u32, TrapType> {
        self.read(offset, 4)
    }
    fn write_word(&mut self, offset: u32, word: u32) -> Result<(), TrapType> {
        self.write(offset, 4, word)
    }
    fn read_halfword(&mut self, offset: u32) -> Result<u16, TrapType> {
        self.read(offset, 2).map(|value| value as u16)
    }
    fn write_halfword(&mut self, offset: u32, halfword: u16) -> Result<(), TrapType> {
        self.write(offset, 2, halfword as u32)
    }
    fn read_byte(&mut self, offset: u32) -> Result<u8, TrapType> {
        self.read(offset, 1).map(|value| value as u8)
    }
    fn write_byte(&mut self, offset: u32, byte: u8) -> Result<(), TrapType> {
        self.write(offset, 1, byte as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::{AtomicAccess, FRAMEBUFFER_ADDRESS, MemoryBus};

    #[test]
    fn params_match_the_device_tree() {
        let framebuffer = SimpleScreenbuffer::new();
        assert_eq!(FRAMEBUFFER_WIDTH, 640);
        assert_eq!(FRAMEBUFFER_HEIGHT, 480);
        assert_eq!(FRAMEBUFFER_STRIDE, 2560);
        assert_eq!(FRAMEBUFFER_SIZE, 0x12c000);
        assert_eq!(framebuffer.params().size(), FRAMEBUFFER_SIZE);
        assert_eq!(framebuffer.params().format, PixelFormat::A8R8G8B8);
    }

    #[test]
    fn word_accesses_update_pixels_and_row_dirty_flags() {
        let mut framebuffer = SimpleScreenbuffer::new();
        let shared = framebuffer.shared();
        assert!(shared.rows().iter().all(ScreenBufferRow::is_dirty));
        for row in shared.rows() {
            assert!(row.take_dirty());
        }

        framebuffer.write_word(0, 0x1122_3344).unwrap();
        assert_eq!(shared.row(0).unwrap().content()[0], 0x1122_3344);
        assert!(shared.row(0).unwrap().take_if_dirty().is_some());
        assert!(shared.row(0).unwrap().take_if_dirty().is_none());
        assert!(shared.row(1).unwrap().take_if_dirty().is_none());
        assert_eq!(framebuffer.read_word(0).unwrap(), 0x1122_3344);
    }

    #[test]
    fn byte_accesses_are_little_endian() {
        let mut framebuffer = SimpleScreenbuffer::new();
        framebuffer.write_byte(1, 0xaa).unwrap();
        assert_eq!(framebuffer.read_word(0).unwrap(), 0x0000_aa00);
        assert_eq!(framebuffer.read_byte(1).unwrap(), 0xaa);
        assert_eq!(framebuffer.read_halfword(0).unwrap(), 0xaa00);
    }

    #[test]
    fn accesses_can_cross_a_row_boundary() {
        let mut framebuffer = SimpleScreenbuffer::new();
        let shared = framebuffer.shared();
        for row in shared.rows() {
            assert!(row.take_dirty());
        }
        let offset = FRAMEBUFFER_STRIDE - 1;
        framebuffer.write_word(offset, 0x4433_2211).unwrap();
        assert_eq!(framebuffer.read_word(offset).unwrap(), 0x4433_2211);
        assert_eq!(framebuffer.read_byte(offset).unwrap(), 0x11);
        assert_eq!(framebuffer.read_byte(FRAMEBUFFER_STRIDE).unwrap(), 0x22);
        assert_eq!(framebuffer.read_byte(FRAMEBUFFER_STRIDE + 1).unwrap(), 0x33);
        assert_eq!(framebuffer.read_byte(FRAMEBUFFER_STRIDE + 2).unwrap(), 0x44);
        assert!(shared.row(0).unwrap().take_dirty());
        assert!(shared.row(1).unwrap().take_dirty());
    }

    #[test]
    fn halfword_can_cross_a_row_boundary() {
        let mut framebuffer = SimpleScreenbuffer::new();
        let shared = framebuffer.shared();
        for row in shared.rows() {
            assert!(row.take_dirty());
        }
        let offset = FRAMEBUFFER_STRIDE - 1;
        framebuffer.write_halfword(offset, 0x2211).unwrap();
        assert_eq!(framebuffer.read_halfword(offset).unwrap(), 0x2211);
        assert_eq!(framebuffer.read_byte(offset).unwrap(), 0x11);
        assert_eq!(framebuffer.read_byte(FRAMEBUFFER_STRIDE).unwrap(), 0x22);
        assert!(shared.row(0).unwrap().take_dirty());
        assert!(shared.row(1).unwrap().take_dirty());
    }

    #[test]
    fn accesses_outside_the_region_fault() {
        let mut framebuffer = SimpleScreenbuffer::new();
        assert_eq!(
            framebuffer.read_word(FRAMEBUFFER_SIZE - 3).unwrap_err(),
            TrapType::LoadAccessFault
        );
        assert_eq!(
            framebuffer.write_word(FRAMEBUFFER_SIZE - 3, 0).unwrap_err(),
            TrapType::StoreAccessFault
        );
        assert_eq!(
            framebuffer.read_byte(FRAMEBUFFER_SIZE).unwrap_err(),
            TrapType::LoadAccessFault
        );
        assert_eq!(
            framebuffer.write_byte(FRAMEBUFFER_SIZE, 0).unwrap_err(),
            TrapType::StoreAccessFault
        );
        assert_eq!(
            framebuffer.write_byte(u32::MAX, 0).unwrap_err(),
            TrapType::StoreAccessFault
        );
    }

    #[test]
    fn the_last_word_is_accessible() {
        let mut framebuffer = SimpleScreenbuffer::new();
        framebuffer
            .write_word(FRAMEBUFFER_SIZE - 4, 0xdead_beef)
            .unwrap();
        assert_eq!(
            framebuffer.read_word(FRAMEBUFFER_SIZE - 4).unwrap(),
            0xdead_beef
        );
    }

    #[test]
    fn faulting_store_leaves_memory_unchanged() {
        let mut framebuffer = SimpleScreenbuffer::new();
        framebuffer.write_word(0, 0x1122_3344).unwrap();
        assert_eq!(
            framebuffer
                .write_word(FRAMEBUFFER_SIZE - 3, 0xaabb_ccdd)
                .unwrap_err(),
            TrapType::StoreAccessFault
        );
        assert_eq!(framebuffer.read_word(0).unwrap(), 0x1122_3344);
        for offset in FRAMEBUFFER_SIZE - 3..FRAMEBUFFER_SIZE {
            assert_eq!(framebuffer.read_byte(offset).unwrap(), 0);
        }
    }

    #[test]
    fn instruction_fetch_and_atomics_fault() {
        let mut framebuffer = SimpleScreenbuffer::new();
        assert_eq!(
            framebuffer.fetch_halfword(0).unwrap_err(),
            TrapType::InstructionAccessFault
        );
        assert_eq!(
            framebuffer.fetch_halfword(FRAMEBUFFER_SIZE).unwrap_err(),
            TrapType::InstructionAccessFault
        );
        assert_eq!(
            framebuffer.read_word_atomically(0).unwrap_err(),
            TrapType::LoadAccessFault
        );
        assert_eq!(
            framebuffer.write_word_atomically(0, 0).unwrap_err(),
            TrapType::StoreAccessFault
        );

        let (mut bus, _audio) = MemoryBus::new();
        assert_eq!(
            bus.read_word_atomically(FRAMEBUFFER_ADDRESS, AtomicAccess::Load)
                .unwrap_err()
                .tcause,
            TrapType::LoadAccessFault
        );
        assert_eq!(
            bus.read_word_atomically(FRAMEBUFFER_ADDRESS, AtomicAccess::Store)
                .unwrap_err()
                .tcause,
            TrapType::StoreAccessFault
        );
        assert_eq!(
            bus.write_word_atomically(FRAMEBUFFER_ADDRESS, 0)
                .unwrap_err()
                .tcause,
            TrapType::StoreAccessFault
        );
    }

    #[test]
    fn dword_range_checks_prevent_partial_stores() {
        let (mut bus, _audio) = MemoryBus::new();
        let start = FRAMEBUFFER_ADDRESS + FRAMEBUFFER_SIZE - 8;
        bus.write_dword(start, 0x1122_3344_5566_7788).unwrap();
        assert_eq!(bus.read_dword(start).unwrap(), 0x1122_3344_5566_7788);

        let last_word = FRAMEBUFFER_ADDRESS + FRAMEBUFFER_SIZE - 4;
        let trap = bus
            .write_dword(last_word, 0xdead_beef_dead_beef)
            .unwrap_err();
        assert_eq!(trap.tcause, TrapType::StoreAccessFault);
        assert_eq!(trap.tval, FRAMEBUFFER_ADDRESS + FRAMEBUFFER_SIZE);
        assert_eq!(bus.read_word(last_word).unwrap(), 0x1122_3344);
    }

    #[test]
    fn shared_buffer_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SharedScreenBuffer>();
        assert_send_sync::<ScreenBufferRow>();
        assert_send_sync::<SimpleScreenbuffer>();
    }

    #[test]
    fn the_bus_routes_the_framebuffer_region() {
        let (mut bus, _audio) = MemoryBus::new();
        let shared = bus.framebuffer.shared();
        bus.write_word(FRAMEBUFFER_ADDRESS, 0x1122_3344).unwrap();
        assert_eq!(bus.read_word(FRAMEBUFFER_ADDRESS).unwrap(), 0x1122_3344);
        assert_eq!(shared.row(0).unwrap().content()[0], 0x1122_3344);
        assert_eq!(
            bus.read_word(FRAMEBUFFER_ADDRESS + FRAMEBUFFER_SIZE)
                .unwrap_err()
                .tcause,
            TrapType::LoadAccessFault
        );
    }
}
