// Copyright 2020 Alibaba Cloud. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::io;
use std::sync::{Arc, Mutex};

use dbs_address_space::{
    AddressSpace, AddressSpaceError, AddressSpaceRegion, MPOL_MF_MOVE, MPOL_PREFERRED, USABLE_END,
};
use dbs_utils::epoll_manager::EpollManager;
use dbs_virtio_devices as virtio;
use kvm_bindings::kvm_userspace_memory_region;
use kvm_ioctls::VmFd;
use serde_derive::{Deserialize, Serialize};

use virtio::mem::{Mem, MemRegionFactory};
use virtio::Error as VirtIoError;
use vm_memory::{
    Address, GuestAddress, GuestAddressSpace, GuestMemory, GuestRegionMmap, GuestUsize, MmapRegion,
};

use crate::address_space_manager::GuestAddressSpaceImpl;
use crate::config_manager::{ConfigItem, DeviceConfigInfo, DeviceConfigInfos};
use crate::device_manager::DbsMmioV2Device;
use crate::device_manager::{DeviceManager, DeviceMgrError, DeviceOpContext};
use crate::vm::VmConfigInfo;

// The flag of whether to use the shared irq.
const USE_SHARED_IRQ: bool = true;
// The flag of whether to use the generic irq.
const USE_GENERIC_IRQ: bool = true;

/// Madvise flag for nocma memory allocation
#[cfg(target_arch = "aarch64")]
const MADV_NOCMA: i32 = 25;

const HUGE_PAGE_2M: usize = 0x200000;

macro_rules! info(
    ($l:expr, $($args:tt)+) => {
        slog::info!($l, $($args)+; slog::o!("subsystem" => "mem_dev_mgr"))
    };
);

macro_rules! debug(
    ($l:expr, $($args:tt)+) => {
        slog::debug!($l, $($args)+; slog::o!("subsystem" => "mem_dev_mgr"))
    };
);

macro_rules! error(
    ($l:expr, $($args:tt)+) => {
        slog::error!($l, $($args)+; slog::o!("subsystem" => "mem_dev_mgr"))
    };
);

// max numa node ids on host
const MAX_NODE: u32 = 64;

/// Errors associated with `MemDeviceConfig`.
#[derive(Debug, thiserror::Error)]
pub enum MemDeviceError {
    /// Invalid virtual machine instance ID.
    #[error("the virtual machine instance ID is invalid")]
    InvalidVMID,

    /// The mem device was already used.
    #[error("the virtio-mem sock path was already added to a different device")]
    MemDeviceAlreadyExists,

    /// Cannot perform the requested operation after booting the microVM.
    #[error("the update operation is not allowed after boot")]
    UpdateNotAllowedPostBoot,

    /// Hotplug size not configured
    #[error("memory hotplug size not configured.")]
    LostHotplugMemoryRegion,

