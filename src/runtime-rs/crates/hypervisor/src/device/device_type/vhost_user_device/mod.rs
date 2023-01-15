// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod vhost_user_blk;
pub mod vhost_user_fs;
pub mod vhost_user_net;
pub mod vhost_user_scsi;

// VhostUserDeviceAttrs represents data shared by most vhost-user devices
pub struct VhostUserConfig {
    pub dev_id: String,
    pub socket_path: String,
    //mac_address is only meaningful for vhost user net device
    pub mac_address: String,
    // These are only meaningful for vhost user fs devices
    pub tag: String,
    pub cache: String,
    pub device_type: String,
    // pci_addr is the PCI address used to identify the slot at which the drive is attached.
    pub pci_addr: Option<String>,
    // Block index of the device if assigned
    pub index: u8,
    pub cache_size: u32,
    pub queue_siez: u32,
}
