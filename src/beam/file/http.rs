//! Handles the bounded HTTP/1 request and fixed download response.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Seek, Write};
use std::net::TcpStream;
use std::time::Duration;

pub(super) fn serve_request(
    mut stream: TcpStream,
    file: &mut File,
    filename: &str,
) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new((&stream).take(8 * 1024));
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;

    let mut header = String::new();
    let headers_complete = loop {
        header.clear();
        match reader.read_line(&mut header)? {
            0 => break false,
            _ if header == "\r\n" || header == "\n" => break true,
            _ => {}
        }
    };
    drop(reader);

    if !request_line.ends_with('\n') || !headers_complete {
        return send_empty_response(&mut stream, "400 Bad Request", None);
    }

    let mut parts = request_line.split_whitespace();
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("GET"), Some("/download"), Some(_), None) => {
            let length = file.metadata()?.len();
            file.rewind()?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"{filename}\"\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n"
            )?;
            io::copy(&mut file.take(length), &mut stream)?;
            Ok(())
        }
        (Some("GET"), Some(_), Some(_), None) => {
            send_empty_response(&mut stream, "404 Not Found", None)
        }
        (Some(_), Some(_), Some(_), None) => send_empty_response(
            &mut stream,
            "405 Method Not Allowed",
            Some("Allow: GET\r\n"),
        ),
        _ => send_empty_response(&mut stream, "400 Bad Request", None),
    }
}

fn send_empty_response(
    stream: &mut TcpStream,
    status: &str,
    extra_headers: Option<&str>,
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {status}\r\n{}Content-Length: 0\r\nConnection: close\r\n\r\n",
        extra_headers.unwrap_or_default()
    )
}
