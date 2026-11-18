use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use rb::{RB, RbConsumer, RbInspector, RbProducer};
use rodio::Source;

use crate::{mmdevice::MMdevice, traps::TrapType};

//Simplest possible fixed-format stereo audio device: the guest pushes 16-bit signed samples one
//halfword at a time, the host pulls them at the rate programmed in the sample-rate register.
//Word reads at offset 0 report how many samples are still queued, so a guest can wait for drain.
//Every access the device does not implement faults, matching how the DTB describes the node (two
//registers, 8 bytes, mmio).
//
//The sample rate is part of the device state rather than a host constant: the guest reads the
//reset value to learn what the device is clocked at, and can write a new rate at any time. The
//audio source reports the register and returns a finite frame length, which is what makes rodio
//re-read `sample_rate()` and retune playback when the guest changes it.
pub const DEFAULT_SAMPLE_RATE: u32 = 44100;
pub const SAMPLE_RATE_MIN: u32 = 8000;
pub const SAMPLE_RATE_MAX: u32 = 192_000;
const SAMPLE_RATE_OFFSET: u32 = 4;

pub struct PrimitiveAudioReceiver {
    internal_buffer: rb::Producer<i16>,
    buffer: rb::SpscRb<i16>,
    sample_rate: Arc<AtomicU32>,
}
impl PrimitiveAudioReceiver {
    pub fn write(&mut self, val: i16) {
        // Drop samples on full queue.
        let _ = self.internal_buffer.write(&[val]);
    }
    pub fn get_size(&self) -> u32 {
        self.buffer.count() as u32
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
    }
    fn set_sample_rate(&self, rate: u32) {
        self.sample_rate
            .store(rate.clamp(SAMPLE_RATE_MIN, SAMPLE_RATE_MAX), Ordering::Relaxed);
    }
}

impl MMdevice for PrimitiveAudioReceiver {
    const MEMORY_REGION_SIZE: u32 = 8;

    fn read_word(&mut self, offset: u32) -> Result<u32, TrapType> {
        match offset {
            0 => Ok(self.get_size()),
            SAMPLE_RATE_OFFSET => Ok(self.sample_rate()),
            _ => Err(TrapType::LoadAccessFault),
        }
    }
    fn write_word(&mut self, offset: u32, word: u32) -> Result<(), TrapType> {
        match offset {
            SAMPLE_RATE_OFFSET => {
                self.set_sample_rate(word);
                Ok(())
            }
            _ => Err(TrapType::StoreAccessFault),
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
}

pub struct PrimitiveAudioProducer {
    internal_buffer: rb::Consumer<i16>,
    sample_rate: Arc<AtomicU32>,
}

impl Source for PrimitiveAudioProducer {
    fn current_frame_len(&self) -> Option<usize> {
        //A finite run length is required for rodio to notice a rate change: the engine
        //re-reads `channels()` and `sample_rate()` when the current frame ends. About
        //100 ms keeps the rechecks cheap and the response prompt.
        let frames = (self.sample_rate() as usize / 10).max(1);
        Some(frames * self.channels() as usize)
    }

    fn channels(&self) -> u16 {
        2
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate.load(Ordering::Relaxed)
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
    //About 4 seconds of stereo backlog at the default rate (less at higher rates)
    //before the receiver starts dropping samples.
    let buffer = rb::SpscRb::new(DEFAULT_SAMPLE_RATE as usize * 8);
    let sample_rate = Arc::new(AtomicU32::new(DEFAULT_SAMPLE_RATE));
    let producer = PrimitiveAudioProducer {
        internal_buffer: buffer.consumer(),
        sample_rate: Arc::clone(&sample_rate),
    };
    (
        PrimitiveAudioReceiver {
            internal_buffer: buffer.producer(),
            buffer,
            sample_rate,
        },
        producer,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rate_register_defaults_and_clamps() {
        let (mut receiver, producer) = create_primitive_audio_pair();
        assert_eq!(receiver.read_word(SAMPLE_RATE_OFFSET), Ok(DEFAULT_SAMPLE_RATE));
        assert_eq!(producer.sample_rate(), DEFAULT_SAMPLE_RATE);

        receiver.write_word(SAMPLE_RATE_OFFSET, 48000).unwrap();
        assert_eq!(receiver.read_word(SAMPLE_RATE_OFFSET), Ok(48000));
        assert_eq!(producer.sample_rate(), 48000);

        receiver.write_word(SAMPLE_RATE_OFFSET, 0).unwrap();
        assert_eq!(producer.sample_rate(), SAMPLE_RATE_MIN);

        receiver.write_word(SAMPLE_RATE_OFFSET, u32::MAX).unwrap();
        assert_eq!(producer.sample_rate(), SAMPLE_RATE_MAX);
    }

    #[test]
    fn finite_frame_length_makes_rodio_recheck_the_rate() {
        let (mut receiver, producer) = create_primitive_audio_pair();
        let at_default = producer.current_frame_len().unwrap();
        assert!(at_default > 0);

        receiver.write_word(SAMPLE_RATE_OFFSET, 96000).unwrap();
        let at_double = producer.current_frame_len().unwrap();
        assert!(at_double > at_default);
    }

    #[test]
    fn queue_counts_written_samples() {
        let (mut receiver, _producer) = create_primitive_audio_pair();
        assert_eq!(receiver.read_word(0), Ok(0));

        receiver.write_halfword(0, (-1i16) as u16).unwrap();
        receiver.write_halfword(0, 0x1234).unwrap();
        assert_eq!(receiver.read_word(0), Ok(2));
    }

    #[test]
    fn unimplemented_accesses_fault() {
        let (mut receiver, _producer) = create_primitive_audio_pair();
        assert_eq!(
            receiver.write_word(0, 0),
            Err(TrapType::StoreAccessFault)
        );
        assert_eq!(receiver.read_word(8), Err(TrapType::LoadAccessFault));
        assert_eq!(
            receiver.write_halfword(SAMPLE_RATE_OFFSET, 0),
            Err(TrapType::StoreAccessFault)
        );
    }
}
