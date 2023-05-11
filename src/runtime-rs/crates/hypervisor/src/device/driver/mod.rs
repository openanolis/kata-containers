// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod vhost_user;
mod virtio_blk;
use anyhow::Result;
use async_trait::async_trait;
pub use virtio_blk::{
    BlockConfig, KATA_BLK_DEV_TYPE, KATA_MMIO_BLK_DEV_TYPE, VIRTIO_BLOCK_MMIO, VIRTIO_BLOCK_PCI,
};
mod virtio_net;
pub use virtio_net::{Address, NetworkConfig};
mod vfio;
pub use vfio::{bind_device_to_host, bind_device_to_vfio, VfioBusMode, VfioConfig};
mod virtio_fs;
pub use virtio_fs::{ShareFsDeviceConfig, ShareFsMountConfig, ShareFsMountType, ShareFsOperation};
mod virtio_vsock;
use crate::Hypervisor as hypervisor;
use std::fmt;
pub use virtio_vsock::{HybridVsockConfig, VsockConfig};

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

#[async_trait]
pub trait Device: Send + Sync {
    // attach is to plug device into VM
    async fn attach(&self, h: &dyn hypervisor) -> Result<()>;
    // detach is to unplug device from VM
    async fn detach(&self, h: &dyn hypervisor) -> Result<Option<u64>>;
    // get_device_info returns device config
    async fn get_device_info(&self) -> DeviceConfig;
    // increase_attach_count is used to increase the attach count for a device
    // return values:
    // * true: no need to do real attach when current attach count is zero, skip following actions.
    // * err error: error while do increase attach count
    async fn increase_attach_count(&mut self) -> Result<bool>;
    // decrease_attach_count is used to decrease the attach count for a device
    // return values:
    // * false: no need to do real dettach when current attach count is not zero, skip following actions.
    // * err error: error while do decrease attach count
    async fn decrease_attach_count(&mut self) -> Result<bool>;
}
