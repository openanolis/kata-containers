// Copyright (C) 2020-2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0
//
// Portions Copyright 2017 The Chromium OS Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license that can be
// found in the THIRD-PARTY file.

use std::convert::TryInto;
#[cfg(any(feature = "tdx", feature = "sev"))]
use std::io::{Seek, SeekFrom};
use std::mem;
use std::ops::Deref;
#[cfg(any(feature = "tdx", feature = "sev"))]
use std::os::unix::io::AsRawFd;

#[cfg(any(feature = "tdx", feature = "sev"))]
use dbs_acpi::acpi::create_acpi_tables_tdx;
use dbs_address_space::AddressSpace;
#[cfg(any(feature = "tdx", feature = "sev"))]
use dbs_address_space::AddressSpaceRegionType;
use dbs_boot::{add_e820_entry, bootparam, layout, mptable, BootParamsWrapper, InitrdConfig};
#[cfg(any(feature = "tdx", feature = "sev"))]
use dbs_tdx::td_shim::{
    hob::{PayloadImageType, PayloadInfo},
    metadata::{TdvfSection, TdvfSectionType},
    TD_SHIM_START,
};
#[cfg(feature = "tdx")]
use dbs_tdx::tdx_ioctls::{tdx_finalize, tdx_init, tdx_init_memory_region};
use dbs_utils::epoll_manager::EpollManager;
use dbs_utils::time::TimestampUs;
#[cfg(all(target_arch = "x86_64", feature = "userspace-ioapic"))]
use kvm_bindings::{kvm_enable_cap, KVM_CAP_SPLIT_IRQCHIP};
use kvm_bindings::{kvm_irqchip, kvm_pit_config, kvm_pit_state2, KVM_PIT_SPEAKER_DUMMY};
use linux_loader::cmdline::Cmdline;
#[cfg(feature = "sev")]
use sev::{firmware::host::Firmware as SevFirmware, launch::sev as sev_launch};
use slog::info;
#[cfg(any(feature = "tdx", feature = "sev"))]
use vm_memory::ByteValued;
use vm_memory::{Address, Bytes, GuestAddress, GuestAddressSpace, GuestMemory, VolatileMemory};

#[cfg(any(feature = "tdx", feature = "sev"))]
use crate::address_space_manager::AddressManagerError;
use crate::address_space_manager::{GuestAddressSpaceImpl, GuestMemoryImpl};
#[cfg(feature = "sev")]
use crate::api::v1::{InstanceState, VmStartingStage};
#[cfg(any(feature = "tdx", feature = "sev"))]
use crate::error::LoadTdDataError;
use crate::error::{Error, Result, StartMicroVmError};
use crate::event_manager::EventManager;
use crate::vm::{Vm, VmError};

