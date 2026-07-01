use crate::grammar::grammar;
use std::collections::{HashMap, HashSet};

pub mod diagnostics;
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
    pub params: Vec<grammar::KiroType>,
    pub return_type: Option<grammar::KiroType>,
    pub doc: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CompilerOptions {
    pub uses_pipes: bool,
}

pub struct Compiler {
    pub known_vars: HashMap<String, VarInfo>,
    pub imported_modules: HashSet<String>,
    pub functions: HashMap<String, FunctionInfo>,
    pub module_functions: HashMap<(String, String), FunctionInfo>,
    pub in_pure_context: bool,
    pub in_failable_fn: bool,
    pub pure_scope_params: HashSet<String>, // Parameters allowed in pure function scope
    pub moved_vars: HashSet<String>,        // Track moved variables to prevent use-after-move
    pub fn_ref_vars: HashSet<String>,       // Vars holding pure function refs
    pub fn_returning_fn: HashSet<String>,   // Function names returning fn(...) -> ...
    pub options: CompilerOptions,
}

const EFFECTFUL_RECURSION_MESSAGE: &str = "Recursive calls are only supported in pure fn. Effectful recursive functions are not supported yet; use a loop or split pure recursion from effects.";

fn find_cycle_from(
    start: &str,
    current: &str,
    graph: &HashMap<String, HashSet<String>>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    path.push(current.to_string());
    if let Some(nexts) = graph.get(current) {
        for next in nexts {
            if next == start {
                let mut cycle = path.clone();
                cycle.push(start.to_string());
                path.pop();
                return Some(cycle);
            }
            if !path.contains(next)
                && let Some(cycle) = find_cycle_from(start, next, graph, path)
            {
                path.pop();
                return Some(cycle);
            }
        }
    }
    path.pop();
    None
}

fn collect_calls_from_block(
    block: &grammar::Block,
    local_functions: &HashSet<String>,
    calls: &mut HashSet<String>,
) {
    for stmt in &block.statements {
        collect_calls_from_statement(stmt, local_functions, calls);
    }
}

fn collect_calls_from_statement(
    stmt: &grammar::Statement,
    local_functions: &HashSet<String>,
    calls: &mut HashSet<String>,
) {
    match stmt {
        grammar::Statement::VarDecl { value, .. } => {
            collect_calls_from_expr(value, local_functions, calls)
        }
        grammar::Statement::AssignStmt { lhs, rhs, .. } => {
            collect_calls_from_expr(lhs, local_functions, calls);
            collect_calls_from_expr(rhs, local_functions, calls);
        }
        grammar::Statement::On {
            condition,
            body,
            else_clause,
            error_clauses,
            ..
        } => {
            collect_calls_from_expr(condition, local_functions, calls);
            collect_calls_from_block(body, local_functions, calls);
            if let Some(off) = else_clause {
                collect_calls_from_block(&off.body, local_functions, calls);
            }
            if let Some(errors) = error_clauses {
                collect_calls_from_error_clauses(errors, local_functions, calls);
            }
        }
        grammar::Statement::LoopOn {
            condition, body, ..
        } => {
            collect_calls_from_expr(condition, local_functions, calls);
            collect_calls_from_block(body, local_functions, calls);
        }
        grammar::Statement::LoopIter {
            iterable,
            step,
            filter,
            body,
            else_clause,
            ..
        } => {
            collect_calls_from_expr(iterable, local_functions, calls);
            if let Some(step) = step {
                collect_calls_from_expr(&step.value, local_functions, calls);
            }
            if let Some(filter) = filter {
                collect_calls_from_expr(&filter.condition, local_functions, calls);
            }
            collect_calls_from_block(body, local_functions, calls);
            if let Some(off) = else_clause {
                collect_calls_from_block(&off.body, local_functions, calls);
            }
        }
        grammar::Statement::Give(_, ch, val) => {
            collect_calls_from_expr(ch, local_functions, calls);
            collect_calls_from_expr(val, local_functions, calls);
        }
        grammar::Statement::Close(_, ch) | grammar::Statement::Print(_, ch) => {
            collect_calls_from_expr(ch, local_functions, calls)
        }
        grammar::Statement::Return(_, expr) => {
            if let Some(expr) = expr {
                collect_calls_from_expr(expr, local_functions, calls);
            }
        }
        grammar::Statement::Check(_, condition, _) => {
            collect_calls_from_expr(condition, local_functions, calls);
        }
        grammar::Statement::ExprStmt(expr) => collect_calls_from_expr(expr, local_functions, calls),
        grammar::Statement::Documented { item, .. } => {
            if let grammar::AnnotatableItem::FunctionDef(def) = item {
                collect_calls_from_block(&def.body, local_functions, calls);
            }
        }
        grammar::Statement::StructDef(_)
        | grammar::Statement::ErrorDef { .. }
        | grammar::Statement::FunctionDef(_)
        | grammar::Statement::RustFnDecl(_)
        | grammar::Statement::Break(_)
        | grammar::Statement::Continue(_)
        | grammar::Statement::Rest(_)
        | grammar::Statement::Import { .. } => {}
    }
}

