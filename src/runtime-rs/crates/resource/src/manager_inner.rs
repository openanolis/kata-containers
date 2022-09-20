// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::sync::Arc;

use crate::{cpu_mem::CpuMemResource, resource_persist::ResourceState};
use agent::{Agent, OnlineCPUMemRequest, Storage};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use hypervisor::Hypervisor;
use kata_types::config::TomlConfig;
use kata_types::mount::Mount;
use oci::LinuxResources;
use persist::sandbox_persist::Persist;

use crate::{
    cgroups::{CgroupArgs, CgroupsResource},
    manager::ManagerArgs,
    network::{self, Network},
    rootfs::{RootFsResource, Rootfs},
    share_fs::{self, ShareFs},
    volume::{Volume, VolumeResource},
    ResourceConfig,
};

const ACPI_MEMORY_HOTPLUG_FACTOR: u32 = 48;

pub(crate) struct ResourceManagerInner {
    sid: String,
    toml_config: Arc<TomlConfig>,
    agent: Arc<dyn Agent>,
    hypervisor: Arc<dyn Hypervisor>,
    network: Option<Arc<dyn Network>>,
    share_fs: Option<Arc<dyn ShareFs>>,

    pub rootfs_resource: RootFsResource,
    pub volume_resource: VolumeResource,
    pub cgroups_resource: CgroupsResource,
    pub cpu_mem_resource: CpuMemResource,
}

impl ResourceManagerInner {
    pub(crate) fn new(
        sid: &str,
        agent: Arc<dyn Agent>,
        hypervisor: Arc<dyn Hypervisor>,
        toml_config: Arc<TomlConfig>,
    ) -> Result<Self> {
        let cgroups_resource = CgroupsResource::new(sid, &toml_config)?;
        let cpu_mem_resource = CpuMemResource::new(toml_config.clone())?;
        Ok(Self {
            sid: sid.to_string(),
            toml_config,
            agent,
            hypervisor,
            network: None,
            share_fs: None,
            rootfs_resource: RootFsResource::new(),
            volume_resource: VolumeResource::new(),
            cgroups_resource,
            cpu_mem_resource,
        })
    }

    pub fn config(&self) -> Arc<TomlConfig> {
        self.toml_config.clone()
    }

    pub async fn prepare_before_start_vm(
        &mut self,
        device_configs: Vec<ResourceConfig>,
    ) -> Result<()> {
        for dc in device_configs {
            match dc {
                ResourceConfig::ShareFs(c) => {
                    let share_fs = share_fs::new(&self.sid, &c).context("new share fs")?;
                    share_fs
                        .setup_device_before_start_vm(self.hypervisor.as_ref())
                        .await
                        .context("setup share fs device before start vm")?;
                    self.share_fs = Some(share_fs);
                }
                ResourceConfig::Network(c) => {
                    let d = network::new(&c).await.context("new network")?;
                    d.setup(self.hypervisor.as_ref())
                        .await
                        .context("setup network")?;
                    self.network = Some(d)
                }
            };
        }

        Ok(())
    }

    async fn handle_interfaces(&self, network: &dyn Network) -> Result<()> {
        for i in network.interfaces().await.context("get interfaces")? {
            // update interface
            info!(sl!(), "update interface {:?}", i);
            self.agent
                .update_interface(agent::UpdateInterfaceRequest { interface: Some(i) })
                .await
                .context("update interface")?;
        }

        Ok(())
    }

    async fn handle_neighbours(&self, network: &dyn Network) -> Result<()> {
        let neighbors = network.neighs().await.context("neighs")?;
        if !neighbors.is_empty() {
            info!(sl!(), "update neighbors {:?}", neighbors);
            self.agent
                .add_arp_neighbors(agent::AddArpNeighborRequest {
                    neighbors: Some(agent::ARPNeighbors { neighbors }),
                })
                .await
                .context("update neighbors")?;
        }
        Ok(())
    }

    async fn handle_routes(&self, network: &dyn Network) -> Result<()> {
        let routes = network.routes().await.context("routes")?;
        if !routes.is_empty() {
            info!(sl!(), "update routes {:?}", routes);
            self.agent
                .update_routes(agent::UpdateRoutesRequest {
                    route: Some(agent::Routes { routes }),
                })
                .await
                .context("update routes")?;
        }
        Ok(())
    }

    pub async fn setup_after_start_vm(&mut self) -> Result<()> {
        if let Some(share_fs) = self.share_fs.as_ref() {
            share_fs
                .setup_device_after_start_vm(self.hypervisor.as_ref())
                .await
                .context("setup share fs device after start vm")?;
        }

        if let Some(network) = self.network.as_ref() {
            let network = network.as_ref();
            self.handle_interfaces(network)
                .await
                .context("handle interfaces")?;
            self.handle_neighbours(network)
                .await
                .context("handle neighbors")?;
            self.handle_routes(network).await.context("handle routes")?;
        }
        Ok(())
    }

    pub async fn get_storage_for_sandbox(&self) -> Result<Vec<Storage>> {
        let mut storages = vec![];
        if let Some(d) = self.share_fs.as_ref() {
            let mut s = d.get_storages().await.context("get storage")?;
            storages.append(&mut s);
        }
        Ok(storages)
    }

    pub async fn handler_rootfs(
        &self,
        cid: &str,
        bundle_path: &str,
        rootfs_mounts: &[Mount],
    ) -> Result<Arc<dyn Rootfs>> {
        self.rootfs_resource
            .handler_rootfs(&self.share_fs, cid, bundle_path, rootfs_mounts)
            .await
    }

