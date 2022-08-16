#![allow(unused)]

mod parser;
mod cli_instance;

use clap::Parser;
use parser::DBSArgs;
use parser::run_with_cli;
use anyhow::{anyhow, Context, Result};

fn main() -> Result<()>{
    let args: DBSArgs = DBSArgs::parse();

    run_with_cli(&args)?;
    Ok(())
}