/// Configures the system and should be called once per vm before starting vcpu
/// threads.
///
/// # Arguments
///
/// * `guest_mem` - The memory to be used by the guest.
/// * `cmdline_addr` - Address in `guest_mem` where the kernel command line was
///   loaded.
/// * `cmdline_size` - Size of the kernel command line in bytes including the
///   null terminator.
/// * `initrd` - Information about where the ramdisk image was loaded in the
///   `guest_mem`.
/// * `boot_cpus` - Number of virtual CPUs the guest will have at boot time.
/// * `max_cpus` - Max number of virtual CPUs the guest will have.
/// * `rsv_mem_bytes` - Reserve memory from microVM..
#[allow(clippy::too_many_arguments)]
fn configure_system<M: GuestMemory>(
    guest_mem: &M,
    address_space: Option<&AddressSpace>,
    cmdline_addr: GuestAddress,
    cmdline_size: usize,
    initrd: &Option<InitrdConfig>,
    boot_cpus: u8,
    max_cpus: u8,
) -> super::Result<()> {
    const KERNEL_BOOT_FLAG_MAGIC: u16 = 0xaa55;
    const KERNEL_HDR_MAGIC: u32 = 0x5372_6448;
    const KERNEL_LOADER_OTHER: u8 = 0xff;
    const KERNEL_MIN_ALIGNMENT_BYTES: u32 = 0x0100_0000; // Must be non-zero.

    let mmio_start = GuestAddress(layout::MMIO_LOW_START);
    let mmio_end = GuestAddress(layout::MMIO_LOW_END);
    let himem_start = GuestAddress(layout::HIMEM_START);

    // Note that this puts the mptable at the last 1k of Linux's 640k base RAM
    mptable::setup_mptable(guest_mem, boot_cpus, max_cpus).map_err(Error::MpTableSetup)?;

    let mut params: BootParamsWrapper = BootParamsWrapper(bootparam::boot_params::default());

    params.0.hdr.type_of_loader = KERNEL_LOADER_OTHER;
    params.0.hdr.boot_flag = KERNEL_BOOT_FLAG_MAGIC;
    params.0.hdr.header = KERNEL_HDR_MAGIC;
    params.0.hdr.cmd_line_ptr = cmdline_addr.raw_value() as u32;
    params.0.hdr.cmdline_size = cmdline_size as u32;
    params.0.hdr.kernel_alignment = KERNEL_MIN_ALIGNMENT_BYTES;
    if let Some(initrd_config) = initrd {
        params.0.hdr.ramdisk_image = initrd_config.address.raw_value() as u32;
        params.0.hdr.ramdisk_size = initrd_config.size as u32;
    }

    add_e820_entry(&mut params.0, 0, layout::EBDA_START, bootparam::E820_RAM)
        .map_err(Error::BootSystem)?;

    let mem_end = address_space.ok_or(Error::AddressSpace)?.last_addr();
    if mem_end < mmio_start {
        add_e820_entry(
            &mut params.0,
            himem_start.raw_value(),
            // it's safe to use unchecked_offset_from because
            // mem_end > himem_start
            mem_end.unchecked_offset_from(himem_start) + 1,
            bootparam::E820_RAM,
        )
        .map_err(Error::BootSystem)?;
    } else {
        add_e820_entry(
            &mut params.0,
            himem_start.raw_value(),
            // it's safe to use unchecked_offset_from because
            // end_32bit_gap_start > himem_start
            mmio_start.unchecked_offset_from(himem_start),
            bootparam::E820_RAM,
        )
        .map_err(Error::BootSystem)?;
        if mem_end > mmio_end {
            add_e820_entry(
                &mut params.0,
                mmio_end.raw_value() + 1,
                // it's safe to use unchecked_offset_from because mem_end > mmio_end
                mem_end.unchecked_offset_from(mmio_end),
                bootparam::E820_RAM,
            )
            .map_err(Error::BootSystem)?;
        }
    }

    let zero_page_addr = GuestAddress(layout::ZERO_PAGE_START);
    guest_mem
        .checked_offset(zero_page_addr, mem::size_of::<bootparam::boot_params>())
        .ok_or(Error::ZeroPagePastRamEnd)?;
    guest_mem
        .write_obj(params, zero_page_addr)
        .map_err(|_| Error::ZeroPageSetup)?;

    Ok(())
}

impl Vm {
    /// Get the status of in-kernel PIT.
    pub fn get_pit_state(&self) -> Result<kvm_pit_state2> {
        self.vm_fd
            .get_pit2()
            .map_err(|e| Error::Vm(VmError::Irq(e)))
    }

    /// Set the status of in-kernel PIT.
    pub fn set_pit_state(&self, pit_state: &kvm_pit_state2) -> Result<()> {
        self.vm_fd
            .set_pit2(pit_state)
            .map_err(|e| Error::Vm(VmError::Irq(e)))
    }

    /// Get the status of in-kernel ioapic.
    pub fn get_irqchip_state(&self, chip_id: u32) -> Result<kvm_irqchip> {
        let mut irqchip: kvm_irqchip = kvm_irqchip {
            chip_id,
            ..kvm_irqchip::default()
        };
        self.vm_fd
            .get_irqchip(&mut irqchip)
            .map(|_| irqchip)
            .map_err(|e| Error::Vm(VmError::Irq(e)))
    }

    /// Set the status of in-kernel ioapic.
    pub fn set_irqchip_state(&self, irqchip: &kvm_irqchip) -> Result<()> {
        self.vm_fd
            .set_irqchip(irqchip)
            .map_err(|e| Error::Vm(VmError::Irq(e)))
    }
}

