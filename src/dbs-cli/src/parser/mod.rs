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

use std::collections::HashMap;
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
use dragonball::{
    api::v1::{
        BlockDeviceConfigInfo, BootSourceConfig,
        InstanceInfo, InstanceState, VmmAction, VmmActionError, VmmData,
        VmmRequest, VmmResponse, VmmService, BootSourceConfigError, DEFAULT_KERNEL_CMDLINE,
    },
    vm::{VmConfigInfo, CpuTopology, KernelConfigInfo},
    Vmm,
    event_manager::EventManager,
};
use std::{
    fs::{File, OpenOptions},
    os::unix::{io::IntoRawFd, prelude::AsRawFd},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread,
    path::{Path, PathBuf}
};
use std::ops::Deref;

const KVM_DEVICE: &str = "/dev/kvm";


pub use args::DBSArgs;

use crate::cli_instance::CliInstance;

pub fn run_with_cli(args: DBSArgs) -> Result<()> {
    let mut cli_instance = CliInstance::new("dbs-cli");

    let kvm = OpenOptions::new().read(true).write(true).open(KVM_DEVICE)?;

    let (to_vmm, from_runtime) = channel();
    let (to_runtime, from_vmm) = channel();

    let vmm_service = VmmService::new(from_runtime, to_runtime);

    cli_instance.to_vmm = Some(to_vmm);
    cli_instance.from_vmm = Some(from_vmm);

    let api_event_fd2 = cli_instance.to_vmm_fd.try_clone().expect("Failed to dup eventfd");
    let vmm = Vmm::new(
        cli_instance.vmm_shared_info.clone(),
        api_event_fd2,
        cli_instance.seccomp.clone(),
        cli_instance.seccomp.clone(),
        Some(kvm.into_raw_fd()),
    ).expect("Failed to start vmm");

    // let cli_instance_copy = Arc::new(RwLock::new(cli_instance)).clone();
    thread::Builder::new().name("set configuration".to_owned())
        .spawn(move || {
            cli_instance.run_vmm_server("dbs-cli", args);

        });

    println!("Begin event handling.");
    let exit_code =
        Vmm::run_vmm_event_loop(Arc::new(Mutex::new(vmm)), vmm_service);
    println!("run vmm thread exited: {}", exit_code);

    return Ok(());
}

