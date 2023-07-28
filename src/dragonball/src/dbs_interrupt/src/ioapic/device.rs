// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//
// Implementation of an intel 82093AA Input/Output Advanced Programmable Interrupt Controller
// See https://pdos.csail.mit.edu/6.828/2016/readings/ia32/ioapic.pdf for a specification.
use crate::kvm::KvmIrqManager;
#[cfg(all(target_arch = "x86_64", feature = "kvm-userspace-ioapic"))]
use crate::InterruptController;
use crate::InterruptIndex;
use byteorder::{ByteOrder, LittleEndian};
use dbs_device::resources::ResourceConstraint;
use dbs_device::{DeviceIo, IoAddress};
use log::{debug, error, info, warn};
use std::any::Any;
use std::sync::{Arc, Mutex, MutexGuard};
use vmm_sys_util::eventfd::EventFd;

use super::Result;
use crate::ioapic::ioapic_status::{IoapicDeviceState, IOREGSEL_OFF, IOWIN_OFF, NUM_IOAPIC_PINS};
use crate::ioapic::rdte::*;

/// Guset address
pub const IOAPIC_START: u64 = 0xfec0_0000;
pub const IOAPIC_SIZE: u64 = 0x20;

/// Ioapic Device
pub struct IoapicDevice {
    state: Mutex<IoapicDeviceState>,
}

impl IoapicDevice {
    /// Create Ioapic Device
    pub fn new(irq_manager: Arc<KvmIrqManager>) -> Result<Self> {
        let mut state = IoapicDeviceState::new(irq_manager)?;
        state.activate()?;
        Ok(IoapicDevice {
            state: Mutex::new(state),
        })
    }

    /// Get resource requirements
    pub fn get_resource_requirements(requests: &mut Vec<ResourceConstraint>) {
        requests.push(ResourceConstraint::PlatformMmioAddress {
            range: Some((IOAPIC_START, IOAPIC_START + IOAPIC_SIZE)),
            align: 0,
            size: IOAPIC_SIZE,
        });
    }

    /// Get Ioapic state
    fn state(&self) -> MutexGuard<IoapicDeviceState> {
        // we don't expect to lock poison
        self.state.lock().unwrap()
    }
}

impl DeviceIo for IoapicDevice {
    /// Device mmio read emulation
    fn read(&self, _base: IoAddress, offset: IoAddress, data: &mut [u8]) {
        if data.len() != std::mem::size_of::<u32>() {
            warn!("Invalid read size on IOAPIC: {}", data.len());
            return;
        }
        debug!("IOAPIC_R @ offset 0x{:x}", offset.raw_value());
        let state = self.state();
        let value: u32 = match offset.raw_value() {
            IOREGSEL_OFF => state.reg_sel,
            IOWIN_OFF => state.ioapic_read(),
            _ => {
                error!("IOAPIC: failed reading at offset {}", offset.raw_value());
                return;
            }
        };
        LittleEndian::write_u32(data, value);
    }

    /// device mmio write emulation
    fn write(&self, _base: IoAddress, offset: IoAddress, data: &[u8]) {
        if data.len() != std::mem::size_of::<u32>() {
            warn!("Invalid write size on IOAPIC: {}", data.len());
            return;
        }
        debug!("IOAPIC_W @ offset 0x{:x}", offset.raw_value());
        let value = LittleEndian::read_u32(data);
        let mut state = self.state();
        match offset.raw_value() {
            IOREGSEL_OFF => state.reg_sel = value,
            IOWIN_OFF => state.ioapic_write(value),
            _ => {
                error!("IOAPIC: failed writing at offset {}", offset.raw_value());
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl InterruptController for IoapicDevice {
    /// Service irq from IOAPIC PINS
    fn service_irq(&self, irq: usize) -> std::io::Result<()> {
        debug!("service irq {}", irq);
        let entry = &mut self.state().reg_entries[irq];
        if interrupt_mask(*entry) == 1 {
            debug!("Interrupt masked!");
            return Ok(());
        }
        // irq already delived, only change status in userspace-ioapic
        debug!("Interrupt successfully delivered");
        // If trigger mode is level sensitive, set the Remote IRR bit.
        // It will be cleared when the EOI is received.
        if trigger_mode(*entry) == 1 {
            set_remote_irr(entry, 1);
        }
        // Clear the Delivery Status bit
        set_delivery_status(entry, 0);
        Ok(())
    }

    /// Get notifier
    fn notifier(&self, irq: usize) -> std::io::Result<EventFd> {
        info!("get notfier fot irq {}", irq);
        let state = self.state();
        let event_fd = state
            .intr_mgr
            .get_group()
            .unwrap()
            .notifier(irq as InterruptIndex)
            .unwrap()
            .try_clone()?;
        Ok(event_fd)
    }

    /// EOI emulation
    fn end_of_interrupt(&self, vec: u8) -> std::io::Result<()> {
        for i in 0..NUM_IOAPIC_PINS {
            let mut state = self.state();
            let entry = &mut state.reg_entries[i];
            // Clear Remote IRR bit
            if vector(*entry) == vec && trigger_mode(*entry) == 1 {
                set_remote_irr(entry, 0);
            }
        }
        Ok(())
    }

    /// Mask irq
    fn mask(&self, irq: usize) -> std::io::Result<()> {
        info!("mask irq {}", irq);
        let mut state = self.state();
        let entry = &mut state.reg_entries[irq];
        set_interrupt_mask(entry, 1);
        state
            .intr_mgr
            .get_group()
            .unwrap()
            .mask(irq as InterruptIndex)?;
        Ok(())
    }

    /// Unmask irq
    fn unmask(&self, irq: usize) -> std::io::Result<()> {
        info!("unmask irq {}", irq);
        let mut state = self.state();
        let entry = &mut state.reg_entries[irq];
        set_interrupt_mask(entry, 0);
        state
            .intr_mgr
            .get_group()
            .unwrap()
            .mask(irq as InterruptIndex)?;
        Ok(())
    }

    /// Enable irq
    fn enable_irq(&self, irq: usize) -> std::io::Result<()> {
        info!("enable irq {}", irq);
        let mut state = self.state();
        let entry = &mut state.reg_entries[irq];
        set_interrupt_mask(entry, 0);
        if state.intr_mgr.is_enabled() {
            Ok(())
        } else {
            error!("IOAPIC: should not enable irq before ioapic enabled");
            Err(std::io::Error::from_raw_os_error(libc::EINVAL))
        }
    }

    /// Disable irq
    fn disable_irq(&self, irq: usize) -> std::io::Result<()> {
        error!("diable irq {}, not supportted", irq);
        Err(std::io::Error::from_raw_os_error(libc::EINVAL))
    }

    /// Update irq
    fn update_irq(&self, irq: usize) -> std::io::Result<()> {
        info!("update irq {}, not support", irq);
        Err(std::io::Error::from_raw_os_error(libc::EINVAL))
    }
}
