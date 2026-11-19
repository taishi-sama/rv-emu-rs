use std::{
    collections::{HashMap, VecDeque},
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender, TryRecvError},
    },
    thread::{self, JoinHandle},
};

use crate::{
    ram::GuestMemory,
    virtio::{
        FEATURE_VERSION_1,
        mmio::VirtioDevice,
        queue::{DESC_F_INDIRECT, DESC_F_WRITE, Descriptor, DescriptorChain, SplitQueue},
    },
};

pub const IRQ: u32 = 12;
pub const QUEUE_MAX: u32 = 128;

const FEATURE_SIZE_MAX: u64 = 1 << 1;
const FEATURE_SEG_MAX: u64 = 1 << 2;
const FEATURE_RO: u64 = 1 << 5;
const FEATURE_FLUSH: u64 = 1 << 9;

const TYPE_IN: u32 = 0;
const TYPE_OUT: u32 = 1;
const TYPE_FLUSH: u32 = 4;

const STATUS_OK: u8 = 0;
const STATUS_IOERR: u8 = 1;
const STATUS_UNSUPP: u8 = 2;

//A request may contain several segments. size_max is 4 MiB and seg_max is 16, so a
//conforming driver's largest chain is 64 MiB; that total also bounds host memory.
const MAX_IO_BYTES: u64 = 64 * 1024 * 1024;
const SIZE_MAX: u32 = 4 * 1024 * 1024;
const SEG_MAX: u32 = 16;
const _: () = assert!(SIZE_MAX as u64 * SEG_MAX as u64 == MAX_IO_BYTES);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockOp {
    Read,
    Write,
    Flush,
}

pub struct BlockRequest {
    pub token: u64,
    pub op: BlockOp,
    pub offset: u64,
    pub len: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockError {
    Io,
}

pub struct BlockCompletion {
    pub token: u64,
    pub result: Result<(), BlockError>,
    pub data: Vec<u8>,
}

//The block device never touches the guest address space from the I/O side. The backend receives
//snapshots of write data and returns read data; only the virtio thread copies into guest RAM.
pub trait BlockIo: Send {
    fn capacity(&self) -> u64;
    fn is_read_only(&self) -> bool;
    fn submit(&mut self, request: BlockRequest) -> bool;
    fn poll(&mut self) -> Option<BlockCompletion>;
    fn failed(&self) -> bool {
        false
    }
    fn shutdown(&mut self) {}
}

struct InFlight {
    head: u16,
    status_addr: u64,
    buffers: Vec<(u32, u32)>,
}

pub struct VirtioBlk {
    io: Box<dyn BlockIo>,
    config: [u8; 0x20],
    config_generation: u32,
    in_flight: HashMap<u64, InFlight>,
}

impl VirtioBlk {
    pub fn new(io: Box<dyn BlockIo>) -> Self {
        let mut config = [0u8; 0x20];
        config[0..8].copy_from_slice(&(io.capacity() / 512).to_le_bytes());
        config[0x08..0x0c].copy_from_slice(&SIZE_MAX.to_le_bytes());
        config[0x0c..0x10].copy_from_slice(&SEG_MAX.to_le_bytes());
        Self {
            io,
            config,
            config_generation: 0,
            in_flight: HashMap::new(),
        }
    }

    fn handle_chain(
        &mut self,
        queue: &mut SplitQueue,
        mem: &mut dyn GuestMemory,
        chain: DescriptorChain,
    ) -> bool {
        if chain.malformed || chain.descriptors.len() < 2 {
            return self.complete_error(queue, mem, &chain, STATUS_IOERR);
        }
        let header = chain.descriptors[0];
        if header.flags & DESC_F_WRITE != 0 || header.len < 16 {
            return self.complete_error(queue, mem, &chain, STATUS_IOERR);
        }
        let Some(header_address) = guest_address(header.addr) else {
            return self.complete_error(queue, mem, &chain, STATUS_IOERR);
        };
        let mut header_bytes = [0u8; 16];
        if !mem.read_bytes(header_address, &mut header_bytes) {
            return self.complete_error(queue, mem, &chain, STATUS_IOERR);
        }
        let request_type = u32::from_le_bytes(header_bytes[0..4].try_into().unwrap());
        let sector = u64::from_le_bytes(header_bytes[8..16].try_into().unwrap());

        let status_descriptor = *chain.descriptors.last().unwrap();
        if status_descriptor.flags & DESC_F_WRITE == 0 || status_descriptor.len < 1 {
            return self.complete_error(queue, mem, &chain, STATUS_IOERR);
        }
        let data = &chain.descriptors[1..chain.descriptors.len() - 1];

        match request_type {
            TYPE_IN => self.submit_read(queue, mem, &chain, data, sector),
            TYPE_OUT => self.submit_write(queue, mem, &chain, data, sector),
            TYPE_FLUSH => self.submit_flush(queue, mem, &chain),
            _ => self.complete_error(queue, mem, &chain, STATUS_UNSUPP),
        }
    }

