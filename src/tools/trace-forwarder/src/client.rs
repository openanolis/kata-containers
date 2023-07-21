// Copyright (c) 2020-2021 Intel Corporation
// Copyright (c) 2023 Alibaba Cloud
// Copyright (c) 2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{BufRead, BufReader, Write, Read};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::net::UnixStream;
use std::thread::{self};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use privdrop::PrivDrop;
use slog::Logger;
use slog::{debug, info, o};
use vsock::VsockStream;

use super::handler;
use crate::handler::SpanHandler;

// Username that is assumed to exist, used when dropping root privileges
// when running with Hybrid VSOCK.
pub const NON_PRIV_USER: &str = "nobody";

const ROOT_DIR: &str = "/";

#[derive(Debug, Clone, PartialEq)]
pub enum VsockType {
    Standard { context_id: u32, port: u32 },
    Hybrid { socket_path: String, port: u32 },
}

#[derive(Debug)]
pub struct VsockTraceClient {
    pub vsock: VsockType,

    pub span_handler: SpanHandler,

    pub logger: Logger,
    pub dump_only: bool,
}

impl VsockTraceClient {
    pub fn new(
        vsock: VsockType,
        logger: &Logger,
        dump_only: bool,
        span_handler: SpanHandler,
    ) -> Self {
        let logger = logger.new(o!("subsystem" => "client"));

        VsockTraceClient {
            vsock,
            span_handler,
            logger,
            dump_only,
        }
    }

    pub async fn start(&mut self, dial_timeout_secs: u64, dial_retry_times: u32) -> Result<()> {
        let mut stream = connect_vsock(
            &self.logger,
            &self.vsock,
            dial_timeout_secs,
            dial_retry_times,
        )
        .await
        .context("connect vsock")?;

        // Debug code
        // Bad UnixStream
        // let mut buf: [u8; 8] = [0; 8];
        // stream.read_exact(&mut buf).context("read stream")?;
        // info!(&self.logger, "read stream buffer: {:?}", buf);

        // handle stream
        handler::handle_connection(
            &self.logger,
            &mut stream,
            &mut self.span_handler,
            self.dump_only,
        )
        .await
        .context("handle connection")?;

        Ok(())
    }
}

pub async fn connect_vsock(
    logger: &Logger,
    vsock: &VsockType,
    timeout: u64,
    retry_times: u32,
) -> Result<UnixStream> {
    match vsock.clone() {
        VsockType::Standard { context_id, port } => {
            let stream = connect_standard_vsock(logger, context_id, port, retry_times).await?;

            // Debug code
            // Bad VsockStream
            // let mut buf: [u8; 8] = [0; 8];
            // stream.read_exact(&mut buf).context("read stream")?;
            // info!(logger, "read stream buffer: {:?}", buf);

            Ok(stream)
        }
        VsockType::Hybrid { socket_path, port } => {
            let logger_priv = logger
                .new(o!("vsock-type" => "hybrid", "vsock-socket-path" => socket_path.clone()));

            let effective = nix::unistd::Uid::effective();
            if !effective.is_root() {
                return Err(anyhow!("You need to be root"));
            }

            let stream =
                connect_hybrid_vsock(logger, socket_path.as_str(), port, timeout, retry_times)
                    .await
                    .context("connect hybrid vsock")?;

            // Having connect to the hvsock, drop privileges
            drop_privs(&logger_priv)?;
            Ok(stream)
        }
    }
}

async fn connect_standard_vsock(
    logger: &Logger,
    cid: u32,
    port: u32,
    retry_times: u32,
) -> Result<UnixStream> {
    for i in 0..retry_times {
        match VsockStream::connect_with_cid_port(cid, port) {
            Ok(stream) => {
                info!(
                    logger,
                    "connect on cid {} port {}, current client fd {}",
                    cid,
                    port,
                    stream.as_raw_fd()
                );

                // Debug Code
                // let mut buf: [u8; 8] = [0; 8];
                // VsockStream ok
                // stream.read_exact(&mut buf).context("read vsock stream")?;
                // info!(logger, "read buffer {:?}", buf);

                let stream = unsafe { UnixStream::from_raw_fd(stream.as_raw_fd()) };

                // UnixStream ok
                // stream.read_exact(&mut buf).context("read vsock stream")?;
                // info!(logger, "read buffer {:?}", buf);

                return Ok(stream);
            }
            Err(e) => {
                debug!(logger, "connect on {} times err: {:?}", i, e);
                thread::sleep(Duration::from_millis(200));
            }
        }
    }

    Err(anyhow!(
        "vsock cid {} port {} connect failed after {} retries!",
        cid,
        port,
        retry_times
    ))
}

async fn connect_hybrid_vsock(
    logger: &Logger,
    socket_path: &str,
    port: u32,
    timeout: u64,
    retry_times: u32,
) -> Result<UnixStream> {
    let connect_once = || -> Result<UnixStream> {
        let mut stream = UnixStream::connect(socket_path).context("connect uds")?;

        stream.set_read_timeout(Some(Duration::new(timeout, 0)))?;
        stream.set_write_timeout(Some(Duration::new(timeout, 0)))?;

        stream.write_all(format!("connect {}\n", port).as_bytes())?;

        let mut reader = BufReader::new(&stream);
        let mut response = String::new();

        reader.read_line(&mut response)?;

        if !response.contains("OK") {
            return Err(anyhow!(
                "handshake error: malformed response code: {:?}",
                response
            ));
        }

        // Unset the timeout to turn the socket to blocking mode
        stream.set_read_timeout(None)?;
        stream.set_write_timeout(None)?;
        Ok(stream)
    };

    for i in 0..retry_times {
        match connect_once() {
            Ok(stream) => {
                info!(
                    logger,
                    "connect vsock on {}, current client fd {}",
                    socket_path,
                    stream.as_raw_fd()
                );
                return Ok(stream);
            }
            Err(e) => {
                debug!(logger, "connect on {} times err: {:?}", i, e);
                thread::sleep(Duration::from_millis(200));
            }
        }
    }

    Err(anyhow!(
        "vsock {} connect failed after {} retries!",
        socket_path,
        retry_times
    ))
}

fn drop_privs(logger: &Logger) -> Result<()> {
    debug!(logger, "Dropping privileges"; "new-user" => NON_PRIV_USER);

    nix::unistd::chdir(ROOT_DIR)
        .map_err(|e| anyhow!("Unable to chdir to {:?}: {:?}", ROOT_DIR, e))?;

    PrivDrop::default()
        .user(NON_PRIV_USER)
        .apply()
        .map_err(|e| anyhow!("Failed to drop privileges to user {}: {}", NON_PRIV_USER, e))?;

    Ok(())
}
