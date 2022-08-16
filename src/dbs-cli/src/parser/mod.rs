// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.
//
// Portions Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

pub mod args;

use std::sync::{RwLock, Arc, Mutex};
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use clap::Parser;
use kvm_ioctls::Kvm;
use seccompiler::{BpfProgram};
use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};
use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap};
// use dbs_utils::epoll_manager::{
//     EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
// };
use hypervisor::dragonball::vmm_instance::VmmInstance;
use anyhow::Result;
use vmm_sys_util::terminal::Terminal;

pub use args::DBSArgs;

use crate::cli_instance::CliInstance;

pub fn run_with_cli(args: &DBSArgs) -> Result<()> {
    let mut cli_instance = CliInstance::new("dbs-cli");
    cli_instance.run_vmm_server("dbs-cli", args);
    return Ok(());
}

