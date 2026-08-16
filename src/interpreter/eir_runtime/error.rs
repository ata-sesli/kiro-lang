use super::*;
use std::fmt;

use crate::eir::VerifyError;

#[derive(Debug)]
pub struct EirRuntimeError {
    pub anchor: SourceAnchor,
    pub kind: EirRuntimeErrorKind,
}

#[derive(Debug)]
pub enum EirRuntimeErrorKind {
    Verification(Vec<VerifyError>),
    InvalidFunction(FunctionId),
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    TypeMismatch {
        expected: String,
        actual: &'static str,
    },
    UninitializedSlot(SlotId),
    UninitializedGlobal(crate::eir::GlobalId),
    PoisonedGlobals,
    EmptyAddress,
    PoisonedAddress,
    MissingField(crate::hir::FieldId),
    ListIndexOutOfBounds {
        index: usize,
        length: usize,
    },
    InvalidListRange {
        start: f64,
        end: f64,
        length: usize,
    },
    BytesIndexOutOfBounds {
        index: usize,
        length: usize,
    },
    MapKeyNotFound(String),
    CheckFailed(String),
    InvalidCallable(String),
    HostCallDenied {
        module: String,
        function: String,
    },
    HostFunctionNotRegistered {
        module: String,
        function: String,
    },
    HostCallFailed(String),
    PipeClosed,
    StepLimitExceeded {
        steps: u64,
        limit: u64,
    },
    CallDepthExceeded {
        depth: usize,
        limit: usize,
    },
    TimeoutExceeded {
        milliseconds: u128,
    },
    Cancelled,
    ReachedUnreachable,
    Thrown(Box<RuntimeVal>),
    ReentrantExecution,
}

impl fmt::Display for EirRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "EIR runtime error at {}:{}..{}: ",
            self.anchor.source().raw(),
            self.anchor.start(),
            self.anchor.end()
        )?;
        match &self.kind {
            EirRuntimeErrorKind::Verification(errors) => {
                write!(formatter, "{} verifier error(s)", errors.len())
            }
            EirRuntimeErrorKind::InvalidFunction(function) => {
                write!(formatter, "invalid function f{}", function.raw())
            }
            EirRuntimeErrorKind::ArgumentCount { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected} argument(s), received {actual}"
                )
            }
            EirRuntimeErrorKind::TypeMismatch { expected, actual } => {
                write!(formatter, "expected {expected}, received {actual}")
            }
            EirRuntimeErrorKind::UninitializedSlot(slot) => {
                write!(formatter, "read uninitialized slot s{}", slot.raw())
            }
            EirRuntimeErrorKind::UninitializedGlobal(global) => {
                write!(formatter, "read uninitialized global g{}", global.raw())
            }
            EirRuntimeErrorKind::PoisonedGlobals => {
                formatter.write_str("module global storage is poisoned")
            }
            EirRuntimeErrorKind::EmptyAddress => {
                formatter.write_str("dereferenced an empty address")
            }
            EirRuntimeErrorKind::PoisonedAddress => {
                formatter.write_str("address storage is poisoned")
            }
            EirRuntimeErrorKind::MissingField(field) => {
                write!(formatter, "missing struct field field{}", field.raw())
            }
            EirRuntimeErrorKind::ListIndexOutOfBounds { index, length } => {
                write!(
                    formatter,
                    "list index {index} out of bounds for length {length}"
                )
            }
            EirRuntimeErrorKind::InvalidListRange { start, end, length } => write!(
                formatter,
                "invalid list range {start}..{end} for length {length}"
            ),
            EirRuntimeErrorKind::BytesIndexOutOfBounds { index, length } => {
                write!(
                    formatter,
                    "byte index {index} out of bounds for length {length}"
                )
            }
            EirRuntimeErrorKind::MapKeyNotFound(key) => {
                write!(formatter, "map key not found: {key}")
            }
            EirRuntimeErrorKind::CheckFailed(message) => {
                write!(formatter, "check failed: {message}")
            }
            EirRuntimeErrorKind::InvalidCallable(value) => {
                write!(formatter, "invalid callable {value}")
            }
            EirRuntimeErrorKind::HostCallDenied { module, function } => {
                write!(formatter, "host call denied for '{module}.{function}'")
            }
            EirRuntimeErrorKind::HostFunctionNotRegistered { module, function } => {
                write!(
                    formatter,
                    "host function '{module}.{function}' is not registered"
                )
            }
            EirRuntimeErrorKind::HostCallFailed(message) => formatter.write_str(message),
            EirRuntimeErrorKind::PipeClosed => formatter.write_str("pipe is closed"),
            EirRuntimeErrorKind::StepLimitExceeded { steps, limit } => {
                write!(formatter, "step limit exceeded ({steps} > {limit})")
            }
            EirRuntimeErrorKind::CallDepthExceeded { depth, limit } => {
                write!(formatter, "call depth exceeded ({depth} > {limit})")
            }
            EirRuntimeErrorKind::TimeoutExceeded { milliseconds } => {
                write!(formatter, "timeout exceeded (>{milliseconds} ms)")
            }
            EirRuntimeErrorKind::Cancelled => formatter.write_str("execution cancelled"),
            EirRuntimeErrorKind::ReachedUnreachable => {
                formatter.write_str("reached an unreachable terminator")
            }
            EirRuntimeErrorKind::Thrown(value) => write!(formatter, "uncaught throw: {value}"),
            EirRuntimeErrorKind::ReentrantExecution => {
                formatter.write_str("runtime already has active frames")
            }
        }
    }
}

impl std::error::Error for EirRuntimeError {}
