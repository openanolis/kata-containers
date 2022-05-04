// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use std::convert::TryInto;
use std::io;
use std::ops::{Index, IndexMut};
use std::sync::Arc;

use dbs_device::DeviceIo;
use dbs_utils::rate_limiter::{RateLimiter, TokenBucket};
use serde_derive::{Deserialize, Serialize};

/// Trait for generic configuration information.
pub trait ConfigItem {
    /// Related errors.
    type E;

    /// Get the unique identifier of the configuration item.
    fn id(&self) -> &str;

    /// Check whether current configuration item conflicts with another one.
    fn check_conflicts(&self, other: &Self) -> std::result::Result<(), Self::E>;
}

/// Struct to manage a group of configuration items.
#[derive(Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct ConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    pub configs: Vec<T>,
}

impl<T> ConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    /// Constructor
    pub fn new() -> Self {
        ConfigInfos {
            configs: Vec::new(),
        }
    }

    /// Insert a configuration item in the group.
    pub fn insert(&mut self, config: T) -> std::result::Result<(), T::E> {
        for item in self.configs.iter() {
            config.check_conflicts(item)?;
        }
        self.configs.push(config);

        Ok(())
    }

    /// Update a configuration item in the group.
    pub fn update(&mut self, config: T, err: T::E) -> std::result::Result<(), T::E> {
        match self.get_index_by_id(&config) {
            None => Err(err),
            Some(index) => {
                for (idx, item) in self.configs.iter().enumerate() {
                    if idx != index {
                        config.check_conflicts(item)?;
                    }
                }
                self.configs[index] = config;
                Ok(())
            }
        }
    }

    /// Insert or update a configuration item in the group.
    pub fn insert_or_update(&mut self, config: T) -> std::result::Result<(), T::E> {
        match self.get_index_by_id(&config) {
            None => {
                for item in self.configs.iter() {
                    config.check_conflicts(item)?;
                }

                self.configs.push(config)
            }
            Some(index) => {
                for (idx, item) in self.configs.iter().enumerate() {
                    if idx != index {
                        config.check_conflicts(item)?;
                    }
                }
                self.configs[index] = config;
            }
        }

        Ok(())
    }

    /// Remove the matching configuration entry.
    pub fn remove(&mut self, config: &T) -> Option<T> {
        if let Some(index) = self.get_index_by_id(config) {
            Some(self.configs.remove(index))
        } else {
            None
        }
    }

    /// Returns an immutable iterator over the config items
    pub fn iter(&self) -> ::std::slice::Iter<T> {
        self.configs.iter()
    }

    /// Get the configuration entry with matching ID.
    pub fn get_by_id(&self, item: &T) -> Option<&T> {
        let id = item.id();

        self.configs.iter().rfind(|cfg| cfg.id() == id)
    }

    fn get_index_by_id(&self, item: &T) -> Option<usize> {
        let id = item.id();
        self.configs.iter().position(|cfg| cfg.id() == id)
    }
}

impl<T> Clone for ConfigInfos<T>
where
    T: ConfigItem + Clone,
{
    fn clone(&self) -> Self {
        ConfigInfos {
            configs: self.configs.clone(),
        }
    }
}

pub struct DeviceInfoGroup<T>
where
    T: ConfigItem + Clone,
{
    pub config: T,
    pub device: Option<Arc<dyn DeviceIo>>,
}
impl<T> DeviceInfoGroup<T>
where
    T: ConfigItem + Clone,
{
    pub fn new(config: T) -> Self {
        DeviceInfoGroup {
            config,
            device: None,
        }
    }

    pub fn new_with_device(config: T, device: Option<Arc<dyn DeviceIo>>) -> Self {
        DeviceInfoGroup { config, device }
    }

    pub fn set_device(&mut self, device: Arc<dyn DeviceIo>) {
        self.device = Some(device);
    }
}

