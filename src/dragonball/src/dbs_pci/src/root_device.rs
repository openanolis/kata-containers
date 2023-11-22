// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
// Portions Copyright 2018 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE-BSD-3-Clause file.
//
// SPDX-License-Identifier: Apache-2.0

//! Pseudo PCI root device to manage accessing to PCI configuration space.
//!
//! To simplify the implementation, it doesn't support the concept of PCI domain, so only PCI buses
//! [0,255] are supported. For most cases, only PCI bus 0 is used as the PCI root bus and all PCI
//! devices directly connect to the PCI root bus.
//!
//! # Configuration Space Access Mechanism #1
//! Two 32-bit I/O locations are used, the first location (0xCF8) is named CONFIG_ADDRESS, and the
//! second (0xCFC) is called CONFIG_DATA. CONFIG_ADDRESS specifies the configuration address that is
//! required to be accesses, while accesses to CONFIG_DATA will actually generate the configuration
//! access and will transfer the data to or from the CONFIG_DATA register.
//!
//! # Memory Mapped PCI Configuration Space Access
//! PCI Express introduced a new way to access PCI configuration space, where it's simply memory
//! mapped and no IO ports are used. This access mechanism is described in PCI Express.
//!
//! Note:
//! - systems that do provide the memory mapped access mechanism are also required to support PCI
//!   access mechanism #1 for backward compatibility.
//! - When a configuration access attempts to select a device that does not exist, the host bridge
//!   will complete the access without error, dropping all data on writes and returning all ones on
//!   reads.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use byteorder::{ByteOrder, NativeEndian};
use dbs_device::device_manager::IoManagerContext;
use dbs_device::resources::DeviceResources;
use dbs_device::DeviceIo;
use dbs_device::IoAddress;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use dbs_device::PioAddress;
use log::debug;

use crate::fill_config_data;
use crate::{Error, PciBus, Result};
#[derive(PartialEq)]
struct PciRootContent {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    // Cached data written to the IO port 0xCF8.
    io_addr: u32,
    buses: HashMap<u32, Arc<PciBus>>,
}

impl PciRootContent {
    pub(crate) fn new() -> Self {
        PciRootContent {
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            io_addr: 0,
            buses: HashMap::new(),
        }
    }
}

/// Pseudo PCI root device to access PCI configuration space.
///
/// Conceptually PCI root device is a system component and doesn't belong to the PCI hierarchy.
pub struct PciRootDevice {
    max_bus_id: u8,
    ioport_base: u16,
    mmio_base: u64,
    mmio_size: u64,
    resources: DeviceResources,
    state: RwLock<PciRootContent>,
}

impl PciRootDevice {
    /// Create a pseudo PCI root device.
    ///
    /// # Arguments
    /// * - `max_bus_id`: maximum PCI bus number supported by the root device instance.
    /// * - `context`: system context to support PCI subsystem.
    /// * - `resources`: resources assigned to/occupied by the PCI root device itself.
    pub fn create(max_bus_id: u8, resources: DeviceResources) -> Result<Self> {
        let mut root = PciRootDevice {
            max_bus_id,
            resources,
            ioport_base: 0,
            mmio_base: 0,
            mmio_size: 0,
            state: RwLock::new(PciRootContent::new()),
        };

        let mut found = false;

        let ioports = root.resources.get_pio_address_ranges();
        match ioports.len() {
            0 => {}
            1 => {
                assert_eq!(ioports[0].1, 8);
                root.ioport_base = ioports[0].0;
                found = true;
            }
            _ => return Err(Error::NoResources),
        }

        let mmios = root.resources.get_mmio_address_ranges();
        match mmios.len() {
            0 => {}
            1 => {
                // Each PCI bus consumes 1MB of MMIO address range.
                assert!(mmios[0].1 >= u64::from(root.max_bus_id) << 20);
                root.mmio_base = mmios[0].0;
                root.mmio_size = mmios[0].1;
                found = true;
            }
            _ => return Err(Error::NoResources),
        }

        // At lease one of IO port or MMIO must be enabled.
        if !found {
            return Err(Error::NoResources);
        }

        Ok(root)
    }

    /// Activate the PCI root device, getting ready to handle PCI configuration space accesses from
    /// the guest.
    pub fn activate<I: IoManagerContext>(root: Arc<PciRootDevice>, io_ctx: &mut I) -> Result<()> {
        let mut tx = io_ctx.begin_tx();
        if let Err(e) = io_ctx.register_device_io(&mut tx, root.clone(), &root.resources) {
            io_ctx.cancel_tx(tx);
            Err(Error::ActivateFailure(e))
        } else {
            io_ctx.commit_tx(tx);
            Ok(())
        }
    }