impl Vm {
    /// Initialize the virtual machine instance.
    ///
    /// It initialize the virtual machine instance by:
    /// 1) initialize virtual machine global state and configuration.
    /// 2) create system devices, such as interrupt controller, PIT etc.
    /// 3) create and start IO devices, such as serial, console, block, net, vsock etc.
    /// 4) create and initialize vCPUs.
    /// 5) configure CPU power management features.
    /// 6) load guest kernel image.
    pub fn init_microvm(
        &mut self,
        epoll_mgr: EpollManager,
        vm_as: GuestAddressSpaceImpl,
        request_ts: TimestampUs,
    ) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM: start initializing microvm ...");

        self.init_tss()?;
        // For x86_64 we need to create the interrupt controller before calling `KVM_CREATE_VCPUS`
        // while on aarch64 we need to do it the other way around.
        #[cfg(all(target_arch = "x86_64", feature = "userspace-ioapic"))]
        {
            if self.vm_config.userspace_ioapic_enabled {
                self.setup_split_irqchips()?;
                self.device_manager.set_userspace_ioapic_enabled(true);
                self.init_devices(epoll_mgr)?;
                let interrupt_controller = self.device_manager.ioapic_manager.get_device().unwrap();
                self.vcpu_manager()
                    .map_err(StartMicroVmError::Vcpu)?
                    .set_interrupt_controller(interrupt_controller);
            } else {
                self.setup_interrupt_controller()?;
                self.create_pit()?;
                self.init_devices(epoll_mgr)?;
            }
        }
        #[cfg(not(all(target_arch = "x86_64", feature = "userspace-ioapic")))]
        {
            self.setup_interrupt_controller()?;
            self.create_pit()?;
            self.init_devices(epoll_mgr)?;
        }

        let reset_event_fd = self.device_manager.get_reset_eventfd().unwrap();
        self.vcpu_manager()
            .map_err(StartMicroVmError::Vcpu)?
            .set_reset_event_fd(reset_event_fd)
            .map_err(StartMicroVmError::Vcpu)?;

        if self.vm_config.cpu_pm == "on" {
            // TODO: add cpu_pm support. issue #4590.
            info!(self.logger, "VM: enable CPU disable_idle_exits capability");
        }

        info!(
            self.logger,
            "VM: checking if it is confidential microvm ..."
        );

