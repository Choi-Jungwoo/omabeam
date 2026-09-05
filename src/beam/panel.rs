//! Panel backend: emits one JSON line then serves (if needed) forever.
//! Service.qml kills the process on panel close.

use std::fs::{self, File};
use std::io::{self, Cursor, IsTerminal, Seek, Write};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::process::Command;

use tiny_http::{Header, Method, Request, Response, Server, StatusCode};

use super::{PORT, input, qr};

enum Payload {
    File(File),
    Bytes(Vec<u8>),
}

pub(crate) fn run(file: Option<PathBuf>, qr_file: PathBuf) -> Result<(), String> {
    // Determine content same as beam::run precedence but without interactive UI.
    let content = match file {
        Some(path) => {
            // Validate file now to emit JSON error quickly
            let f =
                File::open(&path).map_err(|e| format!("could not open {}: {e}", path.display()))?;
            if !f
                .metadata()
                .map_err(|e| format!("could not inspect {}: {e}", path.display()))?
                .is_file()
            {
                return Err(format!("{} is not a regular file", path.display()));
            }
            // Hand off to server path
            return serve_file_payload(f, path, qr_file);
        }
        None => {
            // For panel, always try clipboard first (Process has no tty, but user
            // invoked via bar icon). Fall back to piped stdin if clipboard empty
            // and stdin is piped with data.
            match input::read_clipboard() {
                Ok(c) => c,
                Err(e) if e.contains("clipboard") => {
                    // If clipboard read failed, try piped stdin as fallback
                    if !std::io::stdin().is_terminal() {
                        let mut text = String::new();
                        use std::io::Read;
                        let _ = std::io::stdin().read_to_string(&mut text);
                        if !text.is_empty() {
                            input::Content::Text(text)
                        } else {
                            return Err(e);
                        }
                    } else {
                        return Err(e);
                    }
                }
                Err(e) => return Err(e),
            }
        }
    };

    match content {
        input::Content::Text(text) => {
            if text.is_empty() {
                let msg =
                    "there is no text to share; pipe text into omabeam or copy some text first";
                emit_error(&qr_file, msg);
                return Err(msg.into());
            }
            // Try direct QR (text fits). For panel we don't check terminal overflow —
            // PNG is always rendered; only DataTooLong forces server.
            match qr::render(&text) {
                Ok(_) => {
                    // Fits: encode text directly
                    qr::render_png(&text, &qr_file)?;
                    emit_json(&qr_file, &text, "text", None, None, None, None);
                    // No server, exit immediately
                    Ok(())
                }
                Err(qr::QrError::DataTooLong) => {
                    // Too large → serve as file
                    serve_text_payload(text, qr_file)
                }
                Err(e) => {
                    let msg = format!("could not encode the QR code: {e}");
                    emit_error(&qr_file, &msg);
                    Err(msg)
                }
            }
        }
        input::Content::Image {
            bytes,
            content_type,
        } => serve_bytes_payload(
            bytes,
            clipboard_image_filename(&content_type),
            content_type,
            qr_file,
        ),
        input::Content::File(path) => {
            let f =
                File::open(&path).map_err(|e| format!("could not open {}: {e}", path.display()))?;
            serve_file_payload(f, path, qr_file)
        }
    }
}

fn serve_file_payload(file: File, path: PathBuf, qr_file: PathBuf) -> Result<(), String> {
    let filename = safe_filename(&path);
    let content_type = mime_guess::from_path(&path)
        .first_raw()
        .unwrap_or("application/octet-stream")
        .to_owned();
    serve_payload(Payload::File(file), filename, content_type, qr_file)
}

fn serve_text_payload(text: String, qr_file: PathBuf) -> Result<(), String> {
    let len = text.len();
    let _ = len;
    serve_payload(
        Payload::Bytes(text.into_bytes()),
        "clipboard.txt".into(),
        "text/plain; charset=utf-8".into(),
        qr_file,
    )
}

fn serve_bytes_payload(
    bytes: Vec<u8>,
    filename: String,
    content_type: String,
    qr_file: PathBuf,
) -> Result<(), String> {
    serve_payload(Payload::Bytes(bytes), filename, content_type, qr_file)
}

fn serve_payload(
    payload: Payload,
    filename: String,
    content_type: String,
    qr_file: PathBuf,
) -> Result<(), String> {
    let address = local_ipv4()?;
    let server = Server::http((address, PORT))
        .map_err(|e| format!("could not start the file server on port {PORT}: {e}"))?;
    let route = format!("/{}/download", route_token()?);
    let url = format!("http://{address}:{PORT}{route}");

    qr::render_png(&url, &qr_file)?;

    // Emit JSON BEFORE blocking, so Service.qml can read via SplitParser
    let kind = if filename == "clipboard.txt" {
        "text-link"
    } else if filename.starts_with("clipboard.") {
        "image"
    } else {
        "file"
    };
    emit_json(
        &qr_file,
        &filename,
        kind,
        Some(&url),
        Some(&filename),
        Some(&content_type),
        Some(&qr_file),
    );

    // Now serve forever until killed
    serve_requests(server, payload, filename, content_type, route);
    // unreachable unless server errors — but we keep Ok for type
    Ok(())
}

