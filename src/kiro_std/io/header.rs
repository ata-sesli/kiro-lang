// Kiro Standard Library: IO (stdin/stdout/stderr)
// Glue layer between Kiro and Rust terminal IO.

use kiro_runtime::{HostResult, KiroError, RuntimeVal};
use std::io::{self, Write};

/// Parses flexible boolean text forms into a bool.
/// Accepts: true/false, t/f, 1/0, yes/no, y/n, on/off (case-insensitive).
fn parse_bool_text(s: &str) -> Option<bool> {
    match s.trim().to_ascii_lowercase().as_str() {
        "true" | "t" | "1" | "yes" | "y" | "on" => Some(true),
        "false" | "f" | "0" | "no" | "n" | "off" => Some(false),
        _ => None,
    }
}

/// Writes text to stderr without a trailing newline.
pub async fn eprint(args: Vec<RuntimeVal>) -> HostResult {
    let msg = args[0].as_str()?;
    eprint!("{}", msg);
    Ok(RuntimeVal::Void)
}

/// Writes text to stderr with a trailing newline.
pub async fn eprintline(args: Vec<RuntimeVal>) -> HostResult {
    let msg = args[0].as_str()?;
    eprintln!("{}", msg);
    Ok(RuntimeVal::Void)
}

/// Reads one line from stdin and strips trailing newline characters.
/// Returns `IoError` when reading fails.
pub async fn read_line(_args: Vec<RuntimeVal>) -> HostResult {
    let line = tokio::task::spawn_blocking(|| {
        let mut buf = String::new();
        io::stdin().read_line(&mut buf).map(|_| buf)
    })
    .await
    .map_err(|_| KiroError::new("IoError"))
    .and_then(|res| res.map_err(|_| KiroError::new("IoError")))?;

    Ok(RuntimeVal::from(
        line.trim_end_matches(['\r', '\n']).to_string(),
    ))
}

/// Prints a prompt to stdout, flushes it, then reads one line from stdin.
/// The returned string is trimmed for trailing `\\r` and `\\n`.
/// Returns `IoError` when writing the prompt or reading input fails.
pub async fn input(args: Vec<RuntimeVal>) -> HostResult {
    let prompt = args[0].as_str()?.to_string();

    let line = tokio::task::spawn_blocking(move || {
        let mut stdout = io::stdout();
        write!(stdout, "{}", prompt)?;
        stdout.flush()?;

        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        Ok::<String, io::Error>(buf)
    })
    .await
    .map_err(|_| KiroError::new("IoError"))
    .and_then(|res| res.map_err(|_| KiroError::new("IoError")))?;

    Ok(RuntimeVal::from(
        line.trim_end_matches(['\r', '\n']).to_string(),
    ))
}

/// Parses numeric text into `num` (`f64`).
/// Returns `ParseNumError` when parsing fails.
pub async fn parse_num(args: Vec<RuntimeVal>) -> HostResult {
    let text = args[0].as_str()?.trim();
    let n = text
        .parse::<f64>()
        .map_err(|_| KiroError::new("ParseNumError"))?;
    Ok(RuntimeVal::from(n))
}

/// Prompts the user and parses the result as `num`.
/// Returns `IoError` or `ParseNumError`.
pub async fn input_num(args: Vec<RuntimeVal>) -> HostResult {
    let line = input(args).await?;
    parse_num(vec![line]).await
}

/// Prompts the user and parses the result as `bool`.
/// Returns `IoError` or `ParseBoolError`.
pub async fn input_bool(args: Vec<RuntimeVal>) -> HostResult {
    let line = input(args).await?;
    let text = line.as_str()?.trim();
    let b = parse_bool_text(text).ok_or_else(|| KiroError::new("ParseBoolError"))?;
    Ok(RuntimeVal::from(b))
}
