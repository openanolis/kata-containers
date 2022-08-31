// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;
use kata_types::config::hypervisor::MemoryInfo;

#[async_trait]
pub trait Sandbox: Send + Sync {
    async fn start(&self, netns: Option<String>, dns: Vec<String>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn cleanup(&self, container_id: &str) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;

    // agent function
    async fn agent_sock(&self) -> Result<String>;

    // utils
    async fn set_iptables(&self, is_ipv6: bool, data: Vec<u8>) -> Result<Vec<u8>>;
    async fn get_iptables(&self, is_ipv6: bool) -> Result<Vec<u8>>;
    // sandbox resource management
    async fn meminfo(&self) -> Result<MemoryInfo>;
    async fn update_mem_resource(&self, new_mem: u32, swap_sz_byte: i64) -> Result<()>;
}
