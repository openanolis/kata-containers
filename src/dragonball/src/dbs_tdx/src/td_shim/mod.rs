pub mod hob;
pub mod metadata;
pub use dbs_boot::layout::MMIO_LOW_END;

/// Start address of td-shim
pub const TD_SHIM_START: u64 = MMIO_LOW_END - TD_SHIM_SIZE + 1;
/// Size of td-shim
pub const TD_SHIM_SIZE: u64 = 16u64 << 20;
