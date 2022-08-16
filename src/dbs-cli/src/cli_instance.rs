// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{
    fs::{File, OpenOptions},
    os::unix::{io::IntoRawFd, prelude::AsRawFd},
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Mutex, RwLock,
    },
    thread,
    path::{Path, PathBuf}
};

use anyhow::{anyhow, Context, Result};
use dragonball::{
    api::v1::{
        BlockDeviceConfigInfo, BootSourceConfig, FsDeviceConfigInfo, FsMountConfigInfo,
        InstanceInfo, InstanceState, VirtioNetDeviceConfigInfo, VmmAction, VmmActionError, VmmData,
        VmmRequest, VmmResponse, VmmService, VsockDeviceConfigInfo, BootSourceConfigError, DEFAULT_KERNEL_CMDLINE
    },
    vm::{VmConfigInfo, CpuTopology, KernelConfigInfo},
    Vmm,
    event_manager::EventManager,
    StartMicroVmError
};
use nix::sched::{setns, CloneFlags};
use seccompiler::BpfProgram;
use vmm_sys_util::eventfd::EventFd;

use hypervisor::ShareFsOperation;

use crate::parser::DBSArgs;

const SERIAL_PATH: &str = "/tmp/dbs-cli";


pub enum Request {
    Sync(VmmAction),
}

const DRAGONBALL_VERSION: &str = env!("CARGO_PKG_VERSION");
const REQUEST_RETRY: u32 = 500;
const KVM_DEVICE: &str = "/dev/kvm";

pub struct CliInstance {
    /// VMM instance info directly accessible from runtime
    vmm_shared_info: Arc<RwLock<InstanceInfo>>,
    to_vmm: Option<Sender<VmmRequest>>,
    from_vmm: Option<Receiver<VmmResponse>>,
    to_vmm_fd: EventFd,
    seccomp: BpfProgram,
    vmm_thread: Option<thread::JoinHandle<Result<i32>>>,
}

impl CliInstance {
    pub fn new(id: &str) -> Self {
        let vmm_shared_info = Arc::new(RwLock::new(
            InstanceInfo::new(
                String::from(id),
                DRAGONBALL_VERSION.to_string(),
            )
        ));

        let to_vmm_fd = EventFd::new(libc::EFD_NONBLOCK)
            .unwrap_or_else(|_| panic!("Failed to create eventfd for vmm {}", id));

        CliInstance {
            vmm_shared_info,
            to_vmm: None,
            from_vmm: None,
            to_vmm_fd,
            seccomp: vec![],
            vmm_thread: None,
        }
    }

    pub fn get_shared_info(&self) -> Arc<RwLock<InstanceInfo>> {
        self.vmm_shared_info.clone()
    }

    fn set_instance_id(&mut self, id: &str) {
        let share_info_lock = self.vmm_shared_info.clone();
        share_info_lock.write().unwrap().id = String::from(id);
    }

    pub fn run_vmm_server(&mut self, id: &str, args: &DBSArgs) -> Result<()> {
        let kvm = OpenOptions::new().read(true).write(true).open(KVM_DEVICE)?;

        let (to_vmm, from_runtime) = channel();
        let (to_runtime, from_vmm) = channel();

        self.set_instance_id(id);

        let vmm_service = VmmService::new(from_runtime, to_runtime);

        self.to_vmm = Some(to_vmm);
        self.from_vmm = Some(from_vmm);

        let api_event_fd2 = self.to_vmm_fd.try_clone().expect("Failed to dup eventfd");
        let vmm = Vmm::new(
            self.vmm_shared_info.clone(),
            api_event_fd2,
            self.seccomp.clone(),
            self.seccomp.clone(),
            Some(kvm.into_raw_fd()),
        )
            .expect("Failed to start vmm");

        // configuration
        let mut vm_config = VmConfigInfo {
            vcpu_count: args.create_args.vcpu,
            max_vcpu_count: args.create_args.max_vcpu,
            cpu_pm: args.create_args.cpu_pm.clone(),
            cpu_topology: CpuTopology {
                threads_per_core: args.create_args.cpu_topology.threads_per_core,
                cores_per_die: args.create_args.cpu_topology.cores_per_die,
                dies_per_socket: args.create_args.cpu_topology.dies_per_socket,
                sockets: args.create_args.cpu_topology.sockets,
            },
            vpmu_feature: 0,
            mem_type: args.create_args.mem_type.clone(),
            mem_file_path: args.create_args.mem_file_path.clone(),
            mem_size_mib: args.create_args.mem_size,
            serial_path: Some(String::from(SERIAL_PATH)),
        };
        // check the existence of the serial path (rm it if exist)
        let serial_file = Path::new(SERIAL_PATH);
        if serial_file.exists() {
            std::fs::remove_file(serial_file).unwrap();
        }

        // boot source
        let boot_source_config = BootSourceConfig {
            kernel_path: args.boot_args.kernel_path.clone(),
            initrd_path: args.boot_args.initrd_path.clone(),
            boot_args: args.boot_args.boot_args.clone(),
        };

        // rootfs
        let mut block_device_config_info = BlockDeviceConfigInfo::default();
        block_device_config_info = BlockDeviceConfigInfo {
            drive_id: String::from("rootfs"),
            path_on_host: PathBuf::from(&args.boot_args.rootfs_args.rootfs),
            is_root_device: args.boot_args.rootfs_args.is_root,
            is_read_only: args.boot_args.rootfs_args.is_read_only,
            ..block_device_config_info
        };

        /// put configuration to service
        let vmm_thread = thread::Builder::new()
            .name("CLI".to_owned())
            .spawn(move || {
                println!("Begin event handling.");
                let exit_code =
                    Vmm::run_vmm_event_loop(Arc::new(Mutex::new(vmm)), vmm_service);
                println!("run vmm thread exited: {}", exit_code);
                Ok(exit_code)
            }).expect("failed to start vmm thread");
        self.vmm_thread = Some(vmm_thread);

        /// put configuration to service
        println!("Begin configuring");
        // set vm configuration
        self.set_vm_configuration(vm_config).expect("failed to set vm configuration");

        // set boot source config
        self.put_boot_source(boot_source_config).expect("failed to set boot source");

        // set rootfs
        self.insert_block_device(block_device_config_info).expect("failed to set block device");

        // start micro-vm
        self.instance_start().expect("failed to start micro-vm");

        println!("Configuration Complete.");

        self.vmm_thread.take().unwrap().join();

        Ok(())
    }

