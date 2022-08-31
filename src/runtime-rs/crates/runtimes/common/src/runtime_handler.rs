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
    // NOTE THAT: if static resource management is configured, this returns an Error
    // 1. hotplug vcpu/memory
    //   - vcpu: the sum of each ctr, plus default vcpu
    //   - memory: the sum of each ctr, plus default memory, and setup swap
    // 2. agent will online the resources provided
    pub async fn update_sandbox_resource(&self) -> Result<()> {
        // todo: skip if static resource mgmt
        // calculate the number of vcpu to be updated
        let cpuinfo = self
            .sandbox
            .cpuinfo()
            .await
            .context("failed to get cpuinfo")?;
        let nr_vcpus = self.container_manager.total_vcpus().await? + (cpuinfo.default_vcpus as u32);

        // calculate the memory to be updated
        let meminfo = self
            .sandbox
            .meminfo()
            .await
            .context("failed to get meminfo")?;
        // the unit here is byte
        let (mut mem_sb_byte, need_pod_swap, mut swap_sb_byte) = self
            .container_manager
            .total_mems(meminfo.enable_guest_swap)
            .await
            .context("failed to calculate total memory requirement for containers")?;
        // default_memory is in MiB
        mem_sb_byte += (meminfo.default_memory << 20) as u64;
        if need_pod_swap {
            swap_sb_byte += (meminfo.default_memory << 20) as i64;
        }

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
