//! Owns the interactive terminal presentation without changing transfer behavior.

use std::env;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::thread;
use std::time::Duration;

use terminal_size::{Height, Width, terminal_size};

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const BLUE: &str = "\x1b[34m";
const GREEN: &str = "\x1b[32m";
const CYAN: &str = "\x1b[36m";
const MAGENTA: &str = "\x1b[35m";
const PLAIN_QR: &str = "\x1b[38;2;0;0;0;48;2;255;255;255m";
const QR_BLUE: &str = "\x1b[94m";
const QR_CYAN: &str = "\x1b[96m";
const QR_MAGENTA: &str = "\x1b[95m";
const QR_SCAN: &str = "\x1b[93m";
const CLEAR: &str = "\x1b[2J\x1b[H";
const MIN_FRAME_WIDTH: usize = 64;
const HEADER_HEIGHT: usize = 7;

const LOGO_OMA: [&str; 5] = [
    "██████  ██  ██    ██  ",
    "██  ██  ██████  ██  ██",
    "██  ██  ██████  ██████",
    "██  ██  ██  ██  ██  ██",
    "██████  ██  ██  ██  ██",
];
const LOGO_BEAM: [&str; 5] = [
    "████    ██████    ██    ██  ██",
    "██  ██  ██      ██  ██  ██████",
    "████    ████    ██████  ██████",
    "██  ██  ██      ██  ██  ██  ██",
    "████    ██████  ██  ██  ██  ██",
];
pub(super) fn present_text(text: &str, code: &str) -> io::Result<()> {
    let Some(dimensions) = presentation_dimensions(code) else {
        return print_plain(code);
    };

    present_interactive(
        &format!("TEXT BEAM // {} BYTES", text.len()),
        "NO APP // NO ACCOUNT // NO CLOUD",
        "TEXT",
        code,
        dimensions,
    )
}

pub(super) fn present_file(path: &Path, url: &str, code: &str) -> io::Result<()> {
    let Some(dimensions) = presentation_dimensions(code) else {
        return print_file_plain(path, url, code);
    };

    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    present_interactive(
        &format!("FILE BEAM // {filename}"),
        "CTRL+C // CLOSES THE LINK",
        "FILE",
        code,
        dimensions,
    )
}

fn present_interactive(
    detail: &str,
    note: &str,
    kind: &str,
    code: &str,
    (columns, rows): (usize, usize),
) -> io::Result<()> {
    let code_width = code_width(code);
    let code_height = code.lines().count();
    let frame_width = code_width.max(MIN_FRAME_WIDTH);
    let outer_width = frame_width + 2;
    let content_height = HEADER_HEIGHT + code_height + 4;
    let left = columns.saturating_sub(outer_width) / 2;
    let top = rows.saturating_sub(content_height + 2) / 2;
    let code_indent = left + 1 + frame_width.saturating_sub(code_width) / 2;
    let code_row = top + HEADER_HEIGHT + 3;
    let cursor_row = top + content_height + 1;
    let mut stdout = io::stdout().lock();

    animate_beam(&mut stdout, columns, rows, frame_width, kind)?;
    write!(stdout, "{CLEAR}{}", "\n".repeat(top))?;
    write_header(&mut stdout, left, frame_width, "READY")?;
    write_centered_line(&mut stdout, left, outer_width, detail, CYAN)?;
    writeln!(stdout)?;
    write_colored_code(&mut stdout, code, code_indent)?;
    write_centered_line(&mut stdout, left, outer_width, ">> SCAN TO OPEN <<", GREEN)?;
    write_centered_line(&mut stdout, left, outer_width, note, DIM)?;
    stdout.flush()?;

    animate_scan(&mut stdout, code, code_row, code_indent + 1)?;
    write!(stdout, "\x1b[{cursor_row};1H")?;
    stdout.flush()
}

fn presentation_dimensions(code: &str) -> Option<(usize, usize)> {
    if !io::stdout().is_terminal()
        || env::var_os("NO_COLOR").is_some()
        || !env::var("TERM").is_ok_and(|term| term != "dumb")
    {
        return None;
    }

    let (columns, rows) = terminal_dimensions()?;
    let required_columns = code_width(code).max(MIN_FRAME_WIDTH) + 2;
    let required_rows = HEADER_HEIGHT + code.lines().count() + 5;
    (columns >= required_columns && rows >= required_rows).then_some((columns, rows))
}

fn terminal_dimensions() -> Option<(usize, usize)> {
    terminal_size().map(|(Width(columns), Height(rows))| (usize::from(columns), usize::from(rows)))
}

fn print_plain(code: &str) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{PLAIN_QR}{code}{RESET}")?;
    stdout.flush()
}

fn print_file_plain(path: &Path, url: &str, code: &str) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "Sharing {} at:", path.display())?;
    writeln!(stdout, "{url}")?;
    writeln!(stdout, "{PLAIN_QR}{code}{RESET}")?;
    writeln!(stdout, "Press Ctrl-C to stop.")?;
    stdout.flush()
}

