use crate::{
    utils, BlockDevice, Device, DeviceArgument, GenericConfig, GenericDevice, Hypervisor, IoLimits,
};
use agent::types::Device as AgentDevice;
use anyhow::{anyhow, Result};
use ini::Ini;
use kata_sys_util::rand;
use std::{collections::HashMap, str, sync::Arc};
use tokio::sync::Mutex;
/// VirtioMmio indicates block driver is virtio-mmio based
pub const VIRTIO_MMIO: &str = "virtio-mmio";
pub const VIRTIO_BLOCK: &str = "virtio-blk";
pub const VFIO: &str = "vfio";
const SYS_DEV_PREFIX: &str = "/sys/dev";
pub const KATA_MMIO_BLK_DEV_TYPE: &str = "mmioblk";
pub const KATA_BLK_DEV_TYPE: &str = "blk";
type ArcBoxDevice = Arc<Mutex<Box<dyn Device>>>;

pub struct DeviceManager {
    block_driver: String,
    devices: HashMap<String, ArcBoxDevice>,
    block_index: u64,
    released_index: Vec<u64>,
}

impl DeviceManager {
    pub fn new(block_driver: &str) -> Self {
        let driver = match block_driver {
            VIRTIO_MMIO => VIRTIO_MMIO,
            VIRTIO_BLOCK => VIRTIO_BLOCK,
            VFIO => VFIO,
            _ => "",
        };
        Self {
            block_driver: String::from(driver),
            devices: HashMap::new(),
            block_index: 1,
            released_index: vec![],
        }
    }

    pub async fn try_add_device(
        &mut self,
        dev_info: &mut GenericConfig,
        h: &dyn Hypervisor,
    ) -> Result<String> {
        let dev = self.try_create_device(dev_info).await?;
        let id = dev.lock().await.device_id().await.to_string();
        let skip = dev.lock().await.bump_attach_count(true).await?;
        if skip {
            dev.lock().await.reference().await;
            return Ok(id);
        }
        // prepare arguments to attach device
        let index = self.get_and_set_sandbox_block_index()?;
        let drive_name = utils::get_virt_drive_name(index as i32)?;
        info!(sl!(), "index: {}, drive_name: {}", index, drive_name);
        if let Err(e) = self
            .attach_device(
                &id,
                h,
                DeviceArgument {
                    index: Some(index),
                    drive_name: Some(drive_name),
                    ..Default::default()
                },
            )
            .await
        {
            dev.lock().await.bump_attach_count(false).await?;
            self.unset_sandbox_block_index(index.clone())?;
            self.devices.remove(&id);
            return Err(e);
        }
        // reference device if no error happens
        dev.lock().await.reference().await;
        Ok(id)
    }

    pub async fn generate_agent_device(&self, device_id: String) -> Result<AgentDevice> {
        // Safe because we just attached the device
        let dev = self.get_device_by_id(&device_id).await.unwrap();
        let base_info = dev.lock().await.get_device_info().await?;
        let mut device = AgentDevice {
            container_path: base_info.container_path.clone(),
            ..Default::default()
        };

        match self.get_block_driver().await {
            VIRTIO_MMIO => {
                if let Some(path) = base_info.virt_path {
                    device.id = device_id;
                    device.field_type = KATA_MMIO_BLK_DEV_TYPE.to_string();
                    device.vm_path = path.clone();
                }
            }
            VIRTIO_BLOCK => {
                if let Some(path) = base_info.pci_addr {
                    device.id = device_id;
                    device.field_type = KATA_BLK_DEV_TYPE.to_string();
                    device.vm_path = path.clone();
                }
            }
            _ => (),
        }
        Ok(device)
    }

    pub async fn get_block_driver(&self) -> &str {
        self.block_driver.as_str()
    }

    pub async fn get_device_guest_path(&self, id: &str) -> Option<String> {
        if let Some(device) = self.devices.get(id) {
            if let Ok(dev_info) = device.lock().await.get_device_info().await {
                return dev_info.virt_path;
            }
        }
        None
    }

    async fn attach_device(
        &mut self,
        id: &str,
        h: &dyn Hypervisor,
        da: DeviceArgument,
    ) -> Result<()> {
        if let Some(dev) = self.devices.get(id) {
            dev.lock().await.attach(h, da).await?;
        } else {
            return Err(anyhow!(
                "device with specified ID hasn't been created. {}",
                id
            ));
        }
        Ok(())
    }

    async fn try_create_device(&mut self, dev_info: &mut GenericConfig) -> Result<ArcBoxDevice> {
        if dev_info.major != 0 || dev_info.minor != 0 {
            let path = get_host_path(dev_info)?;
            dev_info.host_path = path;
            info!(sl!(), "device info: {}", dev_info.host_path);
        }

        if let Some(dev) = self
            .find_device(
                dev_info.major,
                dev_info.minor,
                dev_info.host_path.as_str(),
                dev_info.bdf.as_ref(),
            )
            .await
        {
            return Ok(dev);
        }
        // device ID must be generated by manager instead of device itself
        // in case of ID collision
        let id = self.new_device_id()?;
        dev_info.id = id.clone();

        let dev: ArcBoxDevice = if is_block(dev_info) {
            dev_info
                .driver_options
                .insert("block-driver".to_string(), self.block_driver.clone());
            Arc::new(Mutex::new(Box::new(BlockDevice::new(dev_info))))
        } else {
            Arc::new(Mutex::new(Box::new(GenericDevice::new(dev_info))))
        };
        self.devices.insert(id, dev.clone());
        Ok(dev)
    }

