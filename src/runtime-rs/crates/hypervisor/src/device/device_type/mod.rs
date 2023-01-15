// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod block;
pub mod vhost_user_device;
use crate::Hypervisor as hypervisor;
use async_trait::async_trait;
pub use block::{BlockConfig, BlockDevice};
mod network;
pub use network::{Address, NetworkConfig};
mod share_fs_device;
pub use share_fs_device::ShareFsDeviceConfig;
mod vfio;
pub use vfio::{bind_device_to_host, bind_device_to_vfio, VfioBusMode, VfioConfig, VfioDevice};
mod share_fs_mount;
pub use share_fs_mount::{ShareFsMountConfig, ShareFsMountType, ShareFsOperation};
mod vsock;
use anyhow::Result;
pub use vsock::{HybridVsockConfig, VsockConfig};
mod generic;
pub use generic::{GenericConfig, GenericDevice};
use std::fmt;

#[derive(Debug)]
pub enum DeviceConfig {
    Block(BlockConfig),
    Network(NetworkConfig),
    ShareFsDevice(ShareFsDeviceConfig),
    Vfio(VfioConfig),
    ShareFsMount(ShareFsMountConfig),
    Vsock(VsockConfig),
    HybridVsock(HybridVsockConfig),
}

impl fmt::Display for DeviceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Default, Clone)]
pub struct DeviceArgument {
    pub index: Option<u64>,
    pub drive_name: Option<String>,
}

#[async_trait]
pub trait Device: Send + Sync {
    // attach is to plug block device into VM
    async fn attach(&mut self, h: &dyn hypervisor, da: DeviceArgument) -> Result<()>;
    // detach is to unplug block device from VM
    async fn detach(&mut self, h: &dyn hypervisor) -> Result<Option<u64>>;
    // device_id returns device ID
    async fn device_id(&self) -> &str;
    // set_device_info set the device info
    async fn set_device_info(&mut self, device_info: GenericConfig) -> Result<()>;
    // get_device_info returns device config
    async fn get_device_info(&self) -> Result<GenericConfig>;
    // get_major_minor returns device major and minor numbers
    async fn get_major_minor(&self) -> (i64, i64);
    // get_host_path return the device path in the host
    async fn get_host_path(&self) -> &str;
    // get the bus device function id of device
    async fn get_bdf(&self) -> Option<&String>;
    // get_attach_count returns how many times the device has been attached
    async fn get_attach_count(&self) -> u64;
    // increase_attach_count is used to increase the attach count for a device
    // return values:
    // * skip bool: no need to do real attach when current attach count is zero, skip following actions.
    // * err error: error while do increase attach count
    async fn increase_attach_count(&mut self) -> Result<bool>;
    // decrease_attach_count is used to decrease the attach count for a device
    // return values:
    // * skip bool: no need to do real dettach when current attach count is not zero, skip following actions.
    // * err error: error while do decrease attach count
    async fn decrease_attach_count(&mut self) -> Result<bool>;
}
