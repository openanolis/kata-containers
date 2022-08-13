// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::fmt::Debug;

use super::{NetworkModel, NetworkModelType};
use crate::network::{
    network_model::fetch_index, network_pair::get_link_by_name, utils::parse_mac, NetworkPair,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use futures::TryStreamExt;
use scopeguard::defer;

#[derive(Debug)]
pub(crate) struct MacvtapModel {}

impl MacvtapModel {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl NetworkModel for MacvtapModel {
    fn model_type(&self) -> NetworkModelType {
        NetworkModelType::Macvtap
    }

    async fn add(&self, pair: &NetworkPair) -> Result<()> {
        let (connection, handle, _) = rtnetlink::new_connection().context("new connection")?;
        let thread_handler = tokio::spawn(connection);

        defer!({
            thread_handler.abort();
        });

        let tap_index = fetch_index(&handle, pair.tap.tap_iface.name.as_str())
            .await
            .context("fetch tap by index")?;
        let virt_index = fetch_index(&handle, pair.virt_iface.name.as_str())
            .await
            .context("fetch virt by index")?;

        let virt_link = get_link_by_name(&handle, pair.virt_iface.name.as_str())
            .await
            .context("get link by name")?;

        let tap_link = get_link_by_name(&handle, pair.tap.tap_iface.name.as_str())
            .await
            .context("get link by name")?;

        if let Some(_address) = parse_mac(&pair.tap.tap_iface.hard_addr) {
            handle
                .link()
                .set(tap_index)
                .address(tap_link.attrs().hardware_addr.clone())
                .execute()
                .await
                .context("set hardware address for tap link")?;

            handle
                .link()
                .set(tap_index)
                .up()
                .execute()
                .await
                .context("set tap up")?;
        }

        if let Some(_address) = parse_mac(&pair.virt_iface.hard_addr) {
            handle
                .link()
                .set(virt_index)
                .address(virt_link.attrs().hardware_addr.clone())
                .execute()
                .await
                .context("set hardware address for veth")?;
        }
        while let Some(addr_msg) = handle
            .address()
            .get()
            .set_link_index_filter(virt_index)
            .execute()
            .try_next()
            .await?
        {
            handle
                .address()
                .del(addr_msg)
                .execute()
                .await
                .context("clear veth IP addresses")?;
        }

        handle
            .link()
            .set(virt_index)
            .up()
            .execute()
            .await
            .context("set link up")?;

        Ok(())
    }

    async fn del(&self, pair: &NetworkPair) -> Result<()> {
        let (connection, handle, _) = rtnetlink::new_connection().context("new connection")?;
        let thread_handler = tokio::spawn(connection);

        defer!({
            thread_handler.abort();
        });

        let tap_index = fetch_index(&handle, pair.tap.tap_iface.name.as_str())
            .await
            .context("fetch tap by index")?;
        let virt_index = fetch_index(&handle, pair.virt_iface.name.as_str())
            .await
            .context("fetch virt by index")?;

        handle
            .link()
            .del(tap_index)
            .execute()
            .await
            .context("remove tap link")?;

        if let Some(address) = parse_mac(&pair.tap.tap_iface.hard_addr) {
            handle
                .link()
                .set(virt_index)
                .address(address.0.to_vec())
                .execute()
                .await
                .context("set hardware address for veth")?;
        }

        handle
            .link()
            .set(virt_index)
            .down()
            .execute()
            .await
            .context("set link down")?;

        Ok(())
    }
}
