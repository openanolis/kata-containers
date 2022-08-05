// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use kata_types::config::TomlConfig;

use super::{
    vm::{BareVM, VMConfig},
    FactoryBase,
};

#[derive(Debug)]
struct Template {
    template_path: String,
    config: VMConfig,
}

// Template is a vm factory creates VM from a *pre-created and saved* template vm.
// Later vm created from template is just a clone of the template vm, which *readonly*
// shares a portion of initial memory (kernel, initramfs and agent). CPU and memory are
// hot plugged when necessary.
impl Template {
    pub fn new(template_path: String, config: Arc<TomlConfig>) -> Self {
        Self {
            template_path,
            config,
        }
    }
}

impl FactoryBase for Template {
    fn config(&self) -> Arc<VMConfig> {
        Arc::new(self.config)
    }

    async fn get_base_vm(&self, config: &VMConfig) -> Result<BareVM> {
        todo!();
    }

    async fn close_factory(&self) -> Result<()> {
        todo!();
    }
}