        if self.is_tdx_enabled() {
            info!(self.logger, "Intel Trusted Domain microVM");

            #[cfg(feature = "tdx")]
            return self.init_tdx_microvm(vm_as);
            #[cfg(not(feature = "tdx"))]
            return Err(StartMicroVmError::TdxNotSupported);
        } else if self.is_sev_enabled() {
            info!(self.logger, "AMD SEV microVM");

            #[cfg(feature = "sev")]
            return self.init_sev_microvm(vm_as);
            #[cfg(not(feature = "sev"))]
            return Err(StartMicroVmError::SevNotSupported);
        } else {
            info!(self.logger, "None-confidential microvm");

            let vm_memory = vm_as.memory();
            let kernel_loader_result = self.load_kernel(vm_memory.deref(), None)?;
            self.vcpu_manager()
                .map_err(StartMicroVmError::Vcpu)?
                .create_boot_vcpus(request_ts, kernel_loader_result.kernel_load)
                .map_err(StartMicroVmError::Vcpu)?;

            info!(self.logger, "VM: initializing microvm done");
            Ok(())
        }
    }

    /// Complete the remaining VM initialization work.
    #[inline]
    pub fn init_microvm_rest(&mut self) -> std::result::Result<(), StartMicroVmError> {
        #[cfg(feature = "sev")]
        if self.is_sev_enabled() {
            return self.init_sev_microvm_rest();
        }

        Ok(())
    }

    /// Execute system architecture specific configurations.
    ///
    /// 1) set guest kernel boot parameters
    /// 2) setup BIOS configuration data structs, mainly implement the MPSpec.
    pub fn configure_system_arch(
        &self,
        vm_memory: &GuestMemoryImpl,
        cmdline: &Cmdline,
        initrd: Option<InitrdConfig>,
    ) -> std::result::Result<(), StartMicroVmError> {
        let cmdline_addr = GuestAddress(dbs_boot::layout::CMDLINE_START);
        linux_loader::loader::load_cmdline(vm_memory, cmdline_addr, cmdline)
            .map_err(StartMicroVmError::LoadCommandline)?;

        let cmdline_size = cmdline
            .as_cstring()
            .map_err(StartMicroVmError::ProcessCommandlne)?
            .as_bytes_with_nul()
            .len();

        configure_system(
            vm_memory,
            self.address_space.address_space(),
            cmdline_addr,
            cmdline_size,
            &initrd,
            self.vm_config.vcpu_count,
            self.vm_config.max_vcpu_count,
        )
        .map_err(StartMicroVmError::ConfigureSystem)
    }

    /// Initializes the guest memory.
    pub(crate) fn init_tss(&mut self) -> std::result::Result<(), StartMicroVmError> {
        self.vm_fd
            .set_tss_address(dbs_boot::layout::KVM_TSS_ADDRESS.try_into().unwrap())
            .map_err(|e| StartMicroVmError::ConfigureVm(VmError::VmSetup(e)))
    }

    /// Creates the irq chip and an in-kernel device model for the PIT.
    pub(crate) fn setup_interrupt_controller(
        &mut self,
    ) -> std::result::Result<(), StartMicroVmError> {
        self.vm_fd
            .create_irq_chip()
            .map_err(|e| StartMicroVmError::ConfigureVm(VmError::VmSetup(e)))
    }

    /// Creates an in-kernel device model for the PIT.
    pub(crate) fn create_pit(&self) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM: create pit");
        // We need to enable the emulation of a dummy speaker port stub so that writing to port 0x61
        // (i.e. KVM_SPEAKER_BASE_ADDRESS) does not trigger an exit to user space.
        let pit_config = kvm_pit_config {
            flags: KVM_PIT_SPEAKER_DUMMY,
            ..kvm_pit_config::default()
        };

        // Safe because we know that our file is a VM fd, we know the kernel will only read the
        // correct amount of memory from our pointer, and we verify the return result.
        self.vm_fd
            .create_pit2(pit_config)
            .map_err(|e| StartMicroVmError::ConfigureVm(VmError::VmSetup(e)))
    }

    #[cfg(all(target_arch = "x86_64", feature = "userspace-ioapic"))]
    /// Creates spilt irq chips
    pub(crate) fn setup_split_irqchips(&mut self) -> std::result::Result<(), StartMicroVmError> {
        let mut cap = kvm_enable_cap {
            cap: KVM_CAP_SPLIT_IRQCHIP,
            ..Default::default()
        };
        //cap.args[0] = NUM_IOAPIC_PINS as u64;
        cap.args[0] = 24u64;
        self.vm_fd
            .enable_cap(&cap)
            .map_err(|e| StartMicroVmError::ConfigureVm(VmError::VmSetup(e)))
    }

    pub(crate) fn register_events(
        &mut self,
        event_mgr: &mut EventManager,
    ) -> std::result::Result<(), StartMicroVmError> {
        let reset_evt = self
            .device_manager
            .get_reset_eventfd()
            .map_err(StartMicroVmError::DeviceManager)?;
        event_mgr
            .register_exit_eventfd(&reset_evt)
            .map_err(|_| StartMicroVmError::RegisterEvent)?;
        self.reset_eventfd = Some(reset_evt);

        Ok(())
    }
}

