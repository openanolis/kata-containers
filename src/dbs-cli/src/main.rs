// #![allow(unused)]

mod parser;
mod cli_instance;

use clap::Parser;
use parser::DBSArgs;
use parser::run_with_cli;
use anyhow::Result;


extern crate slog_stdlog;
extern crate slog_envlogger;


fn main() -> Result<()>{
    // RUST_LOG=debug ./dbs-cli <args>
    let _guard = slog_envlogger::init().unwrap();

    let args: DBSArgs = DBSArgs::parse();

    run_with_cli(args)?;
    Ok(())
}
