// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

mod block_volume;
mod default_volume;
mod share_fs_volume;
mod shm_volume;

use std::{sync::Arc, vec::Vec};

use anyhow::{Context, Result};
use hypervisor::{device_manager::DeviceManager, Hypervisor};
use nix::sys::stat::{self, SFlag};
use tokio::sync::RwLock;

use crate::share_fs::ShareFs;
pub trait Volume: Send + Sync {
    fn get_volume_mount(&self) -> Result<Vec<oci::Mount>>;
    fn get_storage(&self) -> Result<Vec<agent::Storage>>;
    fn get_device_id(&self) -> Result<Option<String>>;
    fn cleanup(&self) -> Result<()>;
}

#[derive(Default)]
pub struct VolumeResourceInner {
    volumes: Vec<Arc<dyn Volume>>,
}

#[derive(Default)]
pub struct VolumeResource {
    inner: Arc<RwLock<VolumeResourceInner>>,
}

impl VolumeResource {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn handler_volumes(
        &self,
        share_fs: &Option<Arc<dyn ShareFs>>,
        cid: &str,
        oci_mounts: &[oci::Mount],
        d: Arc<RwLock<DeviceManager>>,
        h: &dyn Hypervisor,
        sid: &str,
    ) -> Result<Vec<Arc<dyn Volume>>> {
        let mut volumes: Vec<Arc<dyn Volume>> = vec![];
        info!(sl!(), " oci mount is : {:?}", oci_mounts.clone());
        for m in oci_mounts {
            let mut read_only = false;
            for o in &m.options {
                if o == "ro" {
                    read_only = true;
                    break;
                }
            }
            let volume: Arc<dyn Volume> = if shm_volume::is_shim_volume(m) {
                let shm_size = shm_volume::DEFAULT_SHM_SIZE;
                Arc::new(
                    shm_volume::ShmVolume::new(m, shm_size)
                        .with_context(|| format!("new shm volume {:?}", m))?,
                )
            } else if is_block_volume(m) {
                Arc::new(
                    block_volume::BlockVolume::new(Arc::clone(&d), h, m, read_only, cid, sid)
                        .await
                        .with_context(|| format!("new share fs volume {:?}", m))?,
                )
            } else if share_fs_volume::is_share_fs_volume(m) {
                Arc::new(
                    share_fs_volume::ShareFsVolume::new(share_fs, m, cid)
                        .await
                        .with_context(|| format!("new share fs volume {:?}", m))?,
                )
            } else if is_skip_volume(m) {
                info!(sl!(), "skip volume {:?}", m);
                continue;
            } else {
                Arc::new(
                    default_volume::DefaultVolume::new(m)
                        .with_context(|| format!("new default volume {:?}", m))?,
                )
            };

            volumes.push(volume.clone());
            let mut inner = self.inner.write().await;
            inner.volumes.push(volume);
        }

        Ok(volumes)
    }

    pub async fn dump(&self) {
        let inner = self.inner.read().await;
        for v in &inner.volumes {
            info!(
                sl!(),
                "volume mount {:?}: count {}",
                v.get_volume_mount(),
                Arc::strong_count(v)
            );
        }
    }
}

fn is_skip_volume(_m: &oci::Mount) -> bool {
    // TODO: support volume check
    false
}

fn is_block_volume(m: &oci::Mount) -> bool {
    if m.r#type != "bind" {
        return false;
    }
    if let Ok(fstat) = stat::stat(m.source.as_str()).context(format!("stat {}", m.source)) {
        info!(sl!(), "device stat: {:?}", fstat);
        return SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFBLK;
    }
    return false;
}
