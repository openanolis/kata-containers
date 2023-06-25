// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod trace;

use std::io::{self, ErrorKind};

use futures::StreamExt as _;
use opentelemetry::sdk::export::ExportError;
use slog::{error, o, Logger};
use thiserror::Error;
use tokio::sync::mpsc::error::SendError;
use tokio::{io::AsyncWriteExt, sync::mpsc};
use tokio_vsock::{VsockListener, VsockStream};

const ANY_CID: &str = "any";

#[derive(Debug)]
pub struct Exporter {
    data_tx: mpsc::Sender<Vec<u8>>,
    logger: Logger,
}

#[derive(Error, Debug)]
pub enum ExporterError {
    #[error("serialisation error: {0}")]
    SerialisationError(#[from] bincode::Error),
    #[error("send data error: {0}")]
    SendDataError(#[from] SendError<Vec<u8>>),
}

impl ExportError for ExporterError {
    fn exporter_name(&self) -> &'static str {
        "vsock-exporter"
    }
}

impl Exporter {
    pub fn new(logger: Logger, port: u32, cid: u32) -> io::Result<Self> {
        let cid_str = if cid == libc::VMADDR_CID_ANY {
            ANY_CID.to_string()
        } else {
            format!("{}", cid)
        };

        let logger = logger.new(o!("vsock-cid" => cid_str, "port" => port));
        let logger2 = logger.clone();

        let (data_tx, mut data_rx) = mpsc::channel::<Vec<u8>>(4096);

        let data_tx2 = data_tx.clone();
        let mut conn: Option<VsockStream> = None;

        let listener = VsockListener::bind(cid, port)?;
        let mut incoming = listener.incoming();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // The conn.is_some precondition is evaluated byfore data_rx.recv(), which means
                    // we will not try to read data from buffer when conn.is_none()
                    data = data_rx.recv(), if conn.is_some() => {
                        if let Some(data) = data {
                            if let Err(e) = conn.as_mut().unwrap().write_all(&data).await {
                                conn.take();
                                error!(logger2, "exporter send data error: {:?}", e);

                                if e.kind() == ErrorKind::NotConnected {
                                    let _ = data_tx2.try_send(data);
                                }
                            }
                        } else {
                            // Channel has been closed, and no further values can be received
                            drop(data_tx2);
                            break;
                        }
                    }

                    // Connect / Re-connect by forwarder
                    new_conn = incoming.next() => {
                        if let Some(new_conn) = new_conn {
                            if let Ok(new_conn) = new_conn {
                                conn = Some(new_conn);
                            }
                        } else {
                            // No new connections would come
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self { data_tx, logger })
    }
}
