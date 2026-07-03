//! Kiro Runtime - Shared types for Kiro-Rust FFI
//!
//! This crate defines the runtime value representation and error types
//! used at the boundary between Kiro code and Rust glue functions.

use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

pub const KIRO_RUNTIME_ABI_VERSION: u32 = 2;

pub type HostResult = Result<RuntimeVal, KiroError>;

pub type KiroResult<T> = Result<T, std::sync::Arc<anyhow::Error>>;

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
    Handle(KiroHandle),
    Void,
}

#[derive(Clone)]
pub struct KiroHandle {
    type_name: String,
    value: Arc<dyn std::any::Any + Send + Sync>,
}

impl KiroHandle {
    pub fn new<T>(type_name: impl Into<String>, value: T) -> Self
    where
        T: std::any::Any + Send + Sync + 'static,
    {
        Self {
            type_name: type_name.into(),
            value: Arc::new(value),
        }
    }

    pub fn from_arc(
        type_name: impl Into<String>,
        value: Arc<dyn std::any::Any + Send + Sync>,
    ) -> Self {
        Self {
            type_name: type_name.into(),
            value,
        }
    }

    pub fn type_name(&self) -> &str {
        &self.type_name
    }

    pub fn downcast_ref<T: std::any::Any>(&self) -> Option<&T> {
        self.value.downcast_ref::<T>()
    }
}

impl std::fmt::Debug for KiroHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = Arc::as_ptr(&self.value) as *const ();
        write!(f, "<handle {} {:p}>", self.type_name, raw)
    }
}

impl std::fmt::Display for KiroHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let raw = Arc::as_ptr(&self.value) as *const ();
        write!(f, "<handle {} {:p}>", self.type_name, raw)
    }
}

impl PartialEq for KiroHandle {
    fn eq(&self, other: &Self) -> bool {
        self.type_name == other.type_name && Arc::ptr_eq(&self.value, &other.value)
    }
}

pub fn kiro_runtime_error(code: &str, message: &str) -> ! {
    eprintln!("[{}:runtime] {}", code, message);
    std::process::exit(1);
}

pub fn kiro_runtime_error_help(code: &str, message: &str, help: &str) -> ! {
    eprintln!("[{}:runtime] {}", code, message);
    eprintln!("help: {}", help);
    std::process::exit(1);
}

pub fn kiro_check_failed(message: &str) -> ! {
    kiro_runtime_error("KIRO3001", &format!("Check failed: {}", message));
}

pub fn kiro_adr_or_fail<T>(
    adr: &Option<std::sync::Arc<std::sync::Mutex<T>>>,
) -> std::sync::Arc<std::sync::Mutex<T>> {
    match adr {
        Some(value) => value.clone(),
        None => kiro_runtime_error_help(
            "KIRO3006",
            "Cannot deref an empty address.",
            "Assign it with `ref value` before using `deref`.",
        ),
    }
}

pub type KiroAdrErased = std::sync::Arc<dyn std::any::Any + Send + Sync>;

#[derive(Clone, Debug, Default)]
pub struct KiroAdrVoid(pub Option<KiroAdrErased>);

impl KiroAdrVoid {
    pub fn is_null(&self) -> bool {
        self.0.is_none()
    }
}

impl std::fmt::Display for KiroAdrVoid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(p) => {
                let raw = std::sync::Arc::as_ptr(p) as *const ();
                write!(f, "<adr:void {:p}>", raw)
            }
            None => write!(f, "<adr:void null>"),
        }
    }
}

pub trait KiroGet {
    type Inner;
    fn kiro_get<R>(&self, f: impl FnOnce(&Self::Inner) -> R) -> R;
}

impl<T> KiroGet for std::sync::Arc<std::sync::Mutex<T>> {
    type Inner = T;

    fn kiro_get<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let guard = self.lock().unwrap();
        f(&*guard)
    }
}

impl<T> KiroGet for Option<std::sync::Arc<std::sync::Mutex<T>>> {
    type Inner = T;

    fn kiro_get<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        let arc = self
            .as_ref()
            .expect("Runtime Error: Deferencing null/uninitialized pointer");
        let guard = arc.lock().unwrap();
        f(&*guard)
    }
}

pub trait KiroAt<I, O> {
    fn kiro_at(&self, index: I) -> O;
}

