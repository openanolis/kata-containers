// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, fs, path::Path, sync::Arc};

use crate::share_fs::{do_get_guest_path, do_get_host_path};

use super::{share_fs_volume::generate_mount_path, Volume};
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use hypervisor::{
    device_manager::{
        DeviceManager, KATA_BLK_DEV_TYPE, KATA_MMIO_BLK_DEV_TYPE, VIRTIO_BLOCK, VIRTIO_MMIO,
    },
    GenericConfig, Hypervisor,
};
use nix::sys::stat;
use tokio::sync::RwLock;
pub(crate) struct BlockVolume {
    storage: Option<agent::Storage>,
    mount: oci::Mount,
    device_id: String,
}

/// BlockVolume: block device volume
impl BlockVolume {
    pub(crate) async fn new(
        d: Arc<RwLock<DeviceManager>>,
        h: &dyn Hypervisor,
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
                    io_limits: None,
                    ..Default::default()
                },
                h,
            )
            .await?;

        let file_name = Path::new(&m.source).file_name().unwrap().to_str().unwrap();
        let file_name = generate_mount_path(cid, file_name);
        let guest_path = do_get_guest_path(&file_name, cid, true);
        let host_path = do_get_host_path(&file_name, sid, cid, true, read_only);
        fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create rootfs dir {}: {:?}", host_path, e))?;

        // storage
        let mut storage = Storage::default();

        match d.read().await.get_block_driver().await {
            VIRTIO_MMIO => {
                storage.driver = KATA_MMIO_BLK_DEV_TYPE.to_string();
            }
            VIRTIO_BLOCK => {
                storage.driver = KATA_BLK_DEV_TYPE.to_string();
            }
            _ => (),
        }

        storage.options = if read_only {
            vec!["ro".to_string()]
        } else {
            Vec::new()
        };

        storage.mount_point = guest_path.clone();

        if let Some(path) = d
            .read()
            .await
            .get_device_guest_path(device_id.as_str())
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
        })
    }
}

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

    fn cleanup(&self) -> Result<()> {
        todo!()
    }

    fn get_device_id(&self) -> Result<Option<String>> {
        Ok(Some(self.device_id.clone()))
    }
}
