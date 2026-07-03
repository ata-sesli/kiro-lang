use std::collections::HashMap;
use std::fmt;
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};

use kiro_runtime::RuntimeVal as HostRuntimeVal;

#[derive(Clone, Debug)]
pub enum PipeSender {
    Unbounded(Sender<RuntimeVal>),
    Bounded(SyncSender<RuntimeVal>),
}

#[derive(Clone, Debug)]
pub enum RuntimeVal {
    Float(f64),
    String(String),
    Bool(bool),
    Range(i64, i64),
    Void,
    Pipe(PipeSender, Arc<Mutex<Receiver<RuntimeVal>>>),
    Struct(String, HashMap<String, RuntimeVal>),
    List(Vec<RuntimeVal>),
    Map(HashMap<String, RuntimeVal>),
    Handle(kiro_runtime::KiroHandle),
    // Data Exports, Function ASTs
    Module(
        HashMap<String, RuntimeVal>,
        HashMap<String, crate::grammar::grammar::Statement>,
    ),
    FunctionRef(String),
    // Error: (type_name, description)
    Error(String, String),
    // Pointer: Arc<Mutex<RuntimeVal>>
    Pointer(Arc<Mutex<RuntimeVal>>),
    // Opaque address handle (adr void)
    AdrHandle(Option<Arc<Mutex<RuntimeVal>>>),
    Moved,
}

// Manual implementation to handle Pipe which cannot be compared
impl PartialEq for RuntimeVal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RuntimeVal::Float(a), RuntimeVal::Float(b)) => a == b,
            (RuntimeVal::String(a), RuntimeVal::String(b)) => a == b,
            (RuntimeVal::Bool(a), RuntimeVal::Bool(b)) => a == b,
            (RuntimeVal::Range(s1, e1), RuntimeVal::Range(s2, e2)) => s1 == s2 && e1 == e2,
            (RuntimeVal::Void, RuntimeVal::Void) => true,
            // Pipes are never equal (identity check is hard without ID)
            (RuntimeVal::Pipe(_, _), RuntimeVal::Pipe(_, _)) => false,
            // Structs equality
            (RuntimeVal::Struct(n1, d1), RuntimeVal::Struct(n2, d2)) => n1 == n2 && d1 == d2,
            // Collections equality
            (RuntimeVal::List(l1), RuntimeVal::List(l2)) => l1 == l2,
            (RuntimeVal::Map(m1), RuntimeVal::Map(m2)) => m1 == m2,
            (RuntimeVal::Handle(h1), RuntimeVal::Handle(h2)) => h1 == h2,
            (RuntimeVal::Module(_m1, _f1), RuntimeVal::Module(_m2, _f2)) => false, // Modules identity is tough, assume false for now
            (RuntimeVal::FunctionRef(a), RuntimeVal::FunctionRef(b)) => a == b,
            (RuntimeVal::Error(n1, _), RuntimeVal::Error(n2, _)) => n1 == n2,
            (RuntimeVal::Pointer(p1), RuntimeVal::Pointer(p2)) => Arc::ptr_eq(p1, p2),
            (RuntimeVal::AdrHandle(a1), RuntimeVal::AdrHandle(a2)) => match (a1, a2) {
                (Some(p1), Some(p2)) => Arc::ptr_eq(p1, p2),
                (None, None) => true,
                _ => false,
            },
            (RuntimeVal::Moved, RuntimeVal::Moved) => true,
            _ => false,
        }
    }
}

impl PartialOrd for RuntimeVal {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (RuntimeVal::Float(a), RuntimeVal::Float(b)) => a.partial_cmp(b),
            (RuntimeVal::String(a), RuntimeVal::String(b)) => a.partial_cmp(b),
            // Other types: define an arbitrary order or return None
            _ => None,
        }
    }
}