fn collect_calls_from_error_clauses(
    clauses: &grammar::ErrorClauseList,
    local_functions: &HashSet<String>,
    calls: &mut HashSet<String>,
) {
    collect_calls_from_block(&clauses.first.body, local_functions, calls);
    if let Some(rest) = &clauses.rest {
        collect_calls_from_error_clauses(rest, local_functions, calls);
    }
}

fn collect_calls_from_expr(
    expr: &grammar::Expression,
    local_functions: &HashSet<String>,
    calls: &mut HashSet<String>,
) {
    match expr {
        grammar::Expression::FieldAccess(target, _, _)
        | grammar::Expression::At(target, _, _)
        | grammar::Expression::Push(target, _, _)
        | grammar::Expression::Ref(_, target)
        | grammar::Expression::Deref(_, target)
        | grammar::Expression::Take(_, target)
        | grammar::Expression::Len(_, target)
        | grammar::Expression::RunCall(_, target) => {
            collect_calls_from_expr(target, local_functions, calls);
        }
        grammar::Expression::StructInit(_, _, fields, _) => {
            for field in fields {
                collect_calls_from_expr(&field.value, local_functions, calls);
            }
        }
        grammar::Expression::ListInit(_, _, _, items, _) => {
            for item in items {
                collect_calls_from_expr(item, local_functions, calls);
            }
        }
        grammar::Expression::MapInit(_, _, _, _, pairs, _) => {
            for pair in pairs {
                collect_calls_from_expr(&pair.key, local_functions, calls);
                collect_calls_from_expr(&pair.value, local_functions, calls);
            }
        }
        grammar::Expression::Call(func, _, args, _) => {
            if let grammar::Expression::Variable(v) = &**func
                && local_functions.contains(&v.value)
            {
                calls.insert(v.value.clone());
            }
            collect_calls_from_expr(func, local_functions, calls);
            for arg in args {
                collect_calls_from_expr(arg, local_functions, calls);
            }
        }
        grammar::Expression::Add(lhs, _, rhs)
        | grammar::Expression::Sub(lhs, _, rhs)
        | grammar::Expression::Mul(lhs, _, rhs)
        | grammar::Expression::Div(lhs, _, rhs)
        | grammar::Expression::Eq(lhs, _, rhs)
        | grammar::Expression::Neq(lhs, _, rhs)
        | grammar::Expression::Gt(lhs, _, rhs)
        | grammar::Expression::Lt(lhs, _, rhs)
        | grammar::Expression::Geq(lhs, _, rhs)
        | grammar::Expression::Leq(lhs, _, rhs)
        | grammar::Expression::Range(lhs, _, rhs) => {
            collect_calls_from_expr(lhs, local_functions, calls);
            collect_calls_from_expr(rhs, local_functions, calls);
        }
        grammar::Expression::Number(_)
        | grammar::Expression::StringLit(_)
        | grammar::Expression::BoolLit(_)
        | grammar::Expression::Variable(_)
        | grammar::Expression::MoveExpr(_, _)
        | grammar::Expression::ErrorRef(_)
        | grammar::Expression::AdrInit(_, _)
        | grammar::Expression::PipeInit(_, _, _) => {}
    }
}