    /// guest memory error
    #[error("failed to access guest memory")]
    GuestMemoryError(#[source] vm_memory::mmap::Error),

    /// insert mem device error
    #[error("cannot add virtio-mem device, {0}")]
    InsertDeviceFailed(#[from] DeviceMgrError),

    /// create mem device error
    #[error("cannot create virito-mem device, {0}")]
    CreateMemDevice(#[source] DeviceMgrError),

    /// create mmio device error
    #[error("cannot create virito-mem mmio device, {0}")]
    CreateMmioDevice(#[source] DeviceMgrError),

    /// resize mem device error
    #[error("failure while resizing virtio-mem device")]
    ResizeFailed,

    /// mem device does not exist
    #[error("mem device does not exist")]
    NotExist,

    /// address space region error
    #[error("address space region error, {0}")]
    AddressSpaceRegion(#[source] AddressSpaceError),

    /// Cannot initialize a mem device or add a device to the MMIO Bus.
    #[error("failure while registering mem device: {0}")]
    RegisterMemDevice(#[source] DeviceMgrError),

    /// The mem device id doesn't exist.
    #[error("invalid mem device id '{0}'")]
    InvalidDeviceId(String),
}

/// Configuration information for a virtio-mem device.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct MemDeviceConfigInfo {
    /// Unique identifier of the pmem device
    pub mem_id: String,
    /// Memory size mib
    pub size_mib: u64,
    /// Memory capacity mib
    pub capacity_mib: u64,
    /// Use multi_region or not
    pub multi_region: bool,
    /// host numa node id
    pub host_numa_node_id: Option<u32>,
    /// guest numa node id
    pub guest_numa_node_id: Option<u16>,
    /// Use shared irq
    pub use_shared_irq: Option<bool>,
    /// Use generic irq
    pub use_generic_irq: Option<bool>,
}

impl ConfigItem for MemDeviceConfigInfo {
    type Err = MemDeviceError;

    fn id(&self) -> &str {
        &self.mem_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), MemDeviceError> {
        if self.mem_id.as_str() == other.mem_id.as_str() {
            Err(MemDeviceError::MemDeviceAlreadyExists)
        } else {
            Ok(())
        }
    }
}

/// Mem Device Info
pub type MemDeviceInfo = DeviceConfigInfo<MemDeviceConfigInfo>;

impl ConfigItem for MemDeviceInfo {
    type Err = MemDeviceError;

    fn id(&self) -> &str {
        &self.config.mem_id
    }

    fn check_conflicts(&self, other: &Self) -> Result<(), MemDeviceError> {
        if self.config.mem_id.as_str() == other.config.mem_id.as_str() {
            Err(MemDeviceError::MemDeviceAlreadyExists)
        } else {
            Ok(())
        }
    }
}

/// Wrapper for the collection that holds all the Mem Devices Configs
#[derive(Clone)]
pub struct MemDeviceMgr {
    /// A list of `MemDeviceConfig` objects.
    info_list: DeviceConfigInfos<MemDeviceConfigInfo>,
    pub(crate) use_shared_irq: bool,
}

impl MemDeviceMgr {
    /// Inserts `mem_cfg` in the virtio-mem device configuration list.
    /// If an entry with the same id already exists, it will attempt to update
    /// the existing entry.
    pub fn insert_device(
        &mut self,
        mut ctx: DeviceOpContext,
        mem_cfg: MemDeviceConfigInfo,
    ) -> std::result::Result<(), MemDeviceError> {
        if !cfg!(feature = "hotplug") && ctx.is_hotplug {
            error!(ctx.logger(), "hotplug feature has been disabled.");
            return Err(MemDeviceError::UpdateNotAllowedPostBoot);
        }

        let epoll_mgr = ctx.epoll_mgr.clone().ok_or(MemDeviceError::ResizeFailed)?;

        // If the id of the drive already exists in the list, the operation is update.
        match self.get_index_of_mem_dev(&mem_cfg.mem_id) {
            // Create a new memory device
            None => {
                if !ctx.is_hotplug {
                    return Ok(());
                }

                info!(
                    ctx.logger(),
                    "hot-add memory device: {}, size: 0x{:x}MB.", mem_cfg.mem_id, mem_cfg.size_mib
                );

                let device = Self::create_memory_device(&mem_cfg, &ctx, &epoll_mgr)
                    .map_err(MemDeviceError::CreateMemDevice)?;
                let mmio_device =
                    DeviceManager::create_mmio_virtio_device_with_device_change_notification(
                        Box::new(device),
                        &mut ctx,
                        mem_cfg.use_shared_irq.unwrap_or(self.use_shared_irq),
                        mem_cfg.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                    )
                    .map_err(MemDeviceError::CreateMmioDevice)?;

                #[cfg(not(test))]
                ctx.insert_hotplug_mmio_device(&mmio_device, None)
                    .map_err(|e| {
                        error!(
                            ctx.logger(),
                            "failed to hot-add virtio-mem device {}, {}", &mem_cfg.mem_id, e
                        );
                        MemDeviceError::InsertDeviceFailed(e)
                    })?;

                let index = self.info_list.insert_or_update(&mem_cfg)?;
                self.info_list[index].set_device(mmio_device);
            }

            // Update an existing memory device
            Some(_index) => {
                if ctx.is_hotplug {
                    info!(
                        ctx.logger(),
                        "update memory device: {}, size: 0x{:x}MB.",
                        mem_cfg.mem_id,
                        mem_cfg.size_mib
                    );
                    self.update_memory_size(&mem_cfg)?;
                }
                self.info_list.insert_or_update(&mem_cfg)?;
            }
        }

        Ok(())
    }

    /// Attaches all virtio-mem devices from the MemDevicesConfig.
    pub fn attach_devices(
        &mut self,
        ctx: &mut DeviceOpContext,
    ) -> std::result::Result<(), MemDeviceError> {
        let epoll_mgr = ctx
            .epoll_mgr
            .clone()
            .ok_or(MemDeviceError::CreateMemDevice(
                DeviceMgrError::InvalidOperation,
            ))?;

        for info in self.info_list.iter_mut() {
            let config = &info.config;
            info!(
                ctx.logger(),
                "attach virtio-mem device {}, size 0x{:x}.", config.mem_id, config.size_mib
            );
            // Ignore virtio-mem device with zero memory capacity.
            if config.size_mib == 0 {
                debug!(
                    ctx.logger(),
                    "ignore zero-sizing memory device {}.", config.mem_id
                );
                continue;
            }

            let device = Self::create_memory_device(config, ctx, &epoll_mgr)
                .map_err(MemDeviceError::CreateMemDevice)?;
            let mmio_device =
                DeviceManager::create_mmio_virtio_device_with_device_change_notification(
                    Box::new(device),
                    ctx,
                    config.use_shared_irq.unwrap_or(self.use_shared_irq),
                    config.use_generic_irq.unwrap_or(USE_GENERIC_IRQ),
                )
                .map_err(MemDeviceError::RegisterMemDevice)?;

            info.set_device(mmio_device);
        }

        Ok(())
    }

    fn get_index_of_mem_dev(&self, mem_id: &str) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.mem_id.eq(mem_id))
    }

    fn create_memory_device(
        config: &MemDeviceConfigInfo,
        ctx: &DeviceOpContext,
        epoll_mgr: &EpollManager,
    ) -> std::result::Result<virtio::mem::Mem<GuestAddressSpaceImpl>, DeviceMgrError> {
        let factory = Arc::new(Mutex::new(MemoryRegionFactory::new(
            ctx,
            config.mem_id.clone(),
            config.host_numa_node_id,
        )));

        let mut capacity_mib = config.capacity_mib;
        if capacity_mib == 0 {
            capacity_mib = *USABLE_END >> 20;
        }
        // get boot memory size for calculate alignment
        let boot_mem_size = {
            if let Some(vm_config) = &ctx.vm_config {
                let boot_size = (vm_config.mem_size_mib << 20) as u64;
                // increase 1G memory because of avoiding mmio hole
                match boot_size {
                    x if x > dbs_boot::layout::MMIO_LOW_START => x + (1 << 30),
                    _ => boot_size,
                }
            } else {
                0
            }
        };

        virtio::mem::Mem::new(
            config.mem_id.clone(),
            capacity_mib,
            config.size_mib,
            config.multi_region,
            config.guest_numa_node_id,
            epoll_mgr.clone(),
            factory,
            boot_mem_size,
        )
        .map_err(DeviceMgrError::Virtio)
    }

    /// Removes all virtio-mem devices
    pub fn remove_devices(&self, ctx: &mut DeviceOpContext) -> Result<(), DeviceMgrError> {
        for info in self.info_list.iter() {
            if let Some(device) = &info.device {
                DeviceManager::destroy_mmio_virtio_device(device.clone(), ctx)?;
            }
        }

        Ok(())
    }

    fn update_memory_size(
        &self,
        config: &MemDeviceConfigInfo,
    ) -> std::result::Result<(), MemDeviceError> {
        match self.get_index_of_mem_dev(&config.mem_id) {
            Some(index) => {
                let device = self.info_list[index]
                    .device
                    .as_ref()
                    .ok_or_else(|| MemDeviceError::NotExist)?;
                if let Some(mmio_dev) = device.as_any().downcast_ref::<DbsMmioV2Device>() {
                    let guard = mmio_dev.state();
                    let inner_dev = guard.get_inner_device();
                    if let Some(mem_dev) = inner_dev
                        .as_any()
                        .downcast_ref::<Mem<GuestAddressSpaceImpl>>()
                    {
                        return mem_dev
                            .set_requested_size(config.size_mib as u64)
                            .map_err(|_e| MemDeviceError::ResizeFailed);
                    }
                }
                Ok(())
            }
            None => Err(MemDeviceError::InvalidDeviceId(config.mem_id.clone())),
        }
    }
}

impl Default for MemDeviceMgr {
    /// Create a new `MemDeviceMgr` object..
    fn default() -> Self {
        MemDeviceMgr {
            info_list: DeviceConfigInfos::new(),
            use_shared_irq: USE_SHARED_IRQ,
        }
    }
}

pub(crate) struct MemoryRegionFactory {
    pub(crate) mem_id: String,
    pub(crate) vm_as: GuestAddressSpaceImpl,
    pub(crate) address_space: AddressSpace,
    pub(crate) vm_config: VmConfigInfo,
    pub(crate) vm_fd: Arc<VmFd>,
    pub(crate) logger: Arc<slog::Logger>,
    pub(crate) host_numa_node_id: Option<u32>,
    pub(crate) instance_id: String,
}

impl MemoryRegionFactory {
    fn new(ctx: &DeviceOpContext, mem_id: String, host_numa_node_id: Option<u32>) -> Self {
        let vm_as = ctx.vm_as.as_ref().unwrap().clone();
        let address_space = ctx.address_space.as_ref().unwrap().clone();
        let vm_config = ctx.vm_config.as_ref().unwrap().clone();
        let logger = Arc::new(ctx.logger().new(slog::o!()));

        let shared_info = ctx.shared_info.read().unwrap();
        let instance_id = shared_info.id.clone();

        MemoryRegionFactory {
            mem_id,
            vm_as,
            address_space,
            vm_config,
            vm_fd: ctx.vm_fd.clone(),
            logger,
            host_numa_node_id,
            instance_id,
        }
    }
}

impl MemRegionFactory for MemoryRegionFactory {
    fn create_region(
        &mut self,
        guest_addr: GuestAddress,
        region_len: GuestUsize,
        kvm_slot: u32,
    ) -> std::result::Result<Arc<GuestRegionMmap>, VirtIoError> {
        // create address space region
        let mem_type = self.vm_config.mem_type.as_str();
        let mut mem_file_path = self.vm_config.mem_file_path.clone();
        let mem_file_name = format!(
            "/virtiomem_{}_{}",
            self.instance_id.as_str(),
            self.mem_id.as_str()
        );
        mem_file_path.push_str(mem_file_name.as_str());
        let as_region = AddressSpaceRegion::create_default_memory_region(
            guest_addr,
            region_len,
            self.host_numa_node_id,
            mem_type,
            mem_file_path.as_str(),
            false,
            true,
        )
        .map_err(|e| {
            error!(self.logger, "failed to insert address space region: {}", e);
            VirtIoError::IOError(io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "invalid address space region ({0:#x}, {1:#x})",
                    guest_addr.0, region_len
                ),
            ))
        })?;
        let arc_as_region = Arc::new(as_region.clone());
        info!(
            self.logger,
            "VM: mem_type: {} mem_file_path: {}, numa_node_id: {:?} file_offset: {:?}",
            mem_type,
            mem_file_path,
            self.host_numa_node_id,
            as_region.file_offset()
        );

        let mmap_region = MmapRegion::build(
            as_region.file_offset().cloned(),
            region_len as usize,
            as_region.prot_flags(),
            as_region.perm_flags(),
        )
        .map_err(VirtIoError::NewMmapRegion)?;
        let host_addr: u64 = mmap_region.as_ptr() as u64;

        // thp
        if mem_type == "hugeanon" || mem_type == "hugeshmem" {
            let res = unsafe {
                libc::madvise(
                    host_addr as *mut libc::c_void,
                    region_len as libc::size_t,
                    libc::MADV_HUGEPAGE,
                )
            };
            if res != 0 {
                return Err(VirtIoError::IOError(io::Error::last_os_error()));
            }
        }

        #[cfg(target_arch = "aarch64")]
        if arc_as_region.is_nocma() {
            let res = unsafe {
                libc::madvise(
                    host_addr as *mut libc::c_void,
                    region_len as libc::size_t,
                    MADV_NOCMA,
                )
            };
            if res == 0 {
                info!(self.logger, "virtio-mem: enable nocma.");
            } else {
                info!(
                    self.logger,
                    "Madvise set nocma return {}",
                    io::Error::last_os_error()
                );
            }
        }

        // Handle numa
        if let Some(numa_node_id) = self.host_numa_node_id {
            let nodemask = (1 << numa_node_id) as u64;
            let flags = MPOL_MF_MOVE;
            let res = unsafe {
                libc::syscall(
                    libc::SYS_mbind,
                    host_addr as *mut libc::c_void,
                    region_len as usize,
                    MPOL_PREFERRED,
                    &nodemask as *const u64,
                    MAX_NODE,
                    flags,
                )
            };
            if res < 0 {
                error!(
                    self.logger,
                    "virtio-mem mbind failed when host_numa_node_id: {:?} res: {:?}",
                    numa_node_id,
                    res
                );
            }
        }

        debug!(
            self.logger,
            "kvm slot {}, host_addr {:X}, guest_addr {:X}, host_numa_node_id {:?}.",
            kvm_slot,
            host_addr,
            guest_addr.raw_value(),
            self.host_numa_node_id,
        );

        // add to guest memory mapping
        let kvm_mem_region = kvm_userspace_memory_region {
            slot: kvm_slot,
            flags: 0,
            guest_phys_addr: guest_addr.raw_value(),
            memory_size: region_len,
            userspace_addr: host_addr,
        };
        // Safe because the user mem region is just created, and kvm slot is allocated
        // by resource allocator.
        unsafe {
            self.vm_fd
                .set_user_memory_region(kvm_mem_region)
                .map_err(VirtIoError::SetUserMemoryRegion)?
        };

        info!(
            self.logger,
            "kvm set user memory region: slot: {}, flags: {}, guest_phys_addr: {:X}, memory_size: {}, userspace_addr: {:X}",
            kvm_slot,
            0,
            guest_addr.raw_value(),
            region_len,
            host_addr
        );

        // All value should be valid.
        let guest_mmap_region = Arc::new(GuestRegionMmap::new(mmap_region, guest_addr).unwrap());

        let vm_as_new = self
            .vm_as
            .memory()
            .insert_region(guest_mmap_region.clone())
            .map_err(|e| {
                error!(self.logger, "failed to insert guest memory region.");
                VirtIoError::InsertMmap(e)
            })?;
        self.vm_as.lock().unwrap().replace(vm_as_new);
        self.address_space
            .insert_region(arc_as_region)
            .map_err(|e| {
                error!(self.logger, "failed to insert address space region: {}", e);
                VirtIoError::IOError(io::Error::new(
                    io::ErrorKind::Other,
                    format!(
                        "invalid address space region ({0:#x}, {1:#x})",
                        guest_addr.0, region_len
                    ),
                ))
            })?;

        Ok(guest_mmap_region)
    }

    fn restore_region_addr(
        &self,
        guest_addr: GuestAddress,
    ) -> std::result::Result<*mut u8, VirtIoError> {
        let memory = self.vm_as.memory();
        // NOTE: We can't clone `GuestRegionMmap` reference directly!!!
        //
        // Since an important role of the member `mapping` (type is
        // `MmapRegion`) in `GuestRegionMmap` is to mmap the memory during
        // construction and munmap the memory during drop. However, when the
        // life time of cloned data is over, the drop operation will be
        // performed, which will munmap the origional mmap memory, which will
        // cause some memory in dragonall to be inaccessable. And remember the
        // data structure that was cloned is still alive now, when its life time
        // is over, it will perform the munmap operation again, which will cause
        // a memory exception!
        memory
            .get_host_address(guest_addr)
            .map_err(VirtIoError::GuestMemory)
    }

    fn get_host_numa_node_id(&self) -> Option<u32> {
        self.host_numa_node_id
    }

    fn set_host_numa_node_id(&mut self, host_numa_node_id: Option<u32>) {
        self.host_numa_node_id = host_numa_node_id;
    }
}

#[cfg(test)]
mod tests {
    use vm_memory::GuestMemoryRegion;

