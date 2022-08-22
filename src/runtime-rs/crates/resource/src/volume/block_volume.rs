// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, path::Path, sync::Arc};

use super::{share_fs_volume::generate_mount_path, Volume};
use crate::device::{device::DeviceInfo, manager::DeviceManager};
use crate::share_fs::do_get_guest_path;
use agent::Storage;
use anyhow::{Context, Result};
use hypervisor::Hypervisor;
use nix::sys::stat::{self, SFlag};
use tokio::sync::Mutex;

pub(crate) struct BlockVolume {
    mount: oci::Mount,
    storage: Option<agent::Storage>,
}

/// BlockVolume: block device volume
impl BlockVolume {
    pub(crate) async fn new(
        d: Arc<Mutex<DeviceManager>>,
        h: &dyn Hypervisor,
        m: &oci::Mount,
        read_only: bool,
        cid: &str,
    ) -> Result<Self> {
        let file_name = Path::new(&m.destination)
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        let file_name = generate_mount_path(cid, file_name);
        let fstat = stat::stat(m.source.as_str()).context(format!("stat {}", m.source))?;
        let mut options = HashMap::new();
        if read_only {
            options.insert("read_only".to_string(), "true".to_string());
        }
        let _device_id = d
            .lock()
            .await
            .new_device(
                &mut DeviceInfo {
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
                },
                h,
            )
            .await?;

        let guest_path = do_get_guest_path(&file_name, cid, true);

        // storage
        let mut storage = Storage::default();
        storage.options = if read_only {
            vec!["ro".to_string()]
        } else {
            Vec::new()
        };
        storage.mount_point = guest_path.clone();
        let mut options = m.options.clone();
        storage.options.append(&mut options);
        storage.fs_type = m.r#type.clone();

        Ok(Self {
            mount: oci::Mount {
                destination: m.destination.clone(),
                r#type: "bind".to_string(),
                source: guest_path,
                options: m.options.clone(),
            },
            storage: Some(storage),
        })
    }
}

impl Volume for BlockVolume {
    fn get_volume_mount(&self) -> anyhow::Result<Vec<oci::Mount>> {
        Ok(vec![self.mount.clone()])
    }

    fn get_storage(&self) -> Result<Vec<Storage>> {
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
}

pub(crate) fn is_block_volume(m: &oci::Mount) -> bool {
    if m.r#type != "bind" {
        return false;
    }
    if let Ok(fstat) = stat::stat(m.source.as_str()).context(format!("stat {}", m.source)) {
        return SFlag::from_bits_truncate(fstat.st_mode) == SFlag::S_IFBLK;
    }
    return false;
}
