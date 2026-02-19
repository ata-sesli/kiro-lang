use super::{ErrorCode, ErrorPhase};

#[derive(Debug, Clone)]
pub struct KiroError {
    pub code: ErrorCode,
    pub phase: ErrorPhase,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub help: Option<String>,
}

impl KiroError {
    pub fn new(code: ErrorCode, phase: ErrorPhase, message: impl Into<String>) -> Self {
        Self {
            code,
            phase,
            message: message.into(),
            file: None,
            line: None,
            help: None,
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

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
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

    pub fn parse_failed(file_or_module: &str, details: &str) -> Self {
        Self::new(
            ErrorCode::ParseFailed,
            ErrorPhase::Parse,
            format!("Parse failed: {}", details),
        )
        .with_file(file_or_module)
    }

    pub fn build_graph_failed() -> Self {
        Self::new(
            ErrorCode::BuildGraphFailed,
            ErrorPhase::Compile,
            "Build graph resolution failed.",
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
