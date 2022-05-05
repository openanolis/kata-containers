// Copyright (C) 2019 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use serde_derive::{Deserialize, Serialize};

mod kernel_config;
pub use self::kernel_config::KernelConfigInfo;

/// information for each user defined numa region
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct NumaRegionInfo {
    /// memory size for this region
    pub size: u64,
    /// numa node id on host for this region
    pub host_numa_node_id: Option<u32>,
    /// numa node id on guest for this region
    pub guest_numa_node_id: Option<u32>,
    /// vcpu ids belonging to this region
    pub vcpu_ids: Vec<u32>,
}
