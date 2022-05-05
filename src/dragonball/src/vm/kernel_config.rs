// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::fs::File;

/// Structure to hold guest kernel configuration information.
pub struct KernelConfigInfo {
    /// The descriptor to the kernel file.
    kernel_file: File,
    /// The descriptor to the initrd file, if there is one
    pub initrd_file: Option<File>,
    /// The commandline validated against correctness.
    pub cmdline: linux_loader::cmdline::Cmdline,
    /// Share guest kernel text/ro sections.
    share_ro_sections: String,

}

impl KernelConfigInfo {
    /// Create a KernelConfigInfo instance.
    pub fn new(
        kernel_file: File,
        initrd_file: Option<File>,
        cmdline: linux_loader::cmdline::Cmdline,
        share_ro_sections: String,
    ) -> Self {
        KernelConfigInfo {
            kernel_file,
            initrd_file,
            cmdline,
            share_ro_sections,
        }
    }

    /// Get a mutable reference to the kernel file.
    pub fn kernel_file_mut(&mut self) -> &mut File {
        &mut self.kernel_file
    }

    /// Get a mutable reference to the initrd file.
    pub fn initrd_file_mut(&mut self) -> Option<&mut File> {
        self.initrd_file.as_mut()
    }

    /// Get a shared reference to the guest kernel boot parameter object.
    pub fn kernel_cmdline(&self) -> &linux_loader::cmdline::Cmdline {
        &self.cmdline
    }

    /// Get a mutable reference to the guest kernel boot parameter object.
    pub fn kernel_cmdline_mut(&mut self) -> &mut linux_loader::cmdline::Cmdline {
        &mut self.cmdline
    }

    /// Check whether guest kernel text/ro section sharing is enabled or not.
    pub fn share_ro_sections(&self) -> String {
        self.share_ro_sections.clone()
    }
}
