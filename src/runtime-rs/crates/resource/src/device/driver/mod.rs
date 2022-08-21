pub mod block;
pub mod vfio;
use super::device::DeviceInfo;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use hypervisor::Hypervisor;

pub const MAX_DEV_ID_SIZE: usize = 31;

#[derive(Default, Debug)]
pub struct GenericDevice {
    id: String,
    device_info: DeviceInfo,
    ref_count: u64,
    attach_count: u64,
}

impl GenericDevice {
    pub fn new(dev_info: &DeviceInfo) -> Self {
        Self {
            id: dev_info.id.clone(),
            device_info: dev_info.clone(),
            ref_count: 0,
            attach_count: 0,
        }
    }

    // bumpAttachCount is used to add/minus attach count for a device
    // * attach bool: true means attach, false means detach
    // return values:
    // * skip bool: no need to do real attach/detach, skip following actions.
    // * err error: error while do attach count bump
    pub fn bump_attach_count(&mut self, attach: bool) -> Result<bool> {
        if attach {
            match self.attach_count {
                0 => {
                    // do real attach
                    self.attach_count += 1;
                    Ok(false)
                }
                std::u64::MAX => Err(anyhow!("device was attached too many times")),
                _ => {
                    self.attach_count += 1;
                    Ok(true)
                }
            }
        } else {
            // detach use case
            match self.attach_count {
                0 => Err(anyhow!("detaching a device that wasn't attached")),
                1 => {
                    // do real wrok
                    self.attach_count -= 1;
                    Ok(false)
                }
                _ => {
                    self.attach_count -= 1;
                    Ok(true)
                }
            }
        }
    }
}

#[async_trait]
impl crate::device::device::Device for GenericDevice {
    async fn attach(&mut self, _h: &dyn Hypervisor) -> Result<()> {
        let skip = self.bump_attach_count(true)?;
        if skip {
            return Ok(());
        }
        Err(anyhow!("Failed to attach device {:?}", self.id))
    }

    async fn detach(&mut self, _h: &dyn Hypervisor) -> Result<()> {
        let skip = self.bump_attach_count(false)?;
        if skip {
            return Ok(());
        }
        Err(anyhow!("Failed to detach device {:?}", self.id))
    }

    async fn device_id(&self) -> &str {
        self.id.as_str().clone()
    }

    async fn get_device_info(&self) -> Result<DeviceInfo> {
        Ok(self.device_info.clone())
    }

    async fn get_major_minor(&self) -> (i64, i64) {
        (self.device_info.major, self.device_info.minor)
    }

    async fn get_host_path(&self) -> &str {
        self.device_info.host_path.as_str()
    }

    async fn get_bdf(&self) -> Option<&String> {
        self.device_info.bdf.as_ref()
    }

    async fn get_attach_count(&self) -> u64 {
        self.attach_count
    }

    async fn reference(&mut self) -> u64 {
        self.ref_count = self.ref_count.saturating_add(1);
        self.ref_count
    }

    async fn dereference(&mut self) -> u64 {
        self.ref_count = self.ref_count.saturating_sub(1);
        self.ref_count
    }
}

#[cfg(test)]
mod tests {
    use super::super::device::Device;
    use super::*;
    use std::u64;

    #[actix_rt::test]
    async fn test_bump_attach_count() {
        //type testData struct {
        //    attachCount uint
        //    expectedAC  uint
        //    attach      bool
        //    expectSkip  bool
        //    expectErr   bool
        //}
        let data = vec![
            (0, 1, true, false, false),
            (1, 2, true, true, false),
            (u64::MAX, u64::MAX, true, true, true),
            (0, 0, false, true, true),
            (1, 0, false, false, false),
            (u64::MAX, u64::MAX - 1, false, true, false),
        ];
        let mut dev = GenericDevice::default();
        for (attach_count, expected_ac, attach, expect_skip, expect_err) in data.into_iter() {
            dev.attach_count = attach_count;
            let ret = dev.bump_attach_count(attach);
            if expect_err {
                assert!(ret.is_err());
            } else {
                let skip = ret.unwrap();
                assert_eq!(skip, expect_skip);
            }
            assert_eq!(dev.get_attach_count().await, expected_ac);
        }
    }
}
