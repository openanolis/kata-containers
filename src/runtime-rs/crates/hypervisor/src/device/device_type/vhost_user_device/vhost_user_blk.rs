// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::device_type::{hypervisor, GenericConfig};
use async_trait::async_trait;

use crate::device_type::{Device, DeviceArgument, GenericDevice};

use super::VhostUserConfig;
use anyhow::Result;
// VhostUserBlkDevice is a block vhost-user based device
pub struct VhostUserBlkDevice {
    _drive: VhostUserConfig,
    _base: GenericDevice,
}

#[async_trait]
impl Device for VhostUserBlkDevice {
    async fn attach(&mut self, _h: &dyn hypervisor, _da: DeviceArgument) -> Result<()> {
        todo!()
    }

    async fn detach(&mut self, _h: &dyn hypervisor) -> Result<Option<u64>> {
        todo!()
    }

    async fn device_id(&self) -> &str {
        todo!()
    }

    async fn set_device_info(&mut self, _device_info: GenericConfig) -> Result<()> {
        todo!()
    }

    async fn get_device_info(&self) -> Result<GenericConfig> {
        todo!()
    }

    async fn get_major_minor(&self) -> (i64, i64) {
        todo!()
    }

    async fn get_host_path(&self) -> &str {
        todo!()
    }

    async fn get_bdf(&self) -> Option<&String> {
        todo!()
    }

    async fn get_attach_count(&self) -> u64 {
        todo!()
    }

    async fn increase_attach_count(&mut self) -> Result<bool> {
        todo!()
    }

    async fn decrease_attach_count(&mut self) -> Result<bool> {
        todo!()
    }
}
