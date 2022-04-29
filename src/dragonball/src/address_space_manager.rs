// Copyright (C) 2022 Alibaba Cloud Computing. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Address space abstraction to manage virtual machine's physical address space.
//!
//! The AddressSpace abstraction is introduced to manage virtual machine's physical address space.
//! The regions in virtual machine's physical address space may be used to:
//! 1) map guest virtual memory
//! 2) map MMIO ranges for emulated virtual devices, such as virtio-fs DAX window.
//! 3) map MMIO ranges for pass-through devices, such as PCI device BARs.
//! 4) map MMIO ranges for to vCPU, such as local APIC.
//! 5) not used/available
//!
//! A related abstraction, vm_memory::GuestMemory, is used to access guest virtual memory only.
//! In other words, AddressSpace is the resource owner, and GuestMemory is an accessor for guest
//! virtual memory.

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::os::unix::io::{AsRawFd, FromRawFd};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use arc_swap::ArcSwap;
use dbs_address_space::{
    AddressSpace, AddressSpaceError, AddressSpaceLayout, AddressSpaceRegion,
    AddressSpaceRegionType, NumaNode, NumaNodeInfo, MPOL_MF_MOVE, MPOL_PREFERRED,
};
use dbs_allocator::Constraint;
use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use kvm_ioctls::VmFd;
use log::{debug, error, info, trace, warn};
use nix::sys::mman;
#[cfg(feature = "atomic-guest-memory")]
use vm_memory::atomic::GuestMemoryAtomic;
use vm_memory::{
    Address, FileOffset, GuestAddress, GuestAddressSpace, GuestMemoryMmap, GuestMemoryRegion,
    GuestRegionMmap, GuestUsize, MemoryRegionAddress, MmapRegion,
};

use crate::resource_manager::ResourceManager;
use crate::vm::NumaRegionInfo;

#[cfg(not(feature = "atomic-guest-memory"))]
/// Concrete GuestAddressSpace type used by the VMM.
pub type GuestAddressSpaceImpl = Arc<GuestMemoryMmap>;

#[cfg(feature = "atomic-guest-memory")]
/// Concrete GuestAddressSpace type used by the VMM.
pub type GuestAddressSpaceImpl = GuestMemoryAtomic<GuestMemoryMmap>;

/// Concrete GuestMemory type used by the VMM.
pub type GuestMemoryImpl = <Arc<vm_memory::GuestMemoryMmap> as GuestAddressSpace>::M;
/// Concreate GuestRegion type used by the VMM.
pub type GuestRegionImpl = GuestRegionMmap;

/// The max number of touch thread to do prealloc.
const MAX_TOUCH_THREAD: u64 = 16;

/// Control the actual number of touch thread. Use 1 more touch thread per 4G memory.
/// Due to prealloc is asynchronous, we don't control the min number of touch thread.
const TOUCH_GRANULARITY: u64 = 32;

// max numa node ids on host
const MAX_NODE: u32 = 64;

/// Errors associated with virtual machine address space management.
#[derive(Debug, thiserror::Error)]
pub enum AddressManagerError {
    /// Invalid address space operation.
    #[error("invalid address space operation")]
    InvalidOperation,

    /// Invalid address range.
    #[error("invalid address space region (0x{0:x}, 0x{1:x})")]
    InvalidAddressRange(u64, GuestUsize),

    /// Failure in initializing guest memory.
    #[error("failure while initializing guest memory")]
    MemoryNotInitialized,

    /// No resource available.
    #[error("no available resource")]
    NoAvailableResource,