impl<T: Clone> KiroAt<f64, T> for Vec<T> {
    fn kiro_at(&self, index: f64) -> T {
        let index = index as usize;
        self.get(index).cloned().unwrap_or_else(|| {
            kiro_runtime_error(
                "KIRO3004",
                &format!(
                    "List index out of bounds: index {}, length {}.",
                    index,
                    self.len()
                ),
            )
        })
    }
}

impl<K, V> KiroAt<K, V> for HashMap<K, V>
where
    K: std::hash::Hash + Eq + Clone + std::fmt::Debug,
    V: Clone,
{
    fn kiro_at(&self, key: K) -> V {
        self.get(&key).cloned().unwrap_or_else(|| {
            kiro_runtime_error("KIRO3005", &format!("Map key not found: {:?}.", key))
        })
    }
}

pub trait KiroAdd<Rhs = Self> {
    type Output;
    fn kiro_add(self, rhs: Rhs) -> Self::Output;
}

impl KiroAdd for f64 {
    type Output = f64;

    fn kiro_add(self, rhs: f64) -> f64 {
        self + rhs
    }
}

impl KiroAdd for String {
    type Output = String;

    fn kiro_add(self, rhs: String) -> String {
        format!("{}{}", self, rhs)
    }
}

impl KiroAdd<f64> for String {
    type Output = String;

    fn kiro_add(self, rhs: f64) -> String {
        format!("{}{:.1}", self, rhs)
    }
}

impl KiroAdd<String> for f64 {
    type Output = String;

    fn kiro_add(self, rhs: String) -> String {
        format!("{:.1}{}", self, rhs)
    }
}

impl KiroAdd<bool> for String {
    type Output = String;

    fn kiro_add(self, rhs: bool) -> String {
        format!("{}{}", self, rhs)
    }
}

impl KiroAdd<String> for bool {
    type Output = String;

    fn kiro_add(self, rhs: String) -> String {
        format!("{}{}", self, rhs)
    }
}

impl KiroAdd<KiroResult<String>> for String {
    type Output = String;

    fn kiro_add(self, rhs: KiroResult<String>) -> String {
        match rhs {
            Ok(v) => format!("{}{}", self, v),
            Err(e) => format!("{}Error({})", self, e),
        }
    }
}

impl KiroAdd<KiroResult<f64>> for String {
    type Output = String;

    fn kiro_add(self, rhs: KiroResult<f64>) -> String {
        match rhs {
            Ok(v) => format!("{}{:.1}", self, v),
            Err(e) => format!("{}Error({})", self, e),
        }
    }
}

impl KiroAdd<KiroResult<bool>> for String {
    type Output = String;

    fn kiro_add(self, rhs: KiroResult<bool>) -> String {
        match rhs {
            Ok(v) => format!("{}{}", self, v),
            Err(e) => format!("{}Error({})", self, e),
        }
    }
}

pub trait KiroLen {
    fn kiro_len(&self) -> f64;
}

impl<T> KiroLen for Vec<T> {
    fn kiro_len(&self) -> f64 {
        self.len() as f64
    }
}

impl<K, V> KiroLen for HashMap<K, V> {
    fn kiro_len(&self) -> f64 {
        self.len() as f64
    }
}

impl KiroLen for String {
    fn kiro_len(&self) -> f64 {
        self.len() as f64
    }
}

pub trait KiroIter {
    type Item;
    type IntoIter: Iterator<Item = Self::Item>;
    fn kiro_iter(self) -> Self::IntoIter;
}

impl KiroIter for std::ops::Range<i64> {
    type Item = i64;
    type IntoIter = std::ops::Range<i64>;

    fn kiro_iter(self) -> Self::IntoIter {
        self
    }
}

impl<T> KiroIter for Vec<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn kiro_iter(self) -> Self::IntoIter {
        self.into_iter()
    }
}

impl KiroIter for String {
    type Item = char;
    type IntoIter = std::vec::IntoIter<char>;

    fn kiro_iter(self) -> Self::IntoIter {
        self.chars().collect::<Vec<_>>().into_iter()
    }
}

pub trait AsKiroLoopVar {
    type Out;
    fn as_kiro(self) -> Self::Out;
}

impl AsKiroLoopVar for i64 {
    type Out = f64;

    fn as_kiro(self) -> f64 {
        self as f64
    }
}

impl AsKiroLoopVar for f64 {
    type Out = f64;

    fn as_kiro(self) -> f64 {
        self
    }
}

impl AsKiroLoopVar for char {
    type Out = String;

