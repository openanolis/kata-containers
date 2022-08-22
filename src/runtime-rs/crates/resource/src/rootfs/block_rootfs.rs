// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use agent::Storage;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hypervisor::Hypervisor;
use kata_types::mount::Mount;
use nix::sys::stat;
use std::{collections::HashMap, fs, sync::Arc};
use tokio::sync::Mutex;

use crate::{
    device::{
        device::DeviceInfo,
        manager::{
            DeviceManager, KATA_BLK_DEV_TYPE, KATA_MMIO_BLK_DEV_TYPE, VIRTIO_BLOCK, VIRTIO_MMIO,
        },
    },
    share_fs::{do_get_guest_path, do_get_host_path},
};

use super::{Rootfs, ROOTFS};

pub(crate) struct BlockRootfs {
    guest_path: String,
    mount: oci::Mount,
    _storage: Option<agent::Storage>,
}

impl BlockRootfs {
    pub async fn new(
        d: Arc<Mutex<DeviceManager>>,
        h: &dyn Hypervisor,
        sid: &str,
        cid: &str,
        dev_id: u64,
        _bundle_path: &str,
        rootfs: &Mount,
    ) -> Result<Self> {
        let container_path = do_get_guest_path(ROOTFS, cid, false);
        let host_path = do_get_host_path(ROOTFS, sid, cid, false, false);
        // Create rootfs dir on host to make sure mount point in guest exists, as readonly dir is
        // shared to guest via virtiofs, and guest is unable to create rootfs dir.
        fs::create_dir_all(&host_path)
            .map_err(|e| anyhow!("failed to create rootfs dir {}: {:?}", host_path, e))?;

        let _device_id = d
            .lock()
            .await
            .new_device(
                &mut DeviceInfo {
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
                    io_limits: None,
                },
                h,
            )
            .await?;
        let mut storage = Storage {
            fs_type: String::from(rootfs.fs_type.clone()),
            mount_point: container_path.clone(),
            options: rootfs.options.clone(),
            ..Default::default()
        };

        match d.lock().await.get_block_driver().await {
            VIRTIO_MMIO => {
                storage.driver = KATA_MMIO_BLK_DEV_TYPE.to_string();
            }
            VIRTIO_BLOCK => {
                storage.driver = KATA_BLK_DEV_TYPE.to_string();
            }
            _ => (),
        }

        Ok(Self {
            guest_path: container_path.clone(),
            mount: oci::Mount {
                destination: container_path.clone(),
                source: host_path.clone(),
                options: rootfs.options.clone(),
                ..Default::default()
            },
            _storage: Some(Storage {
                fs_type: String::from(rootfs.fs_type.clone()),
                mount_point: container_path,
                options: rootfs.options.clone(),
                ..Default::default()
            }),
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

    async fn get_storage(&self) -> Result<Vec<Storage>> {
        todo!()
    }
}