    use super::*;
    use crate::test_utils::tests::create_vm_for_test;

    impl Default for MemDeviceConfigInfo {
        fn default() -> Self {
            MemDeviceConfigInfo {
                mem_id: "".to_string(),
                size_mib: 0,
                capacity_mib: 1024,
                multi_region: true,
                host_numa_node_id: None,
                guest_numa_node_id: None,
                use_generic_irq: None,
                use_shared_irq: None,
            }
        }
    }

    #[test]
    fn test_mem_config_check_conflicts() {
        let config = MemDeviceConfigInfo::default();
        let mut config2 = config.clone();
        assert!(config.check_conflicts(&config2).is_err());
        config2.mem_id = "dummy_mem".to_string();
        assert!(config.check_conflicts(&config2).is_ok());
    }

    #[test]
    fn test_create_mem_devices_configs() {
        let mgr = MemDeviceMgr::default();
        assert_eq!(mgr.info_list.len(), 0);
        assert_eq!(mgr.get_index_of_mem_dev(""), None);
    }

    #[test]
    fn test_mem_insert_device() {
        // Init vm for test.
        let mut vm = create_vm_for_test();

        // We don't need to use virtio-mem before start vm
        // Test for standard config with hotplug
        let device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            vm.vm_address_space().cloned(),
            true,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );

