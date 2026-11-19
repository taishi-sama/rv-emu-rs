use crate::{
    mmdevice::MMdevice,
    plic::{InterruptSource, IrqLine, Plic},
    ram::GuestMemory,
    traps::TrapType,
    virtio::{
        CONFIG_OFFSET, FEATURE_VERSION_1, INT_VRING, MAGIC, STATUS_DRIVER_OK, STATUS_FAILED,
        STATUS_FEATURES_OK, VENDOR_ID, VERSION, queue::SplitQueue,
    },
};

const MAGIC_VALUE: u32 = 0x000;
const VERSION_REG: u32 = 0x004;
const DEVICE_ID: u32 = 0x008;
const VENDOR_ID_REG: u32 = 0x00c;
const DEVICE_FEATURES: u32 = 0x010;
const DEVICE_FEATURES_SEL: u32 = 0x014;
const DRIVER_FEATURES: u32 = 0x020;
const DRIVER_FEATURES_SEL: u32 = 0x024;
const QUEUE_SEL: u32 = 0x030;
const QUEUE_NUM_MAX: u32 = 0x034;
const QUEUE_NUM: u32 = 0x038;
const QUEUE_ALIGN: u32 = 0x03c;
const QUEUE_PFN: u32 = 0x040;
const QUEUE_READY: u32 = 0x044;
const QUEUE_NOTIFY: u32 = 0x050;
const INTERRUPT_STATUS: u32 = 0x060;
const INTERRUPT_ACK: u32 = 0x064;
const STATUS: u32 = 0x070;
const QUEUE_DESC_LOW: u32 = 0x080;
const QUEUE_DESC_HIGH: u32 = 0x084;
const QUEUE_AVAIL_LOW: u32 = 0x090;
const QUEUE_AVAIL_HIGH: u32 = 0x094;
const QUEUE_USED_LOW: u32 = 0x0a0;
const QUEUE_USED_HIGH: u32 = 0x0a4;
const CONFIG_GENERATION: u32 = 0x0fc;

//Device-specific side of a virtio-mmio device. The transport owns the register file, queue
//selection and interrupt line; the device owns feature bits, config space and request handling.
pub trait VirtioDevice: Send {
    fn device_id(&self) -> u32;
    fn features(&self) -> u64;
    fn config_read(&self, offset: u32, out: &mut [u8]);
    fn config_generation(&self) -> u32 {
        0
    }
    fn queue_count(&self) -> u16;
    fn queue_max(&self, queue: u16) -> u32;
    fn queue_num_changed(&mut self, _queue: u16, _num: u32) {}
    fn reset(&mut self);
    fn reset_queue(&mut self, queue: u16);
    //Submit all currently available requests. Returns true if a completion was published.
    fn notify(&mut self, queue: u16, q: &mut SplitQueue, mem: &mut dyn GuestMemory) -> bool;
    //Drain asynchronous completions. Returns true if a completion was published.
    fn poll(&mut self, queue: u16, q: &mut SplitQueue, mem: &mut dyn GuestMemory) -> bool;
}

//A virtio-mmio slot with no device behind it. Linux treats DeviceID 0 as a placeholder and does
//not bind a driver, which lets the DTS node exist even when no disk was passed on the command line.
pub struct NoDevice;

impl VirtioDevice for NoDevice {
    fn device_id(&self) -> u32 {
        0
    }
    fn features(&self) -> u64 {
        0
    }
    fn config_read(&self, _offset: u32, _out: &mut [u8]) {}
    fn queue_count(&self) -> u16 {
        0
    }
    fn queue_max(&self, _queue: u16) -> u32 {
        0
    }
    fn reset(&mut self) {}
    fn reset_queue(&mut self, _queue: u16) {}
    fn notify(&mut self, _queue: u16, _q: &mut SplitQueue, _mem: &mut dyn GuestMemory) -> bool {
        false
    }
    fn poll(&mut self, _queue: u16, _q: &mut SplitQueue, _mem: &mut dyn GuestMemory) -> bool {
        false
    }
}

pub struct VirtioMmio {
    irq: IrqLine,
    device: Box<dyn VirtioDevice>,
    queues: Vec<SplitQueue>,
    //NoDevice has no queues. Skip the per-batch poll entirely for the empty placeholder.
    active: bool,
    queue_sel: u32,
    device_features_sel: u32,
    driver_features_sel: u32,
    driver_features: u64,
    status: u8,
    interrupt_status: u32,
    notify_pending: u32,
}

impl VirtioMmio {
    pub fn new(irq: IrqLine, device: Box<dyn VirtioDevice>) -> Self {
        let queues: Vec<SplitQueue> = (0..device.queue_count())
            .map(|_| SplitQueue::default())
            .collect();
        let active = !queues.is_empty();
        Self {
            irq,
            device,
            queues,
            active,
            queue_sel: 0,
            device_features_sel: 0,
            driver_features_sel: 0,
            driver_features: 0,
            status: 0,
            interrupt_status: 0,
            notify_pending: 0,
        }
    }

