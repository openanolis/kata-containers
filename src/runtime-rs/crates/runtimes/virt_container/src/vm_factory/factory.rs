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

use anyhow::{Context, Result};

#[derive(Debug)]
struct VMFactoryConfig {}

#[derive(Debug)]
struct VMFactory {
    factory_impl: Arc<dyn FactoryBase>,
}

impl FactoryBase for VMFactory {
    fn config(&self) -> Arc<VMConfig> {
        self.factory_impl.config()
    }

    async fn get_base_vm(&self, config: &VMConfig) -> Result<BareVM> {
        self.factory_impl.get_base_vm(config)
    }

    async fn close_factory(&self) -> Result<()> {
        self.factory_impl.close_factory()
    }
}

impl Factory for VMFactory {
    async fn get_vm(&self, config: &VMConfig) -> Result<BareVM> {
        todo!()
    }
}
