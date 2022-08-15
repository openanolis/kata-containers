// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

pub mod factory;
pub mod manager;

use clap::{ArgMatches, Command};

// Every plugin implements CliPlugin trait
//  - name:
//      returns the *subcommand name* of the cli plugin
//      e.g. krt3 factory plugin has the name "factory", krt3 exec plugin has the name "exec"
//  - cmd:
//      returns the main cmd with all subcommands attached (e.g. factory_cmd())
//  - matchcases:
//      do the match branching of each subcommands, return true if matched, false otherwise (e.g. factory_matches)
pub trait CliPlugin {
    fn name(&self) -> String;
    fn cmd(&self) -> Command<'static>;
    fn matchcases(&self, matches: &ArgMatches) -> bool;
}

pub fn add_subcmds(mut cmds: Command<'static>, subcmds: &[Command<'static>]) -> Command<'static> {
    for sc in subcmds {
        cmds = cmds.subcommand(sc.clone())
    }
    cmds
}
