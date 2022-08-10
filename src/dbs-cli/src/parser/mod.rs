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
use clap::Parser;
use kvm_ioctls::Kvm;
use seccompiler::{BpfProgram};
use vmm_sys_util::eventfd::{EventFd, EFD_NONBLOCK};
use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap};
use dbs_utils::epoll_manager::{
    EpollManager, EventOps, EventSet, Events, MutEventSubscriber, SubscriberId,
};

pub use args::DBSArgs;
use crate::utils::{CLIResult, CLIError, KernelErrorKind, RootfsErrorKind};

use dragonball::{api::v1::InstanceInfo, Vmm, event_manager::EventManager};
use dragonball::api::v1::VmmData;


// pub fn run_with_cli(args: &DBSArgs) -> CLIResult<()> {
//     let kernel_path = & args.boot_args.kernel_path;
//     let rootfs_path = & args.boot_args.rootfs_args.rootfs;
//
//     // check the existence of kernel
//     let kernel_file = std::path::Path::new(kernel_path);
//     if !kernel_file.exists() || !kernel_file.is_file() {
//         return Err(CLIError::KernelError(KernelErrorKind::KernelNotFound))
//     }
//
//     // check the existence of rootfs
//     let rootfs_file = std::path::Path::new(rootfs_path);
//     if !rootfs_file.exists() || !rootfs_file.is_file() {
//         return Err(CLIError::RootfsError(RootfsErrorKind::RootfsNotFound))
//     }
//
//     // retrieve empty seccomp filters
//     let vmm_seccomp_filters = BpfProgram::default();
//     let vcpu_seccomp_filters = BpfProgram::default();
//
//     // Create a VMM instance
//     let shared_info = std::sync::Arc::new(RwLock::new(InstanceInfo::default()));
//     let event_fd = EventFd::new(EFD_NONBLOCK)?;
//     let kvm_fd = Kvm::open_with_cloexec(true)?;
//     let mut vmm = Vmm::new(
//         shared_info,
//         event_fd,
//         vmm_seccomp_filters,
//         vcpu_seccomp_filters,
//         kvmfd,
//     ).unwrap();
//
//     // event manager
//     let mut epoll_mgr = EpollManager::default();
//     let mut event_mgr = EventManager::new(& Arc::new(Mutex::new(vmm)), epoll_mgr).unwrap();
//
//     // TODO: add Error type
//     let vm = vmm.get_vm_mut().ok_or(std::io::Error)?;
//     if vm.is_vm_initialized() {
//         // TODO: add Error type
//         // return Err(StartMicroVm(MicroVMAlreadyRunning));
//     }
//
//     vm.start_microvm(&mut event_mgr, vmm.vmm_seccomp_filter(), vmm.vcpu_seccomp_filter())?
//
//     return Ok(())
// }
