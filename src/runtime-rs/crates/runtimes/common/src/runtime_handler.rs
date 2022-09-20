// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use kata_types::config::TomlConfig;
use tokio::sync::mpsc::Sender;

use crate::{message::Message, ContainerManager, Sandbox};

#[derive(Clone)]
pub struct RuntimeInstance {
    pub sandbox: Arc<dyn Sandbox>,
    pub container_manager: Arc<dyn ContainerManager>,
}

#[async_trait]
pub trait RuntimeHandler: Send + Sync {
    fn init() -> Result<()>
    where
        Self: Sized;

    fn name() -> String
    where
        Self: Sized;

    fn new_handler() -> Arc<dyn RuntimeHandler>
    where
        Self: Sized;

    async fn new_instance(
        &self,
        sid: &str,
        msg_sender: Sender<Message>,
        config: Arc<TomlConfig>,
    ) -> Result<RuntimeInstance>;

    fn cleanup(&self, id: &str) -> Result<()>;
}

impl RuntimeInstance {
    // NOTE: if static resource management is configured, a warning is logged
    // hotplug vcpu/memory
    //   - vcpu: the sum of each ctr, plus default vcpu
    //   - memory: the sum of each ctr, plus default memory, and setup swap
    // TODO: hotplug not supported returns an error
    pub async fn update_sandbox_resource(&self) -> Result<()> {
        // calculate the number of vcpu to be updated
        let nr_vcpus = self.container_manager.total_vcpus().await?;

        // the unit here is byte
        let (mem_sb_byte, swap_sb_byte) = self
            .container_manager
            .total_mems()
            .await
            .context("failed to calculate total memory requirement for containers")?;

        // todo: handle err if guest does not support hotplug
        // let hypervisor update the cpus
        self.sandbox
            .update_cpu_resource(nr_vcpus)
            .await
            .context("failed to update_cpu_resource")?;

        // todo: setup swap space in guest, when block device hot plug is supported
        // todo: handle err if guest does not support hotplug
        // let hypervisor update the memory
        let mem_sb_mb = (mem_sb_byte >> 20) as u32;
        self.sandbox
            .update_mem_resource(mem_sb_mb, swap_sb_byte)
            .await
            .context("failed to update_mem_resource")?;

        Ok(())
    }
}
