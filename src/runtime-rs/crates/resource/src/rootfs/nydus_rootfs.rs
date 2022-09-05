// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::{Rootfs, TYPE_OVERLAY_FS};
use crate::{
    rootfs::HYBRID_ROOTFS_WRITABLE_LAYER_LOWER_DIR,
    share_fs::{
        do_get_guest_path, do_get_guest_virtiofs_path, get_host_rw_shared_path, rafs_mount,
        KATA_GUEST_SHARE_DIR, PASSTHROUGH_FS_DIR,
    },
};
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::Hypervisor;
use kata_sys_util::mount;
use kata_types::mount::{Mount, NydusExtraOption};
use std::fs;

// Used for nydus rootfs
pub(crate) const NYDUS_ROOTFS_TYPE: &str = "fuse.nydus-overlayfs";
const NYDUS_ROOTFS_V5: &str = "v5";
// Used for Nydus v6 rootfs version
const _NYDUS_ROOTFS_V6: &str = "v6";

const SNAPSHOTDIR: &str = "snapshotdir";
pub(crate) struct NydusRootfs {
    guest_path: String,
    rootfs: Storage,
}

impl NydusRootfs {
    pub async fn new(h: &dyn Hypervisor, sid: &str, cid: &str, rootfs: &Mount) -> Result<Self> {
        let extra_options: NydusExtraOption = get_nydus_extra_options(rootfs)?;
        info!(sl!(), "extra_option {:?}", &extra_options);
        let rafs_meta = &extra_options.source;
        let rafs_mnt =
            do_get_guest_virtiofs_path(HYBRID_ROOTFS_WRITABLE_LAYER_LOWER_DIR, cid, true);
        let rootfs_guest_path = KATA_GUEST_SHARE_DIR.to_string()
            + PASSTHROUGH_FS_DIR
            + &"/".to_string()
            + cid
            + &"/rootfs".to_string();
        let rootfs_storage = match extra_options.fs_version.as_str() {
            NYDUS_ROOTFS_V5 => {
                rafs_mount(
                    h,
                    rafs_meta.to_string(),
                    rafs_mnt,
                    extra_options.config.clone(),
                    None,
                )
                .await?;
                let container_share_dir = get_host_rw_shared_path(sid)
                    .join(PASSTHROUGH_FS_DIR)
                    .join(cid);
                let rootfs_dir = container_share_dir.join("rootfs");
                fs::create_dir_all(rootfs_dir)?;
                let snapshotdir = container_share_dir.join(SNAPSHOTDIR);
                mount::bind_mount_unchecked(
                    extra_options.snapshotdir.clone(),
                    snapshotdir.clone(),
                    true,
                )
                .with_context(|| {
                    format!(
                        "failed to bind mount {} to {:?}",
                        extra_options.snapshotdir, snapshotdir
                    )
                })?;
                let mut options: Vec<String> = Vec::new();
                options.push(
                    "lowerdir=".to_string()
                        + &do_get_guest_path(
                            HYBRID_ROOTFS_WRITABLE_LAYER_LOWER_DIR,
                            cid,
                            false,
                            true,
                        ),
                );
                options.push(
                    "workdir=".to_string()
                        + KATA_GUEST_SHARE_DIR
                        + PASSTHROUGH_FS_DIR
                        + &"/".to_string()
                        + cid
                        + &"/".to_string()
                        + SNAPSHOTDIR
                        + &"/work".to_string(),
                );
                options.push(
                    "upperdir=".to_string()
                        + KATA_GUEST_SHARE_DIR
                        + PASSTHROUGH_FS_DIR
                        + &"/".to_string()
                        + cid
                        + &"/".to_string()
                        + SNAPSHOTDIR
                        + &"/fs".to_string(),
                );
                options.push("index=off".to_string());
                Ok(Storage {
                    driver: TYPE_OVERLAY_FS.to_string(),
                    source: TYPE_OVERLAY_FS.to_string(),
                    fs_type: TYPE_OVERLAY_FS.to_string(),
                    options,
                    mount_point: rootfs_guest_path.clone(),
                    ..Default::default()
                })
            }
            _ => {
                let errstr: &str = "new_nydus_rootfs: invalid nydus rootfs type";
                error!(sl!(), "{}", errstr);
                Err(anyhow!(errstr))
            }
        }?;
        Ok(NydusRootfs {
            guest_path: rootfs_guest_path,
            rootfs: rootfs_storage,
        })
    }
}

#[async_trait]
impl Rootfs for NydusRootfs {
    async fn get_guest_rootfs_path(&self) -> Result<String> {
        Ok(self.guest_path.clone())
    }

    async fn get_rootfs_mount(&self) -> Result<Vec<oci::Mount>> {
        todo!()
    }

    async fn get_storage(&self) -> Option<Storage> {
        Some(self.rootfs.clone())
    }
}

fn get_nydus_extra_options(mount: &Mount) -> Result<NydusExtraOption> {
    let cfg: Vec<&str> = mount
        .options
        .iter()
        .filter(|x| x.starts_with("extraoption="))
        .map(|x| x.as_ref())
        .collect();

    if cfg.len() != 1 {
        let errstr: String = format!(
            "get_nydus_extra_options: Invalid nydus options: {:?}",
            &mount.options
        );
        error!(sl!(), "{}", errstr);
        return Err(anyhow!(errstr));
    }
    let config_raw_data = cfg[0].trim_start_matches("extraoption=");
    let extra_options_buf =
        base64::decode(config_raw_data).context("decode the nydus's base64 extraoption")?;

    serde_json::from_slice(&extra_options_buf).context("deserialize nydus's extraoption")
}
