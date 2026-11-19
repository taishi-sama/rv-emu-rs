use crate::ram::GuestMemory;

pub const DESC_F_NEXT: u16 = 1;
pub const DESC_F_WRITE: u16 = 2;
pub const DESC_F_INDIRECT: u16 = 4;

const AVAIL_F_NO_INTERRUPT: u16 = 1;

#[derive(Clone, Copy, Debug, Default)]
pub struct Descriptor {
    pub addr: u64,
    pub len: u32,
    pub flags: u16,
    pub next: u16,
}

pub struct DescriptorChain {
    pub head: u16,
    pub descriptors: Vec<Descriptor>,
    pub malformed: bool,
}

//Split virtqueue state. The driver owns the descriptor table and available ring; the device owns
//the used ring. All addresses are guest physical addresses.
#[derive(Default)]
pub struct SplitQueue {
    pub num: u32,
    pub ready: bool,
    pub desc: u64,
    pub avail: u64,
    pub used: u64,
    last_avail: u16,
    generation: u64,
}

impl SplitQueue {
    //The driver reinitializes a queue by writing QueueReady=0. Invalidate every token produced
    //before this point so late worker completions cannot complete a reused descriptor.
    pub fn reset_ready(&mut self) {
        self.ready = false;
        self.last_avail = 0;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn reset(&mut self) {
        self.num = 0;
        self.ready = false;
        self.desc = 0;
        self.avail = 0;
        self.used = 0;
        self.last_avail = 0;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn set_num(&mut self, num: u32) {
        self.num = num;
        self.ready = false;
        self.last_avail = 0;
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn set_ready(&mut self, ready: bool) {
        if ready {
            self.ready = true;
            self.last_avail = 0;
        } else {
            self.reset_ready();
        }
    }

    //Tokens identify an in-flight descriptor chain across a queue reset.
    pub fn token(&self, head: u16) -> u64 {
        (self.generation << 16) | head as u64
    }

    pub fn pop_available(&mut self, mem: &mut dyn GuestMemory) -> Option<DescriptorChain> {
        if !self.ready || self.num == 0 {
            return None;
        }
        let avail_idx = read_u16(mem, checked_add(self.avail, 2)?)?;
        if avail_idx == self.last_avail {
            return None;
        }
        //A driver that overruns the ring can only have the last queue-size entries live.
        if avail_idx.wrapping_sub(self.last_avail) as u32 > self.num {
            self.last_avail = avail_idx.wrapping_sub(self.num as u16);
        }
        let slot = (self.last_avail % self.num as u16) as u64;
        let head = read_u16(mem, checked_add(self.avail, 4 + slot * 2)?)?;
        self.last_avail = self.last_avail.wrapping_add(1);

        let mut descriptors = Vec::new();
        let mut malformed = head >= self.num as u16;
        let mut index = head;
        if !malformed {
            for _ in 0..self.num {
                let Some(address) = checked_add(self.desc, index as u64 * 16) else {
                    malformed = true;
                    break;
                };
                let Some(descriptor) = read_descriptor(mem, address) else {
                    malformed = true;
                    break;
                };
                descriptors.push(descriptor);
                if descriptor.flags & DESC_F_NEXT == 0 {
                    break;
                }
                if descriptor.next >= self.num as u16 {
                    malformed = true;
                    break;
                }
                index = descriptor.next;
            }
            if descriptors.len() == self.num as usize
                && descriptors
                    .last()
                    .is_some_and(|descriptor| descriptor.flags & DESC_F_NEXT != 0)
            {
                malformed = true;
            }
        }
        Some(DescriptorChain {
            head,
            descriptors,
            malformed,
        })
    }

    //Publish a completion and return whether the device should raise its interrupt. The used
    //entry is written even when the driver asked for no interrupt.
    pub fn push_used(&mut self, mem: &mut dyn GuestMemory, head: u16, len: u32) -> bool {
        if !self.ready || self.num == 0 {
            return false;
        }
        let Some(index_address) = checked_add(self.used, 2) else {
            return false;
        };
        let Some(used_idx) = read_u16(mem, index_address) else {
            return false;
        };
        let slot = (used_idx % self.num as u16) as u64;
        let element = match checked_add(self.used, 4 + slot * 8) {
            Some(element) => element,
            None => return false,
        };
        if !write_u32(mem, element, head as u32) {
            return false;
        }
        let Some(element_len) = element.checked_add(4) else {
            return false;
        };
        if !write_u32(mem, element_len, len) {
            return false;
        }
        let Some(index_address) = checked_add(self.used, 2) else {
            return false;
        };
        if !write_u16(mem, index_address, used_idx.wrapping_add(1)) {
            return false;
        }
        match read_u16(mem, self.avail) {
            Some(flags) => flags & AVAIL_F_NO_INTERRUPT == 0,
            None => true,
        }
    }
}

fn checked_add(base: u64, offset: u64) -> Option<u64> {
    base.checked_add(offset)
}

fn read_descriptor(mem: &mut dyn GuestMemory, address: u64) -> Option<Descriptor> {
    let mut bytes = [0u8; 16];
    read_bytes(mem, address, &mut bytes).then(|| Descriptor {
        addr: u64::from_le_bytes(bytes[0..8].try_into().unwrap()),
        len: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        flags: u16::from_le_bytes(bytes[12..14].try_into().unwrap()),
        next: u16::from_le_bytes(bytes[14..16].try_into().unwrap()),
    })
}

fn read_bytes(mem: &mut dyn GuestMemory, address: u64, out: &mut [u8]) -> bool {
    let Ok(address) = u32::try_from(address) else {
        return false;
    };
    mem.read_bytes(address, out)
}

fn read_u16(mem: &mut dyn GuestMemory, address: u64) -> Option<u16> {
    let mut bytes = [0u8; 2];
    read_bytes(mem, address, &mut bytes).then(|| u16::from_le_bytes(bytes))
}

fn write_u16(mem: &mut dyn GuestMemory, address: u64, value: u16) -> bool {
    let Ok(address) = u32::try_from(address) else {
        return false;
    };
    mem.write_bytes(address, &value.to_le_bytes())
}

fn write_u32(mem: &mut dyn GuestMemory, address: u64, value: u32) -> bool {
    let Ok(address) = u32::try_from(address) else {
        return false;
    };
    mem.write_bytes(address, &value.to_le_bytes())
}
