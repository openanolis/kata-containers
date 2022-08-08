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

// Cache is a vm factory that creates vm from a *pre-created* *vm-cache*,
// and the pre-created vm should be maintained *alive*
// The purpose of cache factory is speed, since we save the time to start
// a vm completely from scratch.
#[derive(Debug)]
pub struct Cache {
    // TODO: more fields
    config: Arc<VMConfig>,
}

impl Cache {
    #[allow(dead_code)]
    pub fn new(config: Arc<VMConfig>) -> Self {
        Self { config }
    }
}

#[async_trait]
impl FactoryBase for Cache {
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
