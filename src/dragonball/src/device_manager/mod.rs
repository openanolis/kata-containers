// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

//! Device manager to manage IO devices for a virtual machine.

/// Virtual machine console device manager.
pub mod console_manager;
/// Console Manager for virtual machines console device.
pub(crate) use self::console_manager::ConsoleManager;

mod legacy;
pub use self::legacy::Error as LegacyDeviceError;
use self::legacy::LegacyDeviceManager;

/// Errors related to device manager operations.
#[derive(Debug, thiserror::Error)]
pub enum DeviceMgrError {
    /// Failed to manage console devices.
    #[error(transparent)]
    ConsoleManager(console_manager::ConsoleManagerError),
}

/// Specialized version of `std::result::Result` for device manager operations.
pub type Result<T> = ::std::result::Result<T, DeviceMgrError>;
