//! Reads shareable content from piped stdin or the Wayland clipboard.

use std::ffi::OsString;
use std::io::{self, IsTerminal, Read};
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::process::Command;

const URI_LIST: &str = "text/uri-list";
const GNOME_COPIED_FILES: &str = "x-special/gnome-copied-files";

pub(super) enum Content {
    Text(String),
    File(PathBuf),
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

    if let Some(content_type) = offered_file_type(&types) {
        let bytes = read_wl_paste(&["--no-newline", "--type", content_type])?;
        return clipboard_file_path(content_type, &bytes).map(Content::File);
    }

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

fn offered_file_type(types: &str) -> Option<&'static str> {
    [URI_LIST, GNOME_COPIED_FILES]
        .into_iter()
        .find(|candidate| types.lines().any(|offered| offered == *candidate))
}

fn offered_image_type(types: &str) -> Option<&str> {
    types.lines().find(|offered| {
        offered
            .parse::<mime_guess::Mime>()
            .is_ok_and(|mime| mime.type_().as_str() == "image")
    })
}

fn clipboard_file_path(content_type: &str, bytes: &[u8]) -> Result<PathBuf, String> {
    let contents = std::str::from_utf8(bytes)
        .map_err(|_| "clipboard file metadata is invalid; copy the file again".to_owned())?;
    let mut lines = contents
        .lines()
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .filter(|line| !line.is_empty() && !line.starts_with('#'));

    if content_type == GNOME_COPIED_FILES {
        match lines.next() {
            Some("copy") | Some("cut") => {}
            _ => return Err("clipboard file metadata is invalid; copy the file again".into()),
        }
    }

    let uri = lines
        .next()
        .ok_or_else(|| "the clipboard does not contain a file; copy a file first".to_owned())?;
    if lines.next().is_some() {
        return Err("the clipboard contains multiple files; copy one file at a time".into());
    }

    local_file_uri_to_path(uri)
}

fn local_file_uri_to_path(uri: &str) -> Result<PathBuf, String> {
    let rest = uri
        .strip_prefix("file:")
        .ok_or_else(|| "the clipboard item is not a local file".to_owned())?;
    let encoded_path = if let Some(authority_and_path) = rest.strip_prefix("//") {
        let slash = authority_and_path
            .find('/')
            .ok_or_else(|| "the clipboard file URI has no path".to_owned())?;
        let (authority, path) = authority_and_path.split_at(slash);
        if !authority.is_empty() && !authority.eq_ignore_ascii_case("localhost") {
            return Err("the clipboard item is not a local file".into());
        }
        path
    } else {
        rest
    };

    if !encoded_path.starts_with('/') || encoded_path.contains('?') || encoded_path.contains('#') {
        return Err("the clipboard file URI is invalid; copy the file again".into());
    }

    let bytes = percent_decode(encoded_path)?;
    if bytes.contains(&0) {
        return Err("the clipboard file URI is invalid; copy the file again".into());
    }
    Ok(PathBuf::from(OsString::from_vec(bytes)))
}

fn percent_decode(value: &str) -> Result<Vec<u8>, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = bytes.get(index + 1).and_then(|byte| hex_value(*byte));
            let low = bytes.get(index + 2).and_then(|byte| hex_value(*byte));
            let (Some(high), Some(low)) = (high, low) else {
                return Err("the clipboard file URI is invalid; copy the file again".into());
            };
            decoded.push(high * 16 + low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    Ok(decoded)
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
        Err("could not read the clipboard; copy some text, an image, or a file first".into())
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

    #[test]
    fn copied_file_is_one_decoded_local_uri() {
        let types = "image/png\nx-special/gnome-copied-files\ntext/uri-list";
        let contents = b"# copied by a file manager\r\nfile:///tmp/My%20Photo.png\r\n";
        let expected = PathBuf::from("/tmp/My Photo.png");

        assert_eq!(offered_file_type(types), Some(URI_LIST));
        assert_eq!(clipboard_file_path(URI_LIST, contents).unwrap(), expected);
        assert_eq!(
            clipboard_file_path(GNOME_COPIED_FILES, b"copy\nfile:///tmp/My%20Photo.png").unwrap(),
            expected
        );
        assert!(clipboard_file_path(URI_LIST, b"file:///tmp/one\nfile:///tmp/two").is_err());
    }
}
