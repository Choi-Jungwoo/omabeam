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
    /// Generate QR for panel (used by Omarchy bar plugin).
    #[command(hide = true)]
    Panel {
        /// Optional file to share (overrides clipboard)
        #[arg(value_name = "FILE")]
        file: Option<PathBuf>,
        /// Path to write QR PNG
        #[arg(long, value_name = "PATH", default_value = "/tmp/omabeam-qr.png")]
        qr_file: PathBuf,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Some(Command::Setup) => setup::run(),
        Some(Command::Panel { file, qr_file }) => beam::panel::run(file, qr_file),
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
