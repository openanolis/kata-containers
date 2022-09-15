// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use hypervisor::Hypervisor;
use oci::LinuxResources;

/// This struct may be used to track some vm resource information.
/// For example, the mapping relationship between vCPU and CPU can be
/// stored here.
#[derive(Default)]
pub struct VmResource {}

impl VmResource {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn update_vm_resources(
        &self,
        _linux_resources: Option<&LinuxResources>,
        _h: &dyn Hypervisor,
    ) -> Result<()> {
        Ok(())
    }
}
