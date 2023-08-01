// Copyright (c) 2019 Intel Corporation
// Copyright (c) 2023 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0

use crate::sdt::Sdt;

/// start of IOAPIC
pub const IOAPIC_START: u64 = 0xfec0_0000;
/// start of APIC
pub const APIC_START: u64 = 0xfee0_0000;

/// Values for Type in APIC sub-headers
/// APIC_PROCESSOR
pub const ACPI_APIC_PROCESSOR: u8 = 0;
/// APIC IO
pub const ACPI_APIC_IO: u8 = 1;
/// ACPI XRUPT OVERRIDE
pub const ACPI_APIC_XRUPT_OVERRIDE: u8 = 2;
/// MADT CPU ENABLE FLAG
const MADT_CPU_ENABLE_FLAG: usize = 0;

#[allow(dead_code)]
// InterruptSourceOverride
#[repr(packed)]
#[derive(Default)]
struct InterruptSourceOverride {
    r#type: u8,
    length: u8,
    bus: u8,
    source: u8,
    gsi: u32,
    flags: u16,
}

#[allow(dead_code)]
// LOCAL APIC in MADT
#[repr(packed)]
struct LocalApic {
    r#type: u8,
    length: u8,
    processor_id: u8,
    apic_id: u8,
    flags: u32,
}

#[allow(dead_code)]
// IOAPIC in MADT
#[repr(packed)]
#[derive(Default)]
struct Ioapic {
    r#type: u8,
    length: u8,
    ioapic_id: u8,
    _reserved: u8,
    apic_address: u32,
    gsi_base: u32,
}

// create madt
fn create_madt_table(max_vcpus: u8, boot_vcpus: u8) -> Sdt {
    let mut madt = Sdt::new(*b"APIC", 44, 5);
    madt.write(36, APIC_START);
    for cpu in 0..max_vcpus {
        let lapic = LocalApic {
            r#type: ACPI_APIC_PROCESSOR,
            length: 8,
            processor_id: cpu,
            apic_id: cpu,
            flags: if cpu < boot_vcpus {
                1 << MADT_CPU_ENABLE_FLAG
            } else {
                0
            },
        };
        madt.append(lapic);
    }
    madt.append(Ioapic {
        r#type: ACPI_APIC_IO,
        length: 12,
        ioapic_id: 0,
        apic_address: IOAPIC_START as u32,
        gsi_base: 0,
        ..Default::default()
    });
    for i in 1..16 {
        madt.append(InterruptSourceOverride {
            r#type: ACPI_APIC_XRUPT_OVERRIDE,
            length: 10,
            bus: 0,
            source: i as u8,
            gsi: i as u32,
            flags: 0,
        });
    }
    madt
}

// create a null dsdt acpi tables
fn create_dsdt_table() -> Sdt {
    // DSDT
    let mut dsdt = Sdt::new(*b"DSDT", 36, 6);
    let bytes = Vec::new();
    dsdt.append_slice(&bytes);
    dsdt
}

/// create acpi tables
pub fn create_acpi_tables_tdx(max_vcpu_count: u8, vcpu_count: u8) -> Vec<Sdt> {
    let mut tables: Vec<Sdt> = Vec::new();
    // MADT
    let madt = create_madt_table(max_vcpu_count, vcpu_count);
    tables.push(madt);
    // DSDT
    let dsdt = create_dsdt_table();
    tables.push(dsdt);
    tables
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_sdt() {
        let mut sdt = super::Sdt::new(*b"TEST", 40, 1);
        let sum: u8 = sdt
            .as_slice()
            .iter()
            .fold(0u8, |acc, x| acc.wrapping_add(*x));
        assert_eq!(sum, 0);
        sdt.write_u32(36, 0x12345678);
        let sum: u8 = sdt
            .as_slice()
            .iter()
            .fold(0u8, |acc, x| acc.wrapping_add(*x));
        assert_eq!(sum, 0);
    }
    #[test]
    fn test_acpi_tables() {
        let acpi_tables = super::create_acpi_tables_tdx(4 as u8, 4 as u8);
        assert_eq!(acpi_tables.iter().count(), 2);
    }
}
