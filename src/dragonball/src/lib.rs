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

/// Address space manager for virtual machines.
pub mod address_space_manager;
/// Resource manager for virtual machines.
pub mod resource_manager;
/// Virtual machine manager for virtual machines.
pub mod vm;
/// Device manager for virtual machines.
pub mod device_manager;