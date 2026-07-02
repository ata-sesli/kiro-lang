use super::KiroError;
use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, Severity,
    SourceCode,
};
use std::fmt::{self, Display};

pub fn emit_error(err: &KiroError) {
    eprintln!("{}", render_error(err));
}

fn render_error(err: &KiroError) -> String {
    if let Some(report) = KiroMietteDiagnostic::new(err) {
        let mut rendered = String::new();
        let handler = GraphicalReportHandler::new_themed(GraphicalTheme::none()).tab_width(4);
        if handler.render_report(&mut rendered, &report).is_ok() {
            return rendered.trim_end().to_string();
        }
    }

    render_plain_error(err)
}

fn render_plain_error(err: &KiroError) -> String {
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

#[derive(Debug)]
struct KiroMietteDiagnostic {
    heading: String,
    source: NamedSource<String>,
    span_offset: usize,
    span_len: usize,
    label: Option<String>,
    help: Option<String>,
}

impl KiroMietteDiagnostic {
    fn new(err: &KiroError) -> Option<Self> {
        let file = err.file.clone()?;
        let source_text = err.source_text.clone()?;
        let span_offset = err.span_offset?;
        let span_len = err.span_len?;

        Some(Self {
            heading: format!("[{}:{}] {}", err.code, err.phase, err.message),
            source: NamedSource::new(file, source_text),
            span_offset,
            span_len,
            label: err.label.clone(),
            help: help_text(err),
        })
    }
}

impl Display for KiroMietteDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.heading)
    }
}

impl std::error::Error for KiroMietteDiagnostic {}

impl Diagnostic for KiroMietteDiagnostic {
    fn severity(&self) -> Option<Severity> {
        Some(Severity::Error)
    }

    fn help<'a>(&'a self) -> Option<Box<dyn Display + 'a>> {
        self.help
            .as_ref()
            .map(|help| Box::new(help.as_str()) as Box<dyn Display + 'a>)
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.source)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        let label = self.label.clone();
        let span = (self.span_offset, self.span_len);
        Some(Box::new(std::iter::once(
            LabeledSpan::new_primary_with_span(label, span),
        )))
    }
}

fn help_text(err: &KiroError) -> Option<String> {
    match (&err.help, &err.suggestion) {
        (Some(help), Some(suggestion)) => Some(format!("{}\ndid you mean '{}'?", help, suggestion)),
        (Some(help), None) => Some(help.clone()),
        (None, Some(suggestion)) => Some(format!("did you mean '{}'?", suggestion)),
        (None, None) => None,
    }
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

#[cfg(test)]
mod tests {
    use super::render_error;
    use crate::errors::{ErrorCode, ErrorPhase, KiroError};

    #[test]
    fn miette_renderer_uses_full_source_span_when_available() {
        let source = "var count = 1\nio.print(coutn)\n";
        let err = KiroError::new(
            ErrorCode::UnknownName,
            ErrorPhase::Compile,
            "Unknown variable 'coutn'.",
        )
        .with_source_span("main.kiro", source, 2, 10, 5, "unknown variable")
        .with_suggestion("count");

        let rendered = render_error(&err);

        assert!(rendered.contains("[KIRO2004:compile] Unknown variable 'coutn'."));
        assert!(rendered.contains("main.kiro:2:10"));
        assert!(rendered.contains("io.print(coutn)"));
        assert!(rendered.contains("unknown variable"));
        assert!(rendered.contains("help: did you mean 'count'?"));
    }
}
