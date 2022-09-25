// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::Hypervisor;
use async_trait::async_trait;

use self::device_type::{DeviceArgument, GenericConfig};
mod blk_dev_manager;
pub mod device_manager;
pub mod device_type;
mod vfio_dev_manager;
mod vhost_dev_manager;
use agent::types::Device as AgentDevice;
use anyhow::Result;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum DeviceType {
    Block,
    Network,
    ShareFsDevice,
    Vfio,
    ShareFsMount,
    Vsock,
    HybridVsock,
    Vhost,
    Undefined,
}

#[async_trait]
pub trait DeviceManagerInner {
    // try to add device
    async fn try_add_device(
        &mut self,
        dev_info: &mut GenericConfig,
        h: &dyn Hypervisor,
        da: DeviceArgument,
    ) -> Result<String>;
    // try to remove device
    async fn try_remove_device(
        &mut self,
        device_id: &str,
        h: &dyn Hypervisor,
    ) -> Result<Option<u64>>;
    // generate agent device
    async fn generate_agent_device(&self, device_id: String) -> Result<AgentDevice>;
    // get the device guest path
    async fn get_device_guest_path(&self, id: &str) -> Option<String>;
    // get device manager driver options
    async fn get_driver_options(&self) -> Result<String>;
}
