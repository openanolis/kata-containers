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

use std::sync::Arc;

use log::{debug, error, warn};
use vm_memory::GuestAddress;

use super::{Error, Result};
use crate::ioapic::rdte::*;
use crate::kvm::KvmIrqManager;
use crate::{DeviceInterruptManager, DeviceInterruptMode, InterruptIndex};

pub const NUM_IOAPIC_PINS: usize = 24;
const IOAPIC_VERSION_ID: u32 = 0x0017_0011;
// Constants for IOAPIC direct register offset
const IOAPIC_REG_ID: u64 = 0x0000;
const IOAPIC_REG_VERSION: u64 = 0x0001;
const IOAPIC_REG_ARBITRATION_ID: u64 = 0x0002;
/// Register offsets
pub const IOREGSEL_OFF: u64 = 0x0;
pub const IOWIN_OFF: u64 = 0x10;
const IOWIN_SCALE: u64 = 0x2;
const REG_MAX_OFFSET: u64 = IOWIN_OFF + (NUM_IOAPIC_PINS as u64 * 2) - 1;
pub const APIC_START: u64 = 0xfee0_0000;

#[repr(u8)]
enum TriggerMode {
    Edge = 0,
    Level = 1,
}
#[repr(u8)]

enum DeliveryMode {
    Fixed = 0b000,
    Lowest = 0b001,
    Smi = 0b010,        // System management interrupt
    RemoteRead = 0b011, // This is no longer supported by intel.
    Nmi = 0b100,        // Non maskable interrupt
    Init = 0b101,
    Startup = 0b110,
    External = 0b111,
}

// Given an offset that was read from/written to, return a tuple of the relevant IRQ and whether
// the offset refers to the high bits of that register.
fn decode_irq_from_selector(selector: u64) -> (usize, bool) {
    (
        ((selector - IOWIN_OFF) / IOWIN_SCALE) as usize,
        selector & 1 != 0,
    )
}

/// IOAPIC Device State
pub struct IoapicDeviceState {
    id_reg: u32,
    pub reg_sel: u32,
    pub reg_entries: [RedirectionTableEntry; NUM_IOAPIC_PINS],
    used_entries: [bool; NUM_IOAPIC_PINS],
    apic_address: GuestAddress,
    pub intr_mgr: DeviceInterruptManager<Arc<KvmIrqManager>>,
}

impl IoapicDeviceState {
    /// Create Ioapic Device State
    pub fn new(irq_manager: Arc<KvmIrqManager>) -> Result<Self> {
        let intr_mgr = DeviceInterruptManager::new_with_ioapic(irq_manager)
            .map_err(Error::CreateInterruptManager)?;
        Ok(IoapicDeviceState {
            id_reg: 0,
            reg_sel: 0,
            reg_entries: [0x10000; NUM_IOAPIC_PINS],
            used_entries: [false; NUM_IOAPIC_PINS],
            apic_address: GuestAddress(APIC_START),
            intr_mgr,
        })
    }

    /// Active Ioapic Device
    pub fn activate(&mut self) -> Result<()> {
        self.intr_mgr
            .set_working_mode(DeviceInterruptMode::GenericMsiIrq)
            .map_err(Error::CreateInterruptManager)?;
        self.intr_mgr
            .enable()
            .map_err(Error::CreateInterruptManager)?;
        Ok(())
    }

