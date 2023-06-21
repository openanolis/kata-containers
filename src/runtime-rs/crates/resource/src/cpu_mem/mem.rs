// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Context, Ok, Result};
use hypervisor::Hypervisor;
use kata_types::config::TomlConfig;
use oci::LinuxResources;
use tokio::sync::RwLock;

use crate::cpu_mem::initial_size::InitialSizeManager;
use crate::ResourceUpdateOp;

// MIB_TO_BYTES_SHIFT the number to shift needed to convert MiB to Bytes
pub const MIB_TO_BYTES_SHIFT: i32 = 20;

#[derive(Default, Debug, Clone)]
pub struct MemResource {
    /// Current memory
    pub(crate) current_mem: Arc<RwLock<u32>>,

    /// Default memory
    pub(crate) orig_toml_default_mem: u32,

    /// MemResource of each container
    pub(crate) container_mem_resources: Arc<RwLock<HashMap<String, LinuxResources>>>,
}

impl MemResource {
    pub fn new(config: Arc<TomlConfig>, init_size_manager: InitialSizeManager) -> Result<Self> {
        let hypervisor_name = config.runtime.hypervisor_name.clone();
        let hypervisor_config = config
            .hypervisor
            .get(&hypervisor_name)
            .context("failed to get hypervisor")?;

        Ok(Self {
            current_mem: Arc::new(RwLock::new(hypervisor_config.memory_info.default_memory)),
            container_mem_resources: Arc::new(RwLock::new(HashMap::new())),
            orig_toml_default_mem: init_size_manager.get_orig_toml_default_mem(),
        })
    }

    pub(crate) async fn update_mem_resources(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
        hypervisor: &dyn Hypervisor,
    ) -> Result<()> {
        self.update_container_mem_resources(cid, linux_resources, op)
            .await
            .context("update container memory resources")?;
        // the unit here is MB
        let mut mem_sb_mb = self
            .total_mems()
            .await
            .context("failed to calculate total memory requirement for containers")?;
        mem_sb_mb += self.orig_toml_default_mem;
        info!(sl!(), "calculate mem_sb_mb {}", mem_sb_mb);

        let curr_mem = self
            .do_update_mem_resource(mem_sb_mb, hypervisor)
            .await
            .context("failed to update_mem_resource")?;

        self.update_current_mem(curr_mem).await;
        Ok(())
    }

    async fn update_current_mem(&self, new_mem: u32) {
        let mut current_mem = self.current_mem.write().await;
        *current_mem = new_mem;
    }

    async fn get_current_mem(&self) -> u32 {
        let current_mem = self.current_mem.read().await;
        *current_mem
    }

    async fn total_mems(&self) -> Result<u32> {
        let mut mem_sandbox = 0;
        let resources = self.container_mem_resources.read().await;

        for (_, r) in resources.iter() {
            for l in &r.hugepage_limits {
                mem_sandbox += l.limit;
            }

            if let Some(memory) = &r.memory {
                // set current_limit to 0 if memory limit is not set to container
                let _current_limit = memory.limit.map_or(0, |limit| {
                    mem_sandbox += limit as u64;
                    info!(sl!(), "memory sb: {}, memory limit: {}", mem_sandbox, limit);
                    limit
                });
                // TODO support memory guest swap
                // https://github.com/kata-containers/kata-containers/issues/7293
            }
        }

        Ok((mem_sandbox >> MIB_TO_BYTES_SHIFT) as u32)
    }

    // update container_cpu_resources field
    async fn update_container_mem_resources(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
        op: ResourceUpdateOp,
    ) -> Result<()> {
        if let Some(r) = linux_resources {
            let mut resources = self.container_mem_resources.write().await;
            match op {
                ResourceUpdateOp::Add | ResourceUpdateOp::Update => {
                    resources.insert(cid.to_owned(), r.clone());
                }
                ResourceUpdateOp::Del => {
                    resources.remove(cid);
                }
            }
        }
        Ok(())
    }

    async fn do_update_mem_resource(
        &self,
        new_mem: u32,
        hypervisor: &dyn Hypervisor,
    ) -> Result<u32> {
        info!(sl!(), "requesting vmm to update memory to {:?}", new_mem);

        let current_mem = self.get_current_mem().await;
        let (new_memory, _mem_config) = hypervisor
            .resize_memory(current_mem, new_mem)
            .await
            .context("resize memory")?;

        Ok(new_memory)
    }
}
