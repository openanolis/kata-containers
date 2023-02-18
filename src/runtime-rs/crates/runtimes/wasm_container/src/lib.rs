// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use common::{ContainerManager, RuntimeHandler, Sandbox};
pub struct WasmContainer {}

impl WasmContainer {
    pub async fn new() -> Result<Self> {
        Ok(WasmContainer {})
    }
}

#[async_trait]
impl RuntimeHandler for WasmContainer {
    fn init() -> Result<()> {
        Ok(())
    }

    fn name() -> String {
        "wasm_container".to_string()
    }

    fn get_sandbox(&self) -> Arc<dyn Sandbox> {
        todo!()
    }

    fn get_container_manager(&self) -> Arc<dyn ContainerManager> {
        todo!()
    }

    async fn update_sandbox_resource(&self) -> Result<()> {
        todo!()
    }

    fn cleanup(&self, _id: &str) -> Result<()> {
        todo!()
    }
}