    fn submit_read(
        &mut self,
        queue: &mut SplitQueue,
        mem: &mut dyn GuestMemory,
        chain: &DescriptorChain,
        data: &[Descriptor],
        sector: u64,
    ) -> bool {
        let Some(offset) = sector.checked_mul(512) else {
            return self.complete_error(queue, mem, chain, STATUS_IOERR);
        };
        let mut buffers = Vec::with_capacity(data.len());
        let mut total = 0u64;
        for descriptor in data {
            if descriptor.flags & DESC_F_INDIRECT != 0 {
                return self.complete_error(queue, mem, chain, STATUS_UNSUPP);
            }
            if descriptor.flags & DESC_F_WRITE == 0 {
                return self.complete_error(queue, mem, chain, STATUS_IOERR);
            }
            if descriptor.len as u64 > SIZE_MAX as u64
                || total + descriptor.len as u64 > MAX_IO_BYTES
            {
                return self.complete_error(queue, mem, chain, STATUS_IOERR);
            }
            let Some(address) = guest_address(descriptor.addr) else {
                return self.complete_error(queue, mem, chain, STATUS_IOERR);
            };
            buffers.push((address, descriptor.len));
            total += descriptor.len as u64;
        }
        if offset
            .checked_add(total)
            .is_none_or(|end| end > self.io.capacity())
        {
            return self.complete_error(queue, mem, chain, STATUS_IOERR);
        }
        let token = queue.token(chain.head);
        self.in_flight.insert(
            token,
            InFlight {
                head: chain.head,
                status_addr: chain.descriptors.last().unwrap().addr,
                buffers,
            },
        );
        let request = BlockRequest {
            token,
            op: BlockOp::Read,
            offset,
            len: total as u32,
            data: Vec::new(),
        };
        if self.io.submit(request) {
            false
        } else {
            self.fail_token(queue, mem, token)
        }
    }

    fn submit_write(
        &mut self,
        queue: &mut SplitQueue,
        mem: &mut dyn GuestMemory,
        chain: &DescriptorChain,
        data: &[Descriptor],
        sector: u64,
    ) -> bool {
        let Some(offset) = sector.checked_mul(512) else {
            return self.complete_error(queue, mem, chain, STATUS_IOERR);
        };
        let mut bytes = Vec::new();
        let mut total = 0u64;
        for descriptor in data {
            if descriptor.flags & DESC_F_INDIRECT != 0 {
                return self.complete_error(queue, mem, chain, STATUS_UNSUPP);
            }
            if descriptor.flags & DESC_F_WRITE != 0 {
                return self.complete_error(queue, mem, chain, STATUS_IOERR);
            }
            if descriptor.len as u64 > SIZE_MAX as u64
                || total + descriptor.len as u64 > MAX_IO_BYTES
            {
                return self.complete_error(queue, mem, chain, STATUS_IOERR);
            }
            let Some(address) = guest_address(descriptor.addr) else {
                return self.complete_error(queue, mem, chain, STATUS_IOERR);
            };
            let mut buffer = vec![0u8; descriptor.len as usize];
            if !mem.read_bytes(address, &mut buffer) {
                return self.complete_error(queue, mem, chain, STATUS_IOERR);
            }
            bytes.extend_from_slice(&buffer);
            total += descriptor.len as u64;
        }
        if offset
            .checked_add(total)
            .is_none_or(|end| end > self.io.capacity())
        {
            return self.complete_error(queue, mem, chain, STATUS_IOERR);
        }
        let token = queue.token(chain.head);
        self.in_flight.insert(
            token,
            InFlight {
                head: chain.head,
                status_addr: chain.descriptors.last().unwrap().addr,
                buffers: Vec::new(),
            },
        );
        let request = BlockRequest {
            token,
            op: BlockOp::Write,
            offset,
            len: total as u32,
            data: bytes,
        };
        if self.io.submit(request) {
            false
        } else {
            self.fail_token(queue, mem, token)
        }
    }

    fn submit_flush(
        &mut self,
        queue: &mut SplitQueue,
        mem: &mut dyn GuestMemory,
        chain: &DescriptorChain,
    ) -> bool {
        let token = queue.token(chain.head);
        self.in_flight.insert(
            token,
            InFlight {
                head: chain.head,
                status_addr: chain.descriptors.last().unwrap().addr,
                buffers: Vec::new(),
            },
        );
        let request = BlockRequest {
            token,
            op: BlockOp::Flush,
            offset: 0,
            len: 0,
            data: Vec::new(),
        };
        if self.io.submit(request) {
            false
        } else {
            self.fail_token(queue, mem, token)
        }
    }

    fn complete_error(
        &mut self,
        queue: &mut SplitQueue,
        mem: &mut dyn GuestMemory,
        chain: &DescriptorChain,
        status: u8,
    ) -> bool {
        let status_written = if let Some(descriptor) = chain.descriptors.last() {
            descriptor.flags & DESC_F_WRITE != 0
                && descriptor.len >= 1
                && write_status(mem, descriptor.addr, status)
        } else {
            false
        };
        queue.push_used(mem, chain.head, if status_written { 1 } else { 0 })
    }

