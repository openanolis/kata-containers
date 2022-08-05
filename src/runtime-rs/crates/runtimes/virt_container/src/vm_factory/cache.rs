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

// Cache is a vm factory that creates vm from a *pre-created* *vm-cache*,
// and the pre-created vm should be maintained *alive*
// The purpose of cache factory is speed, since we save the time to start
// a vm completely from scratch.
#[derive(Debug)]
struct Cache {
    // TODO: more fields
    config: VMConfig,
}

impl Cache {
    pub fn new(config: Arc<TomlConfig>) -> Self {
        Self { config }
    }
}

impl FactoryBase for Cache {
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
