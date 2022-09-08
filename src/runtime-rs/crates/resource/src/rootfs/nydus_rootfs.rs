// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use super::{Rootfs, TYPE_OVERLAY_FS};
use crate::{
    rootfs::{HYBRID_ROOTFS_WRITABLE_LAYER_LOWER_DIR, RINK_BLOB_CACHE_DIR, RINK_BOOTSTRAP_DIR},
    share_fs::{
        blobfs_mount, do_get_guest_path, do_get_guest_virtiofs_path, get_host_rw_shared_path,
        passthrough_mount, rafs_mount, KATA_GUEST_SHARE_DIR, PASSTHROUGH_FS_DIR,
    },
};
use agent::Storage;
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::Hypervisor;
use kata_sys_util::mount;
use kata_types::mount::{Mount, NydusExtraOption};
use std::{fs, path::Path};

// Used for nydus rootfs
pub(crate) const NYDUS_ROOTFS_TYPE: &str = "fuse.nydus-overlayfs";
const NYDUS_ROOTFS_V5: &str = "v5";
// Used for Nydus v6 rootfs version
const NYDUS_ROOTFS_V6: &str = "v6";

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
        /*
        let rootfs_guest_path = "/".to_string()
            + PASSTHROUGH_FS_DIR
            + &"/".to_string()
            + cid
            + &"/rootfs".to_string();
        */
        let container_share_dir = get_host_rw_shared_path(sid)
            .join(PASSTHROUGH_FS_DIR)
            .join(cid);
        let rootfs_dir = container_share_dir.join("rootfs");
        fs::create_dir_all(rootfs_dir)?;
        //let rootfs_guest_path = do_get_guest_path(PASSTHROUGH_FS_DIR,cid,false,true);
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
            NYDUS_ROOTFS_V6 => {
                blobfs_mount(
                    h,
                    extra_options.source.clone(),
                    do_get_guest_virtiofs_path(RINK_BLOB_CACHE_DIR, cid, true),
                    extra_options.config,
                    None,
                    Some(0),
                )
                .await
                .with_context(|| "failed to mount blob cache dir".to_string())?;

                let bootstrap_dir = {
                    let dir: &Path = extra_options.source.as_str().as_ref();
                    dir.parent().unwrap().to_str().unwrap().to_string()
                };
                passthrough_mount(
                    h,
                    bootstrap_dir,
                    do_get_guest_virtiofs_path(RINK_BOOTSTRAP_DIR, cid, true),
                    Some(0),
                )
                .await
                .with_context(|| "failed to mount bootstrap dir".to_string())?;

                let bootstrap_path = {
                    let bootstrap: &Path = extra_options.source.as_str().as_ref();
                    do_get_guest_path(RINK_BOOTSTRAP_DIR, cid, false, true)
                        + "/"
                        + bootstrap.file_name().unwrap().to_str().unwrap()
                };

                let blob_dir_path = do_get_guest_path(RINK_BLOB_CACHE_DIR, cid, false, true);

                let mut options: Vec<String> = Vec::new();
                options.push("bootstrap_path=".to_string() + bootstrap_path.as_str());
                options.push("blob_dir_path=".to_string() + blob_dir_path.as_str());
                options.push("user_xattr".to_string());

                Ok(Storage {
                    driver: TYPE_OVERLAY_FS.to_string(),
                    source: TYPE_OVERLAY_FS.to_string(),
                    fs_type: "erofs".to_string(),
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
