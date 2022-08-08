// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use agent::Agent;
use hypervisor::Hypervisor;
use kata_types::config::{Agent as AgentConfig, Hypervisor as HypervisorConfig, TomlConfig};

use anyhow::{anyhow, Result};

// VMConfig contains all info that a new blackbox VM needs.
// Acutally we need hypervisor_name, hypervisor config and agent config all of
// these can be extracted without exposing other info if we offer appropriate methods
#[derive(Debug)]
pub struct VMConfig {
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

    pub fn hypervisor_config(&self) -> Result<&HypervisorConfig> {
        let hypervisor_name = self.hypervisor_name();
        let hypervisor_config = self
            .config
            .hypervisor
            .get(&hypervisor_name)
            .ok_or_else(|| anyhow!("failed to get hypervisor for {}", &hypervisor_name))?;
        Ok(hypervisor_config)
    }

    pub fn agent_config(&self) -> Result<&AgentConfig> {
        let agent_name = self.agent_name();
        let agent_config = self
            .config
            .agent
            .get(&agent_name)
            .ok_or_else(|| anyhow!("failed to get agent for {}", &agent_name))?;
        Ok(agent_config)
    }
}

// BareVM abstracts a Virtual Machine with no sandbox/container specific information.
// We mainly expose the *hypervisor and agent* on sandbox/VM launching
//
// The VirtSandbox actually contains all the information we need to do so, but it contains
// too much info, If we use VirtSandbox to abstract the bare vm, there are too much redundant info
// making us hard to maintain this
pub struct BareVM {
    hypervisor: Arc<dyn Hypervisor>,
    agent: Arc<dyn Agent>,
}

impl BareVM {
    pub fn new(hypervisor: Arc<dyn Hypervisor>, agent: Arc<dyn Agent>) -> Self {
        Self { hypervisor, agent }
    }

    pub fn get_hypervisor(&self) -> Arc<dyn Hypervisor> {
        self.hypervisor.clone()
    }

    pub fn get_agent(&self) -> Arc<dyn Agent> {
        self.agent.clone()
    }

    // returns the number of cpus
    pub async fn ncpus(&self) -> i32 {
        self.hypervisor
            .hypervisor_config()
            .await
            .cpu_info
            .default_vcpus
    }

    // returns the size of memory
    pub async fn mem_size(&self) -> u32 {
        self.hypervisor
            .hypervisor_config()
            .await
            .memory_info
            .default_memory
    }
}