    pub fn put_boot_source(&self, boot_source_cfg: BootSourceConfig) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::ConfigureBootSource(
            boot_source_cfg,
        )))
            .context("Failed to configure boot source")?;
        Ok(())
    }

    pub fn instance_start(&self) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::StartMicroVm))
            .context("Failed to start MicroVm")?;
        Ok(())
    }

    pub fn is_uninitialized(&self) -> bool {
        let share_info = self
            .vmm_shared_info
            .read()
            .expect("Failed to read share_info due to poisoned lock");
        matches!(share_info.state, InstanceState::Uninitialized)
    }

    pub fn is_running(&self) -> Result<()> {
        let share_info_lock = self.vmm_shared_info.clone();
        let share_info = share_info_lock
            .read()
            .expect("Failed to read share_info due to poisoned lock");
        if let InstanceState::Running = share_info.state {
            return Ok(());
        }
        Err(anyhow!("vmm is not running"))
    }

    pub fn get_machine_info(&self) -> Result<Box<VmConfigInfo>> {
        if let Ok(VmmData::MachineConfiguration(vm_config)) =
        self.handle_request(Request::Sync(VmmAction::GetVmConfiguration))
        {
            return Ok(vm_config);
        }
        Err(anyhow!("Failed to get machine info"))
    }

    pub fn insert_block_device(&self, device_cfg: BlockDeviceConfigInfo) -> Result<()> {
        self.handle_request_with_retry(Request::Sync(VmmAction::InsertBlockDevice(
            device_cfg.clone(),
        )))
            .with_context(|| format!("Failed to insert block device {:?}", device_cfg))?;
        Ok(())
    }

    pub fn remove_block_device(&self, id: &str) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::RemoveBlockDevice(id.to_string())))
            .with_context(|| format!("Failed to remove block device {:?}", id))?;
        Ok(())
    }

    pub fn set_vm_configuration(&self, vm_config: VmConfigInfo) -> Result<()> {
        self.handle_request(Request::Sync(VmmAction::SetVmConfiguration(
            vm_config.clone(),
        )))
            .with_context(|| format!("Failed to set vm configuration {:?}", vm_config))?;
        Ok(())
    }

    fn send_request(&self, vmm_action: VmmAction) -> Result<VmmResponse> {
        if let Some(ref to_vmm) = self.to_vmm {
            to_vmm
                .send(Box::new(vmm_action.clone()))
                .with_context(|| format!("Failed to send  {:?} via channel ", vmm_action))?;
        } else {
            return Err(anyhow!("to_vmm is None"));
        }

        //notify vmm action
        if let Err(e) = self.to_vmm_fd.write(1) {
            return Err(anyhow!("failed to notify vmm: {}", e));
        }

        if let Some(from_vmm) = self.from_vmm.as_ref() {
            match from_vmm.recv() {
                Err(e) => Err(anyhow!("vmm recv err: {}", e)),
                Ok(vmm_outcome) => Ok(vmm_outcome),
            }
        } else {
            Err(anyhow!("from_vmm is None"))
        }
    }

    fn handle_request(&self, req: Request) -> Result<VmmData> {
        let Request::Sync(vmm_action) = req;
        match self.send_request(vmm_action) {
            Ok(vmm_outcome) => match *vmm_outcome {
                Ok(vmm_data) => Ok(vmm_data),
                Err(vmm_action_error) => Err(anyhow!("vmm action error: {:?}", vmm_action_error)),
            },
            Err(e) => Err(e),
        }
    }

    fn handle_request_with_retry(&self, req: Request) -> Result<VmmData> {
        let Request::Sync(vmm_action) = req;
        for count in 0..REQUEST_RETRY {
            match self.send_request(vmm_action.clone()) {
                Ok(vmm_outcome) => match *vmm_outcome {
                    Ok(vmm_data) => {
                        return Ok(vmm_data);
                    }
                    Err(vmm_action_error) => {
                        if let VmmActionError::UpcallNotReady = vmm_action_error {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            continue;
                        } else {
                            return Err(vmm_action_error.into());
                        }
                    }
                },
                Err(err) => {
                    return Err(err);
                }
            }
        }
        return Err(anyhow::anyhow!(
            "After {} attempts, it still doesn't work.",
            REQUEST_RETRY
        ));
    }
}