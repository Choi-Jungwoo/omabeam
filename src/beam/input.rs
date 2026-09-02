//! Reads text from piped stdin or the Wayland clipboard.

use std::io::{self, IsTerminal, Read};
use std::process::Command;

pub(super) fn read_text() -> Result<String, String> {
    if io::stdin().is_terminal() {
        read_clipboard()
    } else {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("could not read standard input: {error}"))?;
        Ok(text)
    }
}

fn read_clipboard() -> Result<String, String> {
    let output = Command::new("wl-paste")
        .arg("--no-newline")
        .output()
        .map_err(|error| format!("could not run wl-paste; is wl-clipboard installed? ({error})"))?;

    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        let detail = detail.trim();
        return if detail.is_empty() {
            Err("could not read the clipboard; copy some text first".into())
        } else {
            Err(format!("could not read the clipboard: {detail}"))
        };
    }

    String::from_utf8(output.stdout)
        .map_err(|_| "the clipboard does not contain UTF-8 text; copy some text first".into())
}