impl RuntimeVal {
    pub fn as_float(&self) -> Result<f64, String> {
        match self {
            RuntimeVal::Float(f) => Ok(*f),
            _ => Err("Type Error: Expected a number".to_string()),
        }
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            RuntimeVal::Float(f) => *f != 0.0,
            RuntimeVal::Bool(b) => *b,
            RuntimeVal::String(s) => !s.is_empty(),
            RuntimeVal::Void => false,
            RuntimeVal::Moved => false,
            _ => true,
        }
    }

    pub fn to_host_runtime(&self) -> Result<HostRuntimeVal, String> {
        match self {
            RuntimeVal::Float(n) => Ok(HostRuntimeVal::Num(*n)),
            RuntimeVal::String(s) => Ok(HostRuntimeVal::Str(s.clone())),
            RuntimeVal::Bool(b) => Ok(HostRuntimeVal::Bool(*b)),
            RuntimeVal::List(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(item.to_host_runtime()?);
                }
                Ok(HostRuntimeVal::List(out))
            }
            RuntimeVal::Map(map) => {
                let mut out = HashMap::new();
                for (k, v) in map {
                    out.insert(k.clone(), v.to_host_runtime()?);
                }
                Ok(HostRuntimeVal::Map(out))
            }
            RuntimeVal::Handle(handle) => Ok(HostRuntimeVal::Handle(handle.clone())),
            RuntimeVal::Void => Ok(HostRuntimeVal::Void),
            other => Err(format!(
                "Type Error: Cannot pass '{}' to host function.",
                other
            )),
        }
    }

    pub fn from_host_runtime(value: HostRuntimeVal) -> Result<Self, String> {
        match value {
            HostRuntimeVal::Num(n) => Ok(RuntimeVal::Float(n)),
            HostRuntimeVal::Str(s) => Ok(RuntimeVal::String(s)),
            HostRuntimeVal::Bool(b) => Ok(RuntimeVal::Bool(b)),
            HostRuntimeVal::List(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(Self::from_host_runtime(item)?);
                }
                Ok(RuntimeVal::List(out))
            }
            HostRuntimeVal::Map(map) => {
                let mut out = HashMap::new();
                for (k, v) in map {
                    out.insert(k, Self::from_host_runtime(v)?);
                }
                Ok(RuntimeVal::Map(out))
            }
            HostRuntimeVal::Handle(handle) => Ok(RuntimeVal::Handle(handle)),
            HostRuntimeVal::Void => Ok(RuntimeVal::Void),
        }
    }
}

impl fmt::Display for RuntimeVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RuntimeVal::Float(n) => write!(f, "{}", n),
            RuntimeVal::String(s) => write!(f, "{}", s),
            RuntimeVal::Bool(b) => write!(f, "{}", b),
            RuntimeVal::Range(s, e) => write!(f, "{}..{}", s, e),
            RuntimeVal::Void => write!(f, "void"),
            RuntimeVal::Pipe(_, _) => write!(f, "<Pipe>"),
            RuntimeVal::Struct(name, _) => write!(f, "<Struct {}>", name),
            RuntimeVal::List(l) => write!(f, "<List len={}>", l.len()),
            RuntimeVal::Map(m) => write!(f, "<Map len={}>", m.len()),
            RuntimeVal::Handle(handle) => write!(f, "{}", handle),
            RuntimeVal::Module(_, _) => write!(f, "<Module>"),
            RuntimeVal::FunctionRef(name) => write!(f, "<FnRef {}>", name),
            RuntimeVal::Error(name, desc) => write!(f, "Error({}): {}", name, desc),
            RuntimeVal::Pointer(_) => write!(f, "<Pointer>"),
            RuntimeVal::AdrHandle(Some(_)) => write!(f, "<adr:void handle>"),
            RuntimeVal::AdrHandle(None) => write!(f, "<adr:void null>"),
            RuntimeVal::Moved => write!(f, "<Moved>"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Value {
    pub data: RuntimeVal,
    pub is_mutable: bool,
}
