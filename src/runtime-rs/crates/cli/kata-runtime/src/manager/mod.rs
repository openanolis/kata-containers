// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::collections::HashMap;
use std::sync::Arc;

use clap::{ArgMatches, Command, App};

use crate::add_subcmds;
use crate::CliPlugin;

pub struct CliManager {
    plugins: HashMap<String, Arc<dyn CliPlugin>>,
}

impl CliManager {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::default(),
        }
    }

    pub fn cli(&self) -> Command<'static> {
        let mut cmds = App::new("kata-runtime")
            .about("Kata-containers 3.0 runtime cli")
            .subcommand_required(true)
            .arg_required_else_help(true)
            .allow_external_subcommands(true);
        cmds = add_subcmds(cmds, &self.subcmds());
        cmds
    }

    pub fn subcmds(&self) -> Vec<Command<'static>> {
        let mut v: Vec<Command<'static>> = [].to_vec();
        for plugin in self.plugins.values() {
            v.push(plugin.cmd());
        }
        v
    }

    pub fn matchcases(&self, matches: &ArgMatches) -> bool {
        for plugin_name in self.plugins.keys() {
            if matches.subcommand_name().eq(&Some(plugin_name.as_str())) {
                if let Some(plugin) = self.plugins.get(plugin_name) {
                    if plugin.matchcases(matches.subcommand_matches(plugin_name.as_str()).unwrap())
                    {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn add_plugin(&mut self, name: &str, plugin: Arc<dyn CliPlugin>) {
        self.plugins.insert(name.to_string(), plugin);
    }
}

impl Default for CliManager {
    fn default() -> Self {
        Self::new()
    }
}
