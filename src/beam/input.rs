//! Reads shareable content from piped stdin or the Wayland clipboard.

use std::io::{self, IsTerminal, Read};
use std::process::Command;

pub(super) enum Content {
    Text(String),
    Image {
        bytes: Vec<u8>,
        content_type: String,
    },
}

pub(super) fn read() -> Result<Content, String> {
    if io::stdin().is_terminal() {
        read_clipboard()
    } else {
        let mut text = String::new();
        io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("could not read standard input: {error}"))?;
        Ok(Content::Text(text))
    }
}

fn read_clipboard() -> Result<Content, String> {
    let types = read_wl_paste(&["--list-types"])?;
    let types = String::from_utf8(types)
        .map_err(|_| "wl-paste returned invalid clipboard types".to_owned())?;

    if let Some(content_type) = offered_image_type(&types) {
        let bytes = read_wl_paste(&["--no-newline", "--type", content_type])?;
        if bytes.is_empty() {
            return Err("the clipboard image is empty; copy the image again".into());
        }
        return Ok(Content::Image {
            bytes,
            content_type: content_type.to_owned(),
        });
    }

    let bytes = read_wl_paste(&["--no-newline"])?;
    String::from_utf8(bytes)
        .map(Content::Text)
        .map_err(|_| "the clipboard does not contain image or UTF-8 text data".to_owned())
}

fn offered_image_type(types: &str) -> Option<&str> {
    types.lines().find(|offered| {
        offered
            .parse::<mime_guess::Mime>()
            .is_ok_and(|mime| mime.type_().as_str() == "image")
    })
}

fn read_wl_paste(args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("wl-paste")
        .args(args)
        .output()
        .map_err(|error| format!("could not run wl-paste; is wl-clipboard installed? ({error})"))?;

    if output.status.success() {
        return Ok(output.stdout);
    }

    let detail = String::from_utf8_lossy(&output.stderr);
    let detail = detail.trim();
    if detail.is_empty() {
        Err("could not read the clipboard; copy some text or an image first".into())
    } else {
        Err(format!("could not read the clipboard: {detail}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn any_image_type_takes_priority_over_text() {
        let types = "text/plain\nimage/x-custom";

        assert_eq!(offered_image_type(types), Some("image/x-custom"));
    }
}
