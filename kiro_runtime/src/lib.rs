//! Kiro Runtime - Shared types for Kiro-Rust FFI
//!
//! This crate defines the runtime value representation and error types
//! used at the boundary between Kiro code and Rust glue functions.

use std::collections::HashMap;
use std::convert::TryFrom;

pub const KIRO_RUNTIME_ABI_VERSION: u32 = 1;

pub type HostResult = Result<RuntimeVal, KiroError>;

/// Runtime value representation for Kiro types.
/// This enum is used by:
/// - Compiler-generated Rust code
/// - Rust glue functions in header.rs
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeVal {
    Num(f64),
    Str(String),
    Bool(bool),
    List(Vec<RuntimeVal>),
    Map(HashMap<String, RuntimeVal>),
    Void,
}

/// Kiro error type for Rust glue functions.
/// The `name` field must match a Kiro error definition.
#[derive(Clone, Debug)]
pub struct KiroError {
    pub name: String,
    pub message: Option<String>,
}

impl KiroError {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            message: None,
        }
    }

    pub fn message(name: &str, message: impl Into<String>) -> Self {
        Self {
            name: name.to_string(),
            message: Some(message.into()),
        }
    }
}

impl std::fmt::Display for KiroError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(message) = &self.message {
            write!(f, "{}: {}", self.name, message)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

impl std::error::Error for KiroError {}

// --- Conversion: Rust types -> RuntimeVal ---

impl From<f64> for RuntimeVal {
    fn from(v: f64) -> Self {
        RuntimeVal::Num(v)
    }
}

impl From<String> for RuntimeVal {
    fn from(v: String) -> Self {
        RuntimeVal::Str(v)
    }
}

impl From<&str> for RuntimeVal {
    fn from(v: &str) -> Self {
        RuntimeVal::Str(v.to_string())
    }
}

impl From<bool> for RuntimeVal {
    fn from(v: bool) -> Self {
        RuntimeVal::Bool(v)
    }
}

impl From<()> for RuntimeVal {
    fn from(_: ()) -> Self {
        RuntimeVal::Void
    }
}

impl<T: Into<RuntimeVal>> From<Vec<T>> for RuntimeVal {
    fn from(v: Vec<T>) -> Self {
        RuntimeVal::List(v.into_iter().map(|x| x.into()).collect())
    }
}

// --- Conversion: RuntimeVal -> Rust types ---

impl RuntimeVal {
    pub fn expect_arity(args: &[RuntimeVal], expected: usize, fn_name: &str) -> Result<(), KiroError> {
        if args.len() == expected {
            Ok(())
        } else {
            let noun = if expected == 1 { "argument" } else { "arguments" };
            Err(KiroError::message(
                "ArgumentError",
                format!(
                    "{} expected {} {}, got {}.",
                    fn_name,
                    expected,
                    noun,
                    args.len()
                ),
            ))
        }
    }

    pub fn expect_arg<'a>(
        args: &'a [RuntimeVal],
        index: usize,
        fn_name: &str,
    ) -> Result<&'a RuntimeVal, KiroError> {
        args.get(index).ok_or_else(|| {
            KiroError::message(
                "ArgumentError",
                format!("{} missing argument {}.", fn_name, index + 1),
            )
        })
    }

    pub fn as_str(&self) -> Result<&str, KiroError> {
        match self {
            RuntimeVal::Str(s) => Ok(s.as_str()),
            _ => Err(KiroError::message("TypeError", "expected str")),
        }
    }

    pub fn as_num(&self) -> Result<f64, KiroError> {
        match self {
            RuntimeVal::Num(n) => Ok(*n),
            _ => Err(KiroError::message("TypeError", "expected num")),
        }
    }

    pub fn as_bool(&self) -> Result<bool, KiroError> {
        match self {
            RuntimeVal::Bool(b) => Ok(*b),
            _ => Err(KiroError::message("TypeError", "expected bool")),
        }
    }

    pub fn as_list(&self) -> Result<&[RuntimeVal], KiroError> {
        match self {
            RuntimeVal::List(items) => Ok(items.as_slice()),
            _ => Err(KiroError::message("TypeError", "expected list")),
        }
    }

    pub fn as_map(&self) -> Result<&HashMap<String, RuntimeVal>, KiroError> {
        match self {
            RuntimeVal::Map(map) => Ok(map),
            _ => Err(KiroError::message("TypeError", "expected map")),
        }
    }

    pub fn as_void(&self) -> Result<(), KiroError> {
        match self {
            RuntimeVal::Void => Ok(()),
            _ => Err(KiroError::message("TypeError", "expected void")),
        }
    }
}

impl TryFrom<RuntimeVal> for String {
    type Error = KiroError;
    fn try_from(val: RuntimeVal) -> Result<Self, Self::Error> {
        match val {
            RuntimeVal::Str(s) => Ok(s),
            _ => Err(KiroError::new("TypeError")),
        }
    }
}

impl TryFrom<RuntimeVal> for f64 {
    type Error = KiroError;
    fn try_from(val: RuntimeVal) -> Result<Self, Self::Error> {
        match val {
            RuntimeVal::Num(n) => Ok(n),
            _ => Err(KiroError::new("TypeError")),
        }
    }
}

impl TryFrom<RuntimeVal> for bool {
    type Error = KiroError;
    fn try_from(val: RuntimeVal) -> Result<Self, Self::Error> {
        match val {
            RuntimeVal::Bool(b) => Ok(b),
            _ => Err(KiroError::new("TypeError")),
        }
    }
}

impl TryFrom<RuntimeVal> for () {
    type Error = KiroError;
    fn try_from(val: RuntimeVal) -> Result<Self, Self::Error> {
        match val {
            RuntimeVal::Void => Ok(()),
            _ => Err(KiroError::new("TypeError")),
        }
    }
}

impl TryFrom<RuntimeVal> for Vec<String> {
    type Error = KiroError;
    fn try_from(val: RuntimeVal) -> Result<Self, Self::Error> {
        match val {
            RuntimeVal::List(items) => {
                let mut result = Vec::new();
                for item in items {
                    match item {
                        RuntimeVal::Str(s) => result.push(s),
                        _ => return Err(KiroError::new("TypeError")),
                    }
                }
                Ok(result)
            }
            _ => Err(KiroError::new("TypeError")),
        }
    }
}
