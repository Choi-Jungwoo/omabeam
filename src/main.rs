mod beam;
mod setup;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(
    version,
    about = "Beam clipboard content or one file to your phone via QR code",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Configure the Omarchy menu, Super+B shortcut, and firewall.
    Setup,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Command::Setup) => setup::run(),
        None => beam::run(cli.file),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("omabeam: {message}");
            ExitCode::FAILURE
        }
    }
}
