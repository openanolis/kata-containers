// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use std::path::Path;

use crate::utils::{get_cpu_flags, PROC_CPUINFO};

use serde::{Deserialize, Serialize};

const TDX_CPU_FLAG: &str = "tdx";
const TDX_SYS_FIRMWARE_PATH: &str = "/sys/firmware/tdx_seam/";

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub(crate) enum GuestProtectionType {
    TdxPtotection,
    None,
}

pub(crate) fn available_guest_protection() -> Result<GuestProtectionType> {
    let flags = get_cpu_flags(PROC_CPUINFO).context("get cpuinfo flags")?;

    // TDX is supported and properly loaded when the firmware directory exists of "tdx" is part of the CPU flags
    if Path::new(TDX_SYS_FIRMWARE_PATH).exists() || flags[TDX_CPU_FLAG] {
        return Ok(GuestProtectionType::TdxPtotection);
    }

    Ok(GuestProtectionType::None)
}
