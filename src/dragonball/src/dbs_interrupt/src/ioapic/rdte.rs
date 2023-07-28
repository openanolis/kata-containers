// Copyright 2019 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.
//
// Copyright © 2019 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0 AND BSD-3-Clause
//
// Part of implementation of an intel 82093AA Input/Output Advanced Programmable Interrupt Controller
// See https://pdos.csail.mit.edu/6.828/2016/readings/ia32/ioapic.pdf for a specification.
// I/O REDIRECTION TABLE REGISTER
//
// There are 24 I/O Redirection Table entry registers. Each register is a
// dedicated entry for each interrupt input signal. Each register is 64 bits
// split between two 32 bits registers as follow:
//
// 63-56: Destination Field - R/W
// 55-17: Reserved
// 16:    Interrupt Mask - R/W
// 15:    Trigger Mode - R/W
// 14:    Remote IRR - RO
// 13:    Interrupt Input Pin Polarity - R/W
// 12:    Delivery Status - RO
// 11:    Destination Mode - R/W
// 10-8:  Delivery Mode - R/W
// 7-0:   Interrupt Vector - R/W
/// Redirect table entry
pub type RedirectionTableEntry = u64;

pub fn vector(entry: RedirectionTableEntry) -> u8 {
    (entry & 0xffu64) as u8
}

pub fn delivery_mode(entry: RedirectionTableEntry) -> u8 {
    ((entry >> 8) & 0x7u64) as u8
}

pub fn destination_mode(entry: RedirectionTableEntry) -> u8 {
    ((entry >> 11) & 0x1u64) as u8
}
pub fn remote_irr(entry: RedirectionTableEntry) -> u8 {
    ((entry >> 14) & 0x1u64) as u8
}

pub fn trigger_mode(entry: RedirectionTableEntry) -> u8 {
    ((entry >> 15) & 0x1u64) as u8
}

pub fn interrupt_mask(entry: RedirectionTableEntry) -> u8 {
    ((entry >> 16) & 0x1u64) as u8
}

pub fn destination_field(entry: RedirectionTableEntry) -> u8 {
    // When the destination mode is physical, the destination field should only
    // be defined through bits 56-59, as defined in the IOAPIC specification.
    // But from the APIC specification, the APIC ID is always defined on 8 bits::
    // no matter which destination mode is selected. That's why we always
    // retrieve the destination field based on bits 56-63.
    ((entry >> 56) & 0xffu64) as u8
}

pub fn set_delivery_status(entry: &mut RedirectionTableEntry, val: u8) {
    // Clear bit 12
    *entry &= 0xffff_ffff_ffff_efff;
    // Set it with the expected value
    *entry |= u64::from(val & 0x1) << 12;
}

pub fn set_remote_irr(entry: &mut RedirectionTableEntry, val: u8) {
    // Clear bit 14
    *entry &= 0xffff_ffff_ffff_bfff;
    // Set it with the expected value
    *entry |= u64::from(val & 0x1) << 14;
}

pub fn set_interrupt_mask(entry: &mut RedirectionTableEntry, val: u8) {
    // Clear bit 16
    *entry &= 0xffff_ffff_fffe_ffff;
    // Set it with the expected value
    *entry |= u64::from(val & 0x1) << 16;
}

#[cfg(test)]
mod tests {
    use crate::ioapic::rdte::*;
    #[test]
    fn test_rdte() {
        let entry: RedirectionTableEntry = 0x0;
        assert_eq!(vector(entry), 0x0 as u8);
        assert_eq!(delivery_mode(entry), 0x0 as u8);
        assert_eq!(destination_mode(entry), 0x0 as u8);
        assert_eq!(remote_irr(entry), 0x0 as u8);
        assert_eq!(trigger_mode(entry), 0x0 as u8);
        assert_eq!(interrupt_mask(entry), 0x0 as u8);
        assert_eq!(destination_field(entry), 0x0 as u8);
        let entry: RedirectionTableEntry = 0xffff_ffff_ffff_ffff;
        assert_eq!(vector(entry), 0xff as u8);
        assert_eq!(delivery_mode(entry), 0x7 as u8);
        assert_eq!(destination_mode(entry), 0x1 as u8);
        assert_eq!(remote_irr(entry), 0x1 as u8);
        assert_eq!(trigger_mode(entry), 0x1 as u8);
        assert_eq!(interrupt_mask(entry), 0x1 as u8);
        assert_eq!(destination_field(entry), 0xff as u8);
    }
    #[test]
    fn test_set_delivery_status() {
        let mut entry: RedirectionTableEntry = 0xffff_0000;
        set_delivery_status(&mut entry, 0x1u8);
        assert_eq!(entry, 0xffff_1000 as u64);
        let mut entry_1: RedirectionTableEntry = 0xffff_ffff;
        set_delivery_status(&mut entry_1, 0x0u8);
        assert_eq!(entry_1, 0xffff_efff as u64);
    }
    #[test]
    fn test_set_remote_irr() {
        let mut entry: RedirectionTableEntry = 0xffff_0000;
        set_remote_irr(&mut entry, 0x1u8);
        assert_eq!(entry, 0xffff_4000 as u64);
        let mut entry_1: RedirectionTableEntry = 0xffff_ffff;
        set_remote_irr(&mut entry_1, 0x0u8);
        assert_eq!(entry_1, 0xffff_bfff as u64);
    }
    #[test]
    fn test_set_interrupt_mask() {
        let mut entry: RedirectionTableEntry = 0xfff0_0000;
        set_interrupt_mask(&mut entry, 0x1u8);
        assert_eq!(entry, 0xfff1_0000 as u64);
        let mut entry_1: RedirectionTableEntry = 0xffff_ffff;
        set_interrupt_mask(&mut entry_1, 0x0u8);
        assert_eq!(entry_1, 0xfffe_ffff as u64);
    }
}