    pub fn irq(&self) -> IrqLine {
        self.irq
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.active
    }

    //Advance every queue: submit work that was kicked, then drain any completions the backend
    //finished since the last call. This never blocks; the backend owns the host I/O thread.
    pub fn poll(&mut self, mem: &mut dyn GuestMemory) {
        let mut interrupt = false;
        for index in 0..self.queues.len() {
            let bit = queue_bit(index);
            let pending = self.notify_pending & bit != 0;
            if pending {
                self.notify_pending &= !bit;
                if self.status & STATUS_DRIVER_OK != 0 {
                    interrupt |= self
                        .device
                        .notify(index as u16, &mut self.queues[index], mem);
                }
            }
            interrupt |= self.device.poll(index as u16, &mut self.queues[index], mem);
        }
        if interrupt {
            self.interrupt_status |= INT_VRING;
        }
    }

    fn queue(&self) -> Option<&SplitQueue> {
        self.queues.get(self.queue_sel as usize)
    }

    fn queue_mut(&mut self) -> Option<&mut SplitQueue> {
        self.queues.get_mut(self.queue_sel as usize)
    }

    fn write_queue_num(&mut self, value: u32) {
        let index = self.queue_sel as usize;
        if index >= self.queues.len() || self.queues[index].ready {
            return;
        }
        let max = self.device.queue_max(index as u16);
        if value == 0 || !value.is_power_of_two() || value > max {
            return;
        }
        //A driver may only change QueueNum while the queue is disabled, but invalidate any
        //device-side in-flight state anyway so a broken driver cannot complete into a new ring.
        self.device.reset_queue(index as u16);
        self.queues[index].set_num(value);
        self.notify_pending &= !queue_bit(index);
        self.device.queue_num_changed(index as u16, value);
    }

    fn write_queue_ready(&mut self, value: u32) {
        let index = self.queue_sel as usize;
        let Some(queue) = self.queues.get_mut(index) else {
            return;
        };
        if value & 1 != 0 {
            let max = self.device.queue_max(index as u16);
            if !queue.ready && queue.num != 0 && queue.num <= max {
                queue.set_ready(true);
            }
        } else {
            queue.reset_ready();
            self.device.reset_queue(index as u16);
            self.notify_pending &= !queue_bit(index);
        }
    }

    fn write_status(&mut self, value: u32) {
        let mut status = value as u8;
        if status == 0 {
            self.reset();
            return;
        }
        if status & STATUS_FEATURES_OK != 0 {
            let unsupported = self.driver_features & !self.device.features();
            if self.driver_features & FEATURE_VERSION_1 == 0 || unsupported != 0 {
                status = (status & !STATUS_FEATURES_OK) | STATUS_FAILED;
            }
        }
        self.status = status;
    }

    fn reset(&mut self) {
        self.status = 0;
        self.queue_sel = 0;
        self.driver_features = 0;
        self.device_features_sel = 0;
        self.driver_features_sel = 0;
        self.interrupt_status = 0;
        self.notify_pending = 0;
        for queue in &mut self.queues {
            queue.reset();
        }
        self.device.reset();
    }

    fn read_config(&mut self, offset: u32, out: &mut [u8]) {
        out.fill(0);
        self.device.config_read(offset, out);
    }
}

impl MMdevice for VirtioMmio {
    const MEMORY_REGION_SIZE: u32 = 0x1000;

    fn read_word(&mut self, offset: u32) -> Result<u32, TrapType> {
        if offset >= CONFIG_OFFSET {
            let mut bytes = [0u8; 4];
            self.read_config(offset - CONFIG_OFFSET, &mut bytes);
            return Ok(u32::from_le_bytes(bytes));
        }
        let value = match offset {
            MAGIC_VALUE => MAGIC,
            VERSION_REG => VERSION,
            DEVICE_ID => self.device.device_id(),
            VENDOR_ID_REG => VENDOR_ID,
            DEVICE_FEATURES => {
                let shift = (self.device_features_sel & 1) * 32;
                (self.device.features() >> shift) as u32
            }
            DEVICE_FEATURES_SEL => self.device_features_sel,
            DRIVER_FEATURES => {
                let shift = (self.driver_features_sel & 1) * 32;
                (self.driver_features >> shift) as u32
            }
            DRIVER_FEATURES_SEL => self.driver_features_sel,
            QUEUE_SEL => self.queue_sel,
            QUEUE_NUM_MAX => {
                let index = self.queue_sel as usize;
                if index < self.queues.len() {
                    self.device.queue_max(index as u16)
                } else {
                    0
                }
            }
            QUEUE_NUM => self.queue().map_or(0, |queue| queue.num),
            QUEUE_ALIGN | QUEUE_PFN => 0,
            QUEUE_READY => self.queue().map_or(0, |queue| queue.ready as u32),
            QUEUE_NOTIFY => 0,
            INTERRUPT_STATUS => self.interrupt_status,
            INTERRUPT_ACK => 0,
            STATUS => self.status as u32,
            QUEUE_DESC_LOW => self.queue().map_or(0, |queue| queue.desc as u32),
            QUEUE_DESC_HIGH => self.queue().map_or(0, |queue| (queue.desc >> 32) as u32),
            QUEUE_AVAIL_LOW => self.queue().map_or(0, |queue| queue.avail as u32),
            QUEUE_AVAIL_HIGH => self.queue().map_or(0, |queue| (queue.avail >> 32) as u32),
            QUEUE_USED_LOW => self.queue().map_or(0, |queue| queue.used as u32),
            QUEUE_USED_HIGH => self.queue().map_or(0, |queue| (queue.used >> 32) as u32),
            CONFIG_GENERATION => self.device.config_generation(),
            _ => 0,
        };
        Ok(value)
    }

