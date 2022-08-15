// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

//
// A generic interface of cli plugins:
// For each cli plugin, they must provide two interfaces - subcmds and matchcases
//

use std::sync::Arc;

use anyhow::{anyhow, Result};
use kata_runtime::factory::cmd::FactoryCliPlugin;
use kata_runtime::manager::CliManager;
use kata_runtime::CliPlugin;

// NOTE: all cli plugins should be registered `here`
fn register_cli_plugins(cli_manager: &mut CliManager) {
    let mut v = vec![];
    v.push(Arc::new(FactoryCliPlugin::new()));
    for plugin in v {
        cli_manager.add_plugin(&plugin.name(), plugin);
    }
}

fn real_main() -> Result<()> {
    // register the cli plugins and match the subcommands case by case
    let mut cli_manager = CliManager::new();
    register_cli_plugins(&mut cli_manager);
    let matches = cli_manager.cli().get_matches();
    // if matched, return Ok, Error otherwise
    if cli_manager.matchcases(&matches) {
        return Ok(());
    }
    Err(anyhow!(
        "Invalid command {:?}",
        matches.subcommand_name().unwrap_or_default()
    ))
}

fn main() {
    if let Err(err) = real_main() {
        println!("Error: {:?}", err);
    }
}