    fn fail_token(
        &mut self,
        queue: &mut SplitQueue,
        mem: &mut dyn GuestMemory,
        token: u64,
    ) -> bool {
        let Some(in_flight) = self.in_flight.remove(&token) else {
            return false;
        };
        let status_written = write_status(mem, in_flight.status_addr, STATUS_IOERR);
        queue.push_used(mem, in_flight.head, if status_written { 1 } else { 0 })
    }
}

impl VirtioDevice for VirtioBlk {
    fn device_id(&self) -> u32 {
        2
    }

    fn features(&self) -> u64 {
        let mut features = FEATURE_SIZE_MAX | FEATURE_SEG_MAX | FEATURE_FLUSH | FEATURE_VERSION_1;
        if self.io.is_read_only() {
            features |= FEATURE_RO;
        }
        features
    }

    fn config_read(&self, offset: u32, out: &mut [u8]) {
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = self
                .config
                .get(offset as usize + index)
                .copied()
                .unwrap_or(0);
        }
    }

    fn config_generation(&self) -> u32 {
        self.config_generation
    }

    fn queue_count(&self) -> u16 {
        1
    }

    fn queue_max(&self, queue: u16) -> u32 {
        if queue == 0 { QUEUE_MAX } else { 0 }
    }

    fn queue_num_changed(&mut self, queue: u16, num: u32) {
        if queue == 0 {
            let seg_max = num.saturating_sub(2).clamp(1, SEG_MAX);
            self.config[0x0c..0x10].copy_from_slice(&seg_max.to_le_bytes());
            self.config_generation = self.config_generation.wrapping_add(1);
        }
    }

    fn reset(&mut self) {
        self.in_flight.clear();
    }

    fn reset_queue(&mut self, queue: u16) {
        if queue == 0 {
            self.in_flight.clear();
        }
    }

    fn notify(&mut self, queue: u16, q: &mut SplitQueue, mem: &mut dyn GuestMemory) -> bool {
        if queue != 0 {
            return false;
        }
        let mut interrupt = false;
        while let Some(chain) = q.pop_available(mem) {
            interrupt |= self.handle_chain(q, mem, chain);
        }
        interrupt
    }

    fn poll(&mut self, queue: u16, q: &mut SplitQueue, mem: &mut dyn GuestMemory) -> bool {
        if queue != 0 {
            return false;
        }
        let mut interrupt = false;
        while let Some(completion) = self.io.poll() {
            let Some(in_flight) = self.in_flight.remove(&completion.token) else {
                continue;
            };
            let status = match completion.result {
                Ok(()) => {
                    if in_flight.buffers.is_empty()
                        || write_scatter(mem, &in_flight.buffers, &completion.data)
                    {
                        STATUS_OK
                    } else {
                        STATUS_IOERR
                    }
                }
                Err(_) => STATUS_IOERR,
            };
            let status_written = write_status(mem, in_flight.status_addr, status);
            //len counts bytes written into device-writable buffers: data for IN, plus the
            //one-byte status descriptor for every request type.
            let mut used_len = if status_written { 1 } else { 0 };
            if status == STATUS_OK && !in_flight.buffers.is_empty() {
                used_len += completion.data.len() as u32;
            }
            interrupt |= q.push_used(mem, in_flight.head, used_len);
        }
        if self.io.failed() {
            let tokens: Vec<u64> = self.in_flight.keys().copied().collect();
            for token in tokens {
                if let Some(in_flight) = self.in_flight.remove(&token) {
                    write_status(mem, in_flight.status_addr, STATUS_IOERR);
                    interrupt |= q.push_used(mem, in_flight.head, 0);
                }
            }
        }
        interrupt
    }
}

fn guest_address(address: u64) -> Option<u32> {
    u32::try_from(address).ok()
}

fn write_status(mem: &mut dyn GuestMemory, address: u64, status: u8) -> bool {
    if let Some(address) = guest_address(address) {
        mem.write_bytes(address, &[status])
    } else {
        false
    }
}

fn write_scatter(mem: &mut dyn GuestMemory, buffers: &[(u32, u32)], data: &[u8]) -> bool {
    let mut offset = 0usize;
    for &(address, len) in buffers {
        let len = len as usize;
        let Some(end) = offset.checked_add(len) else {
            return false;
        };
        let Some(chunk) = data.get(offset..end) else {
            return false;
        };
        if !mem.write_bytes(address, chunk) {
            return false;
        }
        offset = end;
    }
    offset == data.len()
}

//Synchronous backend used by unit tests and by callers that do not need a host thread.
pub struct ImmediateBlockIo {
    data: Arc<Mutex<Vec<u8>>>,
    read_only: bool,
    completions: VecDeque<BlockCompletion>,
}

impl ImmediateBlockIo {
    pub fn new(data: Vec<u8>, read_only: bool) -> Self {
        Self {
            data: Arc::new(Mutex::new(data)),
            read_only,
            completions: VecDeque::new(),
        }
    }

    pub fn data(&self) -> Arc<Mutex<Vec<u8>>> {
        Arc::clone(&self.data)
    }
}

