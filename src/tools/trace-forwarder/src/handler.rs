// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{ErrorKind, Read};

use anyhow::{anyhow, Context, Result};
use byteorder::ByteOrder;
use byteorder::NetworkEndian;
use opentelemetry::sdk::export::trace::SpanData;
use opentelemetry::sdk::export::trace::SpanExporter;
use opentelemetry_jaeger::Exporter;
use opentelemetry::sdk::trace::Tracer;
use slog::{debug, info, o, Logger};

// The VSOCK "packet" protocol used comprises two elements:
//
// 1) The header (the number of bytes in the payload).
// 2) The payload bytes.
//
// This constant defines the number of bytes used to encode the header on the
// wire. In other words, the first 64-bits of the packet contain a number
// specifying how many bytes are in the remainder of the packet.
//
// Must match the value of the variable of the same name in the agents
// vsock-exporter.
const HEADER_SIZE_BYTES: u64 = std::mem::size_of::<u64>() as u64;

#[derive(Debug)]
pub enum SpanHandler {
    Exporter(Exporter),
    Tracer(Tracer),
}

pub async fn handle_connection<'a>(
    logger: &Logger,
    mut conn: &'a mut (dyn Read + Send),
    span_handler: &mut SpanHandler,
    dump_only: bool,
) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "handler"));

    debug!(logger, "handling connection");

    handle_trace_data(logger.clone(), &mut conn, span_handler, dump_only)
        .await
        .context("handle trace data")?;

    debug!(&logger, "handled connection");

    Ok(())
}

async fn handle_trace_data<'a>(
    logger: Logger,
    reader: &'a mut (dyn Read + Send),
    span_handler: &mut SpanHandler,
    dump_only: bool,
) -> Result<()> {
    loop {
        let mut header: [u8; HEADER_SIZE_BYTES as usize] = [0; HEADER_SIZE_BYTES as usize];

        info!(logger, "waiting for traces");

        match reader.read_exact(&mut header) {
            Ok(_) => debug!(logger, "read header"),
            Err(e) => {
                if e.kind() == ErrorKind::UnexpectedEof {
                    info!(logger, "agent shut down");
                    break;
                }

                return Err(anyhow!("failed to read header: {:}", e));
            }
        };

        let payload_len: u64 = NetworkEndian::read_u64(&header);

        let mut encoded_payload = vec![0; payload_len as usize];

        reader
            .read_exact(&mut encoded_payload)
            .with_context(|| "failed to read payload")?;

        debug!(logger, "read payload");

        let span_data: SpanData =
            bincode::deserialize(&encoded_payload[..]).expect("failed to deserialise payload");

        debug!(logger, "deserialised payload");

        if dump_only {
            debug!(logger, "dump-only: {:?}", span_data);
        } else {
            export_trace_data(span_handler, span_data)
                .await
                .context("export trace data")?;
            debug!(logger, "exported trace spans");
        }
    }

    Ok(())
}

async fn export_trace_data(span_handler: &mut SpanHandler, span_data: SpanData) -> Result<()> {
    match span_handler {
        SpanHandler::Exporter(exporter) => {
            // Call low-level Jaeger exporter to send the trace span immediately.
            let batch = vec![span_data];
            exporter.export(batch).await.context("export trace span")
        }
        SpanHandler::Tracer(tracer) => {
            // Use tracer's processor to expand trace data.
            if let Some(provider) = tracer.provider() {
                let processors = provider.span_processors().iter().peekable();

                for processor in processors {
                    processor.on_end(span_data.clone());
                }
            }
            Ok(())
        }
    }
}
