// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::PathBuf;

use crate::{add_subcmds, CliPlugin};
use clap::{arg, ArgMatches, Command};

pub struct FactoryCliPlugin {
    cmds: Vec<Command<'static>>,
}

impl FactoryCliPlugin {
    pub fn new() -> Self {
        Self {
            cmds: vec![factory_initcmd(), factory_statcmd(), factory_destroycmd()],
        }
    }
}

impl CliPlugin for FactoryCliPlugin {
    fn name(&self) -> String {
        "factory".to_string()
    }

    fn cmd(&self) -> Command<'static> {
        let mut fcmd = Command::new("factory")
            .about("manage VM factory utility")
            .arg_required_else_help(true);
        fcmd = add_subcmds(fcmd, &self.cmds);
        fcmd
    }

    fn matchcases(&self, matches: &ArgMatches) -> bool {
        match matches.subcommand() {
            Some(("init", _)) => {
                // do_init
                println!("init!");
            }
            Some(("destroy", _)) => {
                // do destroy
                println!("destory!");
            }
            Some(("status", _)) => {
                // do status
                println!("status!");
            }
            _ => {
                return false;
            }
        }
        true
    }
}

impl Default for FactoryCliPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// each subcommand's format is defined separately
fn factory_initcmd() -> Command<'static> {
    Command::new("init")
        .about("init vm factory based on kata-runtime configuration")
        .arg_required_else_help(true)
        .arg(
            arg!(<FILE> ... "path to configuration file")
                .value_parser(clap::value_parser!(PathBuf)),
        )
}

fn factory_destroycmd() -> Command<'static> {
    Command::new("destroy").about("destroy vm factory")
}

fn factory_statcmd() -> Command<'static> {
    Command::new("status").about("query the status of VM factory")
}
