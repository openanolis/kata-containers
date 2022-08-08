// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[macro_use]
extern crate slog;

logging::logger_with_subsystem!(sl, "virt-container");

mod container_manager;
pub mod health_check;
pub mod sandbox;
pub mod vm_factory;

use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use common::{message::Message, RuntimeHandler, RuntimeInstance};
use kata_types::config::{hypervisor::register_hypervisor_plugin, DragonballConfig, TomlConfig};
use resource::ResourceManager;
use tokio::sync::mpsc::Sender;
use vm_factory::{factory::VMFactory, get_factory_base, vm::VMConfig, FactoryBase};

unsafe impl Send for VirtContainer {}
unsafe impl Sync for VirtContainer {}
pub struct VirtContainer {}

#[async_trait]
impl RuntimeHandler for VirtContainer {
    fn init() -> Result<()> {
        // register
        let dragonball_config = Arc::new(DragonballConfig::new());
        register_hypervisor_plugin("dragonball", dragonball_config);
        Ok(())
    }

    fn name() -> String {
        "virt_container".to_string()
    }

    fn new_handler() -> Arc<dyn RuntimeHandler> {
        Arc::new(VirtContainer {})
    }

    async fn new_instance(
        &self,
        sid: &str,
        msg_sender: Sender<Message>,
        config: Arc<TomlConfig>,
    ) -> Result<RuntimeInstance> {
        // build vm factory, the vm creation is multiplexed by vm factory
        // TODO: support template and cache
        // FIXME: use Factory.getVM() instead of get_base_vm() for abstraction
        let vf_config = Arc::new(VMConfig::new(config.clone()));
        let factory_impl = get_factory_base(config.clone()).context("get factory_impl")?;
        let factory = VMFactory::new(factory_impl);
        let bare_vm = factory
            .get_base_vm(vf_config.clone())
            .await
            .context("get bare vm")?;

        let hypervisor = bare_vm.get_hypervisor();
        let agent = bare_vm.get_agent();

        let resource_manager = Arc::new(ResourceManager::new(
            sid,
            agent.clone(),
            hypervisor.clone(),
            config,
        )?);
        let pid = std::process::id();

        let sandbox = sandbox::VirtSandbox::new(
            sid,
            msg_sender,
            agent.clone(),
            hypervisor,
            resource_manager.clone(),
        )
        .await
        .context("new virt sandbox")?;
        let container_manager =
            container_manager::VirtContainerManager::new(sid, pid, agent, resource_manager);
        Ok(RuntimeInstance {
            sandbox: Arc::new(sandbox),
            container_manager: Arc::new(container_manager),
        })
    }

    fn cleanup(&self, _id: &str) -> Result<()> {
        // TODO
        Ok(())
    }
}
