// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::{
    device::blk_dev_manager::BlockDeviceManager,
    device_type::{Device, GenericConfig},
    DeviceManagerInner, DeviceType, Hypervisor,
};
use anyhow::{anyhow, Context, Ok, Result};
use ini::Ini;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{Mutex, RwLock};
pub type ArcBoxDevice = Arc<Mutex<Box<dyn Device>>>;
const SYS_DEV_PREFIX: &str = "/sys/dev";
/// VirtioMmio indicates block driver is virtio-mmio based
pub const VIRTIO_MMIO: &str = "virtio-mmio";
pub const VIRTIO_BLOCK: &str = "virtio-blk";
pub const VFIO: &str = "vfio";
pub const KATA_MMIO_BLK_DEV_TYPE: &str = "mmioblk";
pub const KATA_BLK_DEV_TYPE: &str = "blk";
use crate::device::AgentDevice;
#[derive(Clone)]
pub struct DeviceManager {
    pub dev_managers: HashMap<DeviceType, Arc<RwLock<Box<dyn DeviceManagerInner + Send + Sync>>>>,
    hypervisor: Arc<dyn Hypervisor>,
    pub block_driver: String,
}

impl DeviceManager {
    pub fn new(block_driver: String, hypervisor: Arc<dyn Hypervisor>) -> Result<Self> {
        let mut managers =
            HashMap::<DeviceType, Arc<RwLock<Box<dyn DeviceManagerInner + Send + Sync>>>>::new();
        managers.insert(
            DeviceType::Block,
            Arc::new(RwLock::new(Box::new(BlockDeviceManager::new(
                &block_driver,
            )?))),
        );

        // TODO: other device classes will be added later.
        Ok(DeviceManager {
            dev_managers: managers,
            hypervisor,
            block_driver,
        })
    }

    pub async fn try_add_device(
        &self,
        dev_info: &mut GenericConfig,
        class: &DeviceType,
    ) -> Result<String> {
        if let Some(dev_manager) = self.dev_managers.get(class) {
            let device_id = dev_manager
                .write()
                .await
                .try_add_device(dev_info, self.hypervisor.as_ref())
                .await
                .context("failed to add device")?;
            return Ok(device_id);
        }
        Err(anyhow!("invalid device class {:?}", class))
    }

    pub fn get_block_driver(&self) -> String {
        self.block_driver.clone()
    }

    pub async fn get_device_guest_path(&self, id: &str, class: &DeviceType) -> Option<String> {
        if let Some(dev_manager) = self.dev_managers.get(class) {
            return dev_manager.read().await.get_device_guest_path(id).await;
        }
        None
    }

    pub async fn generate_agent_device(
        &self,
        device_id: String,
        class: &DeviceType,
    ) -> Result<AgentDevice> {
        if let Some(dev_manager) = self.dev_managers.get(class) {
            return Ok(dev_manager
                .read()
                .await
                .generate_agent_device(device_id)
                .await
                .context("failed to generate agent device")?);
        }
        Err(anyhow!("invalid device class {:?}", class))
    }

    pub fn new_device_info_oci(
        &self,
        device: &oci::LinuxDevice,
        bdf: Option<String>,
    ) -> Result<GenericConfig> {
        info!(sl!(), "Linux device info: {:?}", device);
        // b      block (buffered) special file
        // c, u   character (unbuffered) special file
        // p      FIFO
        // refer to https://man7.org/linux/man-pages/man1/mknod.1.html

        let allow_device_type: Vec<&str> = vec!["c", "b", "u", "p"];

        if !allow_device_type.contains(&device.r#type.as_str()) {
            return Err(anyhow!("runtime not support device type {}", device.r#type));
        }

        if device.path.is_empty() {
            return Err(anyhow!("container path can not be empty"));
        }

        let file_mode = device.file_mode.unwrap_or(0);
        let uid = device.uid.unwrap_or(0);
        let gid = device.gid.unwrap_or(0);

        let dev_info = GenericConfig {
            host_path: String::new(),
            container_path: device.path.clone(),
            dev_type: device.r#type.clone(),
            major: device.major,
            minor: device.minor,
            file_mode,
            uid,
            gid,
            id: "".to_string(),
            bdf,
            driver_options: HashMap::new(),
            ..Default::default()
        };
        Ok(dev_info)
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
