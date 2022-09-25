// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod block_rootfs;
mod share_fs_rootfs;
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::{device_manager::DeviceManager, Hypervisor};
use kata_types::mount::Mount;
use nix::sys::stat::{self, SFlag};
use std::{sync::Arc, vec::Vec};
use tokio::sync::RwLock;

use crate::share_fs::ShareFs;

const ROOTFS: &str = "rootfs";

#[async_trait]
pub trait Rootfs: Send + Sync {
    async fn get_guest_rootfs_path(&self) -> Result<String>;
    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>>;
    async fn get_storage(&self) -> Result<Option<Storage>>;
    async fn get_device_id(&self) -> Result<Option<String>>;
}

#[derive(Default)]
struct RootFsResourceInner {
    rootfs: Vec<Arc<dyn Rootfs>>,
}

pub struct RootFsResource {
    inner: Arc<RwLock<RootFsResourceInner>>,
}

impl Default for RootFsResource {
    fn default() -> Self {
        Self::new()
    }
}

impl RootFsResource {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(RootFsResourceInner::default())),
        }
    }

    pub async fn handler_rootfs(
        &self,
        share_fs: &Option<Arc<dyn ShareFs>>,
        device_manager: Arc<RwLock<DeviceManager>>,
        h: &dyn Hypervisor,
        sid: &str,
        cid: &str,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
    ) -> Result<Arc<dyn Rootfs>> {
        match rootfs_mounts {
            mounts_vec if is_single_layer_rootfs(mounts_vec) => {
                // Safe as single_layer_rootfs must have one layer
                let layer = &mounts_vec[0];
                let mut inner = self.inner.write().await;
                let (is_block, dev_id) = check_block_device(&layer.source);

                let rootfs = if is_block {
                    if let Some(id) = dev_id {
                        info!(sl!(), "block device: {}", id);
                        let rootfs = Arc::new(
                            block_rootfs::BlockRootfs::new(
                                device_manager,
                                h,
                                sid,
                                cid,
                                id,
                                bundle_path,
                                layer,
                            )
                            .await
                            .context("new block rootfs")?,
                        );
                        return Ok(rootfs);
                    } else {
                        return Err(anyhow!("empty device id"));
                    }
                } else if let Some(share_fs) = share_fs {
                    // share fs rootfs
                    let share_fs_mount = share_fs.get_share_fs_mount();
                    Arc::new(
                        share_fs_rootfs::ShareFsRootfs::new(
                            &share_fs_mount,
                            cid,
                            bundle_path,
                            layer,
                        )
                        .await
                        .context("new share fs rootfs")?,
                    )
                } else {
                    return Err(anyhow!("unsupported rootfs {:?}", &layer));
                };
                inner.rootfs.push(rootfs.clone());
                Ok(rootfs)
            }
            _ => {
                return Err(anyhow!(
                    "unsupported rootfs mounts count {}",
                    rootfs_mounts.len()
                ))
            }
        }
    }

    pub async fn dump(&self) {
        let inner = self.inner.read().await;
        for r in &inner.rootfs {
            info!(
                sl!(),
                "rootfs {:?}: count {}",
                r.get_guest_rootfs_path().await,
                Arc::strong_count(r)
            );
        }
    }
}

fn is_single_layer_rootfs(rootfs_mounts: &[Mount]) -> bool {
    rootfs_mounts.len() == 1
}

fn check_block_device(file: &str) -> (bool, Option<u64>) {
    if file.is_empty() {
        return (false, None);
    }

    match stat::stat(file) {
        Ok(fstat) => {
            if SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFBLK {
                let dev_id = fstat.st_rdev;
                return (true, Some(dev_id));
            }
        }
        Err(_) => return (false, None),
    };

    (false, None)
}
