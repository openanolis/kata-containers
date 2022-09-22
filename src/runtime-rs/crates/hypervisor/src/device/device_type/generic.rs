// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device_type::hypervisor;
use crate::device_type::Device;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::HashMap;

use super::DeviceArgument;

/// GenericConfig is an embedded type that contains device data common to all types of devices.
#[derive(Default, Debug, Clone)]
pub struct GenericConfig {
    /// host_path is device path on host
    pub host_path: String,

    /// container_path is device path inside container
    pub container_path: String,

    /// Type of device: c, b, u or p
    /// c , u - character(unbuffered)
    /// p - FIFO
    /// b - block(buffered) special file
    /// More info in mknod(1).
    pub dev_type: String,

    /// Major, minor numbers for device.
    pub major: i64,
    pub minor: i64,

    /// FileMode permission bits for the device.
    pub file_mode: u32,

    /// user id, id of the device owner.
    pub uid: u32,
    /// gourp id, id of the device group.
    pub gid: u32,
    /// ID for the device that is passed to the hypervisor.
    pub id: String,

    /// The Bus::Device.Function ID if the device is already
    /// bound to VFIO driver.
    pub bdf: Option<String>,
    /// driver_options is specific options for each device driver
    /// for example, for BlockDevice, we can set DriverOptions["blockDriver"]="virtio-blk"
    pub driver_options: HashMap<String, String>,

    // pci_addr is the PCI address used to identify the slot at which the drive is attached.
    pub pci_addr: Option<String>,

    // virt_path at which the device appears inside the VM, outside of the container mount namespace
    pub virt_path: Option<String>,
}

// GenericDevice refers to a device that is neither a VFIO device, block device or VhostUserDevice.
#[derive(Default, Debug)]
pub struct GenericDevice {
    id: String,
    generic_config: GenericConfig,
    attach_count: u64,
}

impl GenericDevice {
    // new creates a new GenericDevice
    pub fn new(dev_info: &GenericConfig) -> Self {
        Self {
            id: dev_info.id.clone(),
            generic_config: dev_info.clone(),
            attach_count: 0,
        }
    }
}

#[async_trait]
impl Device for GenericDevice {
    async fn attach(&mut self, _h: &dyn hypervisor, _da: DeviceArgument) -> Result<()> {
        let skip = self.increase_attach_count().await?;
        if skip {
            return Ok(());
        }
        Ok(())
    }

    async fn detach(&mut self, _h: &dyn hypervisor) -> Result<Option<u64>> {
        let skip = self.decrease_attach_count().await?;
        if skip {
            return Ok(None);
        }
        Ok(None)
    }

    async fn device_id(&self) -> &str {
        self.id.as_str()
    }

    async fn set_device_info(&mut self, device_info: GenericConfig) -> Result<()> {
        self.generic_config = device_info;
        Ok(())
    }

    async fn get_device_info(&self) -> Result<GenericConfig> {
        Ok(self.generic_config.clone())
    }

    async fn get_major_minor(&self) -> (i64, i64) {
        (self.generic_config.major, self.generic_config.minor)
    }

    async fn get_host_path(&self) -> &str {
        self.generic_config.host_path.as_str()
    }

    async fn get_bdf(&self) -> Option<&String> {
        self.generic_config.bdf.as_ref()
    }

    async fn get_attach_count(&self) -> u64 {
        self.attach_count
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::u64;

    #[actix_rt::test]
    async fn test_increase_attach_count() {
        let data = vec![
            (0, 1, false, false),
            (1, 2, true, false),
            (u64::MAX, u64::MAX, true, true),
        ];
        let mut dev = GenericDevice::default();
        for (attach_count, expected_ac, expect_skip, expect_err) in data.into_iter() {
            dev.attach_count = attach_count;
            let ret = dev.increase_attach_count().await;
            if expect_err {
                assert!(ret.is_err());
            } else {
                let skip = ret.unwrap();
                assert_eq!(skip, expect_skip);
            }
            assert_eq!(dev.get_attach_count().await, expected_ac);
        }
    }

    #[actix_rt::test]
    async fn test_decrease_attach_count() {
        let data = vec![
            (0, 0, true, true),
            (1, 0, false, false),
            (u64::MAX, u64::MAX - 1, true, false),
        ];
        let mut dev = GenericDevice::default();
        for (attach_count, expected_ac, expect_skip, expect_err) in data.into_iter() {
            dev.attach_count = attach_count;
            let ret = dev.decrease_attach_count().await;
            if expect_err {
                assert!(ret.is_err());
            } else {
                let skip = ret.unwrap();
                assert_eq!(skip, expect_skip);
            }
            assert_eq!(dev.get_attach_count().await, expected_ac);
        }
    }
}
