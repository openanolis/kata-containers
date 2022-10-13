// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, sync::Arc};

use crate::{
    device_type::{Device, DeviceArgument, GenericConfig},
    DeviceManagerInner, Hypervisor,
};
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::Mutex;

pub struct VhostUserDeviceManager {
    _devices: HashMap<String, Arc<Mutex<dyn Device>>>,
    _driver: String,
}

impl VhostUserDeviceManager {
    pub fn _new() -> Result<Self> {
        Ok(VhostUserDeviceManager {
            _devices: HashMap::new(),
            _driver: "".to_owned(),
        })
    }
}

#[async_trait]
impl DeviceManagerInner for VhostUserDeviceManager {
    async fn try_add_device(
        &mut self,
        _dev_info: &mut GenericConfig,
        _h: &dyn Hypervisor,
        _da: DeviceArgument,
    ) -> Result<String> {
        Ok("".to_owned())
    }

    async fn try_remove_device(
        &mut self,
        _device_id: &str,
        _h: &dyn Hypervisor,
    ) -> Result<Option<u64>> {
        todo!()
    }

    async fn get_device_guest_path(&self, _id: &str) -> Option<String> {
        todo!()
    }

    async fn get_device_vm_path(&self, _id: &str) -> Option<String> {
        todo!()
    }

    async fn get_driver_options(&self) -> Result<String> {
        todo!()
    }
}
