//! Implements the input precedence and transfer flow behind the CLI.

mod file;
mod input;
mod qr;
mod terminal;
mod ui;

use std::path::PathBuf;

pub(super) fn run(file: Option<PathBuf>) -> Result<(), String> {
    match file {
        Some(path) => file::serve(&path),
        None => match input::read()? {
            input::Content::Text(text) => {
                let code = qr::render(&text)?;
                ui::present_text(&text, &code)
                    .map_err(|error| format!("could not print the QR code: {error}"))?;
                if terminal::is_interactive() {
                    terminal::wait_for_key()
                        .map_err(|error| format!("could not wait for a key: {error}"))?;
                }
                Ok(())
            }
            input::Content::Image {
                bytes,
                content_type,
            } => file::serve_image(bytes, content_type),
        },
    }
}