#[cfg(feature = "tdx")]
impl Vm {
    /// Init TD
    fn init_tdx(&self) -> std::result::Result<(), StartMicroVmError> {
        let cpuid = self.vcpu_manager().unwrap().supported_cpuid.clone();
        let max_vcpu_count = self.vm_config().max_vcpu_count as u32;
        tdx_init(&self.vm_fd().as_raw_fd(), &cpuid, max_vcpu_count)
            .map_err(StartMicroVmError::TdxIoctlError)?;
        Ok(())
    }
    /// Finalize TD
    fn finalize_tdx(&self) -> std::result::Result<(), StartMicroVmError> {
        tdx_finalize(&self.vm_fd().as_raw_fd()).map_err(StartMicroVmError::TdxIoctlError)?;
        Ok(())
    }
    // TODO: remove dead code here
    #[allow(dead_code)]
    /// Init TDX memory
    fn init_tdx_memory(
        &mut self,
        host_address: u64,
        guest_address: u64,
        size: u64,
        measure: bool,
    ) -> std::result::Result<(), StartMicroVmError> {
        tdx_init_memory_region(
            &self.vm_fd().as_raw_fd(),
            host_address,
            guest_address,
            size,
            measure,
        )
        .map_err(StartMicroVmError::TdxIoctlError)?;
        Ok(())
    }
    /// Initialize the Intel trusted domian instance.
    ///
    /// It initialize the TD by:
    /// 1) initialize TD
    /// 2) initialize virtual machine global state and configuration(TODO).
    /// 2) create system devices, such as interrupt controller, PIT etc(TODO).
    /// 3) create and start IO devices, such as serial, console, block, net, vsock etc(TODO).
    /// 4) create and initialize vCPUs.
    /// 5) configure CPU power management features.(TODO)
    /// 6) load guest kernel image.(TODO)
    /// 7) add memory region fot TD
    /// 8) finalize TD
    pub fn init_tdx_microvm(
        &mut self,
        vm_as: GuestAddressSpaceImpl,
    ) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM: start initializing tdx microvm ...");
        // init TD before create vcpu
        self.init_tdx()?;
        // create vcpus
        info!(self.logger, "create boot vcpus");
        let boot_vcpu_count = self.vm_config().vcpu_count;
        let max_vcpu_count = self.vm_config().max_vcpu_count;
        self.vcpu_manager()
            .map_err(StartMicroVmError::Vcpu)?
            .create_vcpus(boot_vcpu_count, None, None)
            .map_err(StartMicroVmError::Vcpu)?;

        let vm_memory = vm_as.memory();
        // load firmware to memory
        let sections = self.parse_tdvf_sections()?;
        let (hob_offset, payload_offset, payload_size, cmdline_offset) =
            self.load_firmware(vm_memory.deref(), &sections)?;
        // load payload info to memory
        let payload_info =
            self.load_bzimage_payload(payload_offset, payload_size, vm_memory.deref())?;
        self.load_cmdline(cmdline_offset, vm_memory.deref())?;

        // init vcpus
        self.vcpu_manager()
            .map_err(StartMicroVmError::Vcpu)?
            .init_tdx_vcpus(hob_offset)
            .map_err(StartMicroVmError::Vcpu)?;

        let acpi_tables = create_acpi_tables_tdx(max_vcpu_count, boot_vcpu_count);

        let address_space =
            self.vm_address_space()
                .cloned()
                .ok_or(StartMicroVmError::GuestMemory(
                    AddressManagerError::GuestMemoryNotInitialized,
                ))?;

        // generate hob list
        self.generate_hob_list(
            hob_offset,
            vm_memory.deref(),
            address_space,
            payload_info,
            &acpi_tables,
        )
        .map_err(LoadTdDataError::LoadData)
        .map_err(StartMicroVmError::TdDataLoader)?;
        // init(accept) memory regions
        for section in sections {
            let host_address = vm_memory
                .deref()
                .get_host_address(GuestAddress(section.address))
                .unwrap();
            self.init_tdx_memory(
                host_address as u64,
                section.address,
                section.size,
                section.attributes == 1,
            )?;
        }

        self.finalize_tdx()?;
        info!(self.logger, "VM: initializing tdx microvm done");
        Ok(())
    }
}

impl Vm {
    /// Parse firmware metadata
    #[cfg(any(feature = "tdx", feature = "sev"))]
    pub fn parse_tdvf_sections(
        &mut self,
    ) -> std::result::Result<Vec<TdvfSection>, StartMicroVmError> {
        let kernel_config = self
            .kernel_config
            .as_mut()
            .ok_or(StartMicroVmError::MissingKernelConfig)?;
        // safe to unwarap here as we alredy checked when configuring boot source
        let firmware_file = kernel_config.firmware_file_mut().unwrap();
        dbs_tdx::td_shim::metadata::parse_tdvf_sections(firmware_file)
            .map_err(LoadTdDataError::ParseTdshim)
            .map_err(StartMicroVmError::TdDataLoader)
    }

