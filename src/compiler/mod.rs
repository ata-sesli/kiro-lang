use crate::grammar::grammar;
use std::collections::{HashMap, HashSet};

pub mod expression;
pub mod statement;
pub mod types;

#[derive(Clone, Debug)]
pub struct VarInfo {
    pub is_mutable: bool,
}

#[derive(Clone, Debug)]
pub struct FunctionInfo {
    pub is_pure: bool,
    pub can_error: bool,
    pub doc: Option<String>,
}

pub struct Compiler {
    pub known_vars: HashMap<String, VarInfo>,
    pub imported_modules: HashSet<String>,
    pub functions: HashMap<String, FunctionInfo>,
    pub in_pure_context: bool,
    pub in_failable_fn: bool,
    pub pure_scope_params: HashSet<String>, // Parameters allowed in pure function scope
    pub moved_vars: HashSet<String>,        // Track moved variables to prevent use-after-move
    pub fn_ref_vars: HashSet<String>,       // Vars holding pure function refs
    pub fn_returning_fn: HashSet<String>,   // Function names returning fn(...) -> ...
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            known_vars: HashMap::new(),
            imported_modules: HashSet::new(),
            functions: HashMap::new(),
            in_pure_context: false,
            in_failable_fn: false,
            pure_scope_params: HashSet::new(),
            moved_vars: HashSet::new(),
            fn_ref_vars: HashSet::new(),
            fn_returning_fn: HashSet::new(),
        }
    }

    pub fn pure_fn_ref_name_from_expr(&self, expr: &grammar::Expression) -> Option<String> {
        if let grammar::Expression::Ref(_, target) = expr
            && let grammar::Expression::Variable(v) = &**target
            && let Some(info) = self.functions.get(&v.value)
            && info.is_pure
        {
            return Some(v.value.clone());
        }
        None
    }

    pub fn expr_yields_fn_ref(&self, expr: &grammar::Expression) -> bool {
        if self.pure_fn_ref_name_from_expr(expr).is_some() {
            return true;
        }
        match expr {
            grammar::Expression::Variable(v) => self.fn_ref_vars.contains(&v.value),
            grammar::Expression::Call(func, _, _, _) => {
                if let grammar::Expression::Variable(v) = &**func {
                    self.fn_returning_fn.contains(&v.value)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn compile(&mut self, program: grammar::Program, is_main: bool) -> String {
        let mut output = String::new();
        output.push_str("#![allow(unused)]\n");
        output.push_str("use async_channel;\n");

        if is_main {
            // Import header module for rust fn glue
            output.push_str("mod header;\n");
            // ONLY DEFINED IN MAIN (Shared Runtime)
            // We make everything 'pub' so submodules can use them via 'use crate::*;'
            output.push_str(
                r#"
                #[derive(Clone, Debug)]
                pub struct KiroPipe<T> {
                    pub tx: async_channel::Sender<T>,
                    pub rx: async_channel::Receiver<T>,
                }

                // --- KIRO RESULT (Cloneable Error) ---
                pub type KiroResult<T> = Result<T, std::sync::Arc<anyhow::Error>>;

                // --- KIRO ADR VOID (Opaque Managed Address Handle) ---
                pub type KiroAdrErased = std::sync::Arc<dyn std::any::Any + Send + Sync>;
                #[derive(Clone, Debug, Default)]
                pub struct KiroAdrVoid(pub Option<KiroAdrErased>);
                impl KiroAdrVoid {
                    pub fn is_null(&self) -> bool { self.0.is_none() }
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

                // --- HELPER TRAIT FOR AUTO-DEREF ---
                pub trait KiroGet {
                    type Inner;
                    fn kiro_get<R>(&self, f: impl FnOnce(&Self::Inner) -> R) -> R;
                }
    
                // Pointer (e.g., Arc<Mutex<User>>)
                impl<T> KiroGet for std::sync::Arc<std::sync::Mutex<T>> {
                    type Inner = T;
                    fn kiro_get<R>(&self, f: impl FnOnce(&T) -> R) -> R {
                        let guard = self.lock().unwrap();
                        f(&*guard)
                    }
                }

                // Lazy Pointer / Adr (Option<Arc<Mutex<T>>>)
                impl<T> KiroGet for Option<std::sync::Arc<std::sync::Mutex<T>>> {
                    type Inner = T;
                    fn kiro_get<R>(&self, f: impl FnOnce(&T) -> R) -> R {
                        let arc = self.as_ref().expect("Runtime Error: Deferencing null/uninitialized pointer");
                        let guard = arc.lock().unwrap();
                        f(&*guard)
                    }
                }
    
                // --- KIRO AT TRAIT (Access Command) ---
                pub trait KiroAt<I, O> { fn kiro_at(&self, index: I) -> O; }
    
                // List Implementation
                impl<T: Clone> KiroAt<f64, T> for Vec<T> {
                    fn kiro_at(&self, index: f64) -> T {
                        self.get(index as usize).cloned().expect("Index out of bounds")
                    }
                }
    
                // Map Implementation
                impl<K, V> KiroAt<K, V> for std::collections::HashMap<K, V> 
                where K: std::hash::Hash + Eq + Clone, V: Clone {
                    fn kiro_at(&self, key: K) -> V {
                        self.get(&key).cloned().expect("Key not found")
                    }
                }
    
                // --- KIRO ADD ---
                pub trait KiroAdd<Rhs = Self> { type Output; fn kiro_add(self, rhs: Rhs) -> Self::Output; }
                impl KiroAdd for f64 { type Output = f64; fn kiro_add(self, rhs: f64) -> f64 { self + rhs } }
                impl KiroAdd for String { type Output = String; fn kiro_add(self, rhs: String) -> String { format!("{}{}", self, rhs) } }
                // String + f64
                impl KiroAdd<f64> for String { type Output = String; fn kiro_add(self, rhs: f64) -> String { format!("{}{:.1}", self, rhs) } }
                // f64 + String
                impl KiroAdd<String> for f64 { type Output = String; fn kiro_add(self, rhs: String) -> String { format!("{:.1}{}", self, rhs) } }
                // String + bool
                impl KiroAdd<bool> for String { type Output = String; fn kiro_add(self, rhs: bool) -> String { format!("{}{}", self, rhs) } }
                // bool + String
                impl KiroAdd<String> for bool { type Output = String; fn kiro_add(self, rhs: String) -> String { format!("{}{}", self, rhs) } }
                // String + KiroResult<String>
                impl KiroAdd<KiroResult<String>> for String {
                    type Output = String;
                    fn kiro_add(self, rhs: KiroResult<String>) -> String {
                        match rhs {
                            Ok(v) => format!("{}{}", self, v),
                            Err(e) => format!("{}Error({})", self, e),
                        }
                    }
                }
                // String + KiroResult<f64>
                impl KiroAdd<KiroResult<f64>> for String {
                    type Output = String;
                    fn kiro_add(self, rhs: KiroResult<f64>) -> String {
                        match rhs {
                            Ok(v) => format!("{}{:.1}", self, v),
                            Err(e) => format!("{}Error({})", self, e),
                        }
                    }
                }
                // String + KiroResult<bool>
                impl KiroAdd<KiroResult<bool>> for String {
                    type Output = String;
                    fn kiro_add(self, rhs: KiroResult<bool>) -> String {
                        match rhs {
                            Ok(v) => format!("{}{}", self, v),
                            Err(e) => format!("{}Error({})", self, e),
                        }
                    }
                }
    
                // --- KIRO LEN ---
                pub trait KiroLen { fn kiro_len(&self) -> f64; }
                impl<T> KiroLen for Vec<T> { fn kiro_len(&self) -> f64 { self.len() as f64 } }
                impl<K, V> KiroLen for std::collections::HashMap<K, V> { fn kiro_len(&self) -> f64 { self.len() as f64 } }
                impl KiroLen for String { fn kiro_len(&self) -> f64 { self.len() as f64 } }
    
                // --- KIRO ITER ---
                pub trait KiroIter { type Item; type IntoIter: Iterator<Item = Self::Item>; fn kiro_iter(self) -> Self::IntoIter; }
                impl KiroIter for std::ops::Range<i64> { type Item = i64; type IntoIter = std::ops::Range<i64>; fn kiro_iter(self) -> Self::IntoIter { self } }
                impl<T> KiroIter for Vec<T> { type Item = T; type IntoIter = std::vec::IntoIter<T>; fn kiro_iter(self) -> Self::IntoIter { self.into_iter() } }
                impl KiroIter for String { type Item = char; type IntoIter = std::vec::IntoIter<char>; fn kiro_iter(self) -> Self::IntoIter { self.chars().collect::<Vec<_>>().into_iter() } }
    
                // --- AS KIRO LOOP VAR ---
                pub trait AsKiroLoopVar { type Out; fn as_kiro(self) -> Self::Out; }
                impl AsKiroLoopVar for i64 { type Out = f64; fn as_kiro(self) -> f64 { self as f64 } }
                impl AsKiroLoopVar for f64 { type Out = f64; fn as_kiro(self) -> f64 { self } }
                impl AsKiroLoopVar for char { type Out = String; fn as_kiro(self) -> String { self.to_string() } }
                impl AsKiroLoopVar for String { type Out = String; fn as_kiro(self) -> String { self } }
                impl AsKiroLoopVar for bool { type Out = bool; fn as_kiro(self) -> bool { self } }

                // --- KIRO ASSIGN ---
                pub trait KiroAssign<Rhs> { fn kiro_assign(&mut self, rhs: Rhs); }
                // Default Assignment (Same Types)
                impl<T> KiroAssign<T> for T { fn kiro_assign(&mut self, rhs: T) { *self = rhs; } }
                // Special Assignment: adr void (opaque handle) = adr T (Option<Arc<Mutex<T>>>)
                impl<T: 'static + Send + Sync> KiroAssign<Option<std::sync::Arc<std::sync::Mutex<T>>>> for KiroAdrVoid {
                    fn kiro_assign(&mut self, rhs: Option<std::sync::Arc<std::sync::Mutex<T>>>) {
                        self.0 = rhs.map(|arc| arc as KiroAdrErased);
                    }
                }
                // --- KIRO EQ (Deep Equality) ---
                pub trait KiroEq { fn kiro_eq(&self, other: &Self) -> bool; }
                
                // Primitives
                impl KiroEq for f64 { fn kiro_eq(&self, other: &Self) -> bool { self == other } }
                impl KiroEq for bool { fn kiro_eq(&self, other: &Self) -> bool { self == other } }
                impl KiroEq for String { fn kiro_eq(&self, other: &Self) -> bool { self == other } }
                impl KiroEq for KiroAdrVoid {
                    fn kiro_eq(&self, other: &Self) -> bool {
                        match (&self.0, &other.0) {
                            (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
                            (None, None) => true,
                            _ => false,
                        }
                    }
                }

                // Collections
                impl<T: KiroEq> KiroEq for Vec<T> {
                    fn kiro_eq(&self, other: &Self) -> bool {
                        if self.len() != other.len() { return false; }
                        self.iter().zip(other.iter()).all(|(a, b)| a.kiro_eq(b))
                    }
                }
                impl<K: Eq + std::hash::Hash, V: KiroEq> KiroEq for std::collections::HashMap<K, V> {
                    fn kiro_eq(&self, other: &Self) -> bool {
                        if self.len() != other.len() { return false; }
                        self.iter().all(|(k, v)| other.get(k).map_or(false, |ov| v.kiro_eq(ov)))
                    }
                }

                // Pointers (Arc<Mutex<T>>)
                impl<T: KiroEq> KiroEq for std::sync::Arc<std::sync::Mutex<T>> {
                    fn kiro_eq(&self, other: &Self) -> bool {
                        if std::sync::Arc::ptr_eq(self, other) { return true; }
                        let g1 = self.lock().unwrap();
                        let g2 = other.lock().unwrap();
                        g1.kiro_eq(&*g2)
                    }
                }

                // Lazy Pointers (Option<Arc<Mutex<T>>>)
                impl<T: KiroEq> KiroEq for Option<std::sync::Arc<std::sync::Mutex<T>>> {
                    fn kiro_eq(&self, other: &Self) -> bool {
                        match (self, other) {
                            (Some(a), Some(b)) => a.kiro_eq(b),
                            (None, None) => true,
                            _ => false,
                        }
                    }
                }

                // Pipes are identity-based runtime channels; compare as non-equal by default.
                impl<T> KiroEq for KiroPipe<T> {
                    fn kiro_eq(&self, _other: &Self) -> bool { false }
                }

                // Result (Error Equality)
                // We compare Errors by their Debug string representation for now, as anyhow::Error doesn't impl Eq
                impl<T: KiroEq> KiroEq for KiroResult<T> {
                    fn kiro_eq(&self, other: &Self) -> bool {
                        match (self, other) {
                            (Ok(a), Ok(b)) => a.kiro_eq(b),
                            (Err(a), Err(b)) => format!("{:?}", a) == format!("{:?}", b),
                            _ => false,
                        }
                    }
                }

                // --- KIRO TRUTHY ---
                pub trait KiroTruthy { fn kiro_truthy(&self) -> bool; }
                impl KiroTruthy for bool { fn kiro_truthy(&self) -> bool { *self } }
                impl KiroTruthy for f64 { fn kiro_truthy(&self) -> bool { *self != 0.0 } }
                impl<T, E> KiroTruthy for Result<T, E> { fn kiro_truthy(&self) -> bool { self.is_ok() } }
                "#,
            );
        } else {
            // Submodules use the shared runtime
            output.push_str("use crate::*;\n");
        }

        let mut top_level = String::new();
        let mut body = String::new();

        // 0. Pre-Scan Functions for Metadata (Purity Check)
        // 0. Pre-Scan Functions for Metadata (Purity Check)
        for stmt in &program.statements {
            match stmt {
                grammar::Statement::Documented { doc, item } => {
                    if let grammar::AnnotatableItem::FunctionDef(def) = item {
                        let is_pure = def.pure_kw.is_some();
                        let can_error = def.can_error.is_some();
                        let doc_str = Some(
                            doc.iter()
                                .map(|d| d.content.trim_start_matches("///").trim().to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                        self.functions.insert(
                            def.name.clone(),
                            FunctionInfo {
                                is_pure,
                                can_error,
                                doc: doc_str,
                            },
                        );
                        if matches!(
                            def.return_type,
                            Some(grammar::KiroType::FnType(_, _, _, _, _, _))
                        ) {
                            self.fn_returning_fn.insert(def.name.clone());
                        }
                    }
                }
                grammar::Statement::FunctionDef(def) => {
                    let is_pure = def.pure_kw.is_some();
                    let can_error = def.can_error.is_some();
                    self.functions.insert(
                        def.name.clone(),
                        FunctionInfo {
                            is_pure,
                            can_error,
                            doc: None,
                        },
                    );
                    if matches!(
                        def.return_type,
                        Some(grammar::KiroType::FnType(_, _, _, _, _, _))
                    ) {
                        self.fn_returning_fn.insert(def.name.clone());
                    }
                }
                _ => {}
            }
        }

        for statement in program.statements {
            // Check if it should be hoisted
            let is_hoisted = match &statement {
                grammar::Statement::Import { .. } | grammar::Statement::StructDef(_) => true,
                grammar::Statement::Documented { item, .. } => {
                    matches!(item, grammar::AnnotatableItem::StructDef(_))
                }
                _ => false,
            };

            let line = self.compile_statement(statement);

            if is_hoisted {
                top_level.push_str(&format!("{}\n", line));
            } else {
                body.push_str(&format!("{}\n", line));
            }
        }

        output.push_str(&top_level);

        if is_main {
            output.push_str("#[tokio::main]\nasync fn main(){\n");
            output.push_str(&body);
            output.push_str("}\n");
        } else {
            // If not main, everything (including body) is usually just statements in the file.
            // But valid Rust modules can't have loose statements (print calls) at top level.
            // Kiro modules usually contain functions/structs.
            // If a user puts `print` in a module, it will generate `println!` at top level -> Rust Compile Error.
            // We accept this limitation for now: Modules = Structs + Fns + Imports.
            // But we should still output the body in case it's valid items (like Fns).
            output.push_str(&body);
        }
        output
    }
}
