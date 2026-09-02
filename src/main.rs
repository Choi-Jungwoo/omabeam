mod beam;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser)]
#[command(
    version,
    about = "Beam text, links, or one file to your phone via QR code"
)]
struct Cli {
    #[arg(value_name = "FILE")]
    file: Option<PathBuf>,
}

fn main() -> ExitCode {
    match beam::run(Cli::parse().file) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("omabeam: {message}");
            ExitCode::FAILURE
        }
    }
}
