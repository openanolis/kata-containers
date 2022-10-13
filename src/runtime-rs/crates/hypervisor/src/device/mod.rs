// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod block;
use crate::Hypervisor as hypervisor;
use async_trait::async_trait;
pub use block::{BlockConfig, BlockDevice};
mod network;
pub use network::{Address, NetworkConfig};
mod share_fs_device;
pub use share_fs_device::ShareFsDeviceConfig;
mod vfio;
pub use vfio::{bind_device_to_host, bind_device_to_vfio, VfioBusMode, VfioConfig};
mod share_fs_mount;
pub use share_fs_mount::{ShareFsMountConfig, ShareFsMountType, ShareFsOperation};
mod vsock;
use anyhow::Result;
pub use vsock::VsockConfig;
mod generic;
pub use generic::{GenericConfig, GenericDevice, IoLimits};
use std::fmt;

#[derive(Debug)]
pub enum DeviceConfig {
    Block(BlockConfig),
    Network(NetworkConfig),
    ShareFsDevice(ShareFsDeviceConfig),
    Vfio(VfioConfig),
    ShareFsMount(ShareFsMountConfig),
    Vsock(VsockConfig),
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
    async fn attach(&mut self, h: &dyn hypervisor, da: DeviceArgument) -> Result<()>;
    async fn detach(&mut self, h: &dyn hypervisor) -> Result<()>;
    async fn device_id(&self) -> &str;
    async fn set_device_info(&mut self, di: GenericConfig) -> Result<()>;
    async fn get_device_info(&self) -> Result<GenericConfig>;
    async fn get_major_minor(&self) -> (i64, i64);
    async fn get_host_path(&self) -> &str;
    async fn get_bdf(&self) -> Option<&String>;
    async fn get_attach_count(&self) -> u64;
    // bumpAttachCount is used to add/minus attach count for a device
    // * attach bool: true means attach, false means detach
    // return values:
    // * skip bool: no need to do real attach/detach, skip following actions.
    // * err error: error while do attach count bump
    async fn bump_attach_count(&mut self, attach: bool) -> Result<bool>;
    async fn reference(&mut self) -> u64;
    async fn dereference(&mut self) -> u64;
}
