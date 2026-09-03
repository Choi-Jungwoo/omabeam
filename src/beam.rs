//! Implements the input precedence and transfer flow behind the CLI.

mod file;
mod input;
pub mod panel;
mod qr;
mod terminal;
mod ui;

use std::path::PathBuf;

pub(crate) const PORT: u16 = 61_234;

pub(super) fn run(file: Option<PathBuf>) -> Result<(), String> {
    match file {
        Some(path) => file::serve(&path),
        None => match input::read()? {
            input::Content::Text(text) => {
                if text.is_empty() {
                    return Err(
                        "there is no text to share; pipe text into omabeam or copy some text first"
                            .into(),
                    );
                }
                let code = match qr::render(&text) {
                    Ok(code) => code,
                    Err(qr::QrError::DataTooLong) if terminal::is_interactive() => {
                        return file::serve_text(text);
                    }
                    Err(qr::QrError::DataTooLong) => {
                        return Err(
                            "text is too long for a QR code; shorten it or share a file instead"
                                .into(),
                        );
                    }
                    Err(error) => return Err(format!("could not encode the QR code: {error}")),
                };
                if terminal::is_interactive() && ui::presentation_overflows(&code) {
                    return file::serve_text(text);
                }
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
            input::Content::File(path) => file::serve(&path),
        },
    }
}