impl<T> Clone for DeviceInfoGroup<T>
where
    T: ConfigItem + Clone,
{
    fn clone(&self) -> Self {
        DeviceInfoGroup::new_with_device(self.config.clone(), self.device.clone())
    }
}

pub struct DeviceInfoList<T>
where
    T: ConfigItem + Clone,
{
    info_list: Vec<DeviceInfoGroup<T>>,
}

impl<T> DeviceInfoList<T>
where
    T: ConfigItem + Clone,
{
    pub fn new() -> Self {
        DeviceInfoList {
            info_list: Vec::new(),
        }
    }

    pub fn insert_or_update(&mut self, config: &T) -> std::result::Result<usize, T::E> {
        let device_info = DeviceInfoGroup::new(config.clone());
        Ok(match self.get_index_by_id(config) {
            Some(index) => {
                for (idx, info) in self.info_list.iter().enumerate() {
                    if idx != index {
                        info.config.check_conflicts(config)?;
                    }
                }
                self.info_list[index] = device_info;
                index
            }
            None => {
                for info in self.info_list.iter() {
                    info.config.check_conflicts(config)?;
                }
                self.info_list.push(device_info);
                self.info_list.len() - 1
            }
        })
    }

    pub fn remove(&mut self, index: usize) -> Option<DeviceInfoGroup<T>> {
        if self.info_list.len() > index {
            Some(self.info_list.remove(index))
        } else {
            None
        }
    }

    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.info_list.len()
    }

    pub fn push(&mut self, info: DeviceInfoGroup<T>) {
        self.info_list.push(info);
    }

    pub fn iter(&self) -> std::slice::Iter<DeviceInfoGroup<T>> {
        self.info_list.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<DeviceInfoGroup<T>> {
        self.info_list.iter_mut()
    }

    fn get_index_by_id(&self, config: &T) -> Option<usize> {
        self.info_list
            .iter()
            .position(|info| info.config.id().eq(config.id()))
    }
}

impl<T> Index<usize> for DeviceInfoList<T>
where
    T: ConfigItem + Clone,
{
    type Output = DeviceInfoGroup<T>;
    fn index(&self, idx: usize) -> &Self::Output {
        &self.info_list[idx]
    }
}

impl<T> IndexMut<usize> for DeviceInfoList<T>
where
    T: ConfigItem + Clone,
{
    fn index_mut(&mut self, idx: usize) -> &mut Self::Output {
        &mut self.info_list[idx]
    }
}

impl<T> Clone for DeviceInfoList<T>
where
    T: ConfigItem + Clone,
{
    fn clone(&self) -> Self {
        DeviceInfoList {
            info_list: self.info_list.clone(),
        }
    }
}

/// Configuration information for RateLimiter token bucket.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TokenBucketConfigInfo {
    /// See TokenBucket::size.
    pub size: u64,
    /// See TokenBucket::one_time_burst.
    pub one_time_burst: u64,
    /// See TokenBucket::refill_time.
    pub refill_time: u64,
}

impl TokenBucketConfigInfo {
    fn resize(&mut self, n: u64) {
        self.size /= n;
        self.one_time_burst /= n;
    }
}

impl From<TokenBucketConfigInfo> for TokenBucket {
    fn from(t: TokenBucketConfigInfo) -> TokenBucket {
        (&t).into()
    }
}

impl From<&TokenBucketConfigInfo> for TokenBucket {
    fn from(t: &TokenBucketConfigInfo) -> TokenBucket {
        TokenBucket::new(t.size, t.one_time_burst, t.refill_time)
    }
}

/// Configuration information for RateLimiter objects.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RateLimiterConfigInfo {
    /// Data used to initialize the RateLimiter::bandwidth bucket.
    pub bandwidth: TokenBucketConfigInfo,
    /// Data used to initialize the RateLimiter::ops bucket.
    pub ops: TokenBucketConfigInfo,
}