    /// Update entry for irq
    fn update_entry(&mut self, irq: usize) -> Result<()> {
        debug!("IOAPIC:  update entry for irq {}", irq);
        let entry = self.reg_entries[irq];
        // Validate Destination Mode value, and retrieve Destination ID
        let destination_mode = destination_mode(entry);
        let destination_id = destination_field(entry);
        // When this bit is set, the message is directed to the processor with
        // the lowest interrupt priority among processors that can receive the
        // interrupt.
        let redirection_hint: u8 = 1;
        // Generate MSI message address
        let low_addr: u32 = self.apic_address.0 as u32
            | u32::from(destination_id) << 12
            | u32::from(redirection_hint) << 3
            | u32::from(destination_mode) << 2;
        // Validate Trigger Mode value
        let trigger_mode = trigger_mode(entry);
        match trigger_mode {
            x if (x == TriggerMode::Edge as u8) || (x == TriggerMode::Level as u8) => {}
            _ => return Err(Error::Invalid),
        }
        // Validate Delivery Mode value
        let delivery_mode = delivery_mode(entry);
        match delivery_mode {
            x if (x == DeliveryMode::Fixed as u8)
                || (x == DeliveryMode::Lowest as u8)
                || (x == DeliveryMode::Smi as u8)
                || (x == DeliveryMode::RemoteRead as u8)
                || (x == DeliveryMode::Nmi as u8)
                || (x == DeliveryMode::Init as u8)
                || (x == DeliveryMode::Startup as u8)
                || (x == DeliveryMode::External as u8) => {}
            _ => return Err(Error::Invalid),
        }
        // Generate MSI message data
        let msi_data: u32 = u32::from(trigger_mode) << 15
            | u32::from(remote_irr(entry)) << 14
            | u32::from(delivery_mode) << 8
            | u32::from(vector(entry));
        // self.intr_mgr.reset()?;
        self.intr_mgr
            .set_msi_low_address(irq as InterruptIndex, low_addr)
            .map_err(Error::CreateInterruptManager)?;
        self.intr_mgr
            .set_msi_high_address(irq as InterruptIndex, 0x0)
            .map_err(Error::CreateInterruptManager)?;
        self.intr_mgr
            .set_msi_data(irq as InterruptIndex, msi_data)
            .map_err(Error::CreateInterruptManager)?;
        if self.intr_mgr.is_enabled() {
            self.intr_mgr
                .update(irq as InterruptIndex)
                .map_err(Error::CreateInterruptManager)?;
        } else {
            // enable every time?
            self.intr_mgr
                .enable()
                .map_err(Error::CreateInterruptManager)?;
        }
        Ok(())
    }

    /// ioapic read
    pub fn ioapic_read(&self) -> u32 {
        debug!("IOAPIC_R reg 0x{:x}", self.reg_sel);
        match self.reg_sel as u64 {
            IOAPIC_REG_VERSION => IOAPIC_VERSION_ID,
            IOAPIC_REG_ID | IOAPIC_REG_ARBITRATION_ID => (self.id_reg & 0xf) << 24,
            IOWIN_OFF..=REG_MAX_OFFSET => {
                let (index, is_high_bits) = decode_irq_from_selector(self.reg_sel as u64);
                if index > NUM_IOAPIC_PINS {
                    warn!("IOAPIC index out of range: {}", index);
                    return 0;
                }
                if is_high_bits {
                    (self.reg_entries[index] >> 32) as u32
                } else {
                    (self.reg_entries[index] & 0xffff_ffff) as u32
                }
            }
            _ => {
                error!(
                    "IOAPIC: invalid read from register offset 0x{:x}",
                    self.reg_sel
                );
                0
            }
        }
    }

    /// ioapic write
    pub fn ioapic_write(&mut self, val: u32) {
        debug!("IOAPIC_W reg 0x{:x}, val 0x{:x}", self.reg_sel, val);
        match self.reg_sel as u64 {
            IOAPIC_REG_VERSION => {
                if val == 0 {
                    // Windows writes zero here (see #1791)
                } else {
                    error!(
                        "IOAPIC: invalid write to version register (0x{:x}): 0x{:x}",
                        self.reg_sel, val
                    );
                }
            }
            IOAPIC_REG_ID => self.id_reg = (val >> 24) & 0xf,
            IOWIN_OFF..=REG_MAX_OFFSET => {
                let (index, is_high_bits) = decode_irq_from_selector(self.reg_sel as u64);
                if index > NUM_IOAPIC_PINS {
                    warn!("IOAPIC index out of range: {}", index);
                    return;
                }
                if is_high_bits {
                    self.reg_entries[index] &= 0xffff_ffff;
                    self.reg_entries[index] |= u64::from(val) << 32;
                } else {
                    // Ensure not to override read-only bits:
                    // - Delivery Status (bit 12)
                    // - Remote IRR (bit 14)
                    self.reg_entries[index] &= 0xffff_ffff_0000_5000;
                    self.reg_entries[index] |= u64::from(val) & 0xffff_afff;
                }
                // The entry must be updated through the interrupt source
                // group.
                if let Err(e) = self.update_entry(index) {
                    error!("Failed updating IOAPIC entry: {:?}", e);
                }
                // Store the information this IRQ is now being used.
                self.used_entries[index] = true;
            }
            _ => error!(
                "IOAPIC: invalid write to register offset 0x{:x}",
                self.reg_sel
            ),
        }
    }
}
