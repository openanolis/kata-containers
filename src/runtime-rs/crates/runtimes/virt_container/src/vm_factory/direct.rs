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

// Direct is a vm factory that creates vm directly, i.e. normal way as if we do
// not have a factory at all
#[derive(Debug)]
struct Direct {
    config: VMConfig,
}

impl Direct {
    pub fn new(config: Arc<TomlConfig>) -> Self {
        Self { config }
    }
}

impl FactoryBase for Direct {
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