    /// Load data in firmware image to memory
    #[cfg(any(feature = "tdx", feature = "sev"))]
    #[allow(unused)]
    pub fn load_firmware(
        &mut self,
        vm_memory: &GuestMemoryImpl,
        sections: &[TdvfSection],
    ) -> std::result::Result<(u64, u64, u64, u64), StartMicroVmError> {
        let mut hob_offset: Option<u64> = None;
        let mut payload_offset: Option<u64> = None;
        let mut payload_size: Option<u64> = None;
        let mut cmdline_offset: Option<u64> = None;
        let kernel_config = self
            .kernel_config
            .as_mut()
            .ok_or(StartMicroVmError::MissingKernelConfig)?;
        // safe to unwarap here as we alredy checked when configuring boot source
        let firmware_file = kernel_config.firmware_file_mut().unwrap();
        for section in sections {
            info!(self.logger, "TDVF Section: {:x?}", section);
            match section.r#type {
                TdvfSectionType::Bfv | TdvfSectionType::Cfv => {
                    firmware_file
                        .seek(SeekFrom::Start(section.data_offset as u64))
                        .map_err(LoadTdDataError::ReadTdshim)
                        .map_err(StartMicroVmError::TdDataLoader)?;
                    vm_memory
                        .read_from(
                            GuestAddress(section.address),
                            firmware_file,
                            section.data_size as usize,
                        )
                        .map_err(LoadTdDataError::LoadData)
                        .map_err(StartMicroVmError::TdDataLoader)?;
                }
                TdvfSectionType::TdHob => {
                    hob_offset = Some(section.address);
                }
                TdvfSectionType::Payload => {
                    payload_offset = Some(section.address);
                    payload_size = Some(section.size);
                }
                TdvfSectionType::PayloadParam => {
                    cmdline_offset = Some(section.address);
                }
                _ => {}
            }
        }

        #[cfg(feature = "tdx")]
        if hob_offset.is_none() {
            return Err(StartMicroVmError::TdDataLoader(LoadTdDataError::HobOffset));
        }

        #[cfg(feature = "tdx")]
        if payload_offset.is_none() || payload_size.is_none() {
            return Err(StartMicroVmError::TdDataLoader(
                LoadTdDataError::PayloadOffset,
            ));
        }

