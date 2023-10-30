// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use kata_types::config::TomlConfig;

use super::Volume;

#[derive(Debug)]
pub(crate) struct SealedSecretVolume {
    mount: oci::Mount,
}

/// SealedSecretVolume: sealed secret volume
impl SealedSecretVolume {
    pub fn new(mount: &oci::Mount) -> Result<Self> {
        let mut mount = mount.clone();
        mount.destination = format!("/sealed{}", mount.destination);
        Ok(Self { mount })
    }
}

#[async_trait]
impl Volume for SealedSecretVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(vec![])
    }

    async fn cleanup(&self) -> Result<()> {
        // TODO: Clean up DefaultVolume
        warn!(sl!(), "Cleaning up DefaultVolume is still unimplemented.");
        Ok(())
    }
}

pub(crate) fn is_sealed_secret_volume(toml_config: &Arc<TomlConfig>, m: &oci::Mount) -> bool {
    toml_config.runtime.sealed_secret_enabled && m.source.contains("kubernetes.io~secret")
}