impl RateLimiterConfigInfo {
    /// Update the bandwidth budget configuration.
    pub fn update_bandwidth(&mut self, new_config: TokenBucketConfigInfo) {
        self.bandwidth = new_config;
    }

    /// Update the ops budget configuration.
    pub fn update_ops(&mut self, new_config: TokenBucketConfigInfo) {
        self.ops = new_config;
    }

    /// resize the limiter to its 1/n.
    pub fn resize(&mut self, n: u64) {
        self.bandwidth.resize(n);
        self.ops.resize(n);
    }
}

impl TryInto<RateLimiter> for &RateLimiterConfigInfo {
    type Error = io::Error;

    fn try_into(self) -> Result<RateLimiter, Self::Error> {
        RateLimiter::new(
            self.bandwidth.size,
            self.bandwidth.one_time_burst,
            self.bandwidth.refill_time,
            self.ops.size,
            self.ops.one_time_burst,
            self.ops.refill_time,
        )
    }
}

impl TryInto<RateLimiter> for RateLimiterConfigInfo {
    type Error = io::Error;

    fn try_into(self) -> Result<RateLimiter, Self::Error> {
        RateLimiter::new(
            self.bandwidth.size,
            self.bandwidth.one_time_burst,
            self.bandwidth.refill_time,
            self.ops.size,
            self.ops.one_time_burst,
            self.ops.refill_time,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    pub enum DummyError {
        #[error("configuration entry exists")]
        Exist,
    }

    #[derive(Clone, Debug)]
    pub struct DummyConfigInfo {
        id: String,
        content: String,
    }

    impl ConfigItem for DummyConfigInfo {
        type E = DummyError;

        fn id(&self) -> &str {
            &self.id
        }

        fn check_conflicts(&self, other: &Self) -> Result<(), DummyError> {
            if self.id == other.id || self.content == other.content {
                Err(DummyError::Exist)
            } else {
                Ok(())
            }
        }
    }

    type DummyConfigInfos = ConfigInfos<DummyConfigInfo>;

    #[test]
    fn test_insert_config_info() {
        let mut configs = DummyConfigInfos::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert(config1).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");

        // Test case: cannot insert new item with the same id.
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert(config2).unwrap_err();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");

        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert(config3).unwrap();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");

        // Test case: cannot insert new item with the same content.
        let config4 = DummyConfigInfo {
            id: "3".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert(config4).unwrap_err();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");
    }

    #[test]
    fn test_update_config_info() {
        let mut configs = DummyConfigInfos::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert(config1).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");

        // Test case: succeed to update an existing entry
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.update(config2, DummyError::Exist).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");

        // Test case: cannot update a non-existing entry
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.update(config3, DummyError::Exist).unwrap_err();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");

        // Test case: cannot update an entry with conflicting content
        let config4 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert(config4).unwrap();
        let config5 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "c".to_owned(),
        };
        configs.update(config5, DummyError::Exist).unwrap_err();
    }

    #[test]
    fn test_insert_or_update_config_info() {
        let mut configs = DummyConfigInfos::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert_or_update(config1).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "a");

        // Test case: succeed to update an existing entry
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert_or_update(config2.clone()).unwrap();
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");

        // Add a second entry
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(config3.clone()).unwrap();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");

        // Lookup the first entry
        let config4 = configs
            .get_by_id(&DummyConfigInfo {
                id: "1".to_owned(),
                content: "b".to_owned(),
            })
            .unwrap();
        assert_eq!(config4.id, config2.id);
        assert_eq!(config4.content, config2.content);

        // Lookup the second entry
        let config5 = configs
            .get_by_id(&DummyConfigInfo {
                id: "2".to_owned(),
                content: "c".to_owned(),
            })
            .unwrap();
        assert_eq!(config5.id, config3.id);
        assert_eq!(config5.content, config3.content);

        // Test case: can't insert an entry with conflicting content
        let config6 = DummyConfigInfo {
            id: "3".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(config6).unwrap_err();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");
    }

    #[test]
    fn test_remove_config_info() {
        let mut configs = DummyConfigInfos::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert_or_update(config1).unwrap();
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert_or_update(config2.clone()).unwrap();
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(config3.clone()).unwrap();
        assert_eq!(configs.configs.len(), 2);
        assert_eq!(configs.configs[0].id, "1");
        assert_eq!(configs.configs[0].content, "b");
        assert_eq!(configs.configs[1].id, "2");
        assert_eq!(configs.configs[1].content, "c");

        let config4 = configs
            .remove(&DummyConfigInfo {
                id: "1".to_owned(),
                content: "no value".to_owned(),
            })
            .unwrap();
        assert_eq!(config4.id, config2.id);
        assert_eq!(config4.content, config2.content);
        assert_eq!(configs.configs.len(), 1);
        assert_eq!(configs.configs[0].id, "2");
        assert_eq!(configs.configs[0].content, "c");

        let config5 = configs
            .remove(&DummyConfigInfo {
                id: "2".to_owned(),
                content: "no value".to_owned(),
            })
            .unwrap();
        assert_eq!(config5.id, config3.id);
        assert_eq!(config5.content, config3.content);
        assert_eq!(configs.configs.len(), 0);
    }

    type DummyDeviceInfoList = DeviceInfoList<DummyConfigInfo>;

    #[test]
    fn test_insert_or_update_device_info() {
        let mut configs = DummyDeviceInfoList::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert_or_update(&config1).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "a");

        // Test case: succeed to update an existing entry
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert_or_update(&config2 /*  */).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "b");

        // Add a second entry
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(&config3).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "b");
        assert_eq!(configs[1].config.id, "2");
        assert_eq!(configs[1].config.content, "c");

