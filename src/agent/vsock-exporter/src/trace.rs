// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

// The VSOCK Exporter sends trace spans "out" to the forwarder running on the
// host (which then forwards them on to a trace collector). The data is sent
// via a VSOCK socket that the forwarder process is listening on. To allow the
// forwarder to know how much data to each for each trace span the simplest
// protocol is employed which uses a header packet and the payload (trace
// span) data. The header packet is a simple count of the number of bytes in the
// payload, which allows the forwarder to know how many bytes it must read to
// consume the trace span. The payload is a serialised version of the trace span.

use std::io;

use async_trait::async_trait;
use byteorder::{ByteOrder, NetworkEndian};
use opentelemetry::sdk::export::trace::{ExportResult, SpanData, SpanExporter};
use slog::{error, o, Logger};
use tokio::sync::mpsc;

use crate::{Exporter, ExporterError};

// By default, the VSOCK exporter should talk "out" to the host where the
// forwarder is running.
const DEFAULT_CID: u32 = libc::VMADDR_CID_ANY;

// The VSOCK port the forwarders listens on by default
const DEFAULT_PORT: u32 = 10240;

// Must match the value of the variable of the same name in the trace forwarder.
const HEADER_SIZE_BYTES: u64 = std::mem::size_of::<u64>() as u64;

impl Exporter {
    /// Create a new exporter builder.
    pub fn builder() -> Builder {
        Builder::default()
    }
}

// Send a trace span to the forwarder running on the host.
async fn write_span(data_tx: mpsc::Sender<Vec<u8>>, span: &SpanData) -> Result<(), ExporterError> {
    let data = {
        let body: Vec<u8> =
            bincode::serialize(&span).map_err(|e| ExporterError::SerialisationError(e))?;
        let body_len: u64 = body.len() as u64;
        let mut header: Vec<u8> = vec![0; HEADER_SIZE_BYTES as usize];
        // Encode the header
        NetworkEndian::write_u64(&mut header, body_len);
        header.extend_from_slice(&body);
        header
    };

    data_tx
        .send(data)
        .await
        .map_err(|e| ExporterError::SendDataError(e))
}

async fn handle_batch(
    data_tx: mpsc::Sender<Vec<u8>>,
    batch: &[SpanData],
) -> Result<(), ExporterError> {
    for span_data in batch {
        write_span(data_tx.clone(), span_data).await?;
    }

    Ok(())
}

#[async_trait]
impl SpanExporter for Exporter {
    async fn export(&mut self, batch: Vec<SpanData>) -> ExportResult {
        if let Err(e) = handle_batch(self.data_tx.clone(), &batch).await {
            error!(self.logger, "handle_batch error: {:?}", e);
            return Err(e.into());
        };

        Ok(())
    }

    // No need to do extra cleanup operation.
    // When Exporter is dropped, all Senders are dropped. At this time, the mpsc channel will be closed.
    // In this case, the server task(thread) that accepts ttrpc requests will also shutdown.
    fn shutdown(&mut self) {}
}

#[derive(Debug)]
pub struct Builder {
    port: u32,
    cid: u32,
    logger: Logger,
}

impl Default for Builder {
    fn default() -> Self {
        let logger = Logger::root(slog::Discard, o!());

        Builder {
            cid: DEFAULT_CID,
            port: DEFAULT_PORT,
            logger,
        }
    }
}

impl Builder {
    pub fn with_cid(self, cid: u32) -> Self {
        Builder { cid, ..self }
    }

    pub fn with_port(self, port: u32) -> Self {
        Builder { port, ..self }
    }

    pub fn with_logger(self, logger: &Logger) -> Self {
        Builder {
            logger: logger.new(o!()),
            ..self
        }
    }

    pub fn init(self) -> io::Result<Exporter> {
        let Builder { port, cid, logger } = self;

        Exporter::new(logger, port, cid)
    }
}
