// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[derive(Debug, Default)]
pub struct MemoryConfig {
    pub slot: u32,
    pub size_mb: u32,
    pub addr: u64,
    pub probe: bool,
}