impl BlockIo for ImmediateBlockIo {
    fn capacity(&self) -> u64 {
        self.data.lock().unwrap().len() as u64
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn submit(&mut self, request: BlockRequest) -> bool {
        let token = request.token;
        let completion = {
            let mut data = self.data.lock().unwrap();
            match request.op {
                BlockOp::Read => {
                    match (request.offset as usize).checked_add(request.len as usize) {
                        Some(end) if end <= data.len() => BlockCompletion {
                            token,
                            result: Ok(()),
                            data: data[request.offset as usize..end].to_vec(),
                        },
                        _ => BlockCompletion {
                            token,
                            result: Err(BlockError::Io),
                            data: Vec::new(),
                        },
                    }
                }
                BlockOp::Write => match (request.offset as usize).checked_add(request.len as usize)
                {
                    Some(end)
                        if !self.read_only
                            && end <= data.len()
                            && request.data.len() == request.len as usize =>
                    {
                        data[request.offset as usize..end].copy_from_slice(&request.data);
                        BlockCompletion {
                            token,
                            result: Ok(()),
                            data: Vec::new(),
                        }
                    }
                    _ => BlockCompletion {
                        token,
                        result: Err(BlockError::Io),
                        data: Vec::new(),
                    },
                },
                BlockOp::Flush => BlockCompletion {
                    token,
                    result: Ok(()),
                    data: Vec::new(),
                },
            }
        };
        self.completions.push_back(completion);
        true
    }

    fn poll(&mut self) -> Option<BlockCompletion> {
        self.completions.pop_front()
    }
}

struct FileBackend {
    file: File,
    capacity: u64,
    read_only: bool,
}

impl FileBackend {
    fn open(path: impl AsRef<Path>, read_only: bool) -> io::Result<Self> {
        let file = if read_only {
            OpenOptions::new().read(true).open(path)?
        } else {
            OpenOptions::new().read(true).write(true).open(path)?
        };
        let capacity = file.metadata()?.len();
        if capacity < 512 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "disk image must be at least one 512-byte sector",
            ));
        }
        Ok(Self {
            file,
            capacity,
            read_only,
        })
    }

    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn read_exact_at(&mut self, offset: u64, buffer: &mut [u8]) -> io::Result<()> {
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(buffer)
    }

    fn write_all_at(&mut self, offset: u64, data: &[u8]) -> io::Result<()> {
        if self.read_only {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "disk image is read-only",
            ));
        }
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.read_only {
            Ok(())
        } else {
            self.file.sync_data()
        }
    }
}

enum WorkerMessage {
    Request(BlockRequest),
    Shutdown,
}

pub struct WorkerBlockIo {
    capacity: u64,
    read_only: bool,
    requests: Sender<WorkerMessage>,
    completions: Receiver<BlockCompletion>,
    worker: Option<JoinHandle<()>>,
    alive: bool,
}

impl WorkerBlockIo {
    pub fn open(path: impl AsRef<Path>, read_only: bool) -> io::Result<Self> {
        let backend = FileBackend::open(path, read_only)?;
        let capacity = backend.capacity();
        let read_only = backend.is_read_only();
        let (request_tx, request_rx) = mpsc::channel();
        let (completion_tx, completion_rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("virtio-blk".to_string())
            .spawn(move || worker_loop(backend, request_rx, completion_tx))?;
        Ok(Self {
            capacity,
            read_only,
            requests: request_tx,
            completions: completion_rx,
            worker: Some(worker),
            alive: true,
        })
    }
}

impl BlockIo for WorkerBlockIo {
    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn is_read_only(&self) -> bool {
        self.read_only
    }

    fn submit(&mut self, request: BlockRequest) -> bool {
        if self.requests.send(WorkerMessage::Request(request)).is_err() {
            self.alive = false;
            false
        } else {
            true
        }
    }

    fn poll(&mut self) -> Option<BlockCompletion> {
        match self.completions.try_recv() {
            Ok(completion) => Some(completion),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                self.alive = false;
                None
            }
        }
    }

    fn failed(&self) -> bool {
        !self.alive
    }

    fn shutdown(&mut self) {
        if let Some(worker) = self.worker.take() {
            let _ = self.requests.send(WorkerMessage::Shutdown);
            let _ = worker.join();
        }
    }
}