pub fn program_uses_pipes(program: &grammar::Program) -> bool {
    program.statements.iter().any(statement_uses_pipes)
}

fn block_uses_pipes(block: &grammar::Block) -> bool {
    block.statements.iter().any(statement_uses_pipes)
}

fn error_clauses_use_pipes(clauses: &grammar::ErrorClauseList) -> bool {
    block_uses_pipes(&clauses.first.body)
        || clauses
            .rest
            .as_deref()
            .map(error_clauses_use_pipes)
            .unwrap_or(false)
}

fn item_uses_pipes(item: &grammar::AnnotatableItem) -> bool {
    match item {
        grammar::AnnotatableItem::StructDef(def) => struct_uses_pipes(def),
        grammar::AnnotatableItem::FunctionDef(def) => function_uses_pipes(def),
        grammar::AnnotatableItem::RustFnDecl(def) => rust_fn_uses_pipes(def),
    }
}

fn statement_uses_pipes(stmt: &grammar::Statement) -> bool {
    match stmt {
        grammar::Statement::StructDef(def) => struct_uses_pipes(def),
        grammar::Statement::FunctionDef(def) => function_uses_pipes(def),
        grammar::Statement::RustFnDecl(def) => rust_fn_uses_pipes(def),
        grammar::Statement::VarDecl { value, .. } => expr_uses_pipes(value),
        grammar::Statement::AssignStmt { lhs, rhs, .. } => {
            expr_uses_pipes(lhs) || expr_uses_pipes(rhs)
        }
        grammar::Statement::On {
            condition,
            body,
            else_clause,
            error_clauses,
            ..
        } => {
            expr_uses_pipes(condition)
                || block_uses_pipes(body)
                || else_clause
                    .as_ref()
                    .map(|clause| block_uses_pipes(&clause.body))
                    .unwrap_or(false)
                || error_clauses
                    .as_ref()
                    .map(error_clauses_use_pipes)
                    .unwrap_or(false)
        }
        grammar::Statement::LoopOn {
            condition, body, ..
        } => expr_uses_pipes(condition) || block_uses_pipes(body),
        grammar::Statement::LoopIter {
            iterable,
            step,
            filter,
            body,
            else_clause,
            ..
        } => {
            expr_uses_pipes(iterable)
                || step
                    .as_ref()
                    .map(|step| expr_uses_pipes(&step.value))
                    .unwrap_or(false)
                || filter
                    .as_ref()
                    .map(|filter| expr_uses_pipes(&filter.condition))
                    .unwrap_or(false)
                || block_uses_pipes(body)
                || else_clause
                    .as_ref()
                    .map(|clause| block_uses_pipes(&clause.body))
                    .unwrap_or(false)
        }
        grammar::Statement::Give(_, _, _) | grammar::Statement::Close(_, _) => true,
        grammar::Statement::Return(_, expr) => expr.as_ref().map(expr_uses_pipes).unwrap_or(false),
        grammar::Statement::Check(_, condition, _) => expr_uses_pipes(condition),
        grammar::Statement::ExprStmt(expr) | grammar::Statement::Print(_, expr) => {
            expr_uses_pipes(expr)
        }
        grammar::Statement::Documented { item, .. } => item_uses_pipes(item),
        grammar::Statement::ErrorDef { .. }
        | grammar::Statement::Break(_)
        | grammar::Statement::Continue(_)
        | grammar::Statement::Rest(_)
        | grammar::Statement::Import { .. } => false,
    }
}

fn struct_uses_pipes(def: &grammar::StructDef) -> bool {
    def.fields
        .iter()
        .any(|field| type_uses_pipes(&field.field_type))
}

fn function_uses_pipes(def: &grammar::FunctionDef) -> bool {
    def.params
        .iter()
        .any(|param| type_uses_pipes(&param.command_type))
        || def
            .return_type
            .as_ref()
            .map(type_uses_pipes)
            .unwrap_or(false)
        || block_uses_pipes(&def.body)
}