    fn as_kiro(self) -> String {
        self.to_string()
    }
}

impl AsKiroLoopVar for String {
    type Out = String;

    fn as_kiro(self) -> String {
        self
    }
}

impl AsKiroLoopVar for bool {
    type Out = bool;

    fn as_kiro(self) -> bool {
        self
    }
}

pub trait KiroAssign<Rhs> {
    fn kiro_assign(&mut self, rhs: Rhs);
}

impl<T> KiroAssign<T> for T {
    fn kiro_assign(&mut self, rhs: T) {
        *self = rhs;
    }
}

impl<T: 'static + Send + Sync> KiroAssign<Option<std::sync::Arc<std::sync::Mutex<T>>>>
    for KiroAdrVoid
{
    fn kiro_assign(&mut self, rhs: Option<std::sync::Arc<std::sync::Mutex<T>>>) {
        self.0 = rhs.map(|arc| arc as KiroAdrErased);
    }
}

pub trait KiroEq {
    fn kiro_eq(&self, other: &Self) -> bool;
}

impl KiroEq for f64 {
    fn kiro_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl KiroEq for bool {
    fn kiro_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl KiroEq for String {
    fn kiro_eq(&self, other: &Self) -> bool {
        self == other
    }
}

impl KiroEq for KiroAdrVoid {
    fn kiro_eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: KiroEq> KiroEq for Vec<T> {
    fn kiro_eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter()
            .zip(other.iter())
            .all(|(a, b)| a.kiro_eq(b))
    }
}

impl<K: Eq + std::hash::Hash, V: KiroEq> KiroEq for HashMap<K, V> {
    fn kiro_eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        self.iter()
            .all(|(k, v)| other.get(k).map_or(false, |ov| v.kiro_eq(ov)))
    }
}

impl<T: KiroEq> KiroEq for std::sync::Arc<std::sync::Mutex<T>> {
    fn kiro_eq(&self, other: &Self) -> bool {
        if std::sync::Arc::ptr_eq(self, other) {
            return true;
        }
        let g1 = self.lock().unwrap();
        let g2 = other.lock().unwrap();
        g1.kiro_eq(&*g2)
    }
}

impl<T: KiroEq> KiroEq for Option<std::sync::Arc<std::sync::Mutex<T>>> {
    fn kiro_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Some(a), Some(b)) => a.kiro_eq(b),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<T: KiroEq> KiroEq for KiroResult<T> {
    fn kiro_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Ok(a), Ok(b)) => a.kiro_eq(b),
            (Err(a), Err(b)) => format!("{:?}", a) == format!("{:?}", b),
            _ => false,
        }
    }
}

pub trait KiroTruthy {
    fn kiro_truthy(&self) -> bool;
}

impl KiroTruthy for bool {
    fn kiro_truthy(&self) -> bool {
        *self
    }
}

impl KiroTruthy for f64 {
    fn kiro_truthy(&self) -> bool {
        *self != 0.0
    }
}

impl<T, E> KiroTruthy for Result<T, E> {
    fn kiro_truthy(&self) -> bool {
        self.is_ok()
    }
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
    pub fn handle<T>(type_name: impl Into<String>, value: T) -> Self
    where
        T: std::any::Any + Send + Sync + 'static,
    {
        RuntimeVal::Handle(KiroHandle::new(type_name, value))
    }

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

    pub fn as_handle(&self, expected_type: &str) -> Result<&KiroHandle, KiroError> {
        match self {
            RuntimeVal::Handle(handle) if handle.type_name() == expected_type => Ok(handle),
            RuntimeVal::Handle(handle) => Err(KiroError::message(
                "TypeError",
                format!(
                    "expected handle {}, got handle {}",
                    expected_type,
                    handle.type_name()
                ),
            )),
            _ => Err(KiroError::message(
                "TypeError",
                format!("expected handle {}", expected_type),
            )),
        }
    }
}

impl From<KiroHandle> for RuntimeVal {
    fn from(v: KiroHandle) -> Self {
        RuntimeVal::Handle(v)
    }
}

impl TryFrom<RuntimeVal> for KiroHandle {
    type Error = KiroError;
    fn try_from(val: RuntimeVal) -> Result<Self, Self::Error> {
        match val {
            RuntimeVal::Handle(handle) => Ok(handle),
            _ => Err(KiroError::message("TypeError", "expected handle")),
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