impl Drop for WorkerBlockIo {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn worker_loop(
    mut backend: FileBackend,
    requests: Receiver<WorkerMessage>,
    completions: Sender<BlockCompletion>,
) {
    while let Ok(message) = requests.recv() {
        let request = match message {
            WorkerMessage::Request(request) => request,
            WorkerMessage::Shutdown => break,
        };
        let token = request.token;
        let (result, data) = match request.op {
            BlockOp::Read => {
                let mut data = vec![0u8; request.len as usize];
                match backend.read_exact_at(request.offset, &mut data) {
                    Ok(()) => (Ok(()), data),
                    Err(_) => (Err(BlockError::Io), Vec::new()),
                }
            }
            BlockOp::Write => (
                backend
                    .write_all_at(request.offset, &request.data)
                    .map_err(|_| BlockError::Io),
                Vec::new(),
            ),
            BlockOp::Flush => (backend.flush().map_err(|_| BlockError::Io), Vec::new()),
        };
        if completions
            .send(BlockCompletion {
                token,
                result,
                data,
            })
            .is_err()
        {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use crate::{
        bus::{MemoryBus, VIRTIO_ADDRESS},
        mmdevice::MMdevice,
        plic::{InterruptSource, Plic, PlicContext},
        ram::GuestMemory,
        virtio::{
            MAGIC, STATUS_FAILED, STATUS_FEATURES_OK, VERSION,
            mmio::VirtioMmio,
            queue::{DESC_F_NEXT, DESC_F_WRITE},
        },
    };

    use super::*;

    const TEST_BASE: u32 = 0x8000_0000;
    const TEST_SIZE: usize = 0x20000;
    const DESC_BASE: u32 = TEST_BASE + 0x01000;
    const AVAIL_BASE: u32 = TEST_BASE + 0x02000;
    const USED_BASE: u32 = TEST_BASE + 0x03000;
    const HEADER_ADDR: u32 = TEST_BASE + 0x04000;
    const DATA_ADDR: u32 = TEST_BASE + 0x05000;
    const STATUS_ADDR: u32 = TEST_BASE + 0x06000;

    struct TestMemory {
        data: Vec<u8>,
    }

    impl TestMemory {
        fn new() -> Self {
            Self::with_size(TEST_SIZE)
        }

        fn with_size(size: usize) -> Self {
            Self {
                data: vec![0; size],
            }
        }

        fn offset(&self, address: u32) -> Option<usize> {
            let offset = address.checked_sub(TEST_BASE)? as usize;
            (offset < self.data.len()).then_some(offset)
        }

        fn write_u16(&mut self, address: u32, value: u16) {
            let offset = self.offset(address).unwrap();
            self.data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        }

        fn write_u32(&mut self, address: u32, value: u32) {
            let offset = self.offset(address).unwrap();
            self.data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }

        fn write_u64(&mut self, address: u32, value: u64) {
            let offset = self.offset(address).unwrap();
            self.data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        }

        fn read_byte(&self, address: u32) -> u8 {
            self.data[self.offset(address).unwrap()]
        }

        fn read_u16(&self, address: u32) -> u16 {
            let offset = self.offset(address).unwrap();
            u16::from_le_bytes(self.data[offset..offset + 2].try_into().unwrap())
        }

        fn read_u32(&self, address: u32) -> u32 {
            let offset = self.offset(address).unwrap();
            u32::from_le_bytes(self.data[offset..offset + 4].try_into().unwrap())
        }
    }

    impl GuestMemory for TestMemory {
        fn read_bytes(&self, address: u32, out: &mut [u8]) -> bool {
            let Some(offset) = self.offset(address) else {
                return false;
            };
            let Some(end) = offset.checked_add(out.len()) else {
                return false;
            };
            let Some(bytes) = self.data.get(offset..end) else {
                return false;
            };
            out.copy_from_slice(bytes);
            true
        }

        fn write_bytes(&mut self, address: u32, data: &[u8]) -> bool {
            let Some(offset) = self.offset(address) else {
                return false;
            };
            let Some(end) = offset.checked_add(data.len()) else {
                return false;
            };
            let Some(bytes) = self.data.get_mut(offset..end) else {
                return false;
            };
            bytes.copy_from_slice(data);
            true
        }
    }

    fn new_mmio(disk: Vec<u8>, read_only: bool) -> (VirtioMmio, Arc<Mutex<Vec<u8>>>) {
        let mut plic = Plic::new(33, 2);
        let irq = plic.register_source(IRQ).unwrap();
        let io = ImmediateBlockIo::new(disk, read_only);
        let data = io.data();
        (
            VirtioMmio::new(irq, Box::new(VirtioBlk::new(Box::new(io)))),
            data,
        )
    }

    fn write_reg(mmio: &mut VirtioMmio, offset: u32, value: u32) {
        mmio.write_word(offset, value).unwrap();
    }

    fn read_reg(mmio: &mut VirtioMmio, offset: u32) -> u32 {
        mmio.read_word(offset).unwrap()
    }

    fn program_queue(mmio: &mut VirtioMmio) {
        write_reg(mmio, 0x030, 0); // QueueSel
        write_reg(mmio, 0x038, QUEUE_MAX); // QueueNum
        write_reg(mmio, 0x080, DESC_BASE); // QueueDescLow
        write_reg(mmio, 0x084, 0);
        write_reg(mmio, 0x090, AVAIL_BASE); // QueueAvailLow
        write_reg(mmio, 0x094, 0);
        write_reg(mmio, 0x0a0, USED_BASE); // QueueUsedLow
        write_reg(mmio, 0x0a4, 0);
        write_reg(mmio, 0x044, 1); // QueueReady
        write_reg(mmio, 0x070, 0x07); // ACK | DRIVER | DRIVER_OK
    }

    fn set_descriptor(
        mem: &mut TestMemory,
        index: u32,
        address: u32,
        len: u32,
        flags: u16,
        next: u16,
    ) {
        let base = DESC_BASE + index * 16;
        mem.write_u64(base, u64::from(address));
        mem.write_u32(base + 8, len);
        mem.write_u16(base + 12, flags);
        mem.write_u16(base + 14, next);
    }

    fn publish(mem: &mut TestMemory, head: u16) {
        mem.write_u16(AVAIL_BASE, 0);
        mem.write_u16(AVAIL_BASE + 2, 1);
        mem.write_u16(AVAIL_BASE + 4, head);
    }

    fn setup_one_chain(
        mem: &mut TestMemory,
        request_type: u32,
        sector: u64,
        data_flags: u16,
        with_data: bool,
    ) {
        mem.write_u32(HEADER_ADDR, request_type);
        mem.write_u64(HEADER_ADDR + 8, sector);
        set_descriptor(mem, 0, HEADER_ADDR, 16, DESC_F_NEXT, 1);
        if with_data {
            set_descriptor(mem, 1, DATA_ADDR, 512, DESC_F_NEXT | data_flags, 2);
            set_descriptor(mem, 2, STATUS_ADDR, 1, DESC_F_WRITE, 0);
        } else {
            set_descriptor(mem, 1, STATUS_ADDR, 1, DESC_F_WRITE, 0);
        }
        publish(mem, 0);
    }

    #[test]
    fn bus_exposes_virtio_slot() {
        let (mut bus, _audio) = MemoryBus::new();
        assert_eq!(bus.read_word(VIRTIO_ADDRESS).unwrap(), MAGIC);
        assert_eq!(bus.read_word(VIRTIO_ADDRESS + 4).unwrap(), VERSION);
        assert_eq!(bus.read_word(VIRTIO_ADDRESS + 8).unwrap(), 0);
    }

    #[test]
    fn read_request_is_deferred_until_poll() {
        let mut disk = vec![0u8; 2048];
        disk[512..1024].fill(0xab);
        let (mut mmio, _handle) = new_mmio(disk, false);
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        setup_one_chain(&mut mem, TYPE_IN, 1, DESC_F_WRITE, true);

        write_reg(&mut mmio, 0x050, 0); // QueueNotify
        assert_eq!(mem.read_u16(USED_BASE + 2), 0);

        mmio.poll(&mut mem);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
        assert_eq!(mem.read_u32(USED_BASE + 4), 0);
        assert_eq!(mem.read_u32(USED_BASE + 8), 513);
        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_OK);
        for offset in 0..512 {
            assert_eq!(mem.read_byte(DATA_ADDR + offset), 0xab);
        }
        assert_eq!(read_reg(&mut mmio, 0x060) & 1, 1);
        write_reg(&mut mmio, 0x064, 1);
        assert_eq!(read_reg(&mut mmio, 0x060), 0);
    }

    #[test]
    fn no_interrupt_flag_still_publishes_the_used_entry() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; 1024], false);
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        setup_one_chain(&mut mem, TYPE_IN, 0, DESC_F_WRITE, true);
        mem.write_u16(AVAIL_BASE, 1); //VIRTQ_AVAIL_F_NO_INTERRUPT

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_OK);
        assert_eq!(read_reg(&mut mmio, 0x060) & 1, 0);
    }

    #[test]
    fn attach_disk_replaces_the_placeholder() {
        let path =
            std::env::temp_dir().join(format!("rv-emu-virtio-attach-{}.img", std::process::id()));
        std::fs::write(&path, vec![0u8; 1024]).unwrap();
        let (mut bus, _audio) = MemoryBus::new();
        assert_eq!(bus.read_word(VIRTIO_ADDRESS + 8).unwrap(), 0);
        bus.attach_disk(path.to_str().unwrap(), true).unwrap();
        assert_eq!(bus.read_word(VIRTIO_ADDRESS + 8).unwrap(), 2);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn large_read_uses_multiple_descriptors() {
        const FIRST_LEN: u32 = 3 * 1024 * 1024;
        const SECOND_LEN: u32 = 2 * 1024 * 1024;
        const LARGE_DATA_ADDR: u32 = TEST_BASE + 0x10_0000;

        let total = FIRST_LEN + SECOND_LEN;
        let mut disk = vec![0u8; (total + 512) as usize];
        disk[(total - 1) as usize] = 0x5a;
        let (mut mmio, _handle) = new_mmio(disk, false);
        let mut mem = TestMemory::with_size(8 * 1024 * 1024);
        program_queue(&mut mmio);

        mem.write_u32(HEADER_ADDR, TYPE_IN);
        mem.write_u64(HEADER_ADDR + 8, 0);
        set_descriptor(&mut mem, 0, HEADER_ADDR, 16, DESC_F_NEXT, 1);
        set_descriptor(
            &mut mem,
            1,
            LARGE_DATA_ADDR,
            FIRST_LEN,
            DESC_F_NEXT | DESC_F_WRITE,
            2,
        );
        set_descriptor(
            &mut mem,
            2,
            LARGE_DATA_ADDR + FIRST_LEN,
            SECOND_LEN,
            DESC_F_NEXT | DESC_F_WRITE,
            3,
        );
        set_descriptor(&mut mem, 3, STATUS_ADDR, 1, DESC_F_WRITE, 0);
        publish(&mut mem, 0);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_OK);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
        assert_eq!(mem.read_u32(USED_BASE + 8), total + 1);
        assert_eq!(mem.read_byte(LARGE_DATA_ADDR + total - 1), 0x5a);
    }

    #[test]
    fn oversized_segment_is_rejected() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; SIZE_MAX as usize + 4096], false);
        let mut mem = TestMemory::with_size(8 * 1024 * 1024);
        program_queue(&mut mmio);
        mem.write_u32(HEADER_ADDR, TYPE_IN);
        mem.write_u64(HEADER_ADDR + 8, 0);
        set_descriptor(&mut mem, 0, HEADER_ADDR, 16, DESC_F_NEXT, 1);
        set_descriptor(
            &mut mem,
            1,
            DATA_ADDR,
            SIZE_MAX + 1,
            DESC_F_NEXT | DESC_F_WRITE,
            2,
        );
        set_descriptor(&mut mem, 2, STATUS_ADDR, 1, DESC_F_WRITE, 0);
        publish(&mut mem, 0);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_IOERR);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
    }

    #[test]
    fn full_segment_chain_is_accepted() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; MAX_IO_BYTES as usize], false);
        let mut mem = TestMemory::with_size(8 * 1024 * 1024);
        program_queue(&mut mmio);
        mem.write_u32(HEADER_ADDR, TYPE_IN);
        mem.write_u64(HEADER_ADDR + 8, 0);
        set_descriptor(&mut mem, 0, HEADER_ADDR, 16, DESC_F_NEXT, 1);
        for index in 0..SEG_MAX {
            set_descriptor(
                &mut mem,
                index + 1,
                DATA_ADDR,
                SIZE_MAX,
                DESC_F_NEXT | DESC_F_WRITE,
                (index + 2) as u16,
            );
        }
        set_descriptor(&mut mem, SEG_MAX + 1, STATUS_ADDR, 1, DESC_F_WRITE, 0);
        publish(&mut mem, 0);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_OK);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
        assert_eq!(mem.read_u32(USED_BASE + 8), MAX_IO_BYTES as u32 + 1);
    }

    #[test]
    fn chain_over_the_total_limit_is_rejected() {
        let (mut mmio, _handle) =
            new_mmio(vec![0u8; (SEG_MAX as usize + 1) * SIZE_MAX as usize], false);
        let mut mem = TestMemory::with_size(8 * 1024 * 1024);
        program_queue(&mut mmio);
        mem.write_u32(HEADER_ADDR, TYPE_IN);
        mem.write_u64(HEADER_ADDR + 8, 0);
        set_descriptor(&mut mem, 0, HEADER_ADDR, 16, DESC_F_NEXT, 1);
        //SEG_MAX segments are accepted; one more crosses MAX_IO_BYTES.
        for index in 0..=SEG_MAX {
            set_descriptor(
                &mut mem,
                index + 1,
                DATA_ADDR,
                SIZE_MAX,
                DESC_F_NEXT | DESC_F_WRITE,
                (index + 2) as u16,
            );
        }
        set_descriptor(&mut mem, SEG_MAX + 2, STATUS_ADDR, 1, DESC_F_WRITE, 0);
        publish(&mut mem, 0);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_IOERR);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
    }

    #[test]
    fn write_request_updates_the_backend() {
        let (mut mmio, handle) = new_mmio(vec![0u8; 2048], false);
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        setup_one_chain(&mut mem, TYPE_OUT, 2, 0, true);
        for offset in 0..512 {
            assert!(mem.write_bytes(DATA_ADDR + offset, &[0xcd]));
        }

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_OK);
        assert_eq!(mem.read_u32(USED_BASE + 8), 1);
        let data = handle.lock().unwrap();
        assert!(data[1024..1536].iter().all(|byte| *byte == 0xcd));
    }

    #[test]
    fn flush_request_completes_without_data() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; 1024], false);
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        setup_one_chain(&mut mem, TYPE_FLUSH, 0, 0, false);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_OK);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
        assert_eq!(mem.read_u32(USED_BASE + 8), 1);
    }

    #[test]
    fn unsupported_request_returns_unsupp() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; 1024], false);
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        setup_one_chain(&mut mem, 8, 0, DESC_F_WRITE, true);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_UNSUPP);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
    }

    #[test]
    fn out_of_range_request_returns_ioerr() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; 1024], false);
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        setup_one_chain(&mut mem, TYPE_IN, 100, DESC_F_WRITE, true);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_IOERR);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
    }

    #[test]
    fn read_only_device_rejects_writes() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; 1024], true);
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        setup_one_chain(&mut mem, TYPE_OUT, 0, 0, true);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);

        assert_eq!(mem.read_byte(STATUS_ADDR), STATUS_IOERR);
        assert_ne!(read_reg(&mut mmio, 0x010) & FEATURE_RO as u32, 0);
    }

    #[test]
    fn features_require_version_1() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; 1024], false);
        write_reg(&mut mmio, 0x014, 1);
        assert_eq!(read_reg(&mut mmio, 0x010), 1);
        write_reg(&mut mmio, 0x014, 0);
        let low = read_reg(&mut mmio, 0x010);
        assert_ne!(low & FEATURE_SIZE_MAX as u32, 0);
        assert_ne!(low & FEATURE_SEG_MAX as u32, 0);
        assert_ne!(low & FEATURE_FLUSH as u32, 0);

        write_reg(&mut mmio, 0x024, 1);
        write_reg(&mut mmio, 0x020, 1);
        write_reg(&mut mmio, 0x024, 0);
        write_reg(&mut mmio, 0x020, low);
        write_reg(&mut mmio, 0x070, 0x0b); // ACK | DRIVER | FEATURES_OK
        assert_ne!(read_reg(&mut mmio, 0x070) & STATUS_FEATURES_OK as u32, 0);

        let (mut mmio, _handle) = new_mmio(vec![0u8; 1024], false);
        write_reg(&mut mmio, 0x070, 0x0b);
        assert_eq!(read_reg(&mut mmio, 0x070) & STATUS_FEATURES_OK as u32, 0);
        assert_ne!(read_reg(&mut mmio, 0x070) & STATUS_FAILED as u32, 0);
    }

    #[test]
    fn config_reads_support_byte_half_and_word_widths() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; 2048], false);
        assert_eq!(mmio.read_word(0x100).unwrap(), 4);
        assert_eq!(mmio.read_word(0x104).unwrap(), 0);
        assert_eq!(mmio.read_word(0x108).unwrap(), SIZE_MAX);
        assert_eq!(mmio.read_word(0x10c).unwrap(), SEG_MAX);
        assert_eq!(mmio.read_halfword(0x100).unwrap(), 4);
        assert_eq!(mmio.read_byte(0x100).unwrap(), 4);
        assert_eq!(mmio.read_word(0x0fc).unwrap(), 0);

        write_reg(&mut mmio, 0x030, 0);
        write_reg(&mut mmio, 0x038, 64);
        assert_eq!(mmio.read_word(0x10c).unwrap(), SEG_MAX);
        write_reg(&mut mmio, 0x038, 4);
        assert_eq!(mmio.read_word(0x10c).unwrap(), 2);
        write_reg(&mut mmio, 0x038, 1);
        assert_eq!(mmio.read_word(0x10c).unwrap(), 1);
        assert_eq!(mmio.read_word(0x0fc).unwrap(), 3);
    }

    #[test]
    fn reset_clears_queue_and_interrupt_state() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; 1024], false);
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        setup_one_chain(&mut mem, TYPE_IN, 0, DESC_F_WRITE, true);
        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);
        assert_eq!(read_reg(&mut mmio, 0x060) & 1, 1);

        write_reg(&mut mmio, 0x070, 0);
        assert_eq!(read_reg(&mut mmio, 0x070), 0);
        assert_eq!(read_reg(&mut mmio, 0x044), 0);
        assert_eq!(read_reg(&mut mmio, 0x060), 0);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
    }

    #[test]
    fn malformed_chain_does_not_hang() {
        let (mut mmio, _handle) = new_mmio(vec![0u8; 1024], false);
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        mem.write_u32(HEADER_ADDR, TYPE_IN);
        mem.write_u64(HEADER_ADDR + 8, 0);
        set_descriptor(&mut mem, 0, HEADER_ADDR, 16, DESC_F_NEXT, 0);
        publish(&mut mem, 0);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);
        assert_eq!(mem.read_u16(USED_BASE + 2), 1);
    }

    #[test]
    fn plic_line_follows_interrupt_status() {
        let mut plic = Plic::new(33, 2);
        let irq = plic.register_source(IRQ).unwrap();
        plic.write_word(IRQ * 4, 1).unwrap();
        plic.write_word(0x2080, 1 << IRQ).unwrap();
        let io = ImmediateBlockIo::new(vec![0u8; 1024], false);
        let mut mmio = VirtioMmio::new(irq, Box::new(VirtioBlk::new(Box::new(io))));
        let mut mem = TestMemory::new();
        program_queue(&mut mmio);
        setup_one_chain(&mut mem, TYPE_IN, 0, DESC_F_WRITE, true);

        write_reg(&mut mmio, 0x050, 0);
        mmio.poll(&mut mem);
        mmio.poll_irqs(&mut plic);
        assert!(plic.pending_for(PlicContext::Supervisor));

        write_reg(&mut mmio, 0x064, 1);
        mmio.poll_irqs(&mut plic);
        assert!(!plic.pending_for(PlicContext::Supervisor));
    }

    #[test]
    fn worker_backend_reads_and_writes_a_file() {
        let path = std::env::temp_dir().join(format!("rv-emu-virtio-{}.img", std::process::id()));
        std::fs::write(&path, vec![0u8; 1024]).unwrap();
        let mut io = WorkerBlockIo::open(&path, false).unwrap();

        assert!(io.submit(BlockRequest {
            token: 1,
            op: BlockOp::Read,
            offset: 512,
            len: 512,
            data: Vec::new(),
        }));
        let completion = wait_for_completion(&mut io);
        assert!(completion.result.is_ok());
        assert_eq!(completion.data.len(), 512);

        assert!(io.submit(BlockRequest {
            token: 2,
            op: BlockOp::Write,
            offset: 0,
            len: 512,
            data: vec![0xaa; 512],
        }));
        let completion = wait_for_completion(&mut io);
        assert!(completion.result.is_ok());
        io.shutdown();

        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes[0..512].iter().all(|byte| *byte == 0xaa));
        std::fs::remove_file(&path).ok();
    }

    fn wait_for_completion(io: &mut WorkerBlockIo) -> BlockCompletion {
        for _ in 0..2000 {
            if let Some(completion) = io.poll() {
                return completion;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("worker did not complete in time");
    }
}
