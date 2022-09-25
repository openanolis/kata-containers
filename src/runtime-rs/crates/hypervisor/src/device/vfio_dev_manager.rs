// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, str, sync::Arc};

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::{
    device_type::{Device, DeviceArgument, GenericConfig, VfioDevice},
    DeviceManagerInner, Hypervisor,
};
use agent::types::Device as AgentDevice;

#[allow(dead_code)]
pub struct VfioDeviceManager {
    devices: HashMap<String, Arc<Mutex<VfioDevice>>>,
    driver: String,
}

impl VfioDeviceManager {
    pub fn _new(block_driver: &str) -> Result<Self> {
        Ok(VfioDeviceManager {
            devices: HashMap::new(),
            driver: block_driver.to_string(),
        })
    }

    async fn _find_device_by_bdf(&self, bdf: Option<&String>) -> Option<Arc<Mutex<VfioDevice>>> {
        for dev in self.devices.values() {
            if dev.lock().await.get_bdf().await == bdf {
                return Some(dev.clone());
            }
        }
        None
    }

    async fn _find_device(&self, bdf: Option<&String>) -> Option<Arc<Mutex<VfioDevice>>> {
        if bdf.is_some() {
            self._find_device_by_bdf(bdf).await
        } else {
            None
        }
    }
}

#[async_trait]
impl DeviceManagerInner for VfioDeviceManager {
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

    async fn generate_agent_device(&self, _device_id: String) -> Result<AgentDevice> {
        todo!()
    }

    async fn get_device_guest_path(&self, _id: &str) -> Option<String> {
        todo!()
    }

    async fn get_driver_options(&self) -> Result<String> {
        todo!()
    }
}
