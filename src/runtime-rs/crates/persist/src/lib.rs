// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod network;
pub mod sandbox;
use anyhow::{anyhow, Ok, Result};
use serde::de;
use std::{fs::File, io::BufReader};
pub const KATA_PATH: &str = "/run/kata";
pub const PERSIST_FILE: &str = "persist.json";
use async_trait::async_trait;

#[async_trait]
pub trait Persist
where
    Self: Sized,
{
    /// The type of the object representing the state of the component.
    type State;
    /// The type of the object holding the constructor arguments.
    type ConstructorArgs;

    /// Returns the current state of the component.
    async fn save(&self) -> Result<Self::State>;
    /// Constructs a component from a specified state.
    async fn restore(constructor_args: Self::ConstructorArgs, state: &Self::State) -> Result<Self>;
}

pub fn to_disk<T: serde::Serialize>(value: &T, sid: &str) -> Result<()> {
    let sandbox_file = [KATA_PATH, sid, PERSIST_FILE].join("/");
    let f = File::create(sandbox_file)?;
    let j = serde_json::to_value(value)?;
    serde_json::to_writer_pretty(f, &j)?;
    Ok(())
}

pub fn from_disk<T>(sid: &str) -> Result<T>
where
    T: de::DeserializeOwned,
{
    let sandbox_file = [KATA_PATH, sid, PERSIST_FILE].join("/");
    let file = File::open(sandbox_file).unwrap();
    let reader = BufReader::new(file);
    serde_json::from_reader(reader).map_err(|e| anyhow!(e.to_string()))
}

#[cfg(test)]
#[test]
fn test_to_from_disk() {
    use serde::{Deserialize, Serialize};
    use std::{fs, result::Result::Ok};
    #[derive(Serialize, Deserialize, Debug)]
    struct Kata {
        name: String,
        key: u8,
    }
    let data = Kata {
        name: "kata".to_string(),
        key: 1,
    };
    let sid = "1";
    let sandbox_dir = [KATA_PATH, sid].join("/");
    let sandbox_file = [KATA_PATH, sid, PERSIST_FILE].join("/");
    assert!(fs::create_dir(&sandbox_dir).is_ok());
    assert!(to_disk(&data, sid).is_ok());
    if let Ok(result) = from_disk::<Kata>(sid) {
        assert_eq!(result.name, data.name);
        assert_eq!(result.key, data.key);
    }
    assert!(fs::remove_file(&sandbox_file).is_ok());
    assert!(fs::remove_dir(&sandbox_dir).is_ok());
}