    /// Add a PCI bus instance to be managed by the root device.
    pub fn add_bus(&self, bus: Arc<PciBus>, id: u8) -> Result<()> {
        if id > self.max_bus_id {
            return Err(Error::InvalidBusId(id));
        }
        // Don't expect poisoned lock here.
        self.state
            .write()
            .expect("poisoned lock for PCI root device")
            .buses
            .insert(id as u32, bus);

        Ok(())
    }

    /// Get a PCI bus instance by bus id.
    pub fn get_bus_by_id(&self, id: u8) -> Option<Arc<PciBus>> {
        if id > self.max_bus_id {
            return None;
        }

        // Don't expect poisoned lock here.
        self.state
            .read()
            .expect("poisoned lock for PCI root device")
            .buses
            .get(&(id as u32))
            .cloned()
    }

    /// Get PCI root device resources
    #[cfg(target_arch = "aarch64")]
    pub fn get_device_resources(&self) -> DeviceResources {
        self.resources.clone()
    }
}

impl DeviceIo for PciRootDevice {
    /// At present, only arm will go to this process, because the x86 pci root device does
    /// not allocate mmio resources. This process does not involve the arm proprietary
    /// interface, so it does not use the aarch64 macro to wrap.
    fn read(&self, base: IoAddress, offset: IoAddress, data: &mut [u8]) {
        let offset = offset.raw_value();
        let len = data.len();

        // Only allow naturally aligned Dword, Word and Byte access.
        if (len == 4 || len == 2 || len == 1) && offset as usize & (len - 1) == 0 {
            // Do not expect poisoned lock here.
            let state = self.state.read().unwrap();
            let (b, d, f, o) = parse_mmio_address(offset);
            if let Some(bus) = state.buses.get(&b) {
                return bus.read_config(d, f, o | ((offset as u32) & 0x3), data);
            }
        }

        debug!(
            "Invalid PCI configuration mmio read ({:x}, {})",
            base.raw_value(),
            len
        );

        fill_config_data(data);
    }

