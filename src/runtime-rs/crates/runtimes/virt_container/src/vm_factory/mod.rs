// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod cache;
pub mod direct;
pub mod factory;
pub mod template;
pub mod vm;

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::dragonball::Dragonball;
use hypervisor::Hypervisor;
use kata_types::config::TomlConfig;

use self::direct::Direct;
use self::vm::VMConfig;

const HYPERVISOR_DRAGONBALL: &str = "dragonball";

// FactoryBase trait requires implementations of factory-specific interfaces
// There is a upper level trait called *Factory* that requires this trait, the
// Factory trait handles more generic works like vm config comparison/validation
// and possible CPU/Memory hot plug. Since it is more generic, we are implementing
// that in another trait.
#[async_trait]
pub trait FactoryBase: std::fmt::Debug + Sync + Send {
    fn config(&self) -> Arc<vm::VMConfig>;
    async fn get_base_vm(&self, config: Arc<vm::VMConfig>) -> Result<vm::BareVM>;
    async fn close_factory(&self) -> Result<()>;
    // TODO: async fn get_vm_status(&self) -> Result<>; returns a grpc status
}

// Factory is a more generic trait, which is directly called from upper layers.
// In general, each factory implementation implements FactoryBase trait. Upper layer
// use a object(which implements Factory trait) to multiplex into each factory
// implementation.
// The purpose of this is that Factory trait implements repeated operations that
// all factory implementations need.
#[async_trait]
pub trait Factory: FactoryBase {
    async fn get_vm(&self, config: &vm::VMConfig) -> Result<vm::BareVM>;
}

// return an instance of FactoryBase according to the configuration file.
pub fn get_factory_base(config: Arc<TomlConfig>) -> Result<Arc<dyn FactoryBase>> {
    let vm_config = Arc::new(VMConfig::new(config.clone()));
    info!(
        sl!(),
        "getting factory type {:?}",
        config.runtime.factory_type.clone()
    );
    match config.runtime.factory_type.as_str() {
        "template" => {
            warn!(sl!(), "template factory not supported yet");
        }

        "cache" => {
            warn!(sl!(), "template factory not supported yet");
        }

        _ => {
            return Ok(Arc::new(Direct::new(vm_config)));
        }
    }
    Ok(Arc::new(Direct::new(vm_config)))
}

async fn new_hypervisor(config: Arc<VMConfig>) -> Result<Arc<dyn Hypervisor>> {
    let hypervisor_name = config.hypervisor_name();
    let hypervisor_config = config
        .hypervisor_config()
        .context("get hypervisor config")?;

    // TODO: support other hypervisor
    // issue: https://github.com/kata-containers/kata-containers/issues/4634
    match hypervisor_name.as_str() {
        HYPERVISOR_DRAGONBALL => {
            let mut hypervisor = Dragonball::new();
            hypervisor
                .set_hypervisor_config(hypervisor_config.clone())
                .await;
            Ok(Arc::new(hypervisor))
        }
        _ => Err(anyhow!("Unsupported hypervisor {}", &hypervisor_name)),
    }
}
