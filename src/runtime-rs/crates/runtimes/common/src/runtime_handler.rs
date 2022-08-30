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
    // TODO: support update memory
    // NOTE THAT: if static resource management is configured, this returns an Error
    // update_resources will:
    // 1. calculate the resources required for the virtual machine, and adjust the virtual machine
    // sizing accordingly.
    //      - this recalculate the total number of vcpus instead of adding vcpu directly because
    //        if some of the containers are down/non-running, we can have less vcpu to add
    //      - the total # of vcpus will be the result of the calculation PLUS default vcpu from hypervisor
    // 2. agent will online the resources provided
    pub async fn update_sandbox_resource(&self) -> Result<()> {
        // todo: skip if static resource mgmt
        // todo: support mem update when mem hotplug is supported
        // calculate the number of vcpu to be updated
        let cpuinfo = self
            .sandbox
            .cpuinfo()
            .await
            .context("failed to get cpuinfo")?;
        let nr_vcpus =
            self.container_manager.get_total_vcpus().await? + (cpuinfo.default_vcpus as u32);

        // let hypervisor update the cpus
        // only_plug is true now since dragonball now only support hotplug, not hotunplug
        self.sandbox
            .update_cpu_resource(nr_vcpus, true)
            .await
            .context("fail to update_cpu_resource")?;
        Ok(())
    }
}
