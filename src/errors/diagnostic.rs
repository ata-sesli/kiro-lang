use super::{ErrorCode, ErrorPhase};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: usize,
    pub end: usize,
}

impl SourceSpan {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn len(self) -> usize {
        self.end.saturating_sub(self.start).max(1)
    }
}

#[derive(Debug, Clone)]
pub struct KiroError {
    pub code: ErrorCode,
    pub phase: ErrorPhase,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub label: Option<String>,
    pub source_line: Option<String>,
    pub source_text: Option<String>,
    pub span_offset: Option<usize>,
    pub span_len: Option<usize>,
    pub help: Option<String>,
    pub suggestion: Option<String>,
}

impl KiroError {
    pub fn new(code: ErrorCode, phase: ErrorPhase, message: impl Into<String>) -> Self {
        Self {
            code,
            phase,
            message: message.into(),
            file: None,
            line: None,
            column: None,
            label: None,
            source_line: None,
            source_text: None,
            span_offset: None,
            span_len: None,
            help: None,
            suggestion: None,
        }
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into());
        self
    }

    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn with_source_location(
        mut self,
        file: impl Into<String>,
        line: usize,
        column: usize,
        source_line: impl Into<String>,
        label: impl Into<String>,
    ) -> Self {
        self.file = Some(file.into());
        self.line = Some(line);
        self.column = Some(column);
        self.source_line = Some(source_line.into());
        self.label = Some(label.into());
        self
    }

    pub fn with_source_span(
        mut self,
        file: impl Into<String>,
        source_text: impl Into<String>,
        line: usize,
        column: usize,
        span_len: usize,
        label: impl Into<String>,
    ) -> Self {
        let source_text = source_text.into();
        self.file = Some(file.into());
        self.line = Some(line);
        self.column = Some(column);
        self.source_line = source_line(&source_text, line);
        self.span_offset = byte_offset_at(&source_text, line, column);
        self.span_len = Some(span_len.max(1));
        self.source_text = Some(source_text);
        self.label = Some(label.into());
        self
    }

    pub fn with_byte_span(
        mut self,
        file: impl Into<String>,
        source_text: impl Into<String>,
        span: SourceSpan,
        label: impl Into<String>,
    ) -> Self {
        let source_text = source_text.into();
        let offset = span.start.min(source_text.len());
        let len = span.len();
        let (line, column) = line_column_at(&source_text, offset);
        self.file = Some(file.into());
        self.line = Some(line);
        self.column = Some(column);
        self.source_line = source_line(&source_text, line);
        self.span_offset = Some(offset);
        self.span_len = Some(len);
        self.source_text = Some(source_text);
        self.label = Some(label.into());
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub fn file_not_found(file: &str) -> Self {
        Self::new(
            ErrorCode::FileNotFound,
            ErrorPhase::Cli,
            format!("File '{}' not found.", file),
        )
        .with_file(file)
    }

    pub fn unsupported_keyword(file_or_module: &str, line: usize, keyword: &str) -> Self {
        Self::new(
            ErrorCode::UnsupportedKeyword,
            ErrorPhase::Parse,
            format!("Unsupported keyword '{}'.", keyword),
        )
        .with_file(file_or_module)
        .with_line(line)
        .with_help("Use 'var x = ...' (mutable) or 'x = ...' (immutable).")
    }

    pub fn unsupported_keyword_with_source(
        file_or_module: &str,
        source: &str,
        line: usize,
        column: usize,
        keyword: &str,
    ) -> Self {
        Self::new(
            ErrorCode::UnsupportedKeyword,
            ErrorPhase::Parse,
            format!("Unsupported keyword '{}'.", keyword),
        )
        .with_source_span(
            file_or_module,
            source,
            line,
            column,
            keyword.len(),
            "unsupported keyword",
        )
        .with_help("Use 'var x = ...' (mutable) or 'x = ...' (immutable).")
    }

    pub fn removed_print_statement(
        file_or_module: &str,
        source: &str,
        line: usize,
        column: usize,
    ) -> Self {
        Self::new(
            ErrorCode::UnsupportedKeyword,
            ErrorPhase::Parse,
            "'print' statement was removed.",
        )
        .with_source_span(
            file_or_module,
            source,
            line,
            column,
            "print".len(),
            "removed print statement",
        )
        .with_help("use `import io` and `io.print(value)`")
    }

    pub fn parse_failed(file_or_module: &str, details: &str) -> Self {
        Self::new(
            ErrorCode::ParseFailed,
            ErrorPhase::Parse,
            format!("Parse failed: {}", details),
        )
        .with_file(file_or_module)
    }

    pub fn parse_failed_with_source(
        file_or_module: &str,
        source: &str,
        errors: &[rust_sitter::errors::ParseError],
    ) -> Self {
        let details = if errors.is_empty() {
            "unknown parse error".to_string()
        } else {
            format!("{:?}", errors)
        };
        let err = Self::new(
            ErrorCode::ParseFailed,
            ErrorPhase::Parse,
            format!("Parse failed: {}", details),
        );
        if let Some(first) = errors.first() {
            err.with_byte_span(
                file_or_module,
                source,
                SourceSpan::new(first.start, first.end.max(first.start + 1)),
                "parse error",
            )
        } else {
            err.with_file(file_or_module)
        }
    }

    pub fn build_graph_failed() -> Self {
        Self::new(
            ErrorCode::BuildGraphFailed,
            ErrorPhase::Compile,
            "Build graph resolution failed.",
        )
    }

    pub fn compile_error(
        module: &str,
        code: ErrorCode,
        message: impl Into<String>,
        help: Option<String>,
    ) -> Self {
        let mut err = Self::new(code, ErrorPhase::Compile, message).with_file(module);
        if let Some(help) = help {
            err = err.with_help(help);
        }
        err
    }

    pub fn runtime_check_failed(message: impl Into<String>) -> Self {
        Self::new(
            ErrorCode::CheckFailed,
            ErrorPhase::Runtime,
            format!("Check failed: {}", message.into()),
        )
    }

    pub fn compiler_panic(module: &str, details: &str) -> Self {
        Self::new(
            ErrorCode::CompilerPanic,
            ErrorPhase::Compile,
            details.to_string(),
        )
        .with_file(module)
    }
}

fn source_line(source: &str, target_line: usize) -> Option<String> {
    source
        .lines()
        .nth(target_line.saturating_sub(1))
        .map(str::to_string)
}

fn line_column_at(source: &str, offset: usize) -> (usize, usize) {
    let mut line_start = 0;
    for (idx, line) in source.split_inclusive('\n').enumerate() {
        let line_end = line_start + line.len();
        if offset < line_end {
            return (idx + 1, offset.saturating_sub(line_start) + 1);
        }
        line_start = line_end;
    }
    let line_count = source.lines().count().max(1);
    let column = source.len().saturating_sub(line_start) + 1;
    (line_count, column)
}

fn byte_offset_at(source: &str, target_line: usize, target_column: usize) -> Option<usize> {
    let mut line_start = 0;
    for (idx, line) in source.split_inclusive('\n').enumerate() {
        let line_number = idx + 1;
        if line_number == target_line {
            let column_offset = target_column.saturating_sub(1);
            return (column_offset <= line.len()).then_some(line_start + column_offset);
        }
        line_start += line.len();
    }

    if target_line == source.lines().count().max(1) {
        let column_offset = target_column.saturating_sub(1);
        return (column_offset <= source.len().saturating_sub(line_start))
            .then_some(line_start + column_offset);
    }

    None
}