fn rust_fn_uses_pipes(def: &grammar::RustFnDecl) -> bool {
    def.params
        .iter()
        .any(|param| type_uses_pipes(&param.command_type))
        || type_uses_pipes(&def.return_type)
}

fn expr_uses_pipes(expr: &grammar::Expression) -> bool {
    match expr {
        grammar::Expression::PipeInit(_, _, _) | grammar::Expression::Take(_, _) => true,
        grammar::Expression::AdrInit(_, inner) => type_uses_pipes(inner),
        grammar::Expression::ListInit(_, inner, _, items, _) => {
            type_uses_pipes(inner) || items.iter().any(expr_uses_pipes)
        }
        grammar::Expression::MapInit(_, key, value, _, pairs, _) => {
            type_uses_pipes(key)
                || type_uses_pipes(value)
                || pairs
                    .iter()
                    .any(|pair| expr_uses_pipes(&pair.key) || expr_uses_pipes(&pair.value))
        }
        grammar::Expression::StructInit(_, _, fields, _) => {
            fields.iter().any(|field| expr_uses_pipes(&field.value))
        }
        grammar::Expression::FieldAccess(target, _, _)
        | grammar::Expression::Ref(_, target)
        | grammar::Expression::Deref(_, target)
        | grammar::Expression::Len(_, target)
        | grammar::Expression::RunCall(_, target) => expr_uses_pipes(target),
        grammar::Expression::At(target, _, index) | grammar::Expression::Push(target, _, index) => {
            expr_uses_pipes(target) || expr_uses_pipes(index)
        }
        grammar::Expression::Call(func, _, args, _) => {
            expr_uses_pipes(func) || args.iter().any(expr_uses_pipes)
        }
        grammar::Expression::Add(lhs, _, rhs)
        | grammar::Expression::Sub(lhs, _, rhs)
        | grammar::Expression::Mul(lhs, _, rhs)
        | grammar::Expression::Div(lhs, _, rhs)
        | grammar::Expression::Eq(lhs, _, rhs)
        | grammar::Expression::Neq(lhs, _, rhs)
        | grammar::Expression::Gt(lhs, _, rhs)
        | grammar::Expression::Lt(lhs, _, rhs)
        | grammar::Expression::Geq(lhs, _, rhs)
        | grammar::Expression::Leq(lhs, _, rhs)
        | grammar::Expression::Range(lhs, _, rhs) => expr_uses_pipes(lhs) || expr_uses_pipes(rhs),
        grammar::Expression::Number(_)
        | grammar::Expression::StringLit(_)
        | grammar::Expression::BoolLit(_)
        | grammar::Expression::Variable(_)
        | grammar::Expression::MoveExpr(_, _)
        | grammar::Expression::ErrorRef(_) => false,
    }
}

