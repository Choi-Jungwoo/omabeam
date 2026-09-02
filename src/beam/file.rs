//! Serves one open file on one LAN address and one tokenized download route.

use std::fs::{self, File};
use std::io::{self, Seek};
use std::net::Ipv4Addr;
use std::path::Path;
use std::process::Command;

use tiny_http::{Header, Method, Request, Response, Server};

use super::{qr, ui};

const PORT: u16 = 61_234;

pub(super) fn serve(path: &Path) -> Result<(), String> {
    let file =
        File::open(path).map_err(|error| format!("could not open {}: {error}", path.display()))?;
    if !file
        .metadata()
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?
        .is_file()
    {
        return Err(format!("{} is not a regular file", path.display()));
    }

    let address = local_ipv4()?;
    let server = Server::http((address, PORT))
        .map_err(|error| format!("could not start the file server on port {PORT}: {error}"))?;
    let route = format!("/{}/download", route_token()?);
    let url = format!("http://{address}:{PORT}{route}");
    let filename = safe_filename(path);
    let content_type = mime_guess::from_path(path)
        .first_raw()
        .unwrap_or("application/octet-stream");

    let code = qr::render(&url)?;
    ui::present_file(path, &url, &code)
        .map_err(|error| format!("could not print the QR code: {error}"))?;

    for request in server.incoming_requests() {
        if let Err(error) = serve_request(request, &file, &filename, content_type, &route) {
            eprintln!("omabeam: could not serve request: {error}");
        }
    }

    Ok(())
}

fn serve_request(
    request: Request,
    file: &File,
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

    let mut download = file.try_clone()?;
    download.rewind()?;
    let content_type =
        Header::from_bytes("Content-Type", content_type).expect("MIME type is a valid header");
    let disposition = Header::from_bytes(
        "Content-Disposition",
        format!("inline; filename=\"{filename}\""),
    )
    .expect("safe filename makes a valid header");

    request.respond(
        Response::from_file(download)
            .with_header(content_type)
            .with_header(disposition),
    )
}

fn route_token() -> Result<String, String> {
    let token = fs::read_to_string("/proc/sys/kernel/random/uuid")
        .map_err(|error| format!("could not generate a download token: {error}"))?;
    let token = token.trim();

    if token.is_empty()
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err("the kernel returned an invalid download token".into());
    }

    Ok(token.to_owned())
}

fn local_ipv4() -> Result<Ipv4Addr, String> {
    let output = Command::new("ip")
        .args(["-4", "route", "show", "default"])
        .output()
        .map_err(|error| format!("could not run ip; is iproute2 installed? ({error})"))?;
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
        .find_map(|(word, next)| (word == "src").then_some(next))
        .ok_or_else(|| "the default network route has no source address".to_owned())?
        .parse::<Ipv4Addr>()
        .map_err(|error| format!("the default network route has an invalid address: {error}"))?;

    if address.is_loopback() || address.is_unspecified() {
        Err("could not find a LAN address; connect to a local network first".into())
    } else {
        Ok(address)
    }
}

fn safe_filename(path: &Path) -> String {
    let filename = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' => character,
            _ => '_',
        })
        .collect::<String>();

    if filename.is_empty() {
        "download".into()
    } else {
        filename
    }
}
