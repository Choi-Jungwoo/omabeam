//! Implements the input precedence and transfer flow behind the CLI.

mod file;
mod input;
mod qr;

use std::path::PathBuf;

pub(super) fn run(file: Option<PathBuf>) -> Result<(), String> {
    match file {
        Some(path) => file::serve(&path),
        None => qr::print(&input::read_text()?),
    }
}
