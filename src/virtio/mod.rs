pub mod blk;
pub mod mmio;
pub mod queue;

//Virtio MMIO identity values.
pub const MAGIC: u32 = 0x7472_6976; //"virt"
pub const VERSION: u32 = 2;
pub const VENDOR_ID: u32 = 0x554d_4551;

pub const CONFIG_OFFSET: u32 = 0x100;

//Transport feature bits.
pub const FEATURE_VERSION_1: u64 = 1 << 32;

//Device status bits.
pub const STATUS_ACKNOWLEDGE: u8 = 1;
pub const STATUS_DRIVER: u8 = 2;
pub const STATUS_DRIVER_OK: u8 = 4;
pub const STATUS_FEATURES_OK: u8 = 8;
pub const STATUS_NEEDS_RESET: u8 = 64;
pub const STATUS_FAILED: u8 = 128;

//Interrupt status bits.
pub const INT_VRING: u32 = 1;
pub const INT_CONFIG: u32 = 2;
