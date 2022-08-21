// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

#[derive(Debug, Default, Clone)]
pub struct BlockConfig {
    /// Unique identifier of the drive.
    pub id: String,

    /// Path of the drive.
    pub path_on_host: String,

    /// If set to true, the drive is opened in read-only mode. Otherwise, the
    /// drive is opened as read-write.
    pub is_readonly: bool,

    /// Don't close `path_on_host` file when dropping the device.
    pub no_drop: bool,

    /// device index
    pub index: u64,

    pub io_limits: Option<IoLimits>,
}
#[derive(Debug, Clone, Default)]
pub struct IoLimits {
    pub read_iops: Option<u64>,
    pub write_iops: Option<u64>,
    pub read_bps: Option<u64>,
    pub write_bps: Option<u64>,
}
