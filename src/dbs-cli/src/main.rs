#![allow(unused)]

mod parser;
mod utils;


use clap::Parser;
use parser::DBSArgs;
// use parser::run_with_cli;
use utils::{CLIError, CLIResult, KernelErrorKind, RootfsErrorKind};

fn main() -> CLIResult<()>{
    let args: DBSArgs = DBSArgs::parse();

    // run_with_cli(&args)?;
    Ok(())
}
