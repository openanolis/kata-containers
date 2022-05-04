// Copyright (C) 2022 Alibaba Cloud Computing. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
// Copyright (C) 2020 Alibaba Cloud. All rights reserved.

/// Virtual machine console device manager.
///
/// A virtual console are composed up of two parts: frontend in virtual machine and backend in
/// host OS. A frontend may be serial port, virtio-console etc, a backend may be stdio or Unix
/// domain socket. The manager connects the frontend with the backend.
///
/// Note: unwrap() is used without documentation, it's OK here.
use std::io::{self, Read};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::{Arc, Mutex};

use bytes::{BufMut, BytesMut};
use dbs_legacy_devices::{ConsoleHandler, SerialDevice};
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use vmm_sys_util::terminal::Terminal;

use super::{DeviceMgrError, Result};

const EPOLL_EVENT_SERIAL: u32 = 0;
const EPOLL_EVENT_SERIAL_DATA: u32 = 1;
const EPOLL_EVENT_STDIN: u32 = 2;

/// Errors related to Console manager operations.
#[derive(Debug, thiserror::Error)]
pub enum ConsoleManagerError {
    /// Cannot create unix domain socket for serial port
    #[error("cannot create socket for serial console")]
    CreateSerialSock(#[source] std::io::Error),

    /// An operation on the epoll instance failed due to resource exhaustion or bad configuration.
    #[error("failure while managing epoll event for console fd")]
    EpollMgr(#[source] dbs_utils::epoll_manager::Error),

    /// Cannot set mode for terminal.
    #[error("failure while setting attribute for terminal")]
    StdinHandle(#[source] vmm_sys_util::errno::Error),
}

/// Console manager to manage frontend and backend console devcies.
pub struct ConsoleManager {
    epoll_mgr: EpollManager,
    logger: slog::Logger,
    subscriber_id: Option<SubscriberId>,
    stdin_handle: Option<std::io::Stdin>,
    sock_path: Option<String>,
}

impl ConsoleManager {
    /// Create a console manager instance.
    pub fn new(epoll_mgr: EpollManager, logger: &slog::Logger) -> Self {
        let logger = logger.new(slog::o!("subsystem" => "console_manager"));
        ConsoleManager {
            epoll_mgr,
            logger,
            stdin_handle: None,
            subscriber_id: None,
            sock_path: None,
        }
    }

    /// Create a console backend device by using stdio streams.
    pub fn create_stdio_console(&mut self, device: Arc<Mutex<SerialDevice>>) -> Result<()> {
        let stdin_handle = std::io::stdin();
        stdin_handle
            .lock()
            .set_raw_mode()
            .map_err(|e| DeviceMgrError::ConsoleManager(ConsoleManagerError::StdinHandle(e)))?;

        let handler = ConsoleEpollHandler::new(device, Some(stdin_handle), None, &self.logger);
        self.subscriber_id = Some(self.epoll_mgr.add_subscriber(Box::new(handler)));
        self.stdin_handle = Some(std::io::stdin());

        Ok(())
    }

    /// Create s console backend device by using Unix Domain socket.
    pub fn create_socket_console(
        &mut self,
        device: Arc<Mutex<SerialDevice>>,
        sock_path: String,
    ) -> Result<()> {
        let sock_listener = Self::bind_domain_socket(&sock_path).map_err(|e| {
            DeviceMgrError::ConsoleManager(ConsoleManagerError::CreateSerialSock(e))
        })?;
        let handler = ConsoleEpollHandler::new(device, None, Some(sock_listener), &self.logger);

        self.subscriber_id = Some(self.epoll_mgr.add_subscriber(Box::new(handler)));
        self.sock_path = Some(sock_path);

        Ok(())
    }

    /// Reset the host side terminal to canonical mode.
    pub fn reset_console(&self) -> Result<()> {
        if let Some(stdin_handle) = self.stdin_handle.as_ref() {
            stdin_handle
                .lock()
                .set_canon_mode()
                .map_err(|e| DeviceMgrError::ConsoleManager(ConsoleManagerError::StdinHandle(e)))?;
        }

        Ok(())
    }

    fn bind_domain_socket(serial_path: &str) -> std::result::Result<UnixListener, std::io::Error> {
        use std::fs::remove_file;
        let _ = remove_file(serial_path);

        UnixListener::bind(serial_path)
    }
}

struct ConsoleEpollHandler {
    device: Arc<Mutex<SerialDevice>>,
    stdin_handle: Option<std::io::Stdin>,
    sock_listener: Option<UnixListener>,
    sock_conn: Option<UnixStream>,
    logger: slog::Logger,
}

impl ConsoleEpollHandler {
    fn new(
        device: Arc<Mutex<SerialDevice>>,
        stdin_handle: Option<std::io::Stdin>,
        sock_listener: Option<UnixListener>,
        logger: &slog::Logger,
    ) -> Self {
        ConsoleEpollHandler {
            device,
            stdin_handle,
            sock_listener,
            sock_conn: None,
            logger: logger.new(slog::o!()),
        }
    }

    fn uds_listener_accept(&mut self, ops: &mut EventOps) -> std::io::Result<()> {
        if self.sock_conn.is_some() {
            slog::warn!(self.logger,
                "UDS for serial port 1 already exists, reject the new connection";
                "subsystem" => "console_mgr",
            );
            // Do not expected poisoned lock.
            let _ = self.sock_listener.as_mut().unwrap().accept();
        } else {
            // Safe to unwrap() because self.sock_conn is Some().
            let (conn_sock, _) = self.sock_listener.as_ref().unwrap().accept()?;
            let events = Events::with_data(&conn_sock, EPOLL_EVENT_SERIAL_DATA, EventSet::IN);
            if let Err(e) = ops.add(events) {
                slog::error!(self.logger,
                    "failed to register epoll event for serial, {:?}", e;
                    "subsystem" => "console_mgr",
                );
                return Err(std::io::Error::last_os_error());
            }

            let conn_sock_copy = conn_sock.try_clone()?;
            // Do not expected poisoned lock.
            self.device
                .lock()
                .unwrap()
                .set_output_stream(Some(Box::new(conn_sock_copy)));

            self.sock_conn = Some(conn_sock);
        }

        Ok(())
    }

    fn uds_read_in(&mut self, ops: &mut EventOps) -> std::io::Result<()> {
        let mut should_drop = true;

        if let Some(conn_sock) = self.sock_conn.as_mut() {
            let mut out = [0u8; 64];
            match conn_sock.read(&mut out[..]) {
                Ok(0) => {
                    // Zero-length read means EOF. Remove this conn sock.
                    self.device
                        .lock()
                        .expect("console: poisoned console lock")
                        .set_output_stream(None);
                }
                Ok(count) => {
                    self.device
                        .lock()
                        .expect("console: poisoned console lock")
                        .raw_input(&out[..count])?;
                    should_drop = false;
                }
                Err(e) => {
                    slog::warn!(self.logger,
                        "error while reading serial conn sock: {:?}", e;
                        "subsystem" => "console_mgr"
                    );
                    self.device
                        .lock()
                        .expect("console: poisoned console lock")
                        .set_output_stream(None);
                }
            }
        }

        if should_drop {
            assert!(self.sock_conn.is_some());
            // Safe to unwrap() because self.sock_conn is Some().
            let sock_conn = self.sock_conn.take().unwrap();
            let events = Events::with_data(&sock_conn, EPOLL_EVENT_SERIAL_DATA, EventSet::IN);
            if let Err(e) = ops.remove(events) {
                slog::error!(self.logger,
                    "failed deregister epoll event for UDS, {:?}", e;
                    "subsystem" => "console_mgr"
                );
            }
        }

        Ok(())
    }

    fn stdio_read_in(&mut self, ops: &mut EventOps) -> std::io::Result<()> {
        let mut should_drop = true;

        if self.stdin_handle.is_some() {
            let mut out = [0u8; 64];
            // Safe to unwrap() because self.stdin_handle is Some().
            let stdin_lock = self.stdin_handle.as_ref().unwrap().lock();
            match stdin_lock.read_raw(&mut out[..]) {
                Ok(0) => {
                    // Zero-length read indicates EOF. Remove from pollables.
                    self.device
                        .lock()
                        .expect("console: poisoned console lock")
                        .set_output_stream(None);
                }
                Ok(count) => {
                    self.device
                        .lock()
                        .expect("console: poisoned console lock")
                        .raw_input(&out[..count])?;
                    should_drop = false;
                }
                Err(e) => {
                    slog::warn!(self.logger,
                        "error while reading stdin: {:?}", e;
                        "subsystem" => "console_mgr"
                    );
                    self.device
                        .lock()
                        .expect("console: poisoned console lock")
                        .set_output_stream(None);
                }
            }
        }

        if should_drop {
            let events = Events::with_data_raw(libc::STDIN_FILENO, EPOLL_EVENT_STDIN, EventSet::IN);
            if let Err(e) = ops.remove(events) {
                slog::error!(self.logger,
                    "failed to deregister epoll event for stdin, {:?}", e;
                    "subsystem" => "console_mgr"
                );
            }
        }

        Ok(())
    }
}

impl MutEventSubscriber for ConsoleEpollHandler {
    fn process(&mut self, events: Events, ops: &mut EventOps) {
        let logger = self.logger.new(slog::o!("subsystem" => "console_manager"));
        slog::trace!(logger, "ConsoleEpollHandler::process()");

        let slot = events.data();
        match slot {
            EPOLL_EVENT_SERIAL => {
                if let Err(e) = self.uds_listener_accept(ops) {
                    slog::warn!(self.logger,
                        "failed to accept incoming connection, {:?}", e;
                        "subsystem" => "console_mgr",
                    );
                }
            }
            EPOLL_EVENT_SERIAL_DATA => {
                if let Err(e) = self.uds_read_in(ops) {
                    slog::warn!(self.logger,
                        "failed to read data from UDS, {:?}", e;
                        "subsystem" => "console_mgr",
                    );
                }
            }
            EPOLL_EVENT_STDIN => {
                if let Err(e) = self.stdio_read_in(ops) {
                    slog::warn!(self.logger,
                        "failed to read data from stdin, {:?}", e;
                        "subsystem" => "console_mgr",
                    );
                }
            }
            _ => slog::error!(self.logger,
                "unknown epoll slot number {}", slot;
                "subsystem" => "console_mgr",
            ),
        }
    }

    fn init(&mut self, ops: &mut EventOps) {
        let logger = self.logger.new(slog::o!("subsystem" => "console_manager"));
        slog::trace!(logger, "ConsoleEpollHandler::init()");

        if self.stdin_handle.is_some() {
            slog::info!(logger, "ConsoleEpollHandler: stdin handler");
            let events = Events::with_data_raw(libc::STDIN_FILENO, EPOLL_EVENT_STDIN, EventSet::IN);
            if let Err(e) = ops.add(events) {
                slog::error!(
                    self.logger,
                    "failed to register epoll event for stdin, {:?}", e;
                    "subsystem" => "console_mgr",
                );
            }
        }
        if let Some(sock) = self.sock_listener.as_ref() {
            slog::info!(logger, "ConsoleEpollHandler: sock listener");
            let events = Events::with_data(sock, EPOLL_EVENT_SERIAL, EventSet::IN);
            if let Err(e) = ops.add(events) {
                slog::error!(
                    self.logger,
                    "failed to register epoll event for UDS listener, {:?}", e;
                    "subsystem" => "console_mgr",
                );
            }
        }

        // LiveUpgrade need this
        if let Some(conn) = self.sock_conn.as_ref() {
            slog::info!(logger, "ConsoleEpollHandler: sock connection");
            let events = Events::with_data(conn, EPOLL_EVENT_SERIAL_DATA, EventSet::IN);
            if let Err(e) = ops.add(events) {
                slog::error!(
                    self.logger,
                    "failed to register epoll event for UDS connection, {:?}", e;
                    "subsystem" => "console_mgr",
                );
            }
        }
    }
}

/// Used for printing out dmesg logs
pub struct DmesgWriter {
    buf: BytesMut,
    logger: slog::Logger,
}

impl io::Write for DmesgWriter {
    /// 0000000   [                   0   .   0   3   4   9   1   6   ]       R
    ///          5b  20  20  20  20  30  2e  30  33  34  39  31  36  5d  20  52
    /// 0000020   u   n       /   s   b   i   n   /   i   n   i   t       a   s
    ///          75  6e  20  2f  73  62  69  6e  2f  69  6e  69  74  20  61  73
    /// 0000040       i   n   i   t       p   r   o   c   e   s   s  \r  \n   [
    ///
    /// dmesg message end a line with /r/n . When redirect message to logger, we should
    /// remove the /r/n .
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let arr: Vec<&[u8]> = buf.split(|c| *c == b'\n').collect();
        let count = arr.len();
        for (i, sub) in arr.iter().enumerate() {
            if sub.is_empty() {
                if !self.buf.is_empty() {
                    slog::info!(
                        self.logger,
                        "{}",
                        String::from_utf8_lossy(self.buf.as_ref()).trim_end()
                    );
                    self.buf.clear();
                }
                continue;
            }
            if sub.len() < buf.len() && i < count - 1 {
                slog::info!(
                    self.logger,
                    "{}{}",
                    String::from_utf8_lossy(self.buf.as_ref()).trim_end(),
                    String::from_utf8_lossy(sub).trim_end(),
                );
                self.buf.clear();
                continue;
            }
            self.buf.put_slice(sub);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
