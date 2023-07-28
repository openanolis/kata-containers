// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
mod device;
pub use device::IoapicDevice;
mod ioapic_status;
mod rdte;

/// Ioapic device related errors
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Invalid device resource
    #[error("invalid device resource")]
    InvalidResource,

    /// Interrupt manager create failed
    #[error("interrupt manager create failed: {0}")]
    CreateInterruptManager(#[source] std::io::Error),

    /// Interrupt trigger error
    #[error("failed to trigger: {0}")]
    Trigger(#[source] std::io::Error),

    /// Invalid trigger mode
    #[error("invalid  mode")]
    Invalid,
}

type Result<T> = std::result::Result<T, Error>;
