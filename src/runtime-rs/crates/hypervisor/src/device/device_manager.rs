// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    device::blk_dev_manager::BlockDeviceManager,
    device_type::{Device, DeviceArgument, GenericConfig},
    DeviceManagerInner, DeviceType, Hypervisor,
};
use anyhow::{anyhow, Context, Result};
use ini::Ini;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};

pub type ArcBoxDevice = Arc<Mutex<Box<dyn Device>>>;
const SYS_DEV_PREFIX: &str = "/sys/dev";

/// VIRTIO_BLOCK_MMIO indicates block driver is virtio-mmio based
pub(crate) const VIRTIO_BLOCK_MMIO: &str = "virtio-blk-mmio";
/// VIRTIO_BLOCK_PCI indicates block driver is virtio-pci based
pub(crate) const VIRTIO_BLOCK_PCI: &str = "virtio-blk-pci";

/// block_index and released_block_index are used to search an available block index
/// in Sandbox.
///
/// @block_index generally default is 1 for <vdb>;
/// @released_block_index for blk devices removed and indexes will released at the same time.
#[derive(Clone, Debug, Default)]
struct SharedInfo {
    block_index: u64,
    released_block_index: Vec<u64>,
}

impl SharedInfo {
    fn new(index: u64) -> Self {
        SharedInfo {
            block_index: index,
            released_block_index: vec![],
        }
    }

    // declare the available block index
    fn declare_device_index(&mut self) -> Result<u64> {
        let current_index = if let Some(index) = self.released_block_index.pop() {
            index
        } else {
            self.block_index
        };
        self.block_index += 1;

        Ok(current_index)
    }

    fn release_device_index(&mut self, index: u64) {
        self.released_block_index.push(index);
        self.released_block_index.sort_by(|a, b| b.cmp(a));
    }
}

// Device manager will manage the lifecycle of sandbox device
#[derive(Clone)]
pub struct DeviceManager {
    pub dev_managers: HashMap<DeviceType, Arc<RwLock<Box<dyn DeviceManagerInner + Send + Sync>>>>,
    hypervisor: Arc<dyn Hypervisor>,
    shared_info: SharedInfo,
}

impl DeviceManager {
    pub async fn new(hypervisor: Arc<dyn Hypervisor>) -> Result<Self> {
        let mut managers =
            HashMap::<DeviceType, Arc<RwLock<Box<dyn DeviceManagerInner + Send + Sync>>>>::new();
        // register block device manager
        let block_device_driver = hypervisor
            .hypervisor_config()
            .await
            .blockdev_info
            .block_device_driver;
        managers.insert(
            DeviceType::Block,
            Arc::new(RwLock::new(Box::new(BlockDeviceManager::new(
                &block_device_driver,
            )?))),
        );
        // TODO: other device classes will be added later.
        // https://github.com/kata-containers/kata-containers/issues/6525
        // https://github.com/kata-containers/kata-containers/issues/6526
        Ok(DeviceManager {
            dev_managers: managers,
            hypervisor,
            shared_info: SharedInfo::new(1),
        })
    }

    pub async fn try_add_device(
        &mut self,
        dev_info: &mut GenericConfig,
        class: &DeviceType,
    ) -> Result<String> {
        let da = self
            .make_device_argument(dev_info.dev_type.as_str())
            .context("failed to make device arguments")?;
        if let Some(device_manager) = self.dev_managers.get(class) {
            match device_manager
                .write()
                .await
                .try_add_device(dev_info, self.hypervisor.as_ref(), da.clone())
                .await
            {
                Ok(device_id) => {
                    return Ok(device_id);
                }
                Err(e) => {
                    if let Some(index) = da.index {
                        self.shared_info.release_device_index(index);
                    }
                    return Err(e);
                }
            };
        }
        if let Some(index) = da.index {
            self.shared_info.release_device_index(index);
        }
        Err(anyhow!("invalid device class {:?}", class))
    }

