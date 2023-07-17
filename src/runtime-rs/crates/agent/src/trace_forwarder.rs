// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use kata_trace_forwarder::client::{VsockTraceClient, VsockType};
use kata_trace_forwarder::handler::SpanHandler;
use lazy_static::lazy_static;
use opentelemetry::sdk::trace::Tracer;
use tokio::sync::Mutex;

use crate::sock::{self, SockType};

lazy_static! {
    /// Opentelemetry global did not provide a way to get the Tracer.
    /// Save this global variable here for TracerForwarder processing.
    ///
    /// NOTE: only the KataTracer is built when this AgentTracer will be initialized.
    static ref AGENT_TRACER: Mutex<Option<Tracer>> = Mutex::new(None);
}

pub async fn init_agent_tracer(tracer: Tracer) -> Result<()> {
    let mut tracer_guard = AGENT_TRACER.lock().await;
    *tracer_guard = Some(tracer);

    Ok(())
}

pub(crate) struct TraceForwarder {
    task_handler: Option<tokio::task::JoinHandle<()>>,
}

impl TraceForwarder {
    pub(crate) fn new() -> Self {
        Self { task_handler: None }
    }

    // start connect kata-agent trace vsock, and export the trace data
    pub(crate) async fn start(&mut self, address: &str, port: u32, config: sock::ConnectConfig) -> Result<()> {
        let tracer_guard = AGENT_TRACER.lock().await;
        if tracer_guard.is_some() {
            info!(sl!(), "start trace forwarder");
            let tracer = tracer_guard.as_ref().unwrap();
            let logger = sl!().clone();
            let vsock = match sock::parse(address, port)? {
                SockType::Vsock(vsock) => VsockType::Standard {
                    context_id: vsock.vsock_cid,
                    port: vsock.port,
                },
                SockType::HybridVsock(hvsock) => VsockType::Hybrid {
                    socket_path: hvsock.uds,
                    port: hvsock.port,
                },
            };
            let span_handler = SpanHandler::Tracer(tracer.clone());
            let dial_retry_times = (config.reconnect_timeout_ms / config.dial_timeout_ms) as u32;
            let dial_timeout_secs = config.dial_timeout_ms / 1000;

            let task_handler = tokio::spawn(async move {
                let mut vsock_trace_client =
                    VsockTraceClient::new(vsock, &logger, false, span_handler);
                if let Err(err) = vsock_trace_client.start(dial_timeout_secs, dial_retry_times).await {
                    error!(sl!(), "vsock trace client start failed {:?}", err);
                    return;
                }
            });

            self.task_handler = Some(task_handler)
        }

        Ok(())
    }

    pub(crate) fn stop(&mut self) {
        let task_handler = self.task_handler.take();
        if let Some(handler) = task_handler {
            handler.abort();
            info!(sl!(), "abort trace forwarder thread")
        }
    }
}
