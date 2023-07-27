// Copyright (c) 2023 Alibaba Cloud.
// Copyright © 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
use log::info;
/// Hob related functionality.
use vm_memory::{ByteValued, Bytes, GuestAddress, GuestMemoryError, GuestMemoryMmap};
/// HOB Type
#[repr(u16)]
#[derive(Copy, Clone, Debug)]
enum HobType {
    /// Hand Off
    Handoff = 0x1,
    /// Resource Descriptor
    ResourceDescriptor = 0x3,
    /// Guid Extension
    GuidExtension = 0x4,
    /// Unused
    Unused = 0xfffe,
    /// End Of HOB List
    EndOfHobList = 0xffff,
}
/// Default
impl Default for HobType {
    fn default() -> Self {
        HobType::Unused
    }
}
/// HOB header
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct HobHeader {
    r#type: HobType,
    length: u16,
    reserved: u32,
}
/// HOB hand off info table
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct HobHandoffInfoTable {
    header: HobHeader,
    version: u32,
    boot_mode: u32,
    efi_memory_top: u64,
    efi_memory_bottom: u64,
    efi_free_memory_top: u64,
    efi_free_memory_bottom: u64,
    efi_end_of_hob_list: u64,
}
impl HobHandoffInfoTable {
    pub fn new(efi_end_of_hob_list: u64) -> Self {
        HobHandoffInfoTable {
            header: HobHeader {
                r#type: HobType::Handoff,
                length: std::mem::size_of::<HobHandoffInfoTable>() as u16,
                reserved: 0,
            },
            version: 0x9,
            boot_mode: 0,
            efi_memory_top: 0,
            efi_memory_bottom: 0,
            efi_free_memory_top: 0,
            efi_free_memory_bottom: 0,
            efi_end_of_hob_list,
        }
    }
}
/// HOB end
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct HobEnd {
    header: HobHeader,
}
impl HobEnd {
    fn new() -> Self {
        HobEnd {
            header: HobHeader {
                r#type: HobType::EndOfHobList,
                length: std::mem::size_of::<HobEnd>() as u16,
                reserved: 0,
            },
        }
    }
}
/// Efi Guid
#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq)]
struct EfiGuid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}
impl EfiGuid {
    /// RESOURCE_HOB_GUID
    fn resource() -> Self {
        EfiGuid::default()
    }
    /// HOB_PAYLOAD_INFO_GUID
    /// 0xb96fa412, 0x461f, 0x4be3, {0x8c, 0xd, 0xad, 0x80, 0x5a, 0x49, 0x7a, 0xc0
    fn payload() -> Self {
        EfiGuid {
            data1: 0xb96f_a412,
            data2: 0x461f,
            data3: 0x4be3,
            data4: [0x8c, 0xd, 0xad, 0x80, 0x5a, 0x49, 0x7a, 0xc0],
        }
    }
    /// ACPI_TABLE_HOB_GUID
    /// 0x6a0c5870, 0xd4ed, 0x44f4, {0xa1, 0x35, 0xdd, 0x23, 0x8b, 0x6f, 0xc, 0x8d }
    fn acpi() -> Self {
        EfiGuid {
            data1: 0x6a0c_5870,
            data2: 0xd4ed,
            data3: 0x44f4,
            data4: [0xa1, 0x35, 0xdd, 0x23, 0x8b, 0x6f, 0xc, 0x8d],
        }
    }
}
/// HOB resource descriptor
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct HobResourceDescriptor {
    header: HobHeader,
    efi_guid_type: EfiGuid,
    resource_type: u32,
    resource_attribute: u32,
    physical_start: u64,
    resource_length: u64,
}
impl HobResourceDescriptor {
    fn new(
        resource_type: u32,
        resource_attribute: u32,
        physical_start: u64,
        resource_length: u64,
    ) -> Self {
        HobResourceDescriptor {
            header: HobHeader {
                r#type: HobType::ResourceDescriptor,
                length: std::mem::size_of::<HobResourceDescriptor>() as u16,
                reserved: 0,
            },
            efi_guid_type: EfiGuid::resource(),
            resource_type,
            resource_attribute,
            physical_start,
            resource_length,
        }
    }
}
#[repr(u32)]
#[derive(Clone, Copy, Debug)]
pub enum PayloadImageType {
    ExecutablePayload,
    BzImage,
    RawVmLinux,
}
impl Default for PayloadImageType {
    fn default() -> Self {
        PayloadImageType::ExecutablePayload
    }
}
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
pub struct PayloadInfo {
    pub image_type: PayloadImageType,
    pub entry_point: u64,
}
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct TdPayloadDescription {
    header: HobHeader,
    efi_guid_type: EfiGuid,
    payload_info: PayloadInfo,
}
impl TdPayloadDescription {
    fn new(payload: PayloadInfo) -> Self {
        TdPayloadDescription {
            header: HobHeader {
                r#type: HobType::GuidExtension,
                length: std::mem::size_of::<TdPayloadDescription>() as u16,
                reserved: 0,
            },
            efi_guid_type: EfiGuid::payload(),
            payload_info: payload,
        }
    }
}
/// ACPi
#[repr(C)]
#[derive(Copy, Clone, Default, Debug)]
struct ACPIDescription {
    header: HobHeader,
    efi_guid_type: EfiGuid,
}
impl ACPIDescription {
    fn new(length: u16) -> Self {
        ACPIDescription {
            header: HobHeader {
                r#type: HobType::GuidExtension,
                length,
                reserved: 0,
            },
            // ACPI_TABLE_HOB_GUID
            efi_guid_type: EfiGuid::acpi(),
        }
    }
}
// SAFETY: These data structures only contain a series of integers
unsafe impl ByteValued for HobHeader {}
unsafe impl ByteValued for HobHandoffInfoTable {}
unsafe impl ByteValued for HobResourceDescriptor {}
unsafe impl ByteValued for TdPayloadDescription {}
unsafe impl ByteValued for ACPIDescription {}
unsafe impl ByteValued for HobEnd {}
/// TD HOB
pub struct TdHob {
    start_offset: u64,
    current_offset: u64,
}
fn align_hob(v: u64) -> u64 {
    (v + 7) / 8 * 8
}
impl TdHob {
    /// update offset to align with 8 bytes
    fn update_offset<T>(&mut self) {
        self.current_offset = align_hob(self.current_offset + std::mem::size_of::<T>() as u64)
    }
    /// start wirting hot list
    pub fn start(offset: u64) -> TdHob {
        // Leave a gap to place the HandoffTable at the start as it can only be filled in later
        let mut hob = TdHob {
            start_offset: offset,
            current_offset: offset,
        };
        hob.update_offset::<HobHandoffInfoTable>();
        hob
    }
    /// finish writing hot list
    pub fn finish(&mut self, mem: &GuestMemoryMmap) -> Result<(), GuestMemoryError> {
        // Write end
        let end = HobEnd::new();
        info!("Writing HOB end {:x} {:x?}", self.current_offset, end);
        mem.write_obj(end, GuestAddress(self.current_offset))?;
        self.update_offset::<HobEnd>();
        // Write handoff, delayed as it needs end of HOB list
        let efi_end_of_hob_list = self.current_offset;
        let handoff = HobHandoffInfoTable::new(efi_end_of_hob_list);
        info!("Writing HOB start {:x} {:x?}", self.start_offset, handoff);
        mem.write_obj(handoff, GuestAddress(self.start_offset))
    }
    /// Add resource to TD HOB
    pub fn add_resource(
        &mut self,
        mem: &GuestMemoryMmap,
        physical_start: u64,
        resource_length: u64,
        resource_type: u32,
        resource_attribute: u32,
    ) -> Result<(), GuestMemoryError> {
        let resource_descriptor = HobResourceDescriptor::new(
            resource_type,
            resource_attribute,
            physical_start,
            resource_length,
        );
        info!(
            "Writing HOB resource {:x} {:x?}",
            self.current_offset, resource_descriptor
        );
        mem.write_obj(resource_descriptor, GuestAddress(self.current_offset))?;
        self.update_offset::<HobResourceDescriptor>();
        Ok(())
    }
    /// Add memory resource
    pub fn add_memory_resource(
        &mut self,
        mem: &GuestMemoryMmap,
        physical_start: u64,
        resource_length: u64,
        ram: bool,
    ) -> Result<(), GuestMemoryError> {
        self.add_resource(
            mem,
            physical_start,
            resource_length,
            if ram {
                0x7 // EFI_RESOURCE_MEMORY_UNACCEPT
            } else {
                0x0 // EFI_RESOURCE_SYSTEM_MEMORY
            },

            // TODO:
            // QEMU currently fills it in like this:
            // EFI_RESOURCE_ATTRIBUTE_PRESENT | EFI_RESOURCE_ATTRIBUTE_INITIALIZED|EFI_RESOURCE_ATTRIBUTE_ENCRYPTED  | EFI_RESOURCE_ATTRIBUTE_TESTED
            // which differs from the spec (due to TDVF implementation issue?)
            0x07,
        )
    }
    /// Add mmio resource
    pub fn add_mmio_resource(
        &mut self,
        mem: &GuestMemoryMmap,
        physical_start: u64,
        resource_length: u64,
    ) -> Result<(), GuestMemoryError> {
        self.add_resource(
            mem,
            physical_start,
            resource_length,
            0x1,   // EFI_RESOURCE_MEMORY_MAPPED_IO
            0x403, // EFI_RESOURCE_ATTRIBUTE_PRESENT | EFI_RESOURCE_ATTRIBUTE_INITIALIZED | EFI_RESOURCE_ATTRIBUTE_UNCACHEABLE
        )
    }
    /// Add payload
    pub fn add_payload(
        &mut self,
        mem: &GuestMemoryMmap,
        payload_info: PayloadInfo,
    ) -> Result<(), GuestMemoryError> {
        let payload = TdPayloadDescription::new(payload_info);
        info!(
            "Writing HOB TD_PAYLOAD {:x} {:x?}",
            self.current_offset, payload
        );
        mem.write_obj(payload, GuestAddress(self.current_offset))?;
        self.update_offset::<TdPayloadDescription>();
        Ok(())
    }
    pub fn add_acpi_table(
        &mut self,
        mem: &GuestMemoryMmap,
        table_content: &[u8],
    ) -> Result<(), GuestMemoryError> {
        // We already know the HobGuidType size is 8 bytes multiple, but we
        // need the total size to be 8 bytes multiple. That is why the ACPI
        // table size must be 8 bytes multiple as well.
        let length = std::mem::size_of::<ACPIDescription>() as u16
            + align_hob(table_content.len() as u64) as u16;
        let hob_guid_type = ACPIDescription::new(length);
        info!(
            "Writing HOB ACPI table {:x} {:x?} {:x?}",
            self.current_offset, hob_guid_type, table_content
        );
        mem.write_obj(hob_guid_type, GuestAddress(self.current_offset))?;
        let current_offset = self.current_offset + std::mem::size_of::<ACPIDescription>() as u64;
        // In case the table is quite large, let's make sure we can handle
        // retrying until everything has been correctly copied.
        let mut offset: usize = 0;
        loop {
            let bytes_written = mem.write(
                &table_content[offset..],
                GuestAddress(current_offset + offset as u64),
            )?;
            offset += bytes_written;
            if offset >= table_content.len() {
                break;
            }
        }
        self.current_offset += length as u64;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use crate::td_shim::hob::*;
    #[test]
    fn test_align_hob() {
        assert_eq!(align_hob(0 as u64), 0);
        assert_eq!(align_hob(7 as u64), 1 * 8);
        assert_eq!(align_hob(8 as u64), 1 * 8);
        assert_eq!(align_hob(9 as u64), 2 * 8);
        assert_eq!(align_hob(175 as u64), 22 * 8);
    }
    #[test]
    fn test_payload_description() {
        let payload = PayloadInfo {
            image_type: PayloadImageType::RawVmLinux,
            entry_point: 0x100000,
        };
        // test len of payload_description
        let payload_description = TdPayloadDescription::new(payload.clone());
        let payload_guid = EfiGuid {
            data1: 0xb96f_a412,
            data2: 0x461f,
            data3: 0x4be3,
            data4: [0x8c, 0xd, 0xad, 0x80, 0x5a, 0x49, 0x7a, 0xc0],
        };
        // check guid
        assert_eq!(payload_description.efi_guid_type, payload_guid);
        // check length
        let length: u16 = std::mem::size_of::<TdPayloadDescription>() as u16;
        assert_eq!(payload_description.header.length, length);
    }
    #[test]
    fn test_resource_description() {
        let resource_description = HobResourceDescriptor::new(0x0, 0x0, 0x12345678, 0x100000);
        // check guid
        assert_eq!(resource_description.efi_guid_type, EfiGuid::default());
        // check length
        let length: u16 = std::mem::size_of::<HobResourceDescriptor>() as u16;
        assert_eq!(resource_description.header.length, length);
    }
    #[test]
    fn test_acpi_description() {
        // test len of payload_descr
        let acpi_description = ACPIDescription::new(0x80);
        let guid = EfiGuid {
            data1: 0x6a0c_5870,
            data2: 0xd4ed,
            data3: 0x44f4,
            data4: [0xa1, 0x35, 0xdd, 0x23, 0x8b, 0x6f, 0xc, 0x8d],
        };
        // check guid
        assert_eq!(acpi_description.efi_guid_type, guid);
        // length is variable,  do not check it
    }
}