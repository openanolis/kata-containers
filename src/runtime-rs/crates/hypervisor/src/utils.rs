// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Result};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader},
};

pub const PROC_CPUINFO: &str = "/proc/cpuinfo";

pub fn get_child_threads(pid: u32) -> HashSet<u32> {
    let mut result = HashSet::new();
    let path_name = format!("/proc/{}/task", pid);
    let path = std::path::Path::new(path_name.as_str());
    if path.is_dir() {
        if let Ok(dir) = path.read_dir() {
            for entity in dir {
                if let Ok(entity) = entity.as_ref() {
                    let file_name = entity.file_name();
                    let file_name = file_name.to_str().unwrap_or_default();
                    if let Ok(tid) = file_name.parse::<u32>() {
                        result.insert(tid);
                    }
                }
            }
        }
    }
    result
}

// scan the `flags` sections in given cpuinfo_path, return a hashmap <flag, is_exist>
pub(crate) fn get_cpu_flags(cpuinfo_path: &str) -> Result<HashMap<String, bool>> {
    let f = File::open(cpuinfo_path).context("open cpuinfo file")?;
    let reader = BufReader::new(f);

    let mut flags_lines = vec![];
    for line in reader.lines() {
        if let Ok(line) = line {
            if line.contains("flags") {
                flags_lines.push(line);
            }
        }
    }

    let mut flags_hash = HashMap::new();
    for flags_line in flags_lines {
        // expected format: ["flags", ":", ...] or ["flags:", ...]
        let flags: Vec<&str> = flags_line.split_whitespace().collect();
        if flags.len() < 2 {
            continue;
        }

        for flag in flags {
            flags_hash.entry(flag.to_string()).or_insert(true);
        }
    }

    Ok(flags_hash)
}
