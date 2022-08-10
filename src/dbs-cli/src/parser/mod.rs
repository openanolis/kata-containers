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
use clap::Parser;
use kvm_ioctls::Kvm;
use seccompiler::{BpfProgram};
use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};
use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap};
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};
use dbs_boot;

pub use args::DBSArgs;
use crate::utils::{CLIResult, CLIError, KernelErrorKind, RootfsErrorKind};

use dragonball::{api::v1::InstanceInfo, Vmm, event_manager::EventManager};
use dragonball::api::v1::VmmData;
use dragonball::vm::{CpuTopology, KernelConfigInfo, VmConfigInfo};


pub fn run_with_cli(args: &DBSArgs) -> CLIResult<()> {
    let kernel_path = &args.boot_args.kernel_path;
    let rootfs_path = &args.boot_args.rootfs_args.rootfs;

    // check the existence of kernel
    let kernel_file = std::path::Path::new(kernel_path);
    if !kernel_file.exists() || !kernel_file.is_file() {
        return Err(CLIError::KernelError(KernelErrorKind::KernelNotFound));
    }

    // check the existence of rootfs
    let rootfs_file = std::path::Path::new(rootfs_path);
    if !rootfs_file.exists() || !rootfs_file.is_file() {
        return Err(CLIError::RootfsError(RootfsErrorKind::RootfsNotFound));
    }

    // retrieve empty seccomp filters
    let vmm_seccomp_filters = BpfProgram::default();
    let vcpu_seccomp_filters = BpfProgram::default();

    // Create a VMM instance
    let shared_info = std::sync::Arc::new(RwLock::new(InstanceInfo::default()));
    let event_fd = EventFd::new(EFD_NONBLOCK)?;
    let kvm_fd = Kvm::open_with_cloexec(true)?;
    let mut vmm = Vmm::new(
        shared_info,
        event_fd,
        vmm_seccomp_filters,
        vcpu_seccomp_filters,
        kvmfd,
    ).unwrap();

    // configuration
    let mut vm_config = VmConfigInfo {
        vcpu_count: args.create_args.vcpu,
        max_vcpu_count: args.create_args.max_vcpu,
        cpu_pm: args.create_args.cpu_pm.clone(),
        cpu_topology: CpuTopology {
            threads_per_core: args.create_args.cpu_topology.threads_per_core,
            cores_per_die: args.create_args.cpu_topology.cores_per_die,
            dies_per_socket: args.create_args.cpu_topology.dies_per_socket,
            sockets: args.create_args.cpu_topology.sockets,
        },
        vpmu_feature: 0,
        mem_type: args.create_args.mem_type.clone(),
        mem_file_path: args.create_args.mem_file_path.clone(),
        mem_size_mib: args.create_args.mem_size,
        serial_path: None,
    };

    // boot source
    // TODO: handle error
    let kernel_file = File::open(&args.boot_args.kernel_path).map_err(|_| {})?;
    let initrd_file = match args.boot_args.initrd_path {
        None => None,
        Some(ref path) => Some(File::open(path).map_err(|_| {})?)
    };
    let mut cmdline = linux_loader::cmdline::Cmdline::new(dbs_boot::layout::CMDLINE_MAX_SIZE);
    let boot_args = args.boot_args.boot_args.clone().unwrap_or_else(|| {std::io::Error})?;
    let kernel_config = KernelConfigInfo::new(kernel_file, initrd_file, cmdline);

    // event manager
    let mut epoll_mgr = EpollManager::default();
    let mut event_mgr = EventManager::new(&Arc::new(Mutex::new(vmm)), epoll_mgr).unwrap();

    // TODO: add Error type
    let vm = vmm.get_vm_mut().ok_or(std::io::Error)?;
    if vm.is_vm_initialized() {
        // TODO: add Error type
        // return Err(StartMicroVm(MicroVMAlreadyRunning));
    }
    vm.set_kernel_config(kernel_config);
    vm.set_vm_config(vm_config);

    vm.start_microvm(&mut event_mgr, vmm.vmm_seccomp_filter(), vmm.vcpu_seccomp_filter())?

    return Ok(());
}
