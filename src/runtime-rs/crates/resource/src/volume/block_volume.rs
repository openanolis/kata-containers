// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use async_trait::async_trait;
use std::{collections::HashMap, fs, path::Path, sync::Arc};

use crate::share_fs::{do_get_guest_path, do_get_host_path};

use super::{share_fs_volume::generate_mount_path, Volume};
use agent::Storage;
use anyhow::{anyhow, Context};
use hypervisor::DeviceType::Block;
use hypervisor::{device_manager::DeviceManager, device_type::GenericConfig};
use nix::sys::stat::{self, SFlag};
use tokio::sync::RwLock;

pub(crate) struct BlockVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
    device_manager: Arc<RwLock<DeviceManager>>,
}

/// BlockVolume: block device volume
impl BlockVolume {
    pub(crate) async fn new(
        d: Arc<RwLock<DeviceManager>>,
        m: &oci::Mount,
        read_only: bool,
        cid: &str,
        sid: &str,
    ) -> Result<Self> {
        let fstat = stat::stat(m.source.as_str()).context(format!("stat {}", m.source))?;
        info!(sl!(), "device stat: {:?}", fstat);
        let mut options = HashMap::new();
        if read_only {
            options.insert("read_only".to_string(), "true".to_string());
        }
        let device_id = d
            .write()
            .await
            .try_add_device(
                &mut GenericConfig {
                    host_path: m.source.clone(),
                    container_path: m.destination.clone(),
                    dev_type: "b".to_string(),
                    major: stat::major(fstat.st_rdev) as i64,
                    minor: stat::minor(fstat.st_rdev) as i64,
                    file_mode: 0,
                    uid: 0,
                    gid: 0,
                    id: "".to_string(),
                    bdf: None,
                    driver_options: options,
                    ..Default::default()
                },
                &Block,
            )
            .await
            .context("failed to add device")?;

        let file_name = Path::new(&m.source).file_name().unwrap().to_str().unwrap();
        let file_name = generate_mount_path(cid, file_name);
        let guest_path = do_get_guest_path(&file_name, cid, true, false);
        let host_path = do_get_host_path(&file_name, sid, cid, true, read_only);
        fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create rootfs dir {}: {:?}", host_path, e))?;

        // storage
        let mut storage = Storage {
            driver: d
                .read()
                .await
                .get_driver_options(&Block)
                .await
                .context("failed to get driver options")?,
            options: if read_only {
                vec!["ro".to_string()]
            } else {
                Vec::new()
            },
            mount_point: guest_path.clone(),
            ..Default::default()
        };

        if let Some(path) = d
            .read()
            .await
            .get_device_guest_path(device_id.as_str(), &Block)
            .await
        {
            storage.source = path;
        }

        // If the volume had specified the filesystem type, use it. Otherwise, set it
        // to ext4 since but right now we only support it.
        if m.r#type != "bind" {
            storage.fs_type = m.r#type.clone();
        } else {
            storage.fs_type = "ext4".to_string();
        }

        // mount
        let mount = oci::Mount {
            destination: m.destination.clone(),
            r#type: m.r#type.clone(),
            source: guest_path.clone(),
            options: m.options.clone(),
        };

        Ok(Self {
            storage: Some(storage),
            mount,
            device_id,
            device_manager: d,
        })
    }
}

#[async_trait]
impl Volume for BlockVolume {
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
        self.device_manager
            .write()
            .await
            .try_remove_device(self.device_id.clone(), &Block)
            .await
    }
}

pub(crate) fn is_block_volume(m: &oci::Mount) -> bool {
    if m.r#type != "bind" {
        return false;
    }
    if let Ok(fstat) = stat::stat(m.source.as_str()).context(format!("stat {}", m.source)) {
        info!(sl!(), "device stat: {:?}", fstat);
        return SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFBLK;
    }
    false
}
