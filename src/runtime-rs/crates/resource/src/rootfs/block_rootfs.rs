// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::{Rootfs, ROOTFS};
use crate::share_fs::{do_get_guest_path, do_get_host_path};
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;

use hypervisor::{device_manager::DeviceManager, device_type::GenericConfig};
use kata_types::mount::Mount;
use nix::sys::stat::{self, SFlag};
use std::{collections::HashMap, fs, sync::Arc};
use tokio::sync::RwLock;

pub(crate) struct BlockRootfs {
    guest_path: String,
    device_id: String,
    mount: oci::Mount,
    storage: Option<agent::Storage>,
    device_manager: Arc<RwLock<DeviceManager>>,
}

impl BlockRootfs {
    pub async fn new(
        d: Arc<RwLock<DeviceManager>>,
        sid: &str,
        cid: &str,
        dev_id: u64,
        _bundle_path: &str,
        rootfs: &Mount,
    ) -> Result<Self> {
        let container_path = do_get_guest_path(ROOTFS, cid, false, false);
        let host_path = do_get_host_path(ROOTFS, sid, cid, false, false);
        // Create rootfs dir on host to make sure mount point in guest exists, as readonly dir is
        // shared to guest via virtiofs, and guest is unable to create rootfs dir.
        fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create rootfs dir {}: {:?}", host_path, e))?;

        let generic_device_config = &mut GenericConfig {
            host_path: host_path.clone(),
            container_path: container_path.clone(),
            dev_type: "b".to_string(),
            major: stat::major(dev_id) as i64,
            minor: stat::minor(dev_id) as i64,
            file_mode: 0,
            uid: 0,
            gid: 0,
            id: "".to_string(),
            bdf: None,
            driver_options: HashMap::new(),
            ..Default::default()
        };
        let device_id = d
            .write()
            .await
            .try_add_device(generic_device_config)
            .await
            .context("failed to add deivce")?;

        let mut storage = Storage {
            fs_type: rootfs.fs_type.clone(),
            mount_point: container_path.clone(),
            options: rootfs.options.clone(),
            ..Default::default()
        };

        let field_type = d
            .read()
            .await
            .get_driver_options(&device_id)
            .await
            .context("failed to get driver options")?;

        storage.driver = field_type.clone();

        if let Some(path) = d
            .read()
            .await
            .get_device_vm_path(device_id.as_str(), &field_type)
            .await
        {
            storage.source = path;
        }
        Ok(Self {
            guest_path: container_path.clone(),
            device_id,
            mount: oci::Mount {
                ..Default::default()
            },
            storage: Some(storage),
            device_manager: d,
        })
    }
}

#[async_trait]
impl Rootfs for BlockRootfs {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.clone())
    }

    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    async fn get_storage(&self) -> Option<Storage> {
        self.storage.clone()
    }

    async fn cleanup(&self) -> Result<()> {
        self.device_manager
            .write()
            .await
            .try_remove_device(self.device_id.clone())
            .await
    }
}

pub(crate) fn is_block_rootfs(file: &str) -> Option<u64> {
    if file.is_empty() {
        return None;
    }
    match stat::stat(file) {
        Ok(fstat) => {
            if SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFBLK {
                let dev_id = fstat.st_rdev;
                return Some(dev_id);
            }
        }
        Err(_) => return None,
    };
    None
}
