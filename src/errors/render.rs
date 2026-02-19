use super::KiroError;

pub fn emit_error(err: &KiroError) {
    let where_str = match (&err.file, err.line) {
        (Some(file), Some(line)) => format!("{}:{}", file, line),
        (Some(file), None) => file.clone(),
        _ => "<unknown>".to_string(),
    };

    let mut msg = format!(
        "[{}:{}] {} ({})",
        err.code, err.phase, err.message, where_str
    );
    if let Some(help) = &err.help {
        msg.push_str(&format!("\nhelp: {}", help));
    }

    eprintln!("{:?}", miette::miette!("{}", msg));
}
