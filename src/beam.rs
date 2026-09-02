//! Implements the input precedence and transfer flow behind the CLI.

mod file;
mod input;
mod qr;
mod ui;

use std::path::PathBuf;

pub(super) fn run(file: Option<PathBuf>) -> Result<(), String> {
    match file {
        Some(path) => file::serve(&path),
        None => {
            let text = input::read_text()?;
            let code = qr::render(&text)?;
            ui::present_text(&text, &code)
                .map_err(|error| format!("could not print the QR code: {error}"))
        }
    }
}
