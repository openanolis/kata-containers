// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use async_trait::async_trait;
use scopeguard::defer;

use super::{NetworkModel, NetworkModelType};
use crate::network::{network_model::fetch_index, NetworkPair};

#[derive(Debug)]
pub(crate) struct TcFilterModel {}

impl TcFilterModel {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }
}

#[async_trait]
impl NetworkModel for TcFilterModel {
    fn model_type(&self) -> NetworkModelType {
        NetworkModelType::TcFilter
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

        handle
            .qdisc()
            .add(tap_index as i32)
            .ingress()
            .execute()
            .await
            .context("add tap ingress")?;

        handle
            .qdisc()
            .add(virt_index as i32)
            .ingress()
            .execute()
            .await
            .context("add virt ingress")?;

        handle
            .traffic_filter(tap_index as i32)
            .add()
            .parent(0xffff0000)
            // get protocol with network byte order
            .protocol(0x0003_u16.to_be())
            .redirect(virt_index)
            .execute()
            .await
            .context("add redirect for tap")?;

        handle
            .traffic_filter(virt_index as i32)
            .add()
            .parent(0xffff0000)
            // get protocol with network byte order
            .protocol(0x0003_u16.to_be())
            .redirect(tap_index)
            .execute()
            .await
            .context("add redirect for virt")?;

        Ok(())
    }

    async fn del(&self, pair: &NetworkPair) -> Result<()> {
        let (connection, handle, _) = rtnetlink::new_connection().context("new connection")?;
        let thread_handler = tokio::spawn(connection);
        defer!({
            thread_handler.abort();
        });
        let virt_index = fetch_index(&handle, &pair.virt_iface.name).await?;
        handle.qdisc().del(virt_index as i32).execute().await?;
        Ok(())
    }
}
