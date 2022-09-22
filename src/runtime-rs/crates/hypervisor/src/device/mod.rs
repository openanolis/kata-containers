// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::Hypervisor;
use async_trait::async_trait;

use self::device_type::GenericConfig;
mod blk_dev_manager;
pub mod device_manager;
pub mod device_type;
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
    Undefined,
}

#[async_trait]
pub trait DeviceManagerInner {
    // try to add device
    async fn try_add_device(
        &mut self,
        dev_info: &mut GenericConfig,
        h: &dyn Hypervisor,
    ) -> Result<String>;
    // try to remove device
    async fn try_remove_device(&mut self, device_id: &str, h: &dyn Hypervisor) -> Result<()>;
    // generate agent device
    async fn generate_agent_device(&self, device_id: String) -> Result<AgentDevice>;
}
