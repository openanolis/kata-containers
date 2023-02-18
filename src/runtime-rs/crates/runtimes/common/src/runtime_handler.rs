// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::sync::Arc;

use crate::{ContainerManager, Sandbox};
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait RuntimeHandler: Send + Sync {
    fn init() -> Result<()>
    where
        Self: Sized;

    fn name() -> String
    where
        Self: Sized;

    fn get_sandbox(&self) -> Arc<dyn Sandbox>;

    fn get_container_manager(&self) -> Arc<dyn ContainerManager>;

    async fn update_sandbox_resource(&self) -> Result<()>;

    fn cleanup(&self, id: &str) -> Result<()>;
}
