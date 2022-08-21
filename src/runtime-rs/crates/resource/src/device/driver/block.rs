use super::super::device::DeviceInfo;
use super::GenericDevice;
use anyhow::Result;
use async_trait::async_trait;
use hypervisor::{device::BlockConfig, Device, Hypervisor};

pub struct BlockDevice {
    base: GenericDevice,
    drive: BlockConfig,
}

impl BlockDevice {
    pub fn new(dev_info: &DeviceInfo) -> Self {
        Self {
            base: GenericDevice::new(dev_info),
            drive: BlockConfig {
                id: dev_info.id.clone(),
                path_on_host: dev_info.host_path.clone(),
                ..Default::default()
            },
        }
    }
}

#[async_trait]
impl crate::device::device::Device for BlockDevice {
    async fn attach(&mut self, h: &dyn Hypervisor) -> Result<()> {
        let skip = self.base.bump_attach_count(true)?;
        if skip {
            return Ok(());
        }
        h.add_device(Device::Block(self.drive.clone())).await
    }
    async fn detach(&mut self, h: &dyn Hypervisor) -> Result<()> {
        let skip = self.base.bump_attach_count(false)?;
        if skip {
            return Ok(());
        }
        h.remove_device(Device::Block(self.drive.clone())).await
    }
    async fn get_device_info(&self) -> Result<DeviceInfo> {
        self.base.get_device_info().await
    }

    async fn device_id(&self) -> &str {
        self.base.device_id().await
    }
    async fn get_major_minor(&self) -> (i64, i64) {
        self.base.get_major_minor().await
    }
    async fn get_host_path(&self) -> &str {
        self.base.get_host_path().await
    }
    async fn get_bdf(&self) -> Option<&String> {
        self.base.get_bdf().await
    }
    async fn reference(&mut self) -> u64 {
        self.base.reference().await
    }
    async fn dereference(&mut self) -> u64 {
        self.base.dereference().await
    }

    async fn get_attach_count(&self) -> u64 {
        self.base.get_attach_count().await
    }
}
