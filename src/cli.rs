use clap::{Parser, Subcommand};

/// A simple NVIDIA GPU environment manager for Ubuntu.
#[derive(Debug, Parser)]
#[command(name = "cudaenv", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show the NVIDIA driver installation plan.
    Install,
    /// Display the current GPU environment.
    Status,
    /// Diagnose common NVIDIA driver problems.
    Doctor,
    /// Show an uninstall plan without removing anything.
    Uninstall,
}
