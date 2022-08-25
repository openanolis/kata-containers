// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{self, Error};

use super::endpoint_persist::{EndpointState, VethEndpointState};
use super::Endpoint;
use crate::network::{utils, NetworkPair};
use anyhow::{Context, Result};
use async_trait::async_trait;
use hypervisor::{device::NetworkConfig, Device, Hypervisor};

#[derive(Debug)]
pub struct VethEndpoint {
    net_pair: NetworkPair,
    pub rx_rate_limited: Option<u64>,
    pub tx_rate_limited: Option<u64>,
}

impl VethEndpoint {
    pub async fn new(
        handle: &rtnetlink::Handle,
        name: &str,
        idx: u32,
        model: &str,
        queues: usize,
    ) -> Result<Self> {
        let net_pair = NetworkPair::new(handle, idx, name, model, queues)
            .await
            .context("new networkInterfacePair")?;
        Ok(VethEndpoint {
            net_pair,
            rx_rate_limited: None,
            tx_rate_limited: None,
        })
    }

    fn get_network_config(&self) -> Result<NetworkConfig> {
        let iface = &self.net_pair.tap.tap_iface;
        let guest_mac = utils::parse_mac(&iface.hard_addr).ok_or_else(|| {
            Error::new(
                io::ErrorKind::InvalidData,
                format!("hard_addr {}", &iface.hard_addr),
            )
        })?;
        Ok(NetworkConfig {
            id: self.net_pair.virt_iface.name.clone(),
            host_dev_name: iface.name.clone(),
            guest_mac: Some(guest_mac),
            tx_limited_rate: self.get_tx_rate_limited(),
            rx_limited_rate: self.get_rx_rate_limited(),
        })
    }
}

#[async_trait]
impl Endpoint for VethEndpoint {
    async fn name(&self) -> String {
        self.net_pair.virt_iface.name.clone()
    }

    async fn hardware_addr(&self) -> String {
        self.net_pair.tap.tap_iface.hard_addr.clone()
    }

    async fn attach(&self, h: &dyn Hypervisor) -> Result<()> {
        self.net_pair
            .add_network_model()
            .await
            .context("add network model")?;
        let config = self.get_network_config().context("get network config")?;
        h.add_device(Device::Network(config))
            .await
            .context("Error add device")?;
        Ok(())
    }

    async fn detach(&self, h: &dyn Hypervisor) -> Result<()> {
        self.net_pair
            .del_network_model()
            .await
            .context("del network model")?;
        let config = self.get_network_config().context("get network config")?;
        h.remove_device(Device::Network(config))
            .await
            .context("remove device")?;
        Ok(())
    }

    fn get_rx_rate_limited(&self) -> Option<u64> {
        self.rx_rate_limited
    }

    fn set_rx_rate_limited(&mut self, rate: u64) {
        self.rx_rate_limited = Some(rate);
    }

    fn get_tx_rate_limited(&self) -> Option<u64> {
        self.tx_rate_limited
    }

    fn set_tx_rate_limited(&mut self, rate: u64) {
        self.tx_rate_limited = Some(rate);
    }

    async fn save(&self) -> Option<EndpointState> {
        Some(EndpointState {
            veth_endpoint: Some(VethEndpointState {
                if_name: self.net_pair.virt_iface.name.clone(),
                network_qos: self.net_pair.network_qos,
            }),
            ..Default::default()
        })
    }
}
