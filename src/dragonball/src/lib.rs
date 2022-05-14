// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Dragonball is a sandbox as Virtual Machine Monitor that leverages the
//! Linux Kernel-based Virtual Machine (KVM), and other virtualization
//! features to run a single lightweight micro-virtual machine (microVM).

#![warn(missing_docs)]

//TODO: Remoe this, after the rest of dragonball has been committed.
#![allow(dead_code)]

/// Address space manager for virtual machines.
pub mod address_space_manager;
/// Resource manager for virtual machines.
pub mod resource_manager;
/// Virtual machine manager for virtual machines.
pub mod vm;
/// Device manager for virtual machines.
pub mod device_manager;
/// Errors related to Virtual machine manager.
pub mod error;
/// Signal handler for virtual machines.
pub mod signal_handler;
/// Metrics system.
pub mod metric;
/// KVM operation context for virtual machines.
pub mod kvm_context;

mod io_manager;
pub(crate) use self::io_manager::IoManagerImpl;

mod config_manager;

/// Success exit code.
pub const EXIT_CODE_OK: u8 = 0;
/// Generic error exit code.
pub const EXIT_CODE_GENERIC_ERROR: u8 = 1;
/// Generic exit code for an error considered not possible to occur if the program logic is sound.
pub const EXIT_CODE_UNEXPECTED_ERROR: u8 = 2;
/// Dragonball was shut down after intercepting a restricted system call.
pub const EXIT_CODE_BAD_SYSCALL: u8 = 148;
/// Dragonball was shut down after intercepting `SIGBUS`.
pub const EXIT_CODE_SIGBUS: u8 = 149;
/// Dragonball was shut down after intercepting `SIGSEGV`.
pub const EXIT_CODE_SIGSEGV: u8 = 150;
/// Invalid json passed to the Dragonball process for configuring microvm.
pub const EXIT_CODE_INVALID_JSON: u8 = 151;
/// Bad configuration for microvm's resources, when using a single json.
pub const EXIT_CODE_BAD_CONFIGURATION: u8 = 152;
/// Command line arguments parsing error.
pub const EXIT_CODE_ARG_PARSING: u8 = 153;
