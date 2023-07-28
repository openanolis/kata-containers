// Copyright (C) 2023 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use super::*;
use crate::InterruptSourceGroup;

pub const MAX_LEGACY_IRQS: u32 = 24;

pub(crate) struct UserspaceLegacyIrq {
    ioapic: Arc<dyn InterruptController>,
    base: u32,
    event_fd: EventFd,
}

impl UserspaceLegacyIrq {
    #[allow(clippy::new_ret_no_self)]
    pub(super) fn new(
        ioapic: Arc<dyn InterruptController>,
        base: InterruptIndex,
        count: InterruptIndex,
    ) -> Result<Self> {
        if count != 1 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        if base >= MAX_LEGACY_IRQS {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        let event_fd = ioapic.notifier(base.clone() as usize).unwrap();
        Ok(UserspaceLegacyIrq {
            ioapic,
            base,
            event_fd,
        })
    }
}

impl InterruptSourceGroup for UserspaceLegacyIrq {
    fn interrupt_type(&self) -> InterruptSourceType {
        InterruptSourceType::LegacyIrq
    }

    fn len(&self) -> u32 {
        1
    }

    fn base(&self) -> u32 {
        self.base
    }

    fn enable(&self, configs: &[InterruptSourceConfig]) -> Result<()> {
        if configs.len() != 1 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        self.ioapic.enable_irq(self.base as usize).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to enable IRQ #{}: {:?}", self.base, e),
            )
        })
    }

    fn disable(&self) -> Result<()> {
        self.ioapic.disable_irq(self.base as usize).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to disble IRQ #{}: {:?}", self.base, e),
            )
        })
    }

    fn update(&self, index: InterruptIndex, _config: &InterruptSourceConfig) -> Result<()> {
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        self.ioapic.update_irq(self.base as usize).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to update IRQ #{}: {:?}", self.base, e),
            )
        })
    }

    fn notifier(&self, index: InterruptIndex) -> Option<&EventFd> {
        if index != 0 {
            return None;
        }
        //return None;
        Some(&self.event_fd)
    }

    fn trigger(&self, index: InterruptIndex) -> Result<()> {
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        self.event_fd.write(1).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to inject IRQ #{}: {:?}", self.base, e),
            )
        })?;
        self.ioapic.service_irq(self.base as usize).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to service IRQ #{}: {:?}", self.base, e),
            )
        })
    }

    fn mask(&self, index: InterruptIndex) -> Result<()> {
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        self.ioapic.mask(self.base as usize).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to mask IRQ #{}: {:?}", self.base, e),
            )
        })
    }

    fn unmask(&self, index: InterruptIndex) -> Result<()> {
        if index != 0 {
            return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
        }
        self.ioapic.unmask(self.base as usize).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("failed to mask IRQ #{}: {:?}", self.base, e),
            )
        })
    }
    // Not supportted
    fn get_pending_state(&self, index: InterruptIndex) -> bool {
        if index != 0 {
            return false;
        }
        return false;
    }
}