    pub async fn handler_volumes(
        &self,
        cid: &str,
        oci_mounts: &[oci::Mount],
    ) -> Result<Vec<Arc<dyn Volume>>> {
        self.volume_resource
            .handler_volumes(&self.share_fs, cid, oci_mounts)
            .await
    }

    pub async fn update_cgroups(
        &self,
        cid: &str,
        linux_resources: Option<&LinuxResources>,
    ) -> Result<()> {
        self.cgroups_resource
            .update_cgroups(cid, linux_resources, self.hypervisor.as_ref())
            .await
    }

    pub async fn delete_cgroups(&self) -> Result<()> {
        self.cgroups_resource.delete().await
    }

    pub async fn dump(&self) {
        self.rootfs_resource.dump().await;
        self.volume_resource.dump().await;
    }

    pub(crate) async fn sandbox_cpu_mem_info(&self) -> Result<CpuMemResource> {
        Ok(self.cpu_mem_resource)
    }

    pub async fn update_cpu_resource(&mut self, new_vcpus: u32) -> Result<()> {
        if self.toml_config.runtime.static_resource_mgmt {
            warn!(sl!(), "static resource mgmt is on, no update allowed");
            return Ok(());
        }
        let old_vcpus = self.cpu_mem_resource.current_vcpu()? as u32;
        let (old, new) = self
            .hypervisor
            .resize_vcpu(old_vcpus, new_vcpus)
            .await
            .map_err(|e| {
                match e {
                    // todo: handle err if guest does not support hotplug
                    _ => return anyhow!("error on resizing vcpu"),
                }
            })?;
        self.cpu_mem_resource
            .update_current_vcpu(new as i32)
            .context("resource mgr: failed to update current vcpu")?;

        // if vcpus were increased, ask the agent to online them inside the sandbox
        if old < new {
            let added = new - old;
            info!(sl!(), "request to onlineCpuMem with {:?} cpus", added);
            self.agent
                .online_cpu_mem(OnlineCPUMemRequest {
                    wait: false,
                    nb_cpus: added,
                    cpu_only: true,
                })
                .await
                .context("agent failed to online cpu")?;
        }

        Ok(())
    }

    pub async fn update_memory_resource(
        &mut self,
        new_mem_mb: u32,
        _swap_sz_byte: i64,
    ) -> Result<()> {
        if self.toml_config.runtime.static_resource_mgmt {
            warn!(sl!(), "static resource mgmt is on, no update allowed");
            return Ok(());
        }

        // TODO: if block device hotplug is supported, setup swap space
        // if swap_sz_byte > 0 {
        //     self.hypervisor.setupSwap(swap_sz_byte);
        // }

        // Update Memory --
        // If we're using ACPI hotplug for memory, there's a limitation on the amount of memory which can be hotplugged at a single time.
        // We must have enough free memory in the guest kernel to cover 64bytes per (4KiB) page of memory added for mem_map.
        // See https://github.com/kata-containers/kata-containers/issues/4847 for more details.
        // For a typical pod lifecycle, we expect that each container is added when we start the workloads. Based on this, we'll "assume" that majority
        // of the guest memory is readily available. From experimentation, we see that we can add approximately 48 times what is already provided to
        // the guest workload. For example, a 256 MiB guest should be able to accommodate hotplugging 12 GiB of memory.
        //
        // If virtio-mem is being used, there isn't such a limitation - we can hotplug the maximum allowed memory at a single time.
        //

        // new_mb is the memory each time to hotplug
        let mut new_mb = new_mem_mb;
        // end_mb is the final memory size
        let end_mb = new_mem_mb;

        loop {
            let current_mem = self.cpu_mem_resource.current_mem_mb()?;

            // for each byte of memory in guest, it can hotplug 8 byte memory
            let max_hotplug_mem_mb = match self
                .hypervisor
                .hypervisor_config()
                .await
                .memory_info
                .enable_virtio_mem
            {
                false => current_mem * ACPI_MEMORY_HOTPLUG_FACTOR,
                true => end_mb,
            };

            // verify if the delta exceeds the max hotpluggable memory
            let delta_mb = end_mb - current_mem;
            if delta_mb > max_hotplug_mem_mb {
                new_mb = current_mem + max_hotplug_mem_mb;
            } else {
                new_mb = end_mb;
            }

            self.hypervisor
                .resize_memory(new_mb)
                .await
                .context("failed to update memory")?;

            if new_mb == end_mb {
                break;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Persist for ResourceManagerInner {
    type State = ResourceState;
    type ConstructorArgs = ManagerArgs;

    /// Save a state of ResourceManagerInner
    async fn save(&self) -> Result<Self::State> {
        let mut endpoint_state = vec![];
        if let Some(network) = &self.network {
            if let Some(ens) = network.save().await {
                endpoint_state = ens;
            }
        }
        let cgroup_state = self.cgroups_resource.save().await?;
        Ok(ResourceState {
            endpoint: endpoint_state,
            cgroup_state: Some(cgroup_state),
        })
    }

    /// Restore ResourceManagerInner
    async fn restore(
        resource_args: Self::ConstructorArgs,
        resource_state: Self::State,
    ) -> Result<Self> {
        let args = CgroupArgs {
            sid: resource_args.sid.clone(),
            config: resource_args.config,
        };
        Ok(Self {
            sid: resource_args.sid,
            agent: resource_args.agent,
            hypervisor: resource_args.hypervisor,
            network: None,
            share_fs: None,
            rootfs_resource: RootFsResource::new(),
            volume_resource: VolumeResource::new(),
            cgroups_resource: CgroupsResource::restore(
                args,
                resource_state.cgroup_state.unwrap_or_default(),
            )
            .await?,
            toml_config: Arc::new(TomlConfig::default()),
            cpu_mem_resource: CpuMemResource::default(),
        })
    }
}