fn animate_beam(
    stdout: &mut impl Write,
    columns: usize,
    rows: usize,
    frame_width: usize,
    kind: &str,
) -> io::Result<()> {
    const TRACK_WIDTH: usize = 32;
    const FRAMES: [(usize, u64); 5] = [(5, 20), (14, 28), (22, 40), (28, 58), (32, 90)];

    let left = columns.saturating_sub(frame_width + 2) / 2;
    let top = rows.saturating_sub(HEADER_HEIGHT + 4) / 2;
    for (filled, delay) in FRAMES {
        let percentage = filled * 100 / TRACK_WIDTH;
        write!(stdout, "{CLEAR}{}", "\n".repeat(top))?;
        write_header(
            stdout,
            left,
            frame_width,
            &format!("LINKING {percentage:>3}%"),
        )?;
        writeln!(stdout)?;
        write_beam_track(stdout, columns, filled, TRACK_WIDTH)?;
        write_centered_line(
            stdout,
            0,
            columns,
            &format!("{kind} SIGNAL // LOCAL ONLY"),
            BLUE,
        )?;
        stdout.flush()?;
        thread::sleep(Duration::from_millis(delay));
    }
    Ok(())
}

fn write_beam_track(
    stdout: &mut impl Write,
    columns: usize,
    filled: usize,
    width: usize,
) -> io::Result<()> {
    let left = columns.saturating_sub(width) / 2;
    let beam = filled.saturating_sub(1);
    writeln!(
        stdout,
        "{}{BLUE}{}{MAGENTA}▓{DIM}{}{RESET}",
        " ".repeat(left),
        "▓".repeat(beam),
        "░".repeat(width - filled)
    )
}

fn write_header(stdout: &mut impl Write, left: usize, width: usize, state: &str) -> io::Result<()> {
    let prefix = " ".repeat(left);
    writeln!(stdout, "{prefix}{BLUE}┏{}┓{RESET}", "━".repeat(width))?;
    for (oma, beam) in LOGO_OMA.into_iter().zip(LOGO_BEAM) {
        write_logo_row(stdout, &prefix, width, oma, beam)?;
    }
    write_status_border(stdout, &prefix, width, state)
}

fn write_logo_row(
    stdout: &mut impl Write,
    prefix: &str,
    width: usize,
    oma: &str,
    beam: &str,
) -> io::Result<()> {
    let logo_width = oma.chars().count() + 4 + beam.chars().count();
    let remaining = width.saturating_sub(logo_width);
    let before = remaining / 2;
    let after = remaining - before;

    writeln!(
        stdout,
        "{prefix}{BLUE}┃{}{BOLD}{oma}{RESET}    {CYAN}{BOLD}{beam}{RESET}{}{MAGENTA}┃{RESET}",
        " ".repeat(before),
        " ".repeat(after)
    )
}

fn write_status_border(
    stdout: &mut impl Write,
    prefix: &str,
    width: usize,
    state: &str,
) -> io::Result<()> {
    let status = format!(" LOCAL // {state} ");
    let remaining = width.saturating_sub(status.len());
    let before = remaining / 2;
    let after = remaining - before;

    writeln!(
        stdout,
        "{prefix}{BLUE}┗{}{GREEN}{BOLD}{status}{RESET}{MAGENTA}{}┛{RESET}",
        "━".repeat(before),
        "━".repeat(after)
    )
}

fn write_centered_line(
    stdout: &mut impl Write,
    left: usize,
    width: usize,
    text: &str,
    style: &str,
) -> io::Result<()> {
    let offset = width.saturating_sub(text.chars().count()) / 2;
    writeln!(
        stdout,
        "{}{style}{BOLD}{text}{RESET}",
        " ".repeat(left + offset)
    )
}

fn write_colored_code(stdout: &mut impl Write, code: &str, indent: usize) -> io::Result<()> {
    let line_count = code.lines().count();
    let prefix = " ".repeat(indent);
    for (index, line) in code.lines().enumerate() {
        writeln!(
            stdout,
            "{prefix}{}{line}{RESET}",
            qr_color(index, line_count)
        )?;
    }
    Ok(())
}

fn animate_scan(
    stdout: &mut impl Write,
    code: &str,
    first_row: usize,
    column: usize,
) -> io::Result<()> {
    let lines = code.lines().collect::<Vec<_>>();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            write!(
                stdout,
                "\x1b[{};{column}H{}{}{RESET}",
                first_row + index - 1,
                qr_color(index - 1, lines.len()),
                lines[index - 1]
            )?;
        }
        write!(
            stdout,
            "\x1b[{};{column}H{QR_SCAN}{line}{RESET}",
            first_row + index
        )?;
        stdout.flush()?;
        thread::sleep(Duration::from_millis(16));
    }

    if let Some(last) = lines.last() {
        write!(
            stdout,
            "\x1b[{};{column}H{}{last}{RESET}",
            first_row + lines.len() - 1,
            qr_color(lines.len() - 1, lines.len())
        )?;
    }
    Ok(())
}

fn qr_color(index: usize, line_count: usize) -> &'static str {
    match index * 3 / line_count.max(1) {
        0 => QR_BLUE,
        1 => QR_CYAN,
        _ => QR_MAGENTA,
    }
}

fn code_width(code: &str) -> usize {
    code.lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or_default()
}
