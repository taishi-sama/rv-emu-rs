use std::sync::{atomic::AtomicU32, Arc};

use rb::{RbConsumer, RbInspector, RbProducer, RB};
use rodio::Source;

use crate::{mmdevice::MMdevice, traps::TrapType};

pub struct PrimitiveAudioReceiver {
    internal_buffer: rb::Producer<i16>,
    buffer: rb::SpscRb<i16>,
}
impl PrimitiveAudioReceiver {
    pub fn write(&mut self, val: i16) {
        self.internal_buffer.write(&[val]).unwrap();
    }
    pub fn get_size(&self) -> u32 {
        self.buffer.count() as u32
    }
}

impl MMdevice for PrimitiveAudioReceiver {
    const MEMORY_REGION_SIZE: u32 = 4;

    fn fetch_halfword(&mut self, offset: u32) -> Result<u16, TrapType> {
        match offset {
            0 => todo!("Instruction fetch from the audio device is not implemented"),
            _ => Err(TrapType::InstructionAccessFault),
        }
    }
    fn read_word_atomically(&mut self, offset: u32) -> Result<u32, TrapType> {
        match offset {
            0 => todo!("Atomic access to the audio device is not implemented"),
            _ => Err(TrapType::LoadAccessFault),
        }
    }
    fn write_word_atomically(&mut self, offset: u32, _word: u32) -> Result<(), TrapType> {
        match offset {
            0 => todo!("Atomic access to the audio device is not implemented"),
            _ => Err(TrapType::StoreAccessFault),
        }
    }
    fn read_word(&mut self, offset: u32) -> Result<u32, TrapType> {
        match offset {
            0 => Ok(self.get_size()),
            _ => Err(TrapType::LoadAccessFault),
        }
    }
    fn write_word(&mut self, offset: u32, _word: u32) -> Result<(), TrapType> {
        match offset {
            0 => todo!("Audio word writes are not implemented"),
            _ => Err(TrapType::StoreAccessFault),
        }
    }
    fn read_halfword(&mut self, offset: u32) -> Result<u16, TrapType> {
        match offset {
            0 => todo!("Audio halfword reads are not implemented"),
            _ => Err(TrapType::LoadAccessFault),
        }
    }
    fn write_halfword(&mut self, offset: u32, halfword: u16) -> Result<(), TrapType> {
        match offset {
            0 => {
                self.write(halfword as i16);
                Ok(())
            }
            _ => Err(TrapType::StoreAccessFault),
        }
    }
    fn read_byte(&mut self, offset: u32) -> Result<u8, TrapType> {
        match offset {
            0 => todo!("Audio byte reads are not implemented"),
            _ => Err(TrapType::LoadAccessFault),
        }
    }
    fn write_byte(&mut self, offset: u32, _byte: u8) -> Result<(), TrapType> {
        match offset {
            0 => todo!("Audio byte writes are not implemented"),
            _ => Err(TrapType::StoreAccessFault),
        }
    }
}
pub struct PrimitiveAudioProducer {
    internal_buffer: rb::Consumer<i16>,
}
impl PrimitiveAudioProducer {}

impl Source for PrimitiveAudioProducer {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        44100
    }

    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}
impl Iterator for PrimitiveAudioProducer {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let mut arr = [0i16];
        let res = self.internal_buffer.read(&mut arr);
        if res.is_err() {
            arr[0] = 0
        }
        Some(arr[0] as f32 / (i16::MAX as f32))
    }
}

pub fn create_primitive_audio_pair() -> (PrimitiveAudioReceiver, PrimitiveAudioProducer) {
    let rb = rb::SpscRb::new(44100 * 8);
    let (cons, prod) = (rb.consumer(), rb.producer());
    (
        PrimitiveAudioReceiver {
            internal_buffer: prod,
            buffer: rb,
        },
        PrimitiveAudioProducer {
            internal_buffer: cons,
        },
    )
}
