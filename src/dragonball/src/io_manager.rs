// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use arc_swap::{ArcSwap, Cache};
use dbs_device::device_manager::Error;
use dbs_device::device_manager::IoManager;

pub type Result<T> = std::result::Result<T, Error>;

/// Wrapper over IoManager to support device hotplug.
#[derive(Clone)]
pub struct IoManagerImpl(pub(crate) Cache<Arc<ArcSwap<IoManager>>, Arc<IoManager>>);

impl IoManagerImpl {
    pub fn new(io_manager: Arc<ArcSwap<IoManager>>) -> Self {
        IoManagerImpl(Cache::new(io_manager))
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    /// Read data from IO ports.
    pub fn pio_read(&mut self, addr: u16, data: &mut [u8]) -> Result<()> {
        self.0.load().pio_read(addr, data)
    }

    #[cfg(target_arch = "x86_64")]
    #[inline]
    /// Read data to IO ports.
    pub fn pio_write(&mut self, addr: u16, data: &[u8]) -> Result<()> {
        self.0.load().pio_write(addr, data)
    }

    #[inline]
    /// Read data to MMIO address.
    pub fn mmio_read(&mut self, addr: u64, data: &mut [u8]) -> Result<()> {
        self.0.load().mmio_read(addr, data)
    }

    #[inline]
    /// Write data to MMIO address.
    pub fn mmio_write(&mut self, addr: u64, data: &[u8]) -> Result<()> {
        self.0.load().mmio_write(addr, data)
    }

    #[inline]
    /// Revalidate the inner cache
    pub fn revalidate_cache(&mut self) {
        let _ = self.0.load();
    }

    #[inline]
    pub fn load(&mut self) -> &IoManager {
        self.0.load()
    }
}