        #[cfg(feature = "tdx")]
        if cmdline_offset.is_none() {
            return Err(StartMicroVmError::TdDataLoader(
                LoadTdDataError::PayloadParamsOffset,
            ));
        }
        // Safe to unwrap here
        Ok((
            hob_offset.unwrap(),
            payload_offset.unwrap(),
            payload_size.unwrap(),
            cmdline_offset.unwrap(),
        ))
    }

    /// load bzImage as firmware payload
    #[cfg(any(feature = "tdx", feature = "sev"))]
    pub fn load_bzimage_payload(
        &mut self,
        payload_offset: u64,
        _payload_size: u64,
        vm_memory: &GuestMemoryImpl,
    ) -> std::result::Result<PayloadInfo, StartMicroVmError> {
        let kernel_config = self
            .kernel_config
            .as_mut()
            .ok_or(StartMicroVmError::MissingKernelConfig)?;

        let payload_file = kernel_config.kernel_file_mut();

        let payload_size = payload_file.seek(SeekFrom::End(0)).unwrap();

        payload_file.seek(SeekFrom::Start(0x1f1)).unwrap();

        let mut payload_header = linux_loader::bootparam::setup_header::default();
        payload_header
            .as_bytes()
            .read_from(
                0,
                payload_file,
                std::mem::size_of::<linux_loader::bootparam::setup_header>(),
            )
            .unwrap();

        if payload_header.header != 0x5372_6448 {
            return Err(StartMicroVmError::TdDataLoader(
                LoadTdDataError::LoadPayload,
            ));
        }

        if (payload_header.version < 0x0200) || ((payload_header.loadflags & 0x1) == 0x0) {
            return Err(StartMicroVmError::TdDataLoader(
                LoadTdDataError::LoadPayload,
            ));
        }
        payload_file.seek(SeekFrom::Start(0)).unwrap();
        vm_memory
            .read_from(
                GuestAddress(payload_offset),
                payload_file,
                payload_size as usize,
            )
            .unwrap();

        // Create the payload info that will be inserted into
        // the HOB.
        let payload_info = PayloadInfo {
            image_type: PayloadImageType::BzImage,
            entry_point: payload_offset,
        };
        Ok(payload_info)
    }

    /// load vmlinux as firmware payload
    #[cfg(feature = "tdx")]
    pub fn load_vmlinux_payload(
        &mut self,
        payload_offset: u64,
        payload_size: u64,
        vm_memory: &GuestMemoryImpl,
    ) -> std::result::Result<PayloadInfo, StartMicroVmError> {
        // load kernel
        let kernel_loader_result =
            self.load_kernel(vm_memory.deref(), Some(GuestAddress(payload_offset)))?;
        // Kernel should be loaded into the payload section, Otherwise data won't be accepted by
        // TD. Make sure that the kernel does not overflow this range.

        if kernel_loader_result.kernel_end > (payload_offset + payload_size) {
            info!(
                self.logger,
                "kernel_end: 0x{:x}, payload_offset:0x{:x}, payload_size:0x{:x}",
                kernel_loader_result.kernel_end,
                payload_offset,
                payload_size,
            );
            Err(StartMicroVmError::TdDataLoader(
                LoadTdDataError::LoadPayload,
            ))
        } else {
            let payload_info = PayloadInfo {
                image_type: PayloadImageType::RawVmLinux,
                entry_point: kernel_loader_result.kernel_load.0,
            };
            Ok(payload_info)
        }
    }
    /// load cmdline as firmware param
    pub fn load_cmdline(
        &self,
        cmdline_offset: u64,
        vm_memory: &GuestMemoryImpl,
    ) -> std::result::Result<(), StartMicroVmError> {
        let cmdline = &self
            .kernel_config
            .as_ref()
            .ok_or(StartMicroVmError::MissingKernelConfig)?
            .cmdline;
        linux_loader::loader::load_cmdline(vm_memory, GuestAddress(cmdline_offset), cmdline)
            .map_err(StartMicroVmError::LoadCommandline)?;
        Ok(())
    }
    /// generate hob list fot firmware
    #[cfg(any(feature = "tdx", feature = "sev"))]
    pub fn generate_hob_list(
        &self,
        hob_offset: u64,
        vm_memory: &GuestMemoryImpl,
        address_space: AddressSpace,
        payload_info: PayloadInfo,
        acpi_tables: &[dbs_acpi::sdt::Sdt],
    ) -> std::result::Result<(), vm_memory::GuestMemoryError> {
        let mut hob = dbs_tdx::td_shim::hob::TdHob::start(hob_offset);
        // add memory resource
        let mut memory_regions: Vec<(bool, u64, u64)> = Vec::new();
        address_space
            .walk_regions(|region| {
                match region.region_type() {
                    AddressSpaceRegionType::DefaultMemory => {
                        memory_regions.push((true, region.start_addr().0, region.len()));
                    }
                    AddressSpaceRegionType::Firmware => {
                        memory_regions.push((false, region.start_addr().0, region.len()));
                    }
                    _ => {}
                }
                Ok(())
            })
            .unwrap();
        for (is_ram, start, size) in memory_regions {
            hob.add_memory_resource(vm_memory, start, size, is_ram)?;
        }
        // add mmio resource
        hob.add_mmio_resource(
            vm_memory,
            layout::MMIO_LOW_START,
            TD_SHIM_START - layout::MMIO_LOW_START,
        )?;
        // add payload info
        hob.add_payload(vm_memory, payload_info)?;
        // add acpi tables
        for acpi_table in acpi_tables {
            hob.add_acpi_table(vm_memory, acpi_table.as_slice())?;
        }
        hob.finish(vm_memory)
    }
}

