use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::process::{Child, Command, Output, Stdio};

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run_with_stdin(text: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_omabeam"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start omabeam");
    child
        .stdin
        .take()
        .expect("open stdin")
        .write_all(text)
        .expect("write stdin");
    child.wait_with_output().expect("wait for omabeam")
}

fn run_with_clipboard_script(name: &str, script: &str) -> Output {
    let test_dir = std::env::temp_dir().join(format!("omabeam-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&test_dir).expect("create test directory");
    let wl_paste = test_dir.join("wl-paste");
    std::fs::write(&wl_paste, script).expect("write fake wl-paste");
    let mut permissions = std::fs::metadata(&wl_paste)
        .expect("read fake wl-paste metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&wl_paste, permissions).expect("make fake wl-paste executable");

    let path = format!(
        "{}:{}",
        test_dir.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new("script")
        .args(["-qec", env!("CARGO_BIN_EXE_omabeam"), "/dev/null"])
        .env("PATH", path)
        .env("NO_COLOR", "1")
        .output()
        .expect("run omabeam in a terminal");
    std::fs::remove_dir_all(test_dir).expect("remove test directory");
    output
}

#[test]
fn piped_text_prints_a_terminal_qr_code() {
    let output = run_with_stdin(b"hello");
    let stdout = String::from_utf8(output.stdout).expect("stdout is UTF-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");

    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.starts_with("\x1b[38;2;0;0;0;48;2;255;255;255m"),
        "stdout: {stdout:?}"
    );
    assert!(stdout.lines().count() >= 10, "stdout: {stdout:?}");
}

#[test]
fn terminal_stdin_reads_the_wayland_clipboard() {
    let clipboard_output =
        run_with_clipboard_script("clipboard", "#!/bin/sh\nprintf 'clipboard text'");

    let piped_output = run_with_stdin(b"clipboard text");

    assert!(
        clipboard_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&clipboard_output.stderr)
    );
    assert_eq!(
        String::from_utf8(clipboard_output.stdout)
            .expect("terminal stdout is UTF-8")
            .replace('\r', "")
            .replace("\x1b[?25l", "")
            .replace("\x1b[?25h", ""),
        String::from_utf8(piped_output.stdout).expect("piped stdout is UTF-8")
    );
}

#[test]
fn empty_clipboard_error_says_how_to_recover() {
    let output = run_with_clipboard_script("empty-clipboard", "#!/bin/sh\nexit 1");
    let terminal_output = String::from_utf8(output.stdout).expect("terminal output is UTF-8");

    assert!(!output.status.success());
    assert!(
        terminal_output.contains("copy some text or an image first"),
        "terminal output: {terminal_output:?}"
    );
}

#[test]
fn empty_piped_input_error_says_how_to_recover() {
    let output = run_with_stdin(b"");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");

    assert!(!output.status.success());
    assert!(
        stderr.contains("pipe text into omabeam"),
        "stderr: {stderr:?}"
    );
}

#[test]
fn file_mode_serves_only_the_selected_file() {
    let test_dir = std::env::temp_dir().join(format!("omabeam-file-{}", std::process::id()));
    std::fs::create_dir_all(&test_dir).expect("create test directory");
    let selected = test_dir.join("photo.jpg");
    std::fs::write(&selected, b"selected file").expect("write selected file");
    std::fs::write(test_dir.join("private.txt"), b"private file").expect("write adjacent file");

    let mut server = ChildGuard(
        Command::new(env!("CARGO_BIN_EXE_omabeam"))
            .arg(&selected)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start file server"),
    );
    let mut stdout = BufReader::new(server.0.stdout.take().expect("open server stdout"));
    let mut line = String::new();
    let url = loop {
        line.clear();
        assert_ne!(stdout.read_line(&mut line).expect("read server output"), 0);
        if line.starts_with("http://") {
            break line.trim().to_owned();
        }
    };
    let (address, download_path) = url
        .strip_prefix("http://")
        .expect("HTTP URL")
        .split_once('/')
        .expect("URL path");
    let (host, port) = address.split_once(':').expect("server port");
    assert_eq!(host, default_route_source());
    assert_eq!(port, "61234");
    assert!(
        download_path.ends_with("/download") && download_path != "download",
        "download URL must contain a route token: {url}"
    );
    assert!(
        TcpStream::connect(("127.0.0.1", port.parse::<u16>().expect("numeric port"))).is_err(),
        "file server must not listen on unrelated interfaces"
    );

    let selected_response = http_get(address, &format!("/{download_path}"));
    let (selected_headers, selected_body) = split_response(&selected_response);
    let selected_headers =
        std::str::from_utf8(selected_headers).expect("response headers are UTF-8");
    assert!(selected_headers.starts_with("HTTP/1.1 200 OK\r\n"));
    assert!(selected_headers.contains("\r\nContent-Type: image/jpeg\r\n"));
    assert!(
        selected_headers.contains("\r\nContent-Disposition: inline; filename=\"photo.jpg\"\r\n")
    );
    assert_eq!(selected_body, b"selected file");

    let missing_token_response = http_get(address, "/download");
    assert!(missing_token_response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

    let adjacent_response = http_get(address, "/private.txt");
    assert!(adjacent_response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

    let directory_response = http_get(address, "/");
    assert!(directory_response.starts_with(b"HTTP/1.1 404 Not Found\r\n"));

    drop(server);
    std::fs::remove_dir_all(test_dir).expect("remove test directory");
}

fn http_get(address: &str, path: &str) -> Vec<u8> {
    let mut stream = TcpStream::connect(address).expect("connect to file server");
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .expect("send HTTP request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("read HTTP response");
    response
}

fn split_response(response: &[u8]) -> (&[u8], &[u8]) {
    let body_start = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("HTTP header terminator")
        + 4;
    response.split_at(body_start)
}

fn default_route_source() -> String {
    let output = Command::new("ip")
        .args(["-4", "route", "show", "default"])
        .output()
        .expect("read default route");
    let route = String::from_utf8(output.stdout).expect("default route is UTF-8");
    let mut words = route.split_whitespace();

    while let Some(word) = words.next() {
        if word == "src" {
            return words
                .next()
                .expect("default route source address")
                .to_owned();
        }
    }

    panic!("default route has no source address: {route}");
}
