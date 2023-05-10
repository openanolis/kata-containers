// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub const VIRTIO_BLOCK_MMIO: &str = "virtio-blk-mmio";
use crate::Device;
use crate::{driver::hypervisor, DeviceConfig};
use anyhow::{anyhow, Result};
use async_trait::async_trait;
/// VIRTIO_BLOCK_PCI indicates block driver is virtio-pci based
pub const VIRTIO_BLOCK_PCI: &str = "virtio-blk-pci";
pub const KATA_MMIO_BLK_DEV_TYPE: &str = "mmioblk";
pub const KATA_BLK_DEV_TYPE: &str = "blk";

#[derive(Debug, Clone)]
pub struct BlockConfig {
    /// Unique identifier of the drive.
    pub id: String,

    /// Path of the drive.
    pub path_on_host: String,

    /// If set to true, the drive is opened in read-only mode. Otherwise, the
    /// drive is opened as read-write.
    pub is_readonly: bool,

    /// Don't close `path_on_host` file when dropping the device.
    pub no_drop: bool,

    /// device index
    pub index: u64,

    /// driver type for block device
    pub driver_option: String,

    /// device path in guest
    pub virt_path: String,

    /// device attach count
    pub attach_count: u64,

    /// device major number
    pub major: i64,

    /// device minor number
    pub minor: i64,
}

impl BlockConfig {
    // new creates a new VirtioBlkDevice
    pub fn new(dev_info: BlockConfig) -> Self {
        dev_info
    }
}

#[async_trait]
impl Device for BlockConfig {
    async fn attach(&self, h: &dyn hypervisor) -> Result<()> {
        h.add_device(DeviceConfig::Block(self.clone())).await
    }

    async fn detach(&self, h: &dyn hypervisor) -> Result<u64> {
        h.remove_device(DeviceConfig::Block(self.clone())).await?;
        Ok(self.index)
    }

    async fn get_device_info(&self) -> DeviceConfig {
        DeviceConfig::Block(self.clone())
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        match self.attach_count {
            0 => {
                // do real attach
                self.attach_count += 1;
                Ok(false)
            }
            std::u64::MAX => Err(anyhow!("device was attached too many times")),
            _ => {
                self.attach_count += 1;
                Ok(true)
            }
        }
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        match self.attach_count {
            0 => Err(anyhow!("detaching a device that wasn't attached")),
            1 => {
                // do real wrok
                self.attach_count -= 1;
                Ok(false)
            }
            _ => {
                self.attach_count -= 1;
                Ok(true)
            }
        }
    }
}