    // get_virt_drive_name returns the disk name format for virtio-blk
    // Reference: https://github.com/torvalds/linux/blob/master/drivers/block/virtio_blk.c @c0aa3e0916d7e531e69b02e426f7162dfb1c6c0
    fn get_virt_drive_name(&self, mut index: i32) -> Result<String> {
        if index < 0 {
            return Err(anyhow!("Index cannot be negative"));
        }

        // Prefix used for virtio-block devices
        const PREFIX: &str = "vd";

        // Refer to DISK_NAME_LEN: https://github.com/torvalds/linux/blob/08c521a2011ff492490aa9ed6cc574be4235ce2b/include/linux/genhd.h#L61
        let disk_name_len = 32usize;
        let base = 26i32;

        let suff_len = disk_name_len - PREFIX.len();
        let mut disk_letters = vec![0u8; suff_len];

        let mut i = 0usize;
        while i < suff_len && index >= 0 {
            let letter: u8 = b'a' + (index % base) as u8;
            disk_letters[i] = letter;
            index = (index / base) - 1;
            i += 1;
        }
        if index >= 0 {
            return Err(anyhow!("Index not supported"));
        }
        disk_letters.truncate(i);
        disk_letters.reverse();
        Ok(String::from(PREFIX) + std::str::from_utf8(&disk_letters)?)
    }

    fn make_device_argument(&mut self, dev_type: &str) -> Result<DeviceArgument> {
        // prepare arguments to attach device
        if dev_type == "b" {
            let current_index = self.shared_info.declare_device_index()?;
            let drive_name = self.get_virt_drive_name(current_index as i32)?;

            Ok(DeviceArgument {
                index: Some(current_index),
                drive_name: Some(drive_name),
            })
        } else {
            Ok(DeviceArgument {
                index: None,
                drive_name: None,
            })
        }
    }
}

// get_host_path is used to fetch the host path for the device.
// The path passed in the spec refers to the path that should appear inside the container.
// We need to find the actual device path on the host based on the major-minor numbers of the device.
pub(crate) fn get_host_path(dev_info: &GenericConfig) -> Result<String> {
    if dev_info.container_path.is_empty() {
        return Err(anyhow!("Empty path provided for device"));
    }

    let path_comp = match dev_info.dev_type.as_str() {
        "c" | "u" => "char",
        "b" => "block",
        // for device type p will return an empty string
        _ => return Ok(String::new()),
    };
    let format = format!("{}:{}", dev_info.major, dev_info.minor);
    let sys_dev_path = std::path::Path::new(SYS_DEV_PREFIX)
        .join(path_comp)
        .join(format)
        .join("uevent");
    if let Err(e) = std::fs::metadata(&sys_dev_path) {
        // Some devices(eg. /dev/fuse, /dev/cuse) do not always implement sysfs interface under /sys/dev
        // These devices are passed by default by docker.
        // Simply return the path passed in the device configuration, this does mean that no device renames are
        // supported for these devices.
        if e.kind() == std::io::ErrorKind::NotFound {
            return Ok(dev_info.container_path.clone());
        }
        return Err(e.into());
    }
    let conf = Ini::load_from_file(&sys_dev_path)?;
    let dev_name = conf
        .section::<String>(None)
        .ok_or_else(|| anyhow!("has no section"))?
        .get("DEVNAME")
        .ok_or_else(|| anyhow!("has no DEVNAME"))?;
    Ok(format!("/dev/{}", dev_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dragonball::Dragonball;
    use crate::HypervisorConfig;

    #[actix_rt::test]
    async fn test_get_virt_drive_name() {
        let mut hypervisor = Dragonball::new();
        let mut config = HypervisorConfig::default();
        config.blockdev_info.block_device_driver = "virtio-blk-mmio".to_string();
        hypervisor.set_hypervisor_config(config).await;
        let manager = DeviceManager::new(Arc::new(hypervisor)).await.unwrap();
        for &(input, output) in [
            (0i32, "vda"),
            (25, "vdz"),
            (27, "vdab"),
            (704, "vdaac"),
            (18277, "vdzzz"),
        ]
        .iter()
        {
            let out = manager.get_virt_drive_name(input).unwrap();
            assert_eq!(&out, output);
        }
    }
}