    fn write_word(&mut self, offset: u32, word: u32) -> Result<(), TrapType> {
        if offset >= CONFIG_OFFSET {
            return Ok(());
        }
        match offset {
            DEVICE_FEATURES_SEL => self.device_features_sel = word,
            DRIVER_FEATURES => {
                let shift = (self.driver_features_sel & 1) * 32;
                let mask = u64::from(u32::MAX) << shift;
                self.driver_features = (self.driver_features & !mask) | (u64::from(word) << shift);
            }
            DRIVER_FEATURES_SEL => self.driver_features_sel = word,
            QUEUE_SEL => self.queue_sel = word,
            QUEUE_NUM => self.write_queue_num(word),
            QUEUE_ALIGN | QUEUE_PFN => {}
            QUEUE_READY => self.write_queue_ready(word),
            QUEUE_NOTIFY => {
                let index = word as usize;
                if index < self.queues.len() {
                    self.notify_pending |= queue_bit(index);
                }
            }
            INTERRUPT_ACK => self.interrupt_status &= !word,
            STATUS => self.write_status(word),
            QUEUE_DESC_LOW => {
                if let Some(queue) = self.queue_mut() {
                    queue.desc = (queue.desc & !u64::from(u32::MAX)) | u64::from(word);
                }
            }
            QUEUE_DESC_HIGH => {
                if let Some(queue) = self.queue_mut() {
                    queue.desc = (queue.desc & u64::from(u32::MAX)) | (u64::from(word) << 32);
                }
            }
            QUEUE_AVAIL_LOW => {
                if let Some(queue) = self.queue_mut() {
                    queue.avail = (queue.avail & !u64::from(u32::MAX)) | u64::from(word);
                }
            }
            QUEUE_AVAIL_HIGH => {
                if let Some(queue) = self.queue_mut() {
                    queue.avail = (queue.avail & u64::from(u32::MAX)) | (u64::from(word) << 32);
                }
            }
            QUEUE_USED_LOW => {
                if let Some(queue) = self.queue_mut() {
                    queue.used = (queue.used & !u64::from(u32::MAX)) | u64::from(word);
                }
            }
            QUEUE_USED_HIGH => {
                if let Some(queue) = self.queue_mut() {
                    queue.used = (queue.used & u64::from(u32::MAX)) | (u64::from(word) << 32);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn read_byte(&mut self, offset: u32) -> Result<u8, TrapType> {
        if offset >= CONFIG_OFFSET {
            let mut bytes = [0u8; 1];
            self.read_config(offset - CONFIG_OFFSET, &mut bytes);
            return Ok(bytes[0]);
        }
        let word = self.read_word(offset & !3)?;
        Ok((word >> ((offset & 3) * 8)) as u8)
    }

    fn write_byte(&mut self, offset: u32, _byte: u8) -> Result<(), TrapType> {
        if offset >= CONFIG_OFFSET {
            Ok(())
        } else {
            Err(TrapType::StoreAccessFault)
        }
    }

    fn read_halfword(&mut self, offset: u32) -> Result<u16, TrapType> {
        if offset >= CONFIG_OFFSET {
            let mut bytes = [0u8; 2];
            self.read_config(offset - CONFIG_OFFSET, &mut bytes);
            return Ok(u16::from_le_bytes(bytes));
        }
        let word = self.read_word(offset & !3)?;
        Ok((word >> ((offset & 2) * 8)) as u16)
    }

    fn write_halfword(&mut self, offset: u32, _halfword: u16) -> Result<(), TrapType> {
        if offset >= CONFIG_OFFSET {
            Ok(())
        } else {
            Err(TrapType::StoreAccessFault)
        }
    }
}

impl InterruptSource for VirtioMmio {
    fn poll_irqs(&mut self, plic: &mut Plic) {
        plic.set_level(self.irq, self.interrupt_status != 0);
    }
}

fn queue_bit(index: usize) -> u32 {
    if index < 32 { 1u32 << index } else { 0 }
}
