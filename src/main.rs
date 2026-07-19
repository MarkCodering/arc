mod cli;
mod commands;
mod system;
mod ui;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Install(args) => commands::install::run(args),
        Command::Status => commands::status::run(),
        Command::Doctor => commands::doctor::run(),
        Command::Uninstall => commands::uninstall::run(),
    }
}
