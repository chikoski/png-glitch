use clap::Parser;

use crate::cli::Cli;
use crate::context::Context;

mod cli;
mod command;
mod context;

fn main() {
    let config = Cli::parse();
    if let Err(e) = start(config) {
        println!("{:?}", e);
    }
}

fn start(cli: Cli) -> anyhow::Result<()> {
    let mut context: Context = cli.try_into()?;
    context.start()
}