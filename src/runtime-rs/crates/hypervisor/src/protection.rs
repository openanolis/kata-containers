// Copyright (c) 2022 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use std::{fs, path::Path};

use crate::utils::{get_cpu_flags, PROC_CPUINFO};

use serde::{Deserialize, Serialize};

const TDX_CPU_FLAG: &str = "tdx";
const TDX_SYS_FIRMWARE_PATH: &str = "/sys/firmware/tdx_seam/";
const SEV_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev";
const SNP_KVM_PARAMETER_PATH: &str = "/sys/module/kvm_amd/parameters/sev_snp";

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub(crate) enum GuestProtectionType {
    PefProtection,
    SevProtection,
    SnpProtection,
    TdxPtotection,
    None,
}

pub(crate) fn available_guest_protection() -> Result<GuestProtectionType> {
    let flags = get_cpu_flags(PROC_CPUINFO).context("get cpuinfo flags")?;

    // TDX is supported and properly loaded when the firmware directory exists of "tdx" is part of the CPU flags
    if Path::new(TDX_SYS_FIRMWARE_PATH).exists() || flags[TDX_CPU_FLAG] {
        return Ok(GuestProtectionType::TdxPtotection);
    }

    // SEV is supported and enabled when the kvm module `sev` parameter is set to `1` (or `Y` for linux >= 5.12)
    if let Ok(contents) = fs::read(SEV_KVM_PARAMETER_PATH) {
        if contents.len() > 0 && (contents[0] == b'Y' || contents[0] == b'1') {
            return Ok(GuestProtectionType::SevProtection);
        }
    }

    // SEV-SNP is supported and enabled when the kvm module `sev_snp` parameter is set to `Y`
    // SEV-SNP support infers SEV (-ES) support
    if let Ok(contents) = fs::read(SNP_KVM_PARAMETER_PATH) {
        if contents.len() > 0 && contents[0] == b'Y' {
            return Ok(GuestProtectionType::SnpProtection);
        }
    }

    Ok(GuestProtectionType::None)
}