        // Lookup the first entry
        let config4_id = configs
            .get_index_by_id(&DummyConfigInfo {
                id: "1".to_owned(),
                content: "b".to_owned(),
            })
            .unwrap();
        let config4 = &configs[config4_id].config;
        assert_eq!(config4.id, config2.id);
        assert_eq!(config4.content, config2.content);

        // Lookup the second entry
        let config5_id = configs
            .get_index_by_id(&DummyConfigInfo {
                id: "2".to_owned(),
                content: "c".to_owned(),
            })
            .unwrap();
        let config5 = &configs[config5_id].config;
        assert_eq!(config5.id, config3.id);
        assert_eq!(config5.content, config3.content);

        // Test case: can't insert an entry with conflicting content
        let config6 = DummyConfigInfo {
            id: "3".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(&config6).unwrap_err();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "b");
        assert_eq!(configs[1].config.id, "2");
        assert_eq!(configs[1].config.content, "c");
    }

    #[test]
    fn test_remove_device_info() {
        let mut configs = DummyDeviceInfoList::new();

        let config1 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "a".to_owned(),
        };
        configs.insert_or_update(&config1).unwrap();
        let config2 = DummyConfigInfo {
            id: "1".to_owned(),
            content: "b".to_owned(),
        };
        configs.insert_or_update(&config2).unwrap();
        let config3 = DummyConfigInfo {
            id: "2".to_owned(),
            content: "c".to_owned(),
        };
        configs.insert_or_update(&config3).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].config.id, "1");
        assert_eq!(configs[0].config.content, "b");
        assert_eq!(configs[1].config.id, "2");
        assert_eq!(configs[1].config.content, "c");

        let config4 = configs.remove(0).unwrap().config;
        assert_eq!(config4.id, config2.id);
        assert_eq!(config4.content, config2.content);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].config.id, "2");
        assert_eq!(configs[0].config.content, "c");

        let config5 = configs.remove(0).unwrap().config;
        assert_eq!(config5.id, config3.id);
        assert_eq!(config5.content, config3.content);
        assert_eq!(configs.len(), 0);
    }
}