    /// Failed to create memfd to map anonymous memory.
    #[error("cannot create memfd to map anonymous memory")]
    CreateMemFd(#[source] nix::Error),

    /// Failed to open memory file.
    #[error("cannot open memory file")]
    OpenFile(#[source] std::io::Error),

    /// Failed to set size for memory file.
    #[error("cannot set size for memory file")]
    SetFileSize(#[source] std::io::Error),

    /// Failed to unlink memory file.
    #[error("cannot unlike memory file")]
    UnlinkFile(#[source] nix::Error),

    /// Failed to duplicate fd of memory file.
    #[error("cannot duplicate memory file descriptor")]
    DupFd(#[source] nix::Error),

    /// Failed to create GuestMemory
    #[error("failure in creating guest memory object")]
    CreateGuestMemory(#[source] vm_memory::Error),

    /// Failed to mmap() guest memory
    #[error("cannot mmap() guest memory into current process")]
    MmapGuestMemory(#[source] vm_memory::mmap::MmapRegionError),

    /// Failure in accessing the memory located at some address.
    #[error("cannot access guest memory located at 0x{0:x}")]
    AccessGuestMemory(u64, #[source] vm_memory::mmap::Error),

    /// Failed to set KVM memory slot.
    #[error("failure while configuring KVM memory slot")]
    KvmSetMemorySlot(#[source] kvm_ioctls::Error),

    /// Failed to set madvise on AddressSpaceRegion
    #[error("cannot madvice() on guest memory region")]
    Madvise(#[source] nix::Error),

    /// join threads fail
    #[error("join threads fail")]
    JoinFail,

    /// Failed to access address_space.
    #[error("Failed to access address_space")]
    AddressSpaceNotInitialized,

    /// Failed to create Address Space Region
    #[error("Failed to create Address Space Region {0}")]
    CreateAddressSpaceRegion(#[source] AddressSpaceError),
}

/// Struct to manage virtual machine's physical address space.
pub struct AddressSpaceMgr {
    address_space: Option<AddressSpace>,
    vm_as: Option<GuestAddressSpaceImpl>,
    base_to_slot: Arc<Mutex<HashMap<u64, u32>>>,
    prealloc_handlers: Vec<thread::JoinHandle<()>>,
    prealloc_should_stops: Vec<Arc<ArcSwap<bool>>>,
    numa_nodes: BTreeMap<u32, NumaNode>,
}

impl AddressSpaceMgr {
    /// Query address space manager is initialized or not
    pub fn is_initialized(&self) -> bool {
        self.address_space.is_some()
    }

    /// Create the address space for a virtual machine.
    ///
    /// This method is designed to be called when starting up a virtual machine instead of runtime
    /// or hotplug, so it's expected the virtual machine will be tore down and no strict error
    /// recover.
    #[allow(clippy::too_many_arguments)]
    pub fn create_address_space(
        &mut self,
        res_mgr: &ResourceManager,
        _reserve_memory_bytes: GuestUsize,
        mem_type: &str,
        mem_file_path: &str,
        numa_region_infos: &[NumaRegionInfo],
        vmfd: Option<&Arc<VmFd>>,
        log_dirty_pages: bool,
        mem_prealloc_enabled: bool,
    ) -> Result<(), AddressManagerError> {
        let mut regions = Vec::new();
        let mut start_addr = dbs_boot::layout::GUEST_MEM_START;
        let mut mem_file_path_string = mem_file_path.to_string();
        // TODO: handle with reserve memory if needed
        for info in numa_region_infos.iter() {
            info!("numa_region_info {:?}", info);
            // convert size_in_mib to bytes
            let size = info.size << 20;
            let host_numa_node_id = info.host_numa_node_id;
            let guest_numa_node_id = info.guest_numa_node_id;
            let vcpu_ids = &(info.vcpu_ids);

            // Guest memory does not intersect with the MMIO hole.
            if start_addr > dbs_boot::layout::MMIO_LOW_END
                || start_addr + size <= dbs_boot::layout::MMIO_LOW_START
            {
                let region = Arc::new(
                    AddressSpaceRegion::create_default_memory_region(
                        GuestAddress(start_addr),
                        size,
                        host_numa_node_id,
                        mem_type,
                        &mem_file_path_string,
                        mem_prealloc_enabled,
                        false,
                    )
                    .map_err(AddressManagerError::CreateAddressSpaceRegion)?,
                );
                self.insert_into_numa_nodes(&region, guest_numa_node_id.unwrap_or(0), vcpu_ids);
                regions.push(region);
                info!(
                    "create_address_space new region guest addr 0x{:x}-0x{:x} size {}",
                    start_addr,
                    start_addr + size,
                    size
                );
                start_addr += size;
            } else {
                // Add guest memory below the MMIO hole.
                let below_size = dbs_boot::layout::MMIO_LOW_START - start_addr;
                let region = Arc::new(
                    AddressSpaceRegion::create_default_memory_region(
                        GuestAddress(start_addr),
                        below_size,
                        host_numa_node_id,
                        mem_type,
                        mem_file_path,
                        mem_prealloc_enabled,
                        false,
                    )
                    .map_err(AddressManagerError::CreateAddressSpaceRegion)?,
                );
                self.insert_into_numa_nodes(&region, guest_numa_node_id.unwrap_or(0), vcpu_ids);
                regions.push(region);
                info!(
                    "create_address_space new region guest addr 0x{:x}-0x{:x} size {}",
                    start_addr,
                    dbs_boot::layout::MMIO_LOW_START,
                    below_size
                );
                // Add guest memory above the MMIO hole
                let above_size = size - below_size;
                mem_file_path_string.push('1');
                let region = Arc::new(
                    AddressSpaceRegion::create_default_memory_region(
                        GuestAddress(dbs_boot::layout::MMIO_LOW_END + 1),
                        above_size,
                        host_numa_node_id,
                        mem_type,
                        &mem_file_path_string,
                        mem_prealloc_enabled,
                        false,
                    )
                    .map_err(AddressManagerError::CreateAddressSpaceRegion)?,
                );
                self.insert_into_numa_nodes(&region, guest_numa_node_id.unwrap_or(0), vcpu_ids);
                regions.push(region);
                info!(
                    "create_address_space new region guest addr 0x{:x}-0x{:x} size {}",
                    dbs_boot::layout::MMIO_LOW_END + 1,
                    dbs_boot::layout::MMIO_LOW_END + 1 + above_size,
                    above_size
                );
                start_addr = dbs_boot::layout::MMIO_LOW_END + 1 + above_size;
            }
        }
        let mut vm_memory = GuestMemoryMmap::new();
        for reg in regions.iter() {
            // Allocate used guest memory addresses.
            // These addresses are statically allocated, resource allocation/update should not fail.
            let constraint = Constraint::new(reg.len())
                .min(reg.start_addr().raw_value())
                .max(reg.last_addr().raw_value());
            let _key = res_mgr.allocate_mem_address(&constraint).unwrap();

            // Map region into current process's virtual address space

            let (mmap_reg, mut handlers, should_stop) = Self::create_mmap_region(reg.clone())?;
            while let Some(h) = handlers.pop() {
                self.prealloc_handlers.push(h);
            }
            if let Some(should_stop) = should_stop {
                self.prealloc_should_stops.push(should_stop);
            }
            let mmap_reg = Arc::new(mmap_reg);
            vm_memory = vm_memory
                .insert_region(mmap_reg.clone())
                .map_err(AddressManagerError::CreateGuestMemory)?;

            // Build mapping between GPA <-> HVA, by adding kvm memory slot.
            let slot = res_mgr
                .allocate_kvm_mem_slot(1, None)
                .ok_or(AddressManagerError::NoAvailableResource)?;
            if let Some(fd) = vmfd {
                let host_addr = mmap_reg.get_host_address(MemoryRegionAddress(0)).unwrap();
                info!(
                    "VM: guest memory region {:x} starts at {:x?}",
                    reg.start_addr().raw_value(),
                    host_addr
                );
                let flags = if log_dirty_pages {
                    KVM_MEM_LOG_DIRTY_PAGES
                } else {
                    0
                };
                let mem_region = kvm_userspace_memory_region {
                    slot: slot as u32,
                    guest_phys_addr: reg.start_addr().raw_value(),
                    memory_size: reg.len() as u64,
                    userspace_addr: host_addr as u64,
                    flags,
                };

                // Safe because the guest regions are guaranteed not to overlap.
                unsafe { fd.set_user_memory_region(mem_region) }
                    .map_err(AddressManagerError::KvmSetMemorySlot)?;
            }
            self.base_to_slot
                .lock()
                .unwrap()
                .insert(reg.start_addr().raw_value(), slot as u32);
        }

        let boundary = AddressSpaceLayout::new(
            *dbs_boot::layout::GUEST_PHYS_END,
            dbs_boot::layout::GUEST_MEM_START,
            *dbs_boot::layout::GUEST_MEM_END,
        );
        self.address_space = Some(AddressSpace::from_regions(regions, boundary));

        #[cfg(feature = "atomic-guest-memory")]
        {
            self.vm_as = Some(AddressSpace::convert_into_vm_as(vm_memory));
        }
        #[cfg(not(feature = "atomic-guest-memory"))]
        {
            self.vm_as = Some(Arc::new(vm_memory))
        }

        Ok(())
    }

    /// Mmap the address space region into current process.
    #[allow(clippy::type_complexity)]
    pub fn create_mmap_region(
        region: Arc<AddressSpaceRegion>,
    ) -> Result<
        (
            GuestRegionImpl,
            Vec<thread::JoinHandle<()>>,
            Option<Arc<ArcSwap<bool>>>,
        ),
        AddressManagerError,
    > {
        // Special check for 32bit host with 64bit virtual machines.
        if region.len() > std::usize::MAX as u64 {
            return Err(AddressManagerError::InvalidAddressRange(
                region.start_addr().raw_value(),
                region.len(),
            ));
        }
        // The device MMIO regions may not be backed by memory files, so refuse to mmap them.
        if region.region_type() == AddressSpaceRegionType::DeviceMemory {
            return Err(AddressManagerError::InvalidOperation);
        }

        // The GuestRegionMmap/MmapRegion will take ownership of the FileOffset object,
        // so we have to duplicate the fd here. It's really a dirty design.
        let file_offset = match region.file_offset().as_ref() {
            Some(fo) => {
                let fd =
                    nix::unistd::dup(fo.file().as_raw_fd()).map_err(AddressManagerError::DupFd)?;
                // Safe because we have just duplicated the raw fd.
                let file = unsafe { File::from_raw_fd(fd) };
                let file_offset = FileOffset::new(file, fo.start());
                Some(file_offset)
            }
            None => None,
        };
        let perm_flags = if (region.perm_flags() & libc::MAP_POPULATE) > 0 && region.is_hugepage() {
            // When mem_prealloc_enabled is 'true' and shmem_enabled is 'advise', THP will not
            // work.
            // Reason is that using MAP_POPULATE, normal page will be used although hugeshmem
            // is true, because mmap() using MAP_POPULATE will do prefault which makes madvise()
            // useless. So here we remove MAP_POPULATE.
            region.perm_flags() & (!libc::MAP_POPULATE)
        } else {
            region.perm_flags()
        };

        let mmap_reg = MmapRegion::build(
            file_offset,
            region.len() as usize,
            libc::PROT_READ | libc::PROT_WRITE,
            perm_flags,
        )
        .map_err(AddressManagerError::MmapGuestMemory)?;

        if region.is_anonpage() {
            unsafe {
                mman::madvise(
                    mmap_reg.as_ptr() as *mut libc::c_void,
                    mmap_reg.size(),
                    mman::MmapAdvise::MADV_DONTFORK,
                )
            }
            .map_err(AddressManagerError::Madvise)?;
        }

        if let Some(node_id) = region.host_numa_node_id() {
            let nodemask = (1 << node_id) as u64;
            let res = unsafe {
                libc::syscall(
                    libc::SYS_mbind,
                    mmap_reg.as_ptr() as *mut libc::c_void,
                    mmap_reg.size(),
                    MPOL_PREFERRED,
                    &nodemask as *const u64,
                    MAX_NODE,
                    MPOL_MF_MOVE,
                )
            };
            if res < 0 {
                warn!("mbind failed when host_numa_node_id is: {:?}", node_id);
            }
        }

        let mut opt_should_stop = None;
        let mut handlers: Vec<thread::JoinHandle<()>> = Vec::new();
        if region.is_hugepage() {
            debug!(
                "Setting MADV_HUGEPAGE on AddressSpaceRegion addr {:x?} len {:x?}",
                mmap_reg.as_ptr(),
                mmap_reg.size()
            );

            // Safe because we just create the MmapRegion
            unsafe {
                mman::madvise(
                    mmap_reg.as_ptr() as *mut libc::c_void,
                    mmap_reg.size(),
                    mman::MmapAdvise::MADV_HUGEPAGE,
                )
            }
            .map_err(AddressManagerError::Madvise)?;

            // Here we write a byte per 4KB to perform prefault when MAP_POPULATE and hugeshmem are used.
            if region.perm_flags() & libc::MAP_POPULATE > 0 {
                const PAGE_SIZE: u64 = 4096;
                const PAGE_SHIFT: u32 = 12;
                let addr = mmap_reg.as_ptr() as u64;
                let npage = (mmap_reg.size() as u64) >> PAGE_SHIFT;

                let mut touch_thread = ((mmap_reg.size() as u64) >> TOUCH_GRANULARITY) + 1;
                touch_thread = if touch_thread >= MAX_TOUCH_THREAD {
                    MAX_TOUCH_THREAD
                } else {
                    touch_thread
                };

                let per_npage = npage / touch_thread;
                let should_stop = Arc::new(ArcSwap::from_pointee(false));
                for n in 0..touch_thread {
                    // We think the number of 4k pages should larger than TOUCH_THREAD
                    let start_npage = per_npage * n;
                    let end_npage = if n == (touch_thread - 1) {
                        npage
                    } else {
                        per_npage * (n + 1)
                    };

                    let mut per_addr = addr + (start_npage * PAGE_SIZE);
                    let should_stop = should_stop.clone();
                    let handler = thread::Builder::new()
                        .name("PreallocThread".to_string())
                        .spawn(move || {
                            info!(
                                "PreallocThread start start_npage: {:?}, end_npage: {:?}, per_addr: {:?}, thread_number: {:?}",
                                start_npage, end_npage, per_addr, touch_thread
                            );
                            for _ in start_npage..end_npage {
                                if *should_stop.load_full() {
                                    info!(
                                        "PreallocThread stop start_npage: {:?}, end_npage: {:?}, per_addr: {:?}, thread_number: {:?}",
                                        start_npage, end_npage, per_addr, touch_thread
                                    );
                                    break;
                                }
                                // async pre-allocate :
                                // read first byte of a page, using compare_exchange to prefault, and do not wait for this task to be completed.
                                // compare_exchange here returns type of Result<u8> that:
                                // 1. OK(_) => byte has not been changed, write it back to the address to trigger page fault.
                                // 2. Err(_) => byte has changed during this period, so we don't need to write it again.
                                let addr_ptr = per_addr as *mut u8;
                                let read_byte = unsafe {
                                    std::ptr::read_volatile(addr_ptr)
                                };
                                let atomic_u8 : &AtomicU8 = unsafe {&*(addr_ptr as *mut AtomicU8)};
                                if let Err(x) = atomic_u8.compare_exchange(read_byte, read_byte, Ordering::SeqCst, Ordering::SeqCst) {
                                    trace!("addr_ptr: {:?}, read_byte: {:?}, x: {:?}", addr_ptr, read_byte, x);
                                }
                                per_addr += PAGE_SIZE;
                            }
                            info!(
                                "PreallocThread done start_npage: {:?}, end_npage: {:?}, per_addr: {:?}, thread_number: {:?}",
                                start_npage, end_npage, per_addr, touch_thread
                            );
                        })
                        .expect("Failure occurred in PreallocThread.");

                    handlers.push(handler)
                }
                opt_should_stop.replace(should_stop);
            }
        }

        Ok((
            GuestRegionImpl::new(mmap_reg, region.start_addr())
                .map_err(AddressManagerError::CreateGuestMemory)?,
            handlers,
            opt_should_stop,
        ))
    }

    /// Get the address space object
    pub fn get_address_space(&self) -> Option<&AddressSpace> {
        self.address_space.as_ref()
    }

    /// Get the default guest memory object, which will be used to access virtual machine's default
    /// guest memory.
    pub fn get_vm_as(&self) -> Option<&GuestAddressSpaceImpl> {
        self.vm_as.as_ref()
    }

    /// Get the base to slot map
    pub fn get_base_to_slot_map(&self) -> Arc<Mutex<HashMap<u64, u32>>> {
        self.base_to_slot.clone()
    }

    /// get numa nodes infos from address space manager.
    pub fn get_numa_nodes(&self) -> &BTreeMap<u32, NumaNode> {
        &self.numa_nodes
    }

    /// add cpu and memory numa informations to BtreeMap
    fn insert_into_numa_nodes(
        &mut self,
        region: &Arc<AddressSpaceRegion>,
        guest_numa_node_id: u32,
        vcpu_ids: &[u32],
    ) {
        if let Some(node) = self.numa_nodes.get_mut(&guest_numa_node_id) as Option<&mut NumaNode> {
            node.add_info(&NumaNodeInfo {
                base: region.start_addr(),
                size: region.len(),
            });
            node.add_vcpu_ids(vcpu_ids);
        } else {
            let mut node = NumaNode::new();
            node.add_info(&NumaNodeInfo {
                base: region.start_addr(),
                size: region.len(),
            });
            node.add_vcpu_ids(vcpu_ids);
            self.numa_nodes.insert(guest_numa_node_id as u32, node);
        }
    }

    /// get address boundary from address space manager.
    pub fn get_boundary(&self) -> Result<AddressSpaceLayout, AddressManagerError> {
        self.address_space
            .as_ref()
            .map(|v| v.layout())
            .ok_or(AddressManagerError::AddressSpaceNotInitialized)
    }

    /// Wait prealloc threads done.
    /// If stop is true, force stop the threads
    pub fn wait_prealloc(&mut self, stop: bool) -> Result<(), AddressManagerError> {
        if stop {
            while let Some(should_stop) = self.prealloc_should_stops.pop() {
                should_stop.store(Arc::new(true));
            }
        }
        while let Some(handlers) = self.prealloc_handlers.pop() {
            if let Err(e) = handlers.join() {
                error!("wait_prealloc join fail {:?}", e);
                return Err(AddressManagerError::JoinFail);
            }
        }
        Ok(())
    }
}

impl Default for AddressSpaceMgr {
    /// Create a new empty AddressSpaceMgr
    fn default() -> Self {
        AddressSpaceMgr {
            address_space: None,
            vm_as: None,
            base_to_slot: Arc::new(Mutex::new(HashMap::new())),
            prealloc_handlers: Vec::new(),
            prealloc_should_stops: Vec::new(),
            numa_nodes: BTreeMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Deref;

    use vm_memory::{Bytes, GuestAddressSpace, GuestMemory, GuestMemoryRegion};
    use vmm_sys_util::tempfile::TempFile;

    use super::*;

    #[test]
    fn test_create_address_space() {
        let res_mgr = ResourceManager::new(None);
        let mut as_mgr = AddressSpaceMgr::default();
        let mem_size = 128 << 20;
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: vec![1, 2],
        }];
        as_mgr
            .create_address_space(
                &res_mgr,
                0,
                "shmem",
                "",
                &numa_region_infos,
                None,
                false,
                false,
            )
            .unwrap();

        let vm_as = as_mgr.get_vm_as().unwrap();
        let guard = vm_as.memory();
        let gmem = guard.deref();
        assert_eq!(gmem.num_regions(), 1);

        let reg = gmem.find_region(GuestAddress(mem_size - 1)).unwrap();
        assert_eq!(reg.start_addr(), GuestAddress(0x0));
        assert_eq!(reg.len(), mem_size);
        assert!(gmem.find_region(GuestAddress(mem_size)).is_none());
        assert!(reg.file_offset().is_some());

        let buf = [0x1u8, 0x2u8, 0x3u8, 0x4u8, 0x5u8];
        gmem.write_slice(&buf, GuestAddress(0x0)).unwrap();

        // Update middle of mapped memory region
        let mut val = 0xa5u8;
        gmem.write_obj(val, GuestAddress(0x1)).unwrap();
        val = gmem.read_obj(GuestAddress(0x1)).unwrap();
        assert_eq!(val, 0xa5);
        val = gmem.read_obj(GuestAddress(0x0)).unwrap();
        assert_eq!(val, 1);
        val = gmem.read_obj(GuestAddress(0x2)).unwrap();
        assert_eq!(val, 3);
        val = gmem.read_obj(GuestAddress(0x5)).unwrap();
        assert_eq!(val, 0);

        // Read ahead of mapped memory region
        assert!(gmem.read_obj::<u8>(GuestAddress(mem_size)).is_err());

        let res_mgr = ResourceManager::new(None);
        let mut as_mgr = AddressSpaceMgr::default();
        let mem_size = dbs_boot::layout::MMIO_LOW_START + (1 << 30);
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: vec![1, 2],
        }];
        as_mgr
            .create_address_space(
                &res_mgr,
                0,
                "shmem",
                "",
                &numa_region_infos,
                None,
                false,
                false,
            )
            .unwrap();
        let vm_as = as_mgr.get_vm_as().unwrap();
        let guard = vm_as.memory();
        let gmem = guard.deref();
        assert_eq!(gmem.num_regions(), 2);

        // Test dropping GuestMemoryMmap object releases all resources.
        for _ in 0..10000 {
            let res_mgr = ResourceManager::new(None);
            let mut as_mgr = AddressSpaceMgr::default();
            let mem_size = 1 << 20;
            let numa_region_infos = vec![NumaRegionInfo {
                size: mem_size >> 20,
                host_numa_node_id: None,
                guest_numa_node_id: Some(0),
                vcpu_ids: vec![1, 2],
            }];
            assert!(as_mgr
                .create_address_space(
                    &res_mgr,
                    0,
                    "shmem",
                    "",
                    &numa_region_infos,
                    None,
                    false,
                    false
                )
                .is_ok());
        }
        let file = TempFile::new().unwrap().into_file();
        let fd = file.as_raw_fd();
        // fd should be small enough if there's no leaking of fds.
        assert!(fd < 1000);
    }

    #[test]
    fn test_address_space_mgr_get_boundary() {
        let boundary = AddressSpaceLayout::new(
            *dbs_boot::layout::GUEST_PHYS_END,
            dbs_boot::layout::GUEST_MEM_START,
            *dbs_boot::layout::GUEST_MEM_END,
        );
        let res_mgr = ResourceManager::new(None);
        let mut as_mgr = AddressSpaceMgr::default();
        let mem_size = 128 << 20;
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: vec![1, 2],
        }];
        as_mgr
            .create_address_space(
                &res_mgr,
                0,
                "shmem",
                "",
                &numa_region_infos,
                None,
                false,
                false,
            )
            .unwrap();
        assert_eq!(as_mgr.get_boundary().unwrap(), boundary);
    }

    #[test]
    fn test_address_space_mgr_get_numa_nodes() {
        let res_mgr = ResourceManager::new(None);
        let mut as_mgr = AddressSpaceMgr::default();
        let mem_size = 128 << 20;
        let cpu_vec = vec![1, 2];
        let numa_region_infos = vec![NumaRegionInfo {
            size: mem_size >> 20,
            host_numa_node_id: None,
            guest_numa_node_id: Some(0),
            vcpu_ids: cpu_vec.clone(),
        }];
        as_mgr
            .create_address_space(
                &res_mgr,
                0,
                "shmem",
                "",
                &numa_region_infos,
                None,
                false,
                false,
            )
            .unwrap();
        let mut numa_node = NumaNode::new();
        numa_node.add_info(&NumaNodeInfo {
            base: GuestAddress(0),
            size: mem_size,
        });
        numa_node.add_vcpu_ids(&cpu_vec);

        assert_eq!(*as_mgr.get_numa_nodes().get(&0).unwrap(), numa_node);
    }
}
