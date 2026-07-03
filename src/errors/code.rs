#[derive(Debug, Clone, Copy)]
pub enum ErrorPhase {
    Cli,
    Parse,
    Compile,
    Runtime,
}

impl std::fmt::Display for ErrorPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorPhase::Cli => write!(f, "cli"),
            ErrorPhase::Parse => write!(f, "parse"),
            ErrorPhase::Compile => write!(f, "compile"),
            ErrorPhase::Runtime => write!(f, "runtime"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    FileNotFound,
    UnsupportedKeyword,
    ParseFailed,
    TypeError,
    WrongArgumentCount,
    UnknownName,
    PureViolation,
    MutabilityError,
    ImportError,
    BadUse,
    MissingHostGlue,
    CheckFailed,
    PipeTakeClosed,
    PipeGiveClosed,
    ListIndexOutOfBounds,
    MapKeyNotFound,
    EmptyAddressDeref,
    HostFunctionFailed,
    BuildGraphFailed,
    CompilerPanic,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorCode::FileNotFound => write!(f, "KIRO1001"),
            ErrorCode::UnsupportedKeyword => write!(f, "KIRO1002"),
            ErrorCode::ParseFailed => write!(f, "KIRO1003"),
            ErrorCode::TypeError => write!(f, "KIRO2002"),
            ErrorCode::WrongArgumentCount => write!(f, "KIRO2003"),
            ErrorCode::UnknownName => write!(f, "KIRO2004"),
            ErrorCode::PureViolation => write!(f, "KIRO2005"),
            ErrorCode::MutabilityError => write!(f, "KIRO2006"),
            ErrorCode::ImportError => write!(f, "KIRO2007"),
            ErrorCode::BadUse => write!(f, "KIRO2008"),
            ErrorCode::MissingHostGlue => write!(f, "KIRO2009"),
            ErrorCode::CheckFailed => write!(f, "KIRO3001"),
            ErrorCode::PipeTakeClosed => write!(f, "KIRO3002"),
            ErrorCode::PipeGiveClosed => write!(f, "KIRO3003"),
            ErrorCode::ListIndexOutOfBounds => write!(f, "KIRO3004"),
            ErrorCode::MapKeyNotFound => write!(f, "KIRO3005"),
            ErrorCode::EmptyAddressDeref => write!(f, "KIRO3006"),
            ErrorCode::HostFunctionFailed => write!(f, "KIRO3007"),
            ErrorCode::BuildGraphFailed => write!(f, "KIRO4001"),
            ErrorCode::CompilerPanic => write!(f, "KIRO2001"),
        }
    }
}
