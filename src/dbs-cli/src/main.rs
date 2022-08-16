#![allow(unused)]

mod parser;
mod cli_instance;

use clap::Parser;
use parser::DBSArgs;
use parser::run_with_cli;
use anyhow::{anyhow, Context, Result};

use log::{debug, error, log_enabled, info, Level};
use env_logger;



fn main() -> Result<()>{
    // RUST_LOG=debug ./dbs-cli <args>
    env_logger::init();

    let args: DBSArgs = DBSArgs::parse();

    run_with_cli(&args)?;
    Ok(())
}
