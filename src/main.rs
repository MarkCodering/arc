use clap::Parser;
use arc::cli::Cli;
use std::process::ExitCode;

fn main() -> ExitCode {
    match arc::run(Cli::parse()) {
        Ok(status) => ExitCode::from(status.code()),
        Err(error) => {
            eprintln!("arc could not complete: {error:#}");
            ExitCode::from(arc::EXECUTION_FAILURE_EXIT_CODE)
        }
    }
}