fn serve_requests(
    server: Server,
    payload: Payload,
    filename: String,
    content_type: String,
    route: String,
) {
    for request in server.incoming_requests() {
        if let Err(e) = serve_request(request, &payload, &filename, &content_type, &route) {
            eprintln!("omabeam: could not serve request: {e}");
        }
    }
}

fn serve_request(
    request: Request,
    payload: &Payload,
    filename: &str,
    content_type: &str,
    route: &str,
) -> io::Result<()> {
    if request.method() != &Method::Get {
        let allow = Header::from_bytes("Allow", "GET").expect("static header is valid");
        return request.respond(Response::empty(405).with_header(allow));
    }
    if request.url() != route {
        return request.respond(Response::empty(404));
    }
    let ct = Header::from_bytes("Content-Type", content_type).expect("MIME type is a valid header");
    let disp = Header::from_bytes(
        "Content-Disposition",
        format!("inline; filename=\"{filename}\""),
    )
    .expect("safe filename makes a valid header");
    match payload {
        Payload::File(file) => {
            let mut download = file.try_clone()?;
            download.rewind()?;
            request.respond(
                Response::from_file(download)
                    .with_header(ct)
                    .with_header(disp),
            )
        }
        Payload::Bytes(bytes) => request.respond(Response::new(
            StatusCode(200),
            vec![ct, disp],
            Cursor::new(bytes.as_slice()),
            Some(bytes.len()),
            None,
        )),
    }
}

// ---- helpers reused from file.rs (duplicated to keep diff small) ----

fn local_ipv4() -> Result<Ipv4Addr, String> {
    let output = Command::new("ip")
        .args(["-4", "route", "show", "default"])
        .output()
        .map_err(|e| format!("could not run ip; is iproute2 installed? ({e})"))?;
    if !output.status.success() {
        return Err(format!(
            "could not inspect the local network: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let routes = String::from_utf8(output.stdout)
        .map_err(|_| "ip returned an invalid default route".to_owned())?;
    let route = routes.lines().next().ok_or_else(|| {
        "could not find a LAN address; connect to a local network first".to_owned()
    })?;
    let address = route
        .split_whitespace()
        .zip(route.split_whitespace().skip(1))
        .find_map(|(w, n)| (w == "src").then_some(n))
        .ok_or_else(|| "the default network route has no source address".to_owned())?
        .parse::<Ipv4Addr>()
        .map_err(|e| format!("the default network route has an invalid address: {e}"))?;
    if address.is_loopback() || address.is_unspecified() {
        Err("could not find a LAN address; connect to a local network first".into())
    } else {
        Ok(address)
    }
}

fn route_token() -> Result<String, String> {
    let token = fs::read_to_string("/proc/sys/kernel/random/uuid")
        .map_err(|e| format!("could not generate a download token: {e}"))?;
    let token = token.trim();
    if token.is_empty() || !token.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return Err("the kernel returned an invalid download token".into());
    }
    Ok(token.to_owned())
}

fn safe_filename(path: &Path) -> String {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => c,
            _ => '_',
        })
        .collect::<String>();
    if filename.is_empty() {
        "download".into()
    } else {
        filename
    }
}

fn clipboard_image_filename(content_type: &str) -> String {
    let ext = mime_guess::get_mime_extensions_str(content_type)
        .and_then(|exts| exts.first())
        .copied()
        .unwrap_or("img");
    format!("clipboard.{ext}")
}

// ---- JSON helpers (no serde to keep deps minimal) ----

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out
}

fn emit_json(
    qr_file: &Path,
    detail: &str,
    kind: &str,
    url: Option<&str>,
    filename: Option<&str>,
    content_type: Option<&str>,
    qr_png: Option<&Path>,
) {
    // One JSON line to stdout, flushed for SplitParser
    let qr_str = qr_png
        .or(Some(qr_file))
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut json = String::new();
    json.push_str("{\"kind\":\"");
    json.push_str(&json_escape(kind));
    json.push_str("\",\"detail\":\"");
    json.push_str(&json_escape(detail));
    json.push_str("\",\"qrPng\":\"");
    json.push_str(&json_escape(&qr_str));
    json.push('"');
    if let Some(u) = url {
        json.push_str(",\"url\":\"");
        json.push_str(&json_escape(u));
        json.push('"');
    }
    if let Some(f) = filename {
        json.push_str(",\"filename\":\"");
        json.push_str(&json_escape(f));
        json.push('"');
    }
    if let Some(ct) = content_type {
        json.push_str(",\"contentType\":\"");
        json.push_str(&json_escape(ct));
        json.push('"');
    }
    json.push('}');
    println!("{json}");
    let _ = io::stdout().flush();
}

fn emit_error(qr_file: &Path, msg: &str) {
    let mut json = String::new();
    json.push_str("{\"kind\":\"error\",\"detail\":\"");
    json.push_str(&json_escape(msg));
    json.push_str("\",\"qrPng\":\"");
    json.push_str(&json_escape(&qr_file.display().to_string()));
    json.push_str("\"}");
    println!("{json}");
    let _ = io::stdout().flush();
}
