// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::{
    vm::{BareVM, VMConfig},
    Factory, FactoryBase,
};
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug)]
struct VMFactoryConfig {}

// A more generic abtraction of each factory implementation
#[derive(Debug)]
pub struct VMFactory {
    factory_impl: Arc<dyn FactoryBase>,
}

impl VMFactory {
    pub fn new(factory_impl: Arc<dyn FactoryBase>) -> Self {
        Self { factory_impl }
    }
}

#[async_trait]
impl FactoryBase for VMFactory {
    fn config(&self) -> Arc<VMConfig> {
        self.factory_impl.config()
    }

    async fn get_base_vm(&self, config: Arc<VMConfig>) -> Result<BareVM> {
        self.factory_impl.get_base_vm(config).await
    }

    async fn close_factory(&self) -> Result<()> {
        self.factory_impl.close_factory().await
    }
}

#[async_trait]
impl Factory for VMFactory {
    async fn get_vm(&self, _config: &VMConfig) -> Result<BareVM> {
        todo!()
    }
}
