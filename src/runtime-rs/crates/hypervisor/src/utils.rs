// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use std::collections::HashSet;

pub fn get_child_threads(pid: u32) -> HashSet<u32> {
    let mut result = HashSet::new();
    let path_name = format!("/proc/{}/task", pid);
    let path = std::path::Path::new(path_name.as_str());
    if path.is_dir() {
        if let Ok(dir) = path.read_dir() {
            for entity in dir {
                if let Ok(entity) = entity.as_ref() {
                    let file_name = entity.file_name();
                    let file_name = file_name.to_str().unwrap_or_default();
                    if let Ok(tid) = file_name.parse::<u32>() {
                        result.insert(tid);
                    }
                }
            }
        }
    }
    result
}

// GetVirtDriveName returns the disk name format for virtio-blk
// Reference: https://github.com/torvalds/linux/blob/master/drivers/block/virtio_blk.c @c0aa3e0916d7e531e69b02e426f7162dfb1c6c0
pub fn get_virt_drive_name(mut index: i32) -> Result<String> {
    if index < 0 {
        return Err(anyhow!("Index cannot be negative"));
    }

    // Prefix used for virtio-block devices
    const PREFIX: &str = "vd";

    //Refer to DISK_NAME_LEN: https://github.com/torvalds/linux/blob/08c521a2011ff492490aa9ed6cc574be4235ce2b/include/linux/genhd.h#L61
    let disk_name_len = 32usize;
    let base = 26i32;

    let suff_len = disk_name_len - PREFIX.len();
    let mut disk_letters = vec![0u8; suff_len];

    let mut i = 0usize;
    while i < suff_len && index >= 0 {
        let letter: u8 = b'a' + (index % base) as u8;
        disk_letters[i] = letter;
        index = (index / base) - 1;
        i += 1;
    }
    if index >= 0 {
        return Err(anyhow!("Index not supported"));
    }
    disk_letters.truncate(i);
    disk_letters.reverse();
    Ok(String::from(PREFIX) + std::str::from_utf8(&disk_letters)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_virt_drive_name() {
        for &(input, output) in [
            (0i32, "vda"),
            (25, "vdz"),
            (27, "vdab"),
            (704, "vdaac"),
            (18277, "vdzzz"),
        ]
        .iter()
        {
            let out = get_virt_drive_name(input).unwrap();
            assert_eq!(&out, output);
        }
    }
}
