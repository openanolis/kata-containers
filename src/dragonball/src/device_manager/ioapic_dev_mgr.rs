// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0
//! Device manager for Dragonball Ioapic device.

use std::sync::Arc;

use crate::device_manager::DeviceOpContext;

use dbs_interrupt::ioapic::{Error as IoapicDeviceError, IoapicDevice};
use dbs_device::device_manager::IoManagerContext;
/// Error type for Ioapic device manager

#[derive(Debug, thiserror::Error)]
pub enum IoapicDeviceMgrError {
    ///Ioapic device internal error
    #[error("ioapic device error: {0}")]
    IoapicDevice(IoapicDeviceError),
    /// allocate device resource error
    #[error("allocate device resource error: {0}")]
    AllocateResource(#[source] crate::resource_manager::ResourceError),
    /// Register device io error
    #[error("register device io error: {0}")]
    RegisterDeviceIo(#[source] dbs_device::device_manager::Error),
    /// Ioapic device already attached
    #[error("ioapic device already attached")]
    DeviceAlreadyAttached,
    /// Missing Ioapic device
    #[error("missing ioapic device")]
    MissingDevice,
}

type Result<T> = std::result::Result<T, IoapicDeviceMgrError>;

/// Ioapic device manager
pub struct IoapicDeviceMgr {
    device: Option<Arc<IoapicDevice>>,
}

impl IoapicDeviceMgr {
    /// Get Ioapic device
    pub fn get_device(&self) -> Result<Arc<IoapicDevice>> {
        match &self.device {
            Some(d) => Ok(d.clone()),
            None => Err(IoapicDeviceMgrError::MissingDevice),
        }
    }

    /// Attach Ioapic device
    pub fn attach_device(&mut self, ctx: &mut DeviceOpContext) -> Result<()> {
        if self.device.is_some() {
            return Err(IoapicDeviceMgrError::DeviceAlreadyAttached);
        }
        let device = IoapicDevice::new(ctx.irq_manager.clone())
            .map_err(IoapicDeviceMgrError::IoapicDevice)?;
        self.device = Some(Arc::new(device));
        self.register_device_io(ctx)?;
        Ok(())
    }

    /// Register platform device's resource to io manger
    pub fn register_device_io(&self, ctx: &mut DeviceOpContext) -> Result<()> {
        if let Some(device) = self.device.as_ref() {
            // get resource
            let mut resource_requests = Vec::new();
            IoapicDevice::get_resource_requirements(&mut resource_requests);
            let io_resource = ctx
                .res_manager
                .allocate_device_resources(&resource_requests, false)
                .map_err(IoapicDeviceMgrError::AllocateResource)?;
            let mut tx = ctx.io_context.begin_tx();
            if let Err(e) = ctx
                .io_context
                .register_device_io(&mut tx, device.clone(), &io_resource)
            {
                ctx.io_context.cancel_tx(tx);
                return Err(IoapicDeviceMgrError::RegisterDeviceIo(e));
            }
            ctx.io_context.commit_tx(tx);
        }
        Ok(())
    }
}

impl Default for IoapicDeviceMgr {
    /// IoapicDeviceMgr
    fn default() -> Self {
        Self { device: None }
    }
}