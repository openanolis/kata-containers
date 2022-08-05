// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use agent::Agent;
use hypervisor::Hypervisor;
use kata_types::config::{Agent as AgentConfig, Hypervisor as HypervisorConfig, TomlConfig};

// VMConfig contains all info that a new blackbox VM needs.
// Acutally we need hypervisor_name, hypervisor config and agent config
// all of these can be extracted if we offer appropriate methods
#[derive(Debug)]
pub(crate) struct VMConfig {
    config: Arc<TomlConfig>,
}

impl VMConfig {
    pub fn new(config: Arc<TomlConfig>) -> Self {
        Self { config }
    }

    pub fn hypervisor_name(&self) -> String {
        self.config.runtime.hypervisor_name.clone()
    }

    pub fn agent_name(&self) -> String {
        self.config.runtime.agent_name.clone()
    }

    pub fn hypervisor_config(&self) -> Result<HypervisorConfig> {
        let hypervisor_name = self.hypervisor_name().as_ref();
        let hypervisor_config = self
            .config
            .hypervisor
            .get(hypervisor_name)
            .ok_or_else(|| anyhow!("failed to get hypervisor for {}", &hypervisor_name))
            .context("get hypervisor")?;
        hypervisor_config
    }

    pub fn agent_config(&self) -> Result<AgentConfig> {
        let agent_name = self.agent_name().as_ref();
        let agent_config = self
            .config
            .agent
            .get(agent_name)
            .ok_or_else(|| anyhow!("failed to get agent for {}", &agent_name))
            .context("get agent")?;
        agent_config
    }
}

// BareVM abstracts a Virtual Machine with no sandbox/container specific information.
// The VirtSandbox actually contains all the information we need to do so, but it contains
// too much info, If we use VirtSandbox to abstract the bare vm, the redundant info may
// be harmful for further development.
#[derive(Debug)]
pub(crate) struct BareVM {
    id: String,
    hypervisor: Arc<dyn Hypervisor>,
    agent: Arc<dyn Agent>,
    cpu: u32,
    memory: u32,
    cpu_delta: u32,
}

impl BareVM {}
