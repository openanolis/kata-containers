// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use agent::kata::KataAgent;
use kata_types::config::Agent as AgentConfig;

use anyhow::{Context, Result};
use async_trait::async_trait;

use super::{
    new_hypervisor,
    vm::{BareVM, VMConfig},
    FactoryBase,
};

// Direct is a vm factory that creates vm directly, i.e. normal way as if we do
// not have a factory at all
#[derive(Debug)]
pub struct Direct {
    config: Arc<VMConfig>,
}

impl Direct {
    pub fn new(config: Arc<VMConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl FactoryBase for Direct {
    fn config(&self) -> Arc<VMConfig> {
        self.config.clone()
    }

    // direct factory gets the hypervisor and agent in default way
    async fn get_base_vm(&self, config: Arc<VMConfig>) -> Result<BareVM> {
        let hypervisor = new_hypervisor(config.clone())
            .await
            .context("new hypervisor")?;

        // TODO: add a default config method for kata_types::config::Agent
        // get uds from hypervisor and get config from toml_config
        let agent = Arc::new(KataAgent::new(AgentConfig {
            debug: true,
            enable_tracing: false,
            server_port: 1024,
            log_port: 1025,
            dial_timeout_ms: 10,
            reconnect_timeout_ms: 3_000,
            request_timeout_ms: 30_000,
            health_check_request_timeout_ms: 90_000,
            kernel_modules: Default::default(),
            container_pipe_size: 0,
            debug_console_enabled: false,
        }));

        Ok(BareVM::new(hypervisor, agent))
    }

    // Direct factory is not an actual factory, so we do not need to close it
    async fn close_factory(&self) -> Result<()> {
        Ok(())
    }
}
