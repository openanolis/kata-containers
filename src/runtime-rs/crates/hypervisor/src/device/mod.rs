// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod device_manager;
pub mod device_type;

use crate::device_type::GenericConfig;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum DeviceType {
    Block,
    Network,
    ShareFsDevice,
    Vfio,
    ShareFsMount,
    Vsock,
    HybridVsock,
    Vhost,
    Undefined,
}

pub fn get_device_type(dev_info: &GenericConfig) -> &DeviceType {
    // direct_volume/vfio_volume/spdk_volume:  dev_type "b"; major -1 minor 0
    if dev_info.dev_type == "b" && dev_info.minor >= 0 {
        return &DeviceType::Block;
    }

    &DeviceType::Undefined
}
