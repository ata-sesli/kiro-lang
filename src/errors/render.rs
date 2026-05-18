use super::KiroError;

pub fn emit_error(err: &KiroError) {
    eprintln!("{}", render_error(err));
}

fn render_error(err: &KiroError) -> String {
    let mut msg = format!("[{}:{}] {}", err.code, err.phase, err.message);

    match (&err.file, err.line, err.column) {
        (Some(file), Some(line), Some(column)) => {
            msg.push_str(&format!("\n  --> {}:{}:{}", file, line, column));
            if let Some(source_line) = &err.source_line {
                let width = line.to_string().len();
                msg.push_str(&format!(
                    "\n{:>width$} | {}",
                    line,
                    source_line,
                    width = width
                ));
                if let Some(label) = &err.label {
                    let caret_len = label_caret_len(err, source_line);
                    let spaces = " ".repeat(column.saturating_sub(1));
                    msg.push_str(&format!(
                        "\n{:>width$} | {}{} {}",
                        "",
                        spaces,
                        "^".repeat(caret_len),
                        label,
                        width = width
                    ));
                }
            }
        }
        (Some(file), Some(line), None) => {
            msg.push_str(&format!(" ({}:{})", file, line));
        }
        (Some(file), None, _) => {
            msg.push_str(&format!(" ({})", file));
        }
        _ => {
            msg.push_str(" (<unknown>)");
        }
    }

    if let Some(help) = &err.help {
        msg.push_str(&format!("\nhelp: {}", help));
    }
    if let Some(suggestion) = &err.suggestion {
        msg.push_str(&format!("\nhelp: did you mean '{}'?", suggestion));
    }

    msg
}

fn label_caret_len(err: &KiroError, source_line: &str) -> usize {
    let Some(column) = err.column else {
        return 1;
    };
    let start = column.saturating_sub(1);
    source_line[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '.')
        .count()
        .max(1)
}
