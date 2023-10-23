// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::Path;

use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use kata_types::mount;

use super::Volume;
use crate::share_fs::EPHEMERAL_PATH;

#[derive(Debug)]
pub(crate) struct SecureVolume {
    // storage info
    storage: Vec<Storage>,
    // mount info
    mount: oci::Mount,
}

// SecureVolume: secure volume
// because all the info for secure mount passed from csi driver is store in mountinfo,
// we extract mountinfo from json file to generate grpc request to mount external storage
impl SecureVolume {
    pub(crate) fn new(
        m: &oci::Mount,
        mount_info: &mount::DirectVolumeMountInfo,
        cid: &str,
    ) -> Result<Self> {
        let driver_options = vec![format!(
            "{}={}",
            &mount_info.volume_type,
            serde_json::to_string(&mount_info.metadata)
                .with_context(|| format!("serde to json for {:?}", &mount_info.metadata))?
        )];

        let file_name = Path::new(&m.source)
            .file_name()
            .context("get file name from mount.source")?;

        let guest_path = Path::new(EPHEMERAL_PATH)
            .join(cid)
            .join(file_name)
            .into_os_string()
            .into_string()
            .map_err(|e| anyhow!("failed to get ephemeral path {:?}", e))?;

        let mut mount = m.clone();
        mount.source = guest_path.clone();
        Ok(Self {
            storage: vec![
                Storage {
                    driver: "local".to_string(),
                    driver_options: vec![],
                    source: "".to_string(),
                    fs_type: "".to_string(),
                    fs_group: None,
                    options: vec![],
                    // the mount_point is not used by CDH
                    mount_point: guest_path.clone(),
                },
                Storage {
                    driver: "confidential-data-hub".to_string(),
                    driver_options: driver_options,
                    source: guest_path.clone(),
                    fs_type: mount_info.fs_type.clone(),
                    fs_group: None,
                    options: mount_info.options.clone(),
                    // the mount_point is not used by CDH
                    mount_point: m.destination.clone(),
                },
            ],
            mount,
        })
    }
}

#[async_trait]
impl Volume for SecureVolume {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<agent::Storage>> {
        Ok(self.storage.clone())
    }

    async fn cleanup(&self) -> Result<()> {
        Ok(())
    }
}

pub(crate) fn is_secure_volume(mount_info: &mount::DirectVolumeMountInfo) -> bool {
    mount_info.fs_type == "secure_mount"
}
