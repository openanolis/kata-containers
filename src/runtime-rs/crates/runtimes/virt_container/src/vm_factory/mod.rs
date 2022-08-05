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

use anyhow::Result;
use async_trait::async_trait;

// FactoryBase trait requires implementations of factory-specific interfaces
// There is a upper level trait called *Factory* that requires this trait, the
// Factory trait handles more generic works like vm config comparison/validation
// and possible CPU/Memory hot plug. Since it is more generic, we are implementing
// that in another trait.
#[async_trait]
pub trait FactoryBase: std::fmt::Debug + Sync + Send {
    pub fn config(&self) -> Arc<vm::VMConfig>;
    pub async fn get_base_vm(&self, config: &vm::VMConfig) -> Result<vm::BareVM>;
    pub async fn close_factory(&self) -> Result<()>;
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
    pub async fn get_vm(&self, config: &vm::VMConfig) -> Result<vm::BareVM>;
}