        let dummy_mem_device = MemDeviceConfigInfo::default();
        vm.device_manager_mut()
            .mem_manager
            .insert_device(device_op_ctx, dummy_mem_device.into())
            .unwrap();
        assert_eq!(vm.device_manager().mem_manager.info_list.len(), 1);
    }

    #[test]
    fn test_mem_attach_device() {
        // Init vm and insert mem config for test.
        let mut vm = create_vm_for_test();
        let dummy_mem_device = MemDeviceConfigInfo::default();
        vm.device_manager_mut()
            .mem_manager
            .info_list
            .insert_or_update(&dummy_mem_device)
            .unwrap();
        assert_eq!(vm.device_manager().mem_manager.info_list.len(), 0);

        // Test for standard config
        let mut device_op_ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            vm.vm_address_space().cloned(),
            false,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        vm.device_manager_mut()
            .mem_manager
            .attach_devices(&mut device_op_ctx)
            .unwrap();
        assert_eq!(vm.device_manager().mem_manager.info_list.len(), 1);
    }

    #[test]
    fn test_mem_create_region() {
        let vm = create_vm_for_test();
        let ctx = DeviceOpContext::new(
            Some(vm.epoll_manager().clone()),
            vm.device_manager(),
            Some(vm.vm_as().unwrap().clone()),
            vm.vm_address_space().cloned(),
            true,
            Some(VmConfigInfo::default()),
            vm.shared_info().clone(),
        );
        let mem_id = String::from("mem0");
        let guest_addr = GuestAddress(0x1_0000_0000);
        let region_len = 0x1000_0000;
        let kvm_slot = 2;

        // no vfio manager, no numa node
        let mut factory = MemoryRegionFactory::new(&ctx, mem_id, None);
        let region_opt = factory.create_region(guest_addr, region_len, kvm_slot);
        assert_eq!(region_opt.unwrap().len(), region_len);
    }
}
