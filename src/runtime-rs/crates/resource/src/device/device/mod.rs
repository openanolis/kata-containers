use anyhow::Result;
use async_trait::async_trait;
use hypervisor::Hypervisor;

mod config;
pub use config::DeviceInfo;

pub enum Error {
    MissingInfo(String),
    InvalidData(String),
}

#[async_trait]
pub trait Device: Send + Sync {
    async fn attach(&mut self, h: &dyn Hypervisor) -> Result<()>;
    async fn detach(&mut self, h: &dyn Hypervisor) -> Result<()>;

    async fn device_id(&self) -> &str;
    async fn get_device_info(&self) -> Result<DeviceInfo>;
    async fn get_major_minor(&self) -> (i64, i64);
    async fn get_host_path(&self) -> &str;
    async fn get_bdf(&self) -> Option<&String>;
    async fn get_attach_count(&self) -> u64;

    async fn reference(&mut self) -> u64;
    async fn dereference(&mut self) -> u64;
}