    /// Same as the read interface, currently only this interface is used by the arm.
    /// For specific reasons, please refer to the comment of the read interface.
    fn write(&self, base: IoAddress, offset: IoAddress, data: &[u8]) {
        let offset = offset.raw_value();
        let len = data.len();

        if (len == 4 || len == 2 || len == 1) && offset as usize & (len - 1) == 0 {
            // Safe to unwrap because no legal to generate poisoned RwLock.
            let state = self.state.read().unwrap();
            let (b, d, f, o) = parse_mmio_address(offset);
            if let Some(bus) = state.buses.get(&b) {
                return bus.write_config(d, f, o | (offset as u32 & 0x3), data);
            }
        }

        debug!(
            "Invalid PCI configuration mmio write ({:x}, {})",
            base.raw_value(),
            len
        );
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn pio_read(&self, base: PioAddress, offset: PioAddress, data: &mut [u8]) {
        let offset = offset.raw_value();
        let len = data.len();

        // Only allow naturally aligned Dword, Word and Byte access.
        if (len == 4 || len == 2 || len == 1) && offset as usize & (len - 1) == 0 {
            match offset {
                // Configuration address register
                0..=3 => {
                    // Do not expect poisoned lock here.
                    let io_addr = self.state.read().unwrap().io_addr >> (offset << 3);
                    if len == 4 {
                        NativeEndian::write_u32(data, io_addr);
                    } else if len == 2 {
                        NativeEndian::write_u16(data, io_addr as u16);
                    } else {
                        data[0] = io_addr as u8;
                    }
                    debug!("=>read offset {}, and io_addr: 0x{:x}", offset, io_addr);
                    return;
                }
                // Configuration data register
                4..=7 => {
                    // Do not expect poisoned lock here.
                    let state = self.state.read().unwrap();
                    if state.io_addr & 0x8000_0000 != 0 {
                        let (b, d, f, o) = parse_ioport_address(state.io_addr);
                        if let Some(bus) = state.buses.get(&b) {
                            return bus.read_config(d, f, o | ((offset as u32) & 0x3), data);
                        }
                    }
                }
                _ => {}
            }
        }

        debug!(
            "Invalid PCI configuration ioport read ({:x}, {})",
            base.raw_value(),
            len
        );

        fill_config_data(data);
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn pio_write(&self, base: PioAddress, offset: PioAddress, data: &[u8]) {
        let offset = offset.raw_value();
        let len = data.len();
        // Only allow Dword, Word and Byte access.
        if len != 4 && len != 2 && len != 1 {
            return;
        }

        if (len == 4 || len == 2 || len == 1) && offset as usize & (len - 1) == 0 {
            match offset {
                0..=3 => {
                    // Safe to unwrap because no legal to generate poisoned RwLock.
                    let mut state = self.state.write().unwrap();
                    let shift = (offset & 0x3) * 8;
                    if len == 4 {
                        state.io_addr = NativeEndian::read_u32(data);
                    } else if len == 2 {
                        state.io_addr &= !(0xffffu32 << shift);
                        state.io_addr |= u32::from(NativeEndian::read_u16(data)) << shift;
                    } else {
                        state.io_addr &= !(0xffu32 << shift);
                        state.io_addr |= u32::from(data[0]) << shift;
                    }
                    return;
                }
                4..=7 => {
                    // Safe to unwrap because no legal to generate poisoned RwLock.
                    let state = self.state.read().unwrap();
                    if state.io_addr & 0x8000_0000 != 0 {
                        let (b, d, f, o) = parse_ioport_address(state.io_addr);
                        if let Some(bus) = state.buses.get(&b) {
                            return bus.write_config(d, f, o | (offset as u32 & 0x3), data);
                        }
                    }
                }
                _ => {}
            }
        }

        debug!(
            "Invalid PCI configuration ioport write ({:x}, {})",
            base.raw_value(),
            len
        );
    }

    fn get_assigned_resources(&self) -> DeviceResources {
        self.resources.clone()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
// Parse the CONFIG_ADDRESS register to a (bus, device, function, offset) tuple.
fn parse_ioport_address(address: u32) -> (u32, u32, u32, u32) {
    const BUS_NUMBER_OFFSET: u32 = 16;
    const BUS_NUMBER_MASK: u32 = 0x00ff;
    const DEVICE_NUMBER_OFFSET: u32 = 11;
    const DEVICE_NUMBER_MASK: u32 = 0x1f;
    const FUNCTION_NUMBER_OFFSET: u32 = 8;
    const FUNCTION_NUMBER_MASK: u32 = 0x07;
    const REGISTER_NUMBER_OFFSET: u32 = 0;
    const REGISTER_NUMBER_MASK: u32 = 0xff;
    const REGISTER_NUMBER_HI_OFFSET: u32 = 16;
    const REGISTER_NUMBER_HI_MASK: u32 = 0xf00;

    let bus_number = (address >> BUS_NUMBER_OFFSET) & BUS_NUMBER_MASK;
    let device_number = (address >> DEVICE_NUMBER_OFFSET) & DEVICE_NUMBER_MASK;
    let function_number = (address >> FUNCTION_NUMBER_OFFSET) & FUNCTION_NUMBER_MASK;
    let register_number = ((address >> REGISTER_NUMBER_OFFSET) & REGISTER_NUMBER_MASK)
        | ((address >> REGISTER_NUMBER_HI_OFFSET) & REGISTER_NUMBER_HI_MASK);

    (
        bus_number,
        device_number,
        function_number,
        register_number & !0x3u32,
    )
}

/// Decode MMIO address into (bus, dev, func, offset) tuple.
fn parse_mmio_address(address: u64) -> (u32, u32, u32, u32) {
    let addr = address as u32;
    (
        (addr >> 20) & 0xff,
        (addr >> 15) & 0x1f,
        (addr >> 12) & 0x7,
        addr & 0xfff,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use dbs_device::resources::Resource;

    #[test]
    fn test_parse_address() {
        assert_eq!(parse_mmio_address(0x123456), (0x1, 0x4, 0x3, 0x456));
        assert_eq!(parse_mmio_address(0x10123456), (0x1, 0x4, 0x3, 0x456));
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        assert_eq!(parse_ioport_address(0x1234567), (0x23, 0x8, 0x5, 0x164));
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        assert_eq!(parse_ioport_address(0x81234567), (0x23, 0x8, 0x5, 0x164));
    }

    #[test]
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn test_fill_data() {
        let mut buf = vec![0u8; 4];
        fill_config_data(&mut buf);
        assert_eq!(buf, vec![0xffu8; 4]);
    }

    #[test]
    fn test_new_pci_root() {
        let mut resources = DeviceResources::new();
        resources.append(Resource::PioAddressRange {
            base: 0xCF8,
            size: 8,
        });
        let root = PciRootDevice::create(255, resources).unwrap();

        assert_eq!(root.max_bus_id, 255);
        assert_eq!(root.ioport_base, 0xCF8);
        assert_eq!(root.mmio_base, 0);
        assert_eq!(root.mmio_size, 0);
    }

    #[test]
    #[should_panic]
    fn test_new_resource() {
        let resources = DeviceResources::new();
        let _root = PciRootDevice::create(255, resources).unwrap();
    }
}
