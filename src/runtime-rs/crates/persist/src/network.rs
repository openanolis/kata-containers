// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct PhysicalEndpointState {
    pub bdf: String,
    pub driver: String,
    pub vendor_id: String,
    pub device_id: String,
    pub hard_addr: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct VethEndpointState {
    pub if_name: String,
    pub network_qos: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct EndpointState {
    pub physical_endpoint: Option<PhysicalEndpointState>,
    pub veth_endpoint: Option<VethEndpointState>,
    // TODO : other endpoint
}