    async fn find_device(
        &self,
        major: i64,
        minor: i64,
        host_path: &str,
        bdf: Option<&String>,
    ) -> Option<ArcBoxDevice> {
        if major >= 0 && minor >= 0 {
            return self.find_device_by_major_minor(major, minor).await;
        }

        if bdf.is_some() {
            return self.find_device_by_bdf(bdf).await;
        }

        // the raw file as block device case
        self.find_device_by_host_path(host_path).await
    }

    async fn find_device_by_major_minor(&self, major: i64, minor: i64) -> Option<ArcBoxDevice> {
        for dev in self.devices.values() {
            let mm = dev.lock().await.get_major_minor().await;
            if mm.0 == major && mm.1 == minor {
                return Some(dev.clone());
            }
        }
        None
    }

    async fn find_device_by_bdf(&self, bdf: Option<&String>) -> Option<ArcBoxDevice> {
        for dev in self.devices.values() {
            if dev.lock().await.get_bdf().await == bdf {
                return Some(dev.clone());
            }
        }
        None
    }

    async fn find_device_by_host_path(&self, host_path: &str) -> Option<ArcBoxDevice> {
        for dev in self.devices.values() {
            if host_path == dev.lock().await.get_host_path().await {
                return Some(dev.clone());
            }
        }
        None
    }

    async fn get_device_by_id(&self, id: &str) -> Option<ArcBoxDevice> {
        self.devices.get(id).map(Arc::clone)
    }

    fn new_device_id(&self) -> Result<String> {
        for _ in 0..5 {
            let rand_bytes = rand::RandomBytes::new(8);
            let id = format!("{:x}", rand_bytes);
            if self.devices.get(&id).is_none() {
                return Ok(id.to_string());
            }
        }
        Err(anyhow!("ID are exhausted"))
    }

    // get_and_set_sandbox_block_index retrieves sandbox block index and increments it for
    // subsequent accesses. This index is used to maintain the index at which a
    // block device is assigned to a container in the sandbox.
    fn get_and_set_sandbox_block_index(&mut self) -> Result<u64> {
        let current_index = self.block_index;

        if !self.released_index.is_empty() {
            match self.released_index.pop() {
                Some(index) => Ok(index),
                None => Err(anyhow!("failed to get block index")),
            }
        } else {
            self.block_index += 1;
            Ok(current_index)
        }
    }

    // unsetSandboxBlockIndex deletes the current sandbox block index from BlockIndexMap.
    // This is used to recover from failure while adding a block device.
    fn unset_sandbox_block_index(&mut self, index: u64) -> Result<()> {
        self.released_index.push(index);
        self.released_index.sort_by(|a, b| b.cmp(a));
        Ok(())
    }
}

pub fn new_device_info(
    device: &oci::LinuxDevice,
    bdf: Option<String>,
    io_limits: Option<IoLimits>,
) -> Result<GenericConfig> {
    let allow_device_type: Vec<&str> = vec!["c", "b", "u", "p"];

    info!(sl!(), "device type:{:?}", device.r#type);
    if !allow_device_type.contains(&device.r#type.as_str()) {
        return Err(anyhow!("runtime not support device type {}", device.r#type));
    }

    if device.path.is_empty() {
        return Err(anyhow!("container path can not be empty"));
    }

    let mut file_mode: u32 = 0;
    let mut uid: u32 = 0;
    let mut gid: u32 = 0;
    if device.file_mode.is_some() {
        file_mode = device.file_mode.unwrap();
    }
    if device.uid.is_some() {
        uid = device.uid.unwrap();
    }
    if device.gid.is_some() {
        gid = device.gid.unwrap();
    }

    let dev_info = GenericConfig {
        host_path: String::new(),
        container_path: device.path.clone(),
        dev_type: device.r#type.clone(),
        major: device.major,
        minor: device.minor,
        file_mode: file_mode as u32,
        uid,
        gid,
        id: "".to_string(),
        bdf,
        driver_options: HashMap::new(),
        io_limits,
        ..Default::default()
    };
    Ok(dev_info)
}

fn is_block(dev_info: &GenericConfig) -> bool {
    dev_info.dev_type == "b"
}

// GetHostPath is used to fetch the host path for the device.
// The path passed in the spec refers to the path that should appear inside the container.
// We need to find the actual device path on the host based on the major-minor numbers of the device.
fn get_host_path(dev_info: &GenericConfig) -> Result<String> {
    if dev_info.container_path.is_empty() {
        return Err(anyhow!("Empyt path provided for device"));
    }

    let path_comp = match dev_info.dev_type.as_str() {
        "c" | "u" => "char",
        "b" => "block",
        _ => return Ok(String::new()),
    };
    let format = format!("{}:{}", dev_info.major, dev_info.minor);
    let sys_dev_path = std::path::Path::new(SYS_DEV_PREFIX)
        .join(path_comp)
        .join(format)
        .join("uevent");
    if let Err(e) = std::fs::metadata(&sys_dev_path) {
        // Some devices(eg. /dev/fuse, /dev/cuse) do not always implement sysfs interface under /sys/dev
        // These devices are passed by default by docker.
        // Simply return the path passed in the device configuration, this does mean that no device renames are
        // supported for these devices.
        if e.kind() == std::io::ErrorKind::NotFound {
            return Ok(dev_info.container_path.clone());
        }
        return Err(e.into());
    }
    let conf = Ini::load_from_file(&sys_dev_path)?;
    let dev_name = conf
        .section::<String>(None)
        .ok_or_else(|| anyhow!("has no section"))?
        .get("DEVNAME")
        .ok_or_else(|| anyhow!("has no DEVNAME"))?;
    Ok(format!("/dev/{}", dev_name))
}
