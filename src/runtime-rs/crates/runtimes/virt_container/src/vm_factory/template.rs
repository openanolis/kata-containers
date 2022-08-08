// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use super::{
    vm::{BareVM, VMConfig},
    FactoryBase,
};

use anyhow::Result;
use async_trait::async_trait;

#[derive(Debug)]
#[allow(dead_code)]
pub struct Template {
    template_path: String,
    config: Arc<VMConfig>,
}

// Template is a vm factory creates VM from a *pre-created and saved* template vm.
// Later vm created from template is just a clone of the template vm, which *readonly*
// shares a portion of initial memory (kernel, initramfs and agent). CPU and memory are
// hot plugged when necessary.
impl Template {
    #[allow(dead_code)]
    pub fn new(template_path: String, config: Arc<VMConfig>) -> Self {
        Self {
            template_path,
            config,
        }
    }
}

#[async_trait]
impl FactoryBase for Template {
    fn config(&self) -> Arc<VMConfig> {
        self.config.clone()
    }

    async fn get_base_vm(&self, _config: Arc<VMConfig>) -> Result<BareVM> {
        todo!();
    }

    async fn close_factory(&self) -> Result<()> {
        todo!();
    }
}