#[cfg(feature = "sev")]
impl Vm {
    /// Initialize the SEV microVM.
    pub fn init_sev_microvm(
        &mut self,
        vm_as: GuestAddressSpaceImpl,
    ) -> std::result::Result<(), StartMicroVmError> {
        info!(self.logger, "VM: initializing SEV microVM");

        let vm_memory = vm_as.memory();

        // Load firmware into memory.
        let sections = self.parse_tdvf_sections()?;
        let (hob_offset, payload_offset, payload_size, cmdline_offset) =
            self.load_firmware(vm_memory.deref(), &sections)?;

        // Load payload info into memory.
        let payload_info =
            self.load_bzimage_payload(payload_offset, payload_size, vm_memory.deref())?;
        self.load_cmdline(cmdline_offset, vm_memory.deref())?;

        // Generate TD hob.
        let max_vcpu_count = self.vm_config().max_vcpu_count;
        let boot_vcpu_count = self.vm_config().vcpu_count;

        let acpi_tables = create_acpi_tables_tdx(max_vcpu_count, boot_vcpu_count);
        let address_space =
            self.vm_address_space()
                .cloned()
                .ok_or(StartMicroVmError::GuestMemory(
                    AddressManagerError::GuestMemoryNotInitialized,
                ))?;

        self.generate_hob_list(
            hob_offset,
            vm_memory.deref(),
            address_space,
            payload_info,
            &acpi_tables,
        )
        .map_err(LoadTdDataError::LoadData)
        .map_err(StartMicroVmError::TdDataLoader)?;

        // Do some SEV ioctl(2).

        let sev_fd = SevFirmware::open().map_err(StartMicroVmError::SevIoctlError)?;
        // let platform_status = sev_fd
        //     .platform_status()
        //     .map_err(|e| Into::<std::io::Error>::into(e))
        //     .map_err(sev::error::Error::IoError)
        //     .map_err(StartMicroVmError::SevError)?;
        // let build = platform_status.build;

        // TODO: check is sev-es supported:
        // status.flags & SEV_STATUS_FLAGS_CONFIG_ES
        // SEV_STATUS_FLAGS_CONFIG_ES = 0x0100
        // TODO: check is in-kernel irqchip allowed

        let launcher = sev_launch::Launcher::new_es(self.vm_fd().as_raw_fd(), sev_fd.as_raw_fd())
            .map_err(StartMicroVmError::SevIoctlError)?;
        let start = self
            .vm_config
            .sev_start
            .take()
            .ok_or(StartMicroVmError::SevMissingStart)?;
        let mut launcher = launcher
            .start(start)
            .map_err(StartMicroVmError::SevIoctlError)?;

        self.vcpu_manager()
            .map_err(StartMicroVmError::Vcpu)?
            .create_vcpus(boot_vcpu_count, None, None)
            .map_err(StartMicroVmError::Vcpu)?;

        for region in vm_memory.iter() {
            launcher
                .update_data(unsafe { std::slice::from_raw_parts(region.as_ptr(), region.len()) })
                .map_err(StartMicroVmError::SevIoctlError)?;
        }
        launcher
            .update_vmsa()
            .map_err(StartMicroVmError::SevIoctlError)?;

        let launcher = launcher
            .measure()
            .map_err(StartMicroVmError::SevIoctlError)?;

        let measurement = launcher.measurement();
        self.sev_launcher = Some(launcher);
        self.shared_info
            .write()
            .expect(
                "Failed to start microVM because shared info couldn't \
                    be written due to poisoned lock",
            )
            .state = InstanceState::Starting(VmStartingStage::SevMeasured);

        info!(
            self.logger,
            "VM: SEV microVM measured. Method `start_microvm` returned, waiting \
                to be called again with the secret to start the VM."
        );
        // debug!(self.logger, "VM: SEV measurement = {:?}", measurement);

        Err(StartMicroVmError::SevMeasured(measurement))
    }

    fn init_sev_microvm_rest(&mut self) -> std::result::Result<(), StartMicroVmError> {
        // Safe to unwarp because the current instance state is
        // InstanceState::Starting(VmStartingStage::SevMeasured),
        // at which sev_launcher is Some(..).
        let mut launcher = self.sev_launcher.take().unwrap();
        let secret: sev::launch::sev::Secret = self
            .vm_config
            .sev_secret
            .take()
            .ok_or(StartMicroVmError::SevMissingSecret)?;
        // TODO: guest_address is ?
        let guest = 0;
        launcher
            .inject(&secret, guest)
            .map_err(StartMicroVmError::SevIoctlError)?;
        let _handle = launcher
            .finish()
            .map_err(StartMicroVmError::SevIoctlError)?;

        Ok(())
    }
}
