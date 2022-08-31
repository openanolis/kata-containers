// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use kata_sys_util::sl;
use kata_types::config::TomlConfig;
use slog::info;
use tokio::sync::mpsc::Sender;
// MIBTOBYTESSHIFT the number to shift needed to convert MiB to Bytes
pub const MIB_TO_BYTES_SHIFT: i32 = 20;
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
        info!(sl!(), "update_sandbox_resource");
        // calculate the memory to be updated
        let meminfo = self
            .sandbox
            .meminfo()
            .await
            .context("failed to get meminfo")?;
        // the unit here is byte
        let (mem_sb_byte, need_pod_swap, swap_sb_byte) = self
            .container_manager
            .total_mems(meminfo.enable_guest_swap)
            .await
            .context("failed to calculate total memory requirement for containers")?;
        // default_memory is in MiB
        //info!(sl!(),"calculate add mem_sb_mb {}",mem_sb_byte);
        //info!(sl!(),"calculate default memory {}",meminfo.default_memory);
        /*
        mem_sb_byte += (meminfo.default_memory << MIB_TO_BYTES_SHIFT) as u64;
        if need_pod_swap {
            swap_sb_byte += (meminfo.default_memory << MIB_TO_BYTES_SHIFT) as i64;
        }*/

        // todo: setup swap space in guest, when block device hot plug is supported
        // todo: handle err if guest does not support hotplug
        // let hypervisor update the memory
        let mut mem_sb_mb = (mem_sb_byte >> MIB_TO_BYTES_SHIFT) as u32;
        mem_sb_mb += meminfo.default_memory;
        if need_pod_swap {
            let swap_sb_mb = (swap_sb_byte >> MIB_TO_BYTES_SHIFT) as u32;
            mem_sb_mb += swap_sb_mb;
        }
        info!(sl!(), "calculate mem_sb_mb {}", mem_sb_mb);
        self.sandbox
            .update_mem_resource(mem_sb_mb, swap_sb_byte)
            .await
            .context("failed to update_mem_resource")?;
        Ok(())
    }
}
