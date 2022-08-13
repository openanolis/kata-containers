// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod macvtap_model;
pub mod none_model;
pub mod route_model;
pub mod tc_filter_model;
pub mod test_network_model;
use std::sync::Arc;

use anyhow::{Context, Result};
use async_trait::async_trait;
use rtnetlink::Handle;

use super::NetworkPair;

pub(crate) const TC_FILTER_NET_MODEL_STR: &str = "tcfilter";
pub(crate) const ROUTE_NET_MODEL_STR: &str = "route";
pub(crate) const MACVTAP_NET_MODEL_STR: &str = "macvtap";

#[derive(PartialEq)]
pub enum NetworkModelType {
    NoneModel,
    TcFilter,
    Route,
    Macvtap,
}

#[async_trait]
pub trait NetworkModel: std::fmt::Debug + Send + Sync {
    fn model_type(&self) -> NetworkModelType;
    async fn add(&self, net_pair: &NetworkPair) -> Result<()>;
    async fn del(&self, net_pair: &NetworkPair) -> Result<()>;
}

pub fn new(model: &str) -> Result<Arc<dyn NetworkModel>> {
    match model {
        TC_FILTER_NET_MODEL_STR => Ok(Arc::new(
            tc_filter_model::TcFilterModel::new().context("new tc filter model")?,
        )),
        ROUTE_NET_MODEL_STR => Ok(Arc::new(
            route_model::RouteModel::new().context("new route model")?,
        )),
        MACVTAP_NET_MODEL_STR => Ok(Arc::new(
            macvtap_model::MacvtapModel::new().context("new none model")?,
        )),
        _ => Ok(Arc::new(
            none_model::NoneModel::new().context("new none model")?,
        )),
    }
}

pub(crate) async fn fetch_index(handle: &Handle, name: &str) -> Result<u32> {
    let link = crate::network::network_pair::get_link_by_name(handle, name)
        .await
        .context("get link by name")?;
    let base = link.attrs();
    Ok(base.index)
}
