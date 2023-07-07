// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::net::SocketAddr;

use anyhow::{Context, Result};
use opentelemetry::KeyValue;

pub fn create_jaeger_trace_exporter(
    jaeger_service_name: String,
    jaeger_host: String,
    jaeger_port: u32,
) -> Result<opentelemetry_jaeger::Exporter> {
    let exporter_type = "jaeger";

    let jaeger_addr = format!("{}:{}", jaeger_host, jaeger_port);

    let socket_addr: SocketAddr = jaeger_addr.parse().context("parse jaeger addr")?;

    let exporter = opentelemetry_jaeger::new_pipeline()
        .with_service_name(jaeger_service_name)
        .with_agent_endpoint(socket_addr.to_string())
        .with_tags(vec![KeyValue::new("exporter", exporter_type)])
        .init_exporter()
        .context("create exporter")?;

    Ok(exporter)
}
