//! Serves one open file on one LAN address and one fixed download path.

mod http;

use std::fs::{self, File};
use std::io::{self, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;
use std::process::Command;

use super::qr;

const PORT: u16 = 61_234;

pub(super) fn serve(path: &Path) -> Result<(), String> {
    let mut file =
        File::open(path).map_err(|error| format!("could not open {}: {error}", path.display()))?;
    if !file
        .metadata()
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?
        .is_file()
    {
        return Err(format!("{} is not a regular file", path.display()));
    }

    let address = local_ipv4()?;
    let listener = TcpListener::bind((address, PORT))
        .map_err(|error| format!("could not start the file server on port {PORT}: {error}"))?;
    let route = format!("/{}/download", route_token()?);
    let url = format!("http://{address}:{PORT}{route}");
    let filename = safe_filename(path);

    println!("Sharing {} at:", path.display());
    println!("{url}");
    qr::print(&url)?;
    println!("Press Ctrl-C to stop.");
    io::stdout()
        .flush()
        .map_err(|error| format!("could not print the QR code: {error}"))?;

    for connection in listener.incoming() {
        let stream = connection.map_err(|error| format!("file server failed: {error}"))?;
        if let Err(error) = http::serve_request(stream, &mut file, &filename, &route) {
            eprintln!("omabeam: could not serve request: {error}");
        }
    }

    Ok(())
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
