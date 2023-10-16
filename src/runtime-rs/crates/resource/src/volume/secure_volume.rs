// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::Storage;
use anyhow::{Context, Result};
use async_trait::async_trait;
use kata_types::mount;

use super::Volume;

#[derive(Debug)]
pub(crate) struct SecureVolume {
    // storage info
    storage: Option<Storage>,
    // mount info
    mount: oci::Mount,
}

// SecureVolume: secure volume
// because all the info for secure mount passed from csi driver is store in mountinfo,
// we extract mountinfo from json file to generate grpc request to mount external storage
impl SecureVolume {
    pub(crate) fn new(m: &oci::Mount) -> Result<Self> {
        let mount_info = mount::get_volume_mount_info(&m.source)
            .with_context(|| format!("get mount info for {}", &m.source))?;
        let driver_options = vec![format!(
            "{}={}",
            &mount_info.volume_type,
            serde_json::to_string(&mount_info.metadata)
                .with_context(|| format!("serde to json for {:?}", &mount_info.metadata))?
        )];
        Ok(Self {
            storage: Some(Storage {
                driver: "confidential-data-hub".to_string(),
                driver_options: driver_options,
                source: m.source.clone(),
                fs_type: mount_info.fs_type.clone(),
                fs_group: None,
                options: mount_info.options.clone(),
                mount_point: m.destination.clone(),
            }),
            mount: m.clone(),
        })
    }
}

#[async_trait]
impl Volume for SecureVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        let s = if let Some(s) = self.storage.as_ref() {
            vec![s.clone()]
        } else {
            vec![]
        };
        Ok(s)
    }

    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }
}

pub(crate) fn is_secure_volume(m: &oci::Mount) -> bool {
    m.r#type == "secure_mount"
}
