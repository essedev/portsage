use clap::Parser;
use portsage_cli::cli::Cli;
use std::process::ExitCode;

fn main() -> ExitCode {
    let cli = Cli::parse();
    match portsage_cli::run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            let _ = portsage_cli::output::print_error(&e.message());
            ExitCode::from(e.exit_code())
        }
    }
}