fn type_uses_pipes(ty: &grammar::KiroType) -> bool {
    match ty {
        grammar::KiroType::Pipe(_, _) => true,
        grammar::KiroType::Adr(_, inner) | grammar::KiroType::List(_, inner) => {
            type_uses_pipes(inner)
        }
        grammar::KiroType::Map(_, key, value) => type_uses_pipes(key) || type_uses_pipes(value),
        grammar::KiroType::FnType(_, _, params, _, _, ret) => {
            params.iter().any(type_uses_pipes) || type_uses_pipes(ret)
        }
        grammar::KiroType::Num
        | grammar::KiroType::Str
        | grammar::KiroType::Bool
        | grammar::KiroType::Void
        | grammar::KiroType::Custom(_) => false,
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            known_vars: HashMap::new(),
            imported_modules: HashSet::new(),
            functions: HashMap::new(),
            module_functions: HashMap::new(),
            in_pure_context: false,
            in_failable_fn: false,
            pure_scope_params: HashSet::new(),
            moved_vars: HashSet::new(),
            fn_ref_vars: HashSet::new(),
            fn_returning_fn: HashSet::new(),
            options: CompilerOptions::default(),
        }
    }

    pub fn with_module_functions(
        module_functions: HashMap<(String, String), FunctionInfo>,
    ) -> Self {
        let mut compiler = Self::new();
        compiler.module_functions = module_functions;
        compiler
    }

    pub fn with_options(
        module_functions: HashMap<(String, String), FunctionInfo>,
        options: CompilerOptions,
    ) -> Self {
        let mut compiler = Self::with_module_functions(module_functions);
        compiler.options = options;
        compiler
    }

    pub fn collect_program_functions(program: &grammar::Program) -> HashMap<String, FunctionInfo> {
        let mut functions = HashMap::new();
        for stmt in &program.statements {
            match stmt {
                grammar::Statement::Documented { doc, item } => match item {
                    grammar::AnnotatableItem::FunctionDef(def) => {
                        let doc_str = Some(
                            doc.iter()
                                .map(|d| d.content.trim_start_matches("///").trim().to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                        functions.insert(
                            def.name.clone(),
                            FunctionInfo {
                                is_pure: def.pure_kw.is_some(),
                                can_error: def.can_error.is_some(),
                                params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                                return_type: def.return_type.clone(),
                                doc: doc_str,
                            },
                        );
                    }
                    grammar::AnnotatableItem::RustFnDecl(def) => {
                        let doc_str = Some(
                            doc.iter()
                                .map(|d| d.content.trim_start_matches("///").trim().to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                        functions.insert(
                            def.name.clone(),
                            FunctionInfo {
                                is_pure: false,
                                can_error: def.can_error.is_some(),
                                params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                                return_type: Some(def.return_type.clone()),
                                doc: doc_str,
                            },
                        );
                    }
                    _ => {}
                },
                grammar::Statement::FunctionDef(def) => {
                    functions.insert(
                        def.name.clone(),
                        FunctionInfo {
                            is_pure: def.pure_kw.is_some(),
                            can_error: def.can_error.is_some(),
                            params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                            return_type: def.return_type.clone(),
                            doc: None,
                        },
                    );
                }
                grammar::Statement::RustFnDecl(def) => {
                    functions.insert(
                        def.name.clone(),
                        FunctionInfo {
                            is_pure: false,
                            can_error: def.can_error.is_some(),
                            params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                            return_type: Some(def.return_type.clone()),
                            doc: None,
                        },
                    );
                }
                _ => {}
            }
        }
        functions
    }

    pub fn call_function_info(&self, func: &grammar::Expression) -> Option<FunctionInfo> {
        match func {
            grammar::Expression::Variable(v) => self.functions.get(&v.value).cloned(),
            grammar::Expression::FieldAccess(target, _, field) => {
                if let grammar::Expression::Variable(module) = &**target
                    && self.imported_modules.contains(&module.value)
                {
                    self.module_functions
                        .get(&(module.value.clone(), field.value.clone()))
                        .cloned()
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn call_name(&self, func: &grammar::Expression) -> String {
        match func {
            grammar::Expression::Variable(v) => v.value.clone(),
            grammar::Expression::FieldAccess(target, _, field) => {
                if let grammar::Expression::Variable(module) = &**target {
                    format!("{}.{}", module.value, field.value)
                } else {
                    "<computed function>".to_string()
                }
            }
            _ => "<computed function>".to_string(),
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

    pub fn validate_effectful_recursion(
        &self,
        program: &grammar::Program,
        module: &str,
    ) -> Result<(), crate::errors::KiroError> {
        let local_functions: HashSet<String> = self.functions.keys().cloned().collect();
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();

        for stmt in &program.statements {
            match stmt {
                grammar::Statement::FunctionDef(def) => {
                    let mut calls = HashSet::new();
                    collect_calls_from_block(&def.body, &local_functions, &mut calls);
                    graph.insert(def.name.clone(), calls);
                }
                grammar::Statement::Documented { item, .. } => {
                    if let grammar::AnnotatableItem::FunctionDef(def) = item {
                        let mut calls = HashSet::new();
                        collect_calls_from_block(&def.body, &local_functions, &mut calls);
                        graph.insert(def.name.clone(), calls);
                    }
                }
                _ => {}
            }
        }

        for name in graph.keys() {
            let mut path = Vec::new();
            if let Some(cycle) = find_cycle_from(name, name, &graph, &mut path)
                && cycle.iter().any(|n| {
                    self.functions
                        .get(n)
                        .map(|info| !info.is_pure)
                        .unwrap_or(false)
                })
            {
                return Err(crate::errors::KiroError::compile_error(
                    module,
                    crate::errors::ErrorCode::PureViolation,
                    EFFECTFUL_RECURSION_MESSAGE,
                    None,
                ));
            }
        }

        Ok(())
    }

    fn validate_effectful_recursion_or_panic(&self, program: &grammar::Program) {
        if let Err(err) = self.validate_effectful_recursion(program, "<module>") {
            panic!("{}", err.message);
        }
    }

    pub fn effectful_recursion_message() -> &'static str {
        EFFECTFUL_RECURSION_MESSAGE
    }

    pub fn compile(&mut self, program: grammar::Program, is_main: bool) -> String {
        let emits_pipes = self.options.uses_pipes || program_uses_pipes(&program);
        let mut output = String::new();
        output.push_str("#![allow(unused)]\n");
        if emits_pipes {
            output.push_str("use async_channel;\n");
        }

        if is_main {
            // Import header module for rust fn glue
            output.push_str("mod header;\n");
            // ONLY DEFINED IN MAIN (Shared Runtime)
            // We make everything 'pub' so submodules can use them via 'use crate::*;'
            if emits_pipes {
                output.push_str(
                    r#"
                #[derive(Clone, Debug)]
                pub struct KiroPipe<T> {
                    pub tx: async_channel::Sender<T>,
                    pub rx: async_channel::Receiver<T>,
                }
                "#,
                );
            }
            output.push_str(
                r#"
                // --- KIRO RESULT (Cloneable Error) ---
                pub type KiroResult<T> = Result<T, std::sync::Arc<anyhow::Error>>;

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

                pub fn kiro_adr_or_fail<T>(adr: &Option<std::sync::Arc<std::sync::Mutex<T>>>) -> std::sync::Arc<std::sync::Mutex<T>> {
                    match adr {
                        Some(value) => value.clone(),
                        None => kiro_runtime_error_help(
                            "KIRO3006",
                            "Cannot deref an empty address.",
                            "Assign it with `ref value` before using `deref`.",
                        ),
                    }
                }

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
                        let index = index as usize;
                        self.get(index).cloned().unwrap_or_else(|| {
                            kiro_runtime_error(
                                "KIRO3004",
                                &format!("List index out of bounds: index {}, length {}.", index, self.len()),
                            )
                        })
                    }
                }
    
                // Map Implementation
                impl<K, V> KiroAt<K, V> for std::collections::HashMap<K, V> 
                where K: std::hash::Hash + Eq + Clone + std::fmt::Debug, V: Clone {
                    fn kiro_at(&self, key: K) -> V {
                        self.get(&key).cloned().unwrap_or_else(|| {
                            kiro_runtime_error(
                                "KIRO3005",
                                &format!("Map key not found: {:?}.", key),
                            )
                        })
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
            if emits_pipes {
                output.push_str(
                    r#"
                // Pipes are identity-based runtime channels; compare as non-equal by default.
                impl<T> KiroEq for KiroPipe<T> {
                    fn kiro_eq(&self, _other: &Self) -> bool { false }
                }
                "#,
                );
            }
        } else {
            // Submodules use the shared runtime
            output.push_str("use crate::*;\n");
        }

        let mut top_level = String::new();
        let mut body = String::new();

        // 0. Pre-Scan Functions for Metadata (Purity Check)
        self.functions = Self::collect_program_functions(&program);
        for (name, info) in &self.functions {
            if matches!(
                info.return_type,
                Some(grammar::KiroType::FnType(_, _, _, _, _, _))
            ) {
                self.fn_returning_fn.insert(name.clone());
            }
        }
        self.validate_effectful_recursion_or_panic(&program);

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
