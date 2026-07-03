use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::errors::ErrorCode;
use crate::grammar::AstSpan;
use crate::grammar::grammar as ast;
use crate::interpreter::registry::{FunctionEntry, FunctionRegistry};
use crate::interpreter::values::{PipeSender, RuntimeVal, Value};
use crate::interpreter::{
    HostCallCtx, HostFnHandler, HostMode, HostRegistry, InterpreterLimits, LoadedModule,
    ModuleLoader, StatementResult,
};
use crate::ir::{IrBinaryOp, IrErrorClause, IrExpr, IrFunction, IrModule, IrStmt};

#[derive(Debug, Clone)]
pub struct RuntimeErrorSite {
    pub code: ErrorCode,
    pub span: AstSpan,
    pub label: String,
    pub help: Option<String>,
}

pub struct SessionRuntime {
    module: IrModule,
    globals: HashMap<String, Value>,
    frames: Vec<HashMap<String, Value>>,
    registry: FunctionRegistry,
    error_types: HashMap<String, String>,
    pure_scope_params: HashSet<String>,
    in_pure_mode: bool,
    in_failable_fn: bool,
    current_module: String,
    current_dir: PathBuf,
    host_mode: HostMode,
    host_registry: HostRegistry,
    limits: InterpreterLimits,
    step_count: u64,
    call_depth: usize,
    started_at: Option<Instant>,
    module_loader: Option<Arc<dyn ModuleLoader>>,
    module_cache: HashMap<String, RuntimeVal>,
    declarations_loaded: bool,
    last_error_site: Option<RuntimeErrorSite>,
}

impl SessionRuntime {
    pub fn new(module: IrModule, base_dir: PathBuf) -> Self {
        let current_module = module.name.clone();
        let mut runtime = Self {
            module,
            globals: HashMap::new(),
            frames: Vec::new(),
            registry: FunctionRegistry::new(),
            error_types: HashMap::new(),
            pure_scope_params: HashSet::new(),
            in_pure_mode: false,
            in_failable_fn: false,
            current_module,
            current_dir: base_dir,
            host_mode: HostMode::Simulate,
            host_registry: HostRegistry::default(),
            limits: InterpreterLimits::default(),
            step_count: 0,
            call_depth: 0,
            started_at: None,
            module_loader: None,
            module_cache: HashMap::new(),
            declarations_loaded: false,
            last_error_site: None,
        };
        runtime.register_module_declarations();
        runtime
    }

    pub fn registry(&self) -> &FunctionRegistry {
        &self.registry
    }

    pub fn global(&self, name: &str) -> Option<&Value> {
        self.globals.get(name)
    }

    pub fn set_current_module(&mut self, module: impl Into<String>) {
        self.current_module = module.into();
    }

    pub fn set_host_mode(&mut self, mode: HostMode) {
        self.host_mode = mode;
    }

    pub fn set_limits(&mut self, limits: InterpreterLimits) {
        self.limits = limits;
    }

    pub fn set_module_loader(&mut self, loader: Arc<dyn ModuleLoader>) {
        self.module_loader = Some(loader);
    }

    pub fn register_host_fn(
        &mut self,
        module: impl Into<String>,
        name: impl Into<String>,
        handler: HostFnHandler,
    ) {
        let module = module.into();
        let name = name.into();
        self.host_registry
            .register(module.clone(), name.clone(), handler.clone());
        let _ = self.registry.attach_host_handler(&module, &name, handler);
    }

    pub fn run(&mut self) -> Result<(), String> {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        for stmt in self.module.statements.clone() {
            let result = self.execute_statement(stmt)?;
            match result {
                StatementResult::Normal(_) => {}
                StatementResult::Return(_) => return Ok(()),
                StatementResult::Break | StatementResult::Continue => {
                    return Err("Cannot break/continue outside of loop".to_string());
                }
            }
        }
        Ok(())
    }

    pub fn take_last_error_site(&mut self) -> Option<RuntimeErrorSite> {
        self.last_error_site.take()
    }

    pub fn call_function(
        &mut self,
        module: &str,
        name: &str,
        args: Vec<RuntimeVal>,
    ) -> Result<RuntimeVal, String> {
        if module == self.current_module {
            self.ensure_declarations_loaded()?;
        }
        self.enter_call()?;
        let result = self.call_function_inner(module, name, args);
        self.exit_call();
        result
    }

    fn register_module_declarations(&mut self) {
        for function in self.module.functions.values().cloned() {
            self.registry
                .register_interpreted(&self.module.name, function);
        }
        for declaration in self.module.rust_functions.values().cloned() {
            let module = self.module.name.clone();
            let name = declaration.name.clone();
            self.registry.register_host_decl(&module, declaration);
            if let Some(handler) = self.host_registry.get(&module, &name) {
                let _ = self.registry.attach_host_handler(&module, &name, handler);
            }
        }
    }

    fn ensure_declarations_loaded(&mut self) -> Result<(), String> {
        if self.declarations_loaded {
            return Ok(());
        }
        self.declarations_loaded = true;
        for stmt in self.module.statements.clone() {
            match stmt {
                IrStmt::ErrorDef { .. }
                | IrStmt::StructDef { .. }
                | IrStmt::HandleDef { .. }
                | IrStmt::FunctionDef(_)
                | IrStmt::RustFnDecl(_)
                | IrStmt::Import { .. } => {
                    self.execute_statement(stmt)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn call_function_inner(
        &mut self,
        module: &str,
        name: &str,
        arg_values: Vec<RuntimeVal>,
    ) -> Result<RuntimeVal, String> {
        let entry = self
            .registry
            .get(module, name)
            .cloned()
            .ok_or_else(|| format!("Undefined function: '{}.{}'", module, name))?;

        match entry {
            FunctionEntry::InterpretedKiro { function } => {
                self.call_interpreted_function(module, &function, arg_values)
            }
            FunctionEntry::HostNative {
                declaration,
                handler,
            } => self.call_host_function(module, &declaration, handler, arg_values),
            FunctionEntry::CompiledKiro { .. } => Err(format!(
                "Compiled function '{}.{}' is not executable in interpreter v2.",
                module, name
            )),
        }
    }

    fn call_interpreted_function(
        &mut self,
        _module: &str,
        function: &IrFunction,
        arg_values: Vec<RuntimeVal>,
    ) -> Result<RuntimeVal, String> {
        let params = &function.signature.params;
        if params.len() != arg_values.len() {
            return Err(format!(
                "Function '{}' expects {} args, got {}.",
                function.name,
                params.len(),
                arg_values.len()
            ));
        }

        if function.signature.is_pure {
            for arg in &arg_values {
                if self.value_is_mutable_argument(arg) {
                    return Err(
                        "Pure Function Error: Pure functions only accept immutable values."
                            .to_string(),
                    );
                }
            }
        }

        let old_mode = self.in_pure_mode;
        let old_params = self.pure_scope_params.clone();
        let old_failable = self.in_failable_fn;

        let mut frame = HashMap::new();
        for (param, value) in params.iter().zip(arg_values.into_iter()) {
            frame.insert(
                param.name.clone(),
                Value {
                    data: value,
                    is_mutable: !function.signature.is_pure,
                },
            );
        }

        if function.signature.is_pure {
            self.in_pure_mode = true;
            self.pure_scope_params = params.iter().map(|p| p.name.clone()).collect();
        }
        self.in_failable_fn = function.signature.can_error;
        self.frames.push(frame);

        let result = self.execute_block(function.body.clone());

        self.frames.pop();
        self.in_pure_mode = old_mode;
        self.pure_scope_params = old_params;
        self.in_failable_fn = old_failable;

        let result = result?;
        let out = match result {
            StatementResult::Normal(v) | StatementResult::Return(v) => Ok(v),
            StatementResult::Break | StatementResult::Continue => {
                Err("Error: 'break' or 'continue' leaked from function body.".to_string())
            }
        }?;

        let expects_void = match &function.signature.return_type {
            None => true,
            Some(ast::KiroType::Void) => true,
            Some(_) => false,
        };
        if expects_void && !matches!(out, RuntimeVal::Void) {
            return Err(format!(
                "Type Error: Function '{}' has void return type but returned a value. Add an explicit return type (e.g. -> num).",
                function.name
            ));
        }
        if !expects_void && matches!(out, RuntimeVal::Void) {
            return Err(format!(
                "Type Error: Function '{}' expects a return value but returned void.",
                function.name
            ));
        }

        Ok(out)
    }

    fn call_host_function(
        &mut self,
        module: &str,
        declaration: &crate::ir::IrRustFunction,
        handler: Option<HostFnHandler>,
        arg_values: Vec<RuntimeVal>,
    ) -> Result<RuntimeVal, String> {
        let signature = &declaration.signature;
        if signature.params.len() != arg_values.len() {
            return Err(format!(
                "Function '{}.{}' expects {} args, got {}.",
                module,
                declaration.name,
                signature.params.len(),
                arg_values.len()
            ));
        }
        for (idx, (param, arg)) in signature.params.iter().zip(arg_values.iter()).enumerate() {
            if !matches_kiro_type(arg, &param.ty) {
                return Err(format!(
                    "Type Error: Argument {} for '{}.{}' does not match declared type.",
                    idx + 1,
                    module,
                    declaration.name
                ));
            }
        }

        match self.host_mode {
            HostMode::Deny => Err(format!(
                "Interpreter Error: Host call denied for '{}.{}'.",
                module, declaration.name
            )),
            HostMode::Simulate => Ok(mock_value(
                signature
                    .return_type
                    .as_ref()
                    .unwrap_or(&ast::KiroType::Void),
            )),
            HostMode::Execute => {
                let handler = handler
                    .or_else(|| self.host_registry.get(module, &declaration.name))
                    .ok_or_else(|| {
                        format!(
                            "Interpreter Error: Host function '{}.{}' is not registered.",
                            module, declaration.name
                        )
                    })?;

                let mut host_args = Vec::with_capacity(arg_values.len());
                for arg in &arg_values {
                    host_args.push(arg.to_host_runtime()?);
                }

                let host_ctx = HostCallCtx {
                    module_name: module.to_string(),
                    function_name: declaration.name.clone(),
                    step_count: self.step_count,
                };

                match handler(host_ctx, host_args) {
                    Ok(value) => {
                        let out = RuntimeVal::from_host_runtime(value)?;
                        if !matches_kiro_type(
                            &out,
                            signature
                                .return_type
                                .as_ref()
                                .unwrap_or(&ast::KiroType::Void),
                        ) {
                            return Err(format!(
                                "Type Error: Host function '{}.{}' returned a value that does not match declared type.",
                                module, declaration.name
                            ));
                        }
                        Ok(out)
                    }
                    Err(host_err) => {
                        if signature.can_error {
                            Ok(RuntimeVal::Error(
                                host_err.name.clone(),
                                host_err.to_string(),
                            ))
                        } else {
                            Err(format!(
                                "Host Error: '{}.{}' failed with '{}', but function is not declared failable.",
                                module, declaration.name, host_err
                            ))
                        }
                    }
                }
            }
        }
    }

    fn execute_statement(&mut self, stmt: IrStmt) -> Result<StatementResult, String> {
        self.tick()?;
        match stmt {
            IrStmt::ErrorDef {
                name, description, ..
            } => {
                self.error_types
                    .insert(name, description.unwrap_or_default());
                Ok(StatementResult::Normal(RuntimeVal::Void))
            }
            IrStmt::StructDef { .. } => Ok(StatementResult::Normal(RuntimeVal::Void)),
            IrStmt::HandleDef { .. } => Ok(StatementResult::Normal(RuntimeVal::Void)),
            IrStmt::VarDecl { name, value, .. } => {
                let value = self.eval_expr(value)?;
                self.define_var(
                    name.clone(),
                    Value {
                        data: value,
                        is_mutable: true,
                    },
                );
                if self.in_pure_mode {
                    self.pure_scope_params.insert(name);
                }
                Ok(StatementResult::Normal(RuntimeVal::Void))
            }
            IrStmt::Assign { lhs, rhs, .. } => {
                let value = self.eval_expr(rhs)?;
                self.assign(lhs, value)?;
                Ok(StatementResult::Normal(RuntimeVal::Void))
            }
            IrStmt::On {
                condition,
                body,
                else_body,
                error_clauses,
                ..
            } => self.execute_on(condition, body, else_body, error_clauses),
            IrStmt::LoopOn {
                condition, body, ..
            } => {
                loop {
                    let value = self.eval_expr(condition.clone())?;
                    if !value.is_truthy() {
                        break;
                    }
                    match self.execute_block(body.clone())? {
                        StatementResult::Normal(_) | StatementResult::Continue => {}
                        StatementResult::Break => break,
                        StatementResult::Return(value) => {
                            return Ok(StatementResult::Return(value));
                        }
                    }
                }
                Ok(StatementResult::Normal(RuntimeVal::Void))
            }
            IrStmt::LoopIter {
                iterator,
                iterable,
                step,
                filter,
                body,
                else_body,
                ..
            } => self.execute_loop_iter(iterator, iterable, step, filter, body, else_body),
            IrStmt::FunctionDef(_) | IrStmt::RustFnDecl(_) => {
                Ok(StatementResult::Normal(RuntimeVal::Void))
            }
            IrStmt::Give {
                channel,
                value,
                span,
            } => {
                if self.in_pure_mode {
                    return Err("Pure Function Error: 'give' is forbidden.".to_string());
                }
                let channel = self.eval_expr(channel)?;
                let value = self.eval_expr(value)?;
                match channel {
                    RuntimeVal::Pipe(sender, _) => {
                        match sender {
                            PipeSender::Unbounded(tx) => tx.send(value).map_err(|_| {
                                self.record_runtime_site(
                                    ErrorCode::PipeGiveClosed,
                                    span,
                                    "closed pipe",
                                    None,
                                );
                                "Pipe receiver is closed; cannot give a value.".to_string()
                            })?,
                            PipeSender::Bounded(tx) => tx.send(value).map_err(|_| {
                                self.record_runtime_site(
                                    ErrorCode::PipeGiveClosed,
                                    span,
                                    "closed pipe",
                                    None,
                                );
                                "Pipe receiver is closed; cannot give a value.".to_string()
                            })?,
                        }
                        Ok(StatementResult::Normal(RuntimeVal::Void))
                    }
                    _ => Err("Runtime Error: 'give' expects a pipe.".to_string()),
                }
            }
            IrStmt::Close { .. } => Ok(StatementResult::Normal(RuntimeVal::Void)),
            IrStmt::Return { value, .. } => {
                let value = match value {
                    Some(expr) => self.eval_expr(expr)?,
                    None => RuntimeVal::Void,
                };
                Ok(StatementResult::Return(value))
            }
            IrStmt::Break { .. } => Ok(StatementResult::Break),
            IrStmt::Continue { .. } => Ok(StatementResult::Continue),
            IrStmt::Rest { .. } => {
                if self.in_pure_mode {
                    return Err("Pure Function Error: 'rest' is forbidden.".to_string());
                }
                Ok(StatementResult::Normal(RuntimeVal::Void))
            }
            IrStmt::Check {
                condition,
                message,
                span,
            } => match self.eval_expr(condition)? {
                RuntimeVal::Bool(true) => Ok(StatementResult::Normal(RuntimeVal::Void)),
                RuntimeVal::Bool(false) => {
                    self.record_runtime_site(ErrorCode::CheckFailed, span, "failed check", None);
                    Err(format!(
                        "Check failed: {}",
                        message.unwrap_or_else(|| "check failed".to_string())
                    ))
                }
                other => Err(format!(
                    "Type Error: Check condition must be bool, got '{}'.",
                    other
                )),
            },
            IrStmt::Import { module_name, .. } => {
                self.import_module(&module_name)?;
                Ok(StatementResult::Normal(RuntimeVal::Void))
            }
            IrStmt::Expr(expr) => {
                let value = self.eval_expr(expr)?;
                Ok(StatementResult::Normal(value))
            }
        }
    }

    fn execute_block(&mut self, body: Vec<IrStmt>) -> Result<StatementResult, String> {
        let mut last = RuntimeVal::Void;
        for stmt in body {
            match self.execute_statement(stmt)? {
                StatementResult::Normal(value) => last = value,
                other => return Ok(other),
            }
        }
        Ok(StatementResult::Normal(last))
    }

    fn execute_on(
        &mut self,
        condition: IrExpr,
        body: Vec<IrStmt>,
        else_body: Option<Vec<IrStmt>>,
        error_clauses: Vec<IrErrorClause>,
    ) -> Result<StatementResult, String> {
        let value = self.eval_expr(condition)?;
        if let RuntimeVal::Error(err_name, err_desc) = &value {
            for clause in error_clauses {
                if clause
                    .error_type
                    .as_ref()
                    .is_none_or(|name| name == err_name)
                {
                    let result = self.execute_block(clause.body)?;
                    match result {
                        StatementResult::Normal(RuntimeVal::Void) => {
                            if self.in_failable_fn {
                                return Ok(StatementResult::Return(RuntimeVal::Error(
                                    err_name.clone(),
                                    err_desc.clone(),
                                )));
                            }
                            return Ok(StatementResult::Normal(RuntimeVal::Void));
                        }
                        other => return Ok(other),
                    }
                }
            }
            if self.in_failable_fn {
                return Ok(StatementResult::Return(value));
            }
            return Err(format!("Unhandled error: {}", err_name));
        }

        if value.is_truthy() {
            self.execute_block(body)
        } else if let Some(else_body) = else_body {
            self.execute_block(else_body)
        } else {
            Ok(StatementResult::Normal(RuntimeVal::Void))
        }
    }

    fn execute_loop_iter(
        &mut self,
        iterator: String,
        iterable: IrExpr,
        step: Option<IrExpr>,
        filter: Option<IrExpr>,
        body: Vec<IrStmt>,
        else_body: Option<Vec<IrStmt>>,
    ) -> Result<StatementResult, String> {
        let iterable = self.eval_expr(iterable)?;
        let items = match iterable {
            RuntimeVal::Range(start, end) => {
                let step = if let Some(step) = step {
                    self.eval_expr(step)?.as_float()? as i64
                } else {
                    1
                };
                if step == 0 {
                    return Err("Loop Error: step cannot be zero".to_string());
                }
                let mut out = Vec::new();
                let mut current = start;
                while current < end {
                    out.push(RuntimeVal::Float(current as f64));
                    current += step;
                }
                out
            }
            RuntimeVal::List(items) => items,
            RuntimeVal::String(text) => text
                .chars()
                .map(|ch| RuntimeVal::String(ch.to_string()))
                .collect(),
            _ => return Err("Loop Error: Can only loop over ranges, lists, or strings".to_string()),
        };

        for item in items {
            self.frames.push(HashMap::new());
            self.define_var(
                iterator.clone(),
                Value {
                    data: item,
                    is_mutable: false,
                },
            );

            let run_body = if let Some(filter) = filter.clone() {
                self.eval_expr(filter)?.is_truthy()
            } else {
                true
            };

            let result = if run_body {
                self.execute_block(body.clone())?
            } else if let Some(else_body) = else_body.clone() {
                self.execute_block(else_body)?
            } else {
                StatementResult::Normal(RuntimeVal::Void)
            };

            self.frames.pop();

            match result {
                StatementResult::Normal(_) | StatementResult::Continue => {}
                StatementResult::Break => break,
                StatementResult::Return(value) => return Ok(StatementResult::Return(value)),
            }
        }

        Ok(StatementResult::Normal(RuntimeVal::Void))
    }

    fn eval_expr(&mut self, expr: IrExpr) -> Result<RuntimeVal, String> {
        self.tick()?;
        match expr {
            IrExpr::StructInit { name, fields, .. } => {
                let mut data = HashMap::new();
                for field in fields {
                    data.insert(field.name, self.eval_expr(field.value)?);
                }
                Ok(RuntimeVal::Struct(name, data))
            }
            IrExpr::ListInit { items, .. } => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(self.eval_expr(item)?);
                }
                Ok(RuntimeVal::List(out))
            }
            IrExpr::MapInit { pairs, .. } => {
                let mut out = HashMap::new();
                for pair in pairs {
                    out.insert(
                        self.eval_expr(pair.key)?.to_string(),
                        self.eval_expr(pair.value)?,
                    );
                }
                Ok(RuntimeVal::Map(out))
            }
            IrExpr::FieldAccess { target, field, .. } => {
                let value = self.eval_expr(*target)?;
                self.field_access(value, &field)
            }
            IrExpr::At {
                collection,
                key,
                span,
            } => {
                let collection = self.eval_expr(*collection)?;
                let key = self.eval_expr(*key)?;
                match collection {
                    RuntimeVal::List(items) => {
                        let idx = key.as_float()? as usize;
                        match items.get(idx).cloned() {
                            Some(value) => Ok(value),
                            None => {
                                self.record_runtime_site(
                                    ErrorCode::ListIndexOutOfBounds,
                                    span,
                                    "index out of bounds",
                                    None,
                                );
                                Err(format!(
                                    "List index out of bounds: index {}, length {}.",
                                    idx,
                                    items.len()
                                ))
                            }
                        }
                    }
                    RuntimeVal::Map(map) => {
                        let lookup_key = key.to_string();
                        let display_key = match &key {
                            RuntimeVal::String(value) => format!("\"{}\"", value),
                            _ => lookup_key.clone(),
                        };
                        match map.get(&lookup_key).cloned() {
                            Some(value) => Ok(value),
                            None => {
                                self.record_runtime_site(
                                    ErrorCode::MapKeyNotFound,
                                    span,
                                    "missing key",
                                    None,
                                );
                                Err(format!("Map key not found: {}.", display_key))
                            }
                        }
                    }
                    _ => Err("Cannot use 'at' on this type".to_string()),
                }
            }
            IrExpr::Push {
                collection, value, ..
            } => {
                let _ = self.eval_expr(*collection)?;
                let _ = self.eval_expr(*value)?;
                Ok(RuntimeVal::Void)
            }
            IrExpr::Bool(value, _) => Ok(RuntimeVal::Bool(value)),
            IrExpr::Number(value, _) => Ok(RuntimeVal::Float(value)),
            IrExpr::String(value, _) => Ok(RuntimeVal::String(value)),
            IrExpr::Variable(name, _) => self.lookup_variable(&name),
            IrExpr::Move { name, .. } => self.move_variable(&name),
            IrExpr::ErrorRef(name, _) => self
                .error_types
                .get(&name)
                .cloned()
                .map(|desc| RuntimeVal::Error(name.clone(), desc))
                .ok_or_else(|| format!("Interpreter Error: Unknown error type '{}'.", name)),
            IrExpr::AdrInit { .. } => Ok(RuntimeVal::AdrHandle(None)),
            IrExpr::PipeInit { capacity, .. } => {
                if let Some(capacity) = capacity {
                    let (tx, rx) = mpsc::sync_channel(capacity);
                    Ok(RuntimeVal::Pipe(
                        PipeSender::Bounded(tx),
                        Arc::new(Mutex::new(rx)),
                    ))
                } else {
                    let (tx, rx) = mpsc::channel();
                    Ok(RuntimeVal::Pipe(
                        PipeSender::Unbounded(tx),
                        Arc::new(Mutex::new(rx)),
                    ))
                }
            }
            IrExpr::Take { target, span } => {
                if self.in_pure_mode {
                    return Err("Pure Function Error: 'take' is forbidden.".to_string());
                }
                match self.eval_expr(*target)? {
                    RuntimeVal::Pipe(_, rx) => {
                        let rx = rx.lock().unwrap();
                        rx.recv().map_err(|_| {
                            self.record_runtime_site(
                                ErrorCode::PipeTakeClosed,
                                span,
                                "closed pipe",
                                None,
                            );
                            "Pipe is closed; cannot take a value.".to_string()
                        })
                    }
                    _ => Err("Runtime Error: 'take' expects a pipe.".to_string()),
                }
            }
            IrExpr::Len { target, .. } => match self.eval_expr(*target)? {
                RuntimeVal::String(text) => Ok(RuntimeVal::Float(text.len() as f64)),
                RuntimeVal::List(items) => Ok(RuntimeVal::Float(items.len() as f64)),
                RuntimeVal::Map(items) => Ok(RuntimeVal::Float(items.len() as f64)),
                _ => Err("Runtime Error: 'len' only supports string, list, map.".to_string()),
            },
            IrExpr::Ref { target, .. } => {
                if let IrExpr::Variable(name, _) = &*target
                    && let Some(FunctionEntry::InterpretedKiro { function }) =
                        self.registry.get(&self.current_module, name)
                    && function.signature.is_pure
                {
                    return Ok(RuntimeVal::FunctionRef(name.clone()));
                }
                let value = self.eval_expr(*target)?;
                Ok(RuntimeVal::Pointer(Arc::new(Mutex::new(value))))
            }
            IrExpr::Deref { target, span } => match self.eval_expr(*target)? {
                RuntimeVal::Pointer(ptr) | RuntimeVal::AdrHandle(Some(ptr)) => {
                    Ok(ptr.lock().unwrap().clone())
                }
                RuntimeVal::AdrHandle(None) => {
                    self.record_runtime_site(
                        ErrorCode::EmptyAddressDeref,
                        span,
                        "empty address",
                        Some("Assign it with `ref value` before using `deref`."),
                    );
                    Err("Cannot deref an empty address.".to_string())
                }
                _ => Err("Runtime Error: 'deref' expects a pointer.".to_string()),
            },
            IrExpr::Call { target, args, .. } => self.eval_call(*target, args),
            IrExpr::RunCall { target, .. } => self.eval_expr(*target),
            IrExpr::Binary { op, lhs, rhs, .. } => self.eval_binary(op, *lhs, *rhs),
        }
    }

    fn record_runtime_site(
        &mut self,
        code: ErrorCode,
        span: Option<AstSpan>,
        label: &str,
        help: Option<&str>,
    ) {
        if let Some(span) = span {
            self.last_error_site = Some(RuntimeErrorSite {
                code,
                span,
                label: label.to_string(),
                help: help.map(str::to_string),
            });
        }
    }

    fn eval_call(&mut self, target: IrExpr, args: Vec<IrExpr>) -> Result<RuntimeVal, String> {
        if let Some((module, function)) = std_io_display_call(&target)
            && self.lookup_raw(&module).is_some()
        {
            if self.in_pure_mode {
                return Err(format!(
                    "Pure function cannot call impure/async function '{}.{}' inside a pure function.",
                    module, function
                ));
            }
            if args.len() != 1 {
                return Err(format!(
                    "Function '{}.{}' expects 1 args, got {}.",
                    module,
                    function,
                    args.len()
                ));
            }
            let value = self.eval_expr(args[0].clone())?;
            match function.as_str() {
                "print" => println!("{}", value),
                "write" => print!("{}", value),
                "eprint" => eprint!("{}", value),
                "eprintline" => eprintln!("{}", value),
                _ => unreachable!("std io helper checked before execution"),
            }
            return Ok(RuntimeVal::Void);
        }

        let (module, name) = match target {
            IrExpr::Variable(name, _) => {
                if let Some(value) = self.lookup_raw(&name)
                    && let RuntimeVal::FunctionRef(target) = &value.data
                {
                    (self.current_module.clone(), target.clone())
                } else {
                    (self.current_module.clone(), name)
                }
            }
            IrExpr::FieldAccess { target, field, .. } => match *target {
                IrExpr::Variable(module, _) => {
                    let value = self.lookup_variable(&module)?;
                    if matches!(value, RuntimeVal::Module(_, _)) {
                        (module, field)
                    } else {
                        return Err("Target of field access is not a module.".to_string());
                    }
                }
                _ => return Err("Expected module access for function call".to_string()),
            },
            _ => return Err("Expected function name or module access".to_string()),
        };

        let mut values = Vec::with_capacity(args.len());
        for arg in args {
            values.push(self.eval_expr(arg)?);
        }
        self.call_function(&module, &name, values)
    }

    fn eval_binary(
        &mut self,
        op: IrBinaryOp,
        lhs: IrExpr,
        rhs: IrExpr,
    ) -> Result<RuntimeVal, String> {
        if op == IrBinaryOp::Range {
            let start = self.eval_expr(lhs)?.as_float()? as i64;
            let end = self.eval_expr(rhs)?.as_float()? as i64;
            return Ok(RuntimeVal::Range(start, end));
        }

        let left = self.eval_expr(lhs)?;
        let right = self.eval_expr(rhs)?;
        match op {
            IrBinaryOp::Add => match (left, right) {
                (RuntimeVal::Float(a), RuntimeVal::Float(b)) => Ok(RuntimeVal::Float(a + b)),
                (RuntimeVal::String(a), b) => Ok(RuntimeVal::String(format!("{}{}", a, b))),
                (a, RuntimeVal::String(b)) => Ok(RuntimeVal::String(format!("{}{}", a, b))),
                _ => Err("Runtime Error: Can only ADD numbers or strings".to_string()),
            },
            IrBinaryOp::Sub => numbers(left, right, |a, b| a - b, "SUBTRACT"),
            IrBinaryOp::Mul => numbers(left, right, |a, b| a * b, "MULTIPLY"),
            IrBinaryOp::Div => numbers(left, right, |a, b| a / b, "DIVIDE"),
            IrBinaryOp::Eq => Ok(RuntimeVal::Bool(left == right)),
            IrBinaryOp::Neq => Ok(RuntimeVal::Bool(left != right)),
            IrBinaryOp::Gt => Ok(RuntimeVal::Bool(left > right)),
            IrBinaryOp::Lt => Ok(RuntimeVal::Bool(left < right)),
            IrBinaryOp::Geq => Ok(RuntimeVal::Bool(left >= right)),
            IrBinaryOp::Leq => Ok(RuntimeVal::Bool(left <= right)),
            IrBinaryOp::Range => unreachable!("range returned before binary evaluation"),
        }
    }

    fn assign(&mut self, lhs: IrExpr, value: RuntimeVal) -> Result<(), String> {
        match lhs {
            IrExpr::Variable(name, _) => self.assign_var(&name, value),
            IrExpr::FieldAccess { target, field, .. } => {
                let mut path = vec![field];
                let mut current = *target;
                while let IrExpr::FieldAccess { target, field, .. } = current {
                    path.push(field);
                    current = *target;
                }
                let root = match current {
                    IrExpr::Variable(name, _) => name,
                    _ => return Err("Assignment target must start with a variable.".to_string()),
                };
                self.assign_nested_field(&root, path, value)
            }
            IrExpr::Deref { target, .. } => match self.eval_expr(*target)? {
                RuntimeVal::Pointer(ptr) | RuntimeVal::AdrHandle(Some(ptr)) => {
                    *ptr.lock().unwrap() = value;
                    Ok(())
                }
                _ => Err("Runtime Error: Assignment target is not a pointer.".to_string()),
            },
            _ => Err("Invalid left-hand side for assignment.".to_string()),
        }
    }

    fn field_access(&self, value: RuntimeVal, field: &str) -> Result<RuntimeVal, String> {
        match value {
            RuntimeVal::Struct(_, fields) => fields
                .get(field)
                .cloned()
                .ok_or_else(|| format!("Field '{}' not found", field)),
            RuntimeVal::Module(exports, _) => exports
                .get(field)
                .cloned()
                .ok_or_else(|| format!("Export '{}' not found in module", field)),
            RuntimeVal::Pointer(ptr) | RuntimeVal::AdrHandle(Some(ptr)) => {
                match &*ptr.lock().unwrap() {
                    RuntimeVal::Struct(_, fields) => fields
                        .get(field)
                        .cloned()
                        .ok_or_else(|| format!("Field '{}' not found in pointer", field)),
                    _ => Err(format!(
                        "Cannot access field '{}' on pointer to non-struct",
                        field
                    )),
                }
            }
            _ => Err(format!(
                "Cannot access field '{}' on this type {:?}",
                field, value
            )),
        }
    }

    fn import_module(&mut self, module_name: &str) -> Result<(), String> {
        let loaded = self.load_module(module_name)?;
        if let Some(module_value) = self.module_cache.get(&loaded.cache_key).cloned() {
            self.globals.insert(
                module_name.to_string(),
                Value {
                    data: module_value,
                    is_mutable: false,
                },
            );
            return Ok(());
        }

        let program = crate::grammar::parse(&loaded.source)
            .map_err(|e| format!("Parse Error in module '{}': {:?}", module_name, e))?;
        let module = IrModule::lower(module_name, program);
        let mut child = SessionRuntime::new(module, loaded.base_dir);
        child.set_current_module(module_name.to_string());
        child.host_mode = self.host_mode;
        child.host_registry = self.host_registry.clone();
        child.limits = self.limits.clone();
        child.step_count = self.step_count;
        child.call_depth = self.call_depth;
        child.started_at = self.started_at;
        child.module_loader = self.module_loader.clone();
        child.module_cache = self.module_cache.clone();
        child.register_module_declarations();

        child.run()?;

        self.registry.extend_from(child.registry());
        self.module_cache = child.module_cache;
        self.step_count = child.step_count;
        self.call_depth = child.call_depth;
        self.started_at = child.started_at;

        let exports = child
            .globals
            .iter()
            .map(|(name, value)| (name.clone(), value.data.clone()))
            .collect();
        let module_value = RuntimeVal::Module(exports, HashMap::new());
        self.module_cache
            .insert(loaded.cache_key.clone(), module_value.clone());
        self.globals.insert(
            module_name.to_string(),
            Value {
                data: module_value,
                is_mutable: false,
            },
        );
        Ok(())
    }

    fn load_module(&self, module_name: &str) -> Result<LoadedModule, String> {
        if let Some(loader) = &self.module_loader {
            return loader.load(module_name, &self.current_dir);
        }

        let cache_key = if let Some(canonical) = crate::canonical_std_module_name(module_name) {
            format!("std://{}", canonical)
        } else {
            let full_path = self.current_dir.join(format!("{}.kiro", module_name));
            std::fs::canonicalize(&full_path)
                .unwrap_or(full_path)
                .to_string_lossy()
                .to_string()
        };

        let (source, base_dir) =
            if let Some(canonical) = crate::canonical_std_module_name(module_name) {
                let asset_path = crate::std_asset_path(module_name, &format!("{}.kiro", canonical))
                    .expect("known std module should have an asset path");
                let content = crate::StdAssets::get(&asset_path)
                    .map(|f| std::str::from_utf8(f.data.as_ref()).unwrap().to_string())
                    .ok_or_else(|| {
                        format!(
                            "Standard library module '{}' not found in embedded assets",
                            module_name
                        )
                    })?;
                (content, self.current_dir.clone())
            } else if module_name.starts_with("std_") {
                return Err(format!(
                    "Standard library module '{}' not found in embedded assets",
                    module_name
                ));
            } else {
                let full_path = self.current_dir.join(format!("{}.kiro", module_name));
                let resolved = std::fs::canonicalize(&full_path).unwrap_or(full_path.clone());
                let content = std::fs::read_to_string(&resolved)
                    .map_err(|_| format!("Module '{}' not found", resolved.display()))?;
                let parent = resolved
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."));
                (content, parent)
            };

        Ok(LoadedModule {
            cache_key,
            source,
            base_dir,
        })
    }

    fn lookup_variable(&self, name: &str) -> Result<RuntimeVal, String> {
        if let Some(desc) = self.error_types.get(name) {
            return Ok(RuntimeVal::Error(name.to_string(), desc.clone()));
        }
        if self.in_pure_mode && !self.pure_scope_params.contains(name) {
            return Err(format!(
                "Compiler Error: Pure function cannot capture external variable '{}'. Only parameters and local variables are allowed.",
                name
            ));
        }
        let value = self
            .lookup_raw(name)
            .map(|entry| entry.data.clone())
            .ok_or_else(|| format!("ERROR: Variable '{}' not found.", name))?;
        if matches!(value, RuntimeVal::Moved) {
            return Err(format!(
                "Interpreter Error: Variable '{}' was moved and cannot be used.",
                name
            ));
        }
        Ok(value)
    }

    fn lookup_raw(&self, name: &str) -> Option<&Value> {
        for frame in self.frames.iter().rev() {
            if let Some(value) = frame.get(name) {
                return Some(value);
            }
        }
        self.globals.get(name)
    }

    fn define_var(&mut self, name: String, value: Value) {
        if let Some(frame) = self.frames.last_mut() {
            frame.insert(name, value);
        } else {
            self.globals.insert(name, value);
        }
    }

    fn assign_var(&mut self, name: &str, value: RuntimeVal) -> Result<(), String> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(entry) = frame.get_mut(name) {
                return update_entry(name, entry, value);
            }
        }

        if self.frames.is_empty() {
            if let Some(entry) = self.globals.get_mut(name) {
                return update_entry(name, entry, value);
            }
            self.globals.insert(
                name.to_string(),
                Value {
                    data: value,
                    is_mutable: false,
                },
            );
            return Ok(());
        }

        if let Some(global) = self.globals.get(name) {
            if !global.is_mutable {
                return Err(format!("ERROR: '{}' is immutable.", name));
            }
        }
        self.frames.last_mut().expect("frame exists").insert(
            name.to_string(),
            Value {
                data: value,
                is_mutable: false,
            },
        );
        Ok(())
    }

    fn move_variable(&mut self, name: &str) -> Result<RuntimeVal, String> {
        if self.in_pure_mode {
            return Err("Interpreter Error: 'move' is forbidden in pure functions.".to_string());
        }
        for frame in self.frames.iter_mut().rev() {
            if let Some(entry) = frame.get_mut(name) {
                return move_entry(name, entry);
            }
        }
        if let Some(entry) = self.globals.get_mut(name) {
            return move_entry(name, entry);
        }
        Err(format!("Interpreter Error: Variable '{}' not found.", name))
    }

    fn assign_nested_field(
        &mut self,
        root_name: &str,
        path: Vec<String>,
        value: RuntimeVal,
    ) -> Result<(), String> {
        for frame in self.frames.iter_mut().rev() {
            if let Some(entry) = frame.get_mut(root_name) {
                if !entry.is_mutable {
                    return Err(format!("Variable '{}' is immutable.", root_name));
                }
                return update_nested_field(&mut entry.data, path, value);
            }
        }
        let entry = self
            .globals
            .get_mut(root_name)
            .ok_or_else(|| format!("Variable '{}' not found", root_name))?;
        if !entry.is_mutable {
            return Err(format!("Variable '{}' is immutable.", root_name));
        }
        update_nested_field(&mut entry.data, path, value)
    }

    fn value_is_mutable_argument(&self, _arg: &RuntimeVal) -> bool {
        false
    }

    fn tick(&mut self) -> Result<(), String> {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        self.step_count = self.step_count.saturating_add(1);

        if let Some(limit) = self.limits.max_steps
            && self.step_count > limit
        {
            return Err(format!(
                "Interpreter Error: Step limit exceeded ({} > {}).",
                self.step_count, limit
            ));
        }
        if let (Some(timeout), Some(started_at)) = (self.limits.timeout, self.started_at)
            && started_at.elapsed() > timeout
        {
            return Err(format!(
                "Interpreter Error: Timeout exceeded (>{} ms).",
                timeout.as_millis()
            ));
        }
        Ok(())
    }

    fn enter_call(&mut self) -> Result<(), String> {
        self.call_depth = self.call_depth.saturating_add(1);
        if let Some(limit) = self.limits.max_call_depth
            && self.call_depth > limit
        {
            let current_depth = self.call_depth;
            self.call_depth = self.call_depth.saturating_sub(1);
            return Err(format!(
                "Interpreter Error: Call depth limit exceeded ({} > {}).",
                current_depth, limit
            ));
        }
        Ok(())
    }

    fn exit_call(&mut self) {
        self.call_depth = self.call_depth.saturating_sub(1);
    }
}

fn std_io_display_call(target: &IrExpr) -> Option<(String, String)> {
    if let IrExpr::FieldAccess { target, field, .. } = target
        && let IrExpr::Variable(module, _) = &**target
        && crate::is_std_io_module_name(module)
        && crate::is_std_io_display_function(field)
    {
        return Some((module.clone(), field.clone()));
    }
    None
}

fn numbers(
    left: RuntimeVal,
    right: RuntimeVal,
    op: impl FnOnce(f64, f64) -> f64,
    verb: &str,
) -> Result<RuntimeVal, String> {
    match (left, right) {
        (RuntimeVal::Float(a), RuntimeVal::Float(b)) => Ok(RuntimeVal::Float(op(a, b))),
        _ => Err(format!("Runtime Error: Can only {} numbers", verb)),
    }
}

fn matches_kiro_type(value: &RuntimeVal, ty: &ast::KiroType) -> bool {
    match ty {
        ast::KiroType::Num => matches!(value, RuntimeVal::Float(_)),
        ast::KiroType::Str => matches!(value, RuntimeVal::String(_)),
        ast::KiroType::Bool => matches!(value, RuntimeVal::Bool(_)),
        ast::KiroType::List(_, _) => matches!(value, RuntimeVal::List(_)),
        ast::KiroType::Map(_, _, _) => matches!(value, RuntimeVal::Map(_)),
        ast::KiroType::Void => matches!(value, RuntimeVal::Void),
        ast::KiroType::Custom(name) => {
            matches!(value, RuntimeVal::Struct(struct_name, _) if struct_name == &name.value)
                || matches!(value, RuntimeVal::Handle(handle) if handle.type_name() == name.value)
        }
        _ => true,
    }
}

fn mock_value(ty: &ast::KiroType) -> RuntimeVal {
    match ty {
        ast::KiroType::Num => RuntimeVal::Float(0.0),
        ast::KiroType::Str => RuntimeVal::String("MOCK_STRING".to_string()),
        ast::KiroType::Bool => RuntimeVal::Bool(false),
        ast::KiroType::List(_, _) => RuntimeVal::List(vec![]),
        ast::KiroType::Map(_, _, _) => RuntimeVal::Map(HashMap::new()),
        ast::KiroType::Custom(name) => {
            RuntimeVal::Handle(kiro_runtime::KiroHandle::new(name.value.clone(), ()))
        }
        ast::KiroType::Void | ast::KiroType::FnType(_, _, _, _, _, _) => RuntimeVal::Void,
        _ => RuntimeVal::Void,
    }
}

fn update_entry(name: &str, entry: &mut Value, value: RuntimeVal) -> Result<(), String> {
    if !entry.is_mutable {
        return Err(format!("ERROR: '{}' is immutable.", name));
    }
    entry.data = if matches!(entry.data, RuntimeVal::AdrHandle(_)) {
        match value {
            RuntimeVal::Pointer(ptr) => RuntimeVal::AdrHandle(Some(ptr)),
            RuntimeVal::AdrHandle(handle) => RuntimeVal::AdrHandle(handle),
            other => other,
        }
    } else {
        value
    };
    Ok(())
}

fn move_entry(name: &str, entry: &mut Value) -> Result<RuntimeVal, String> {
    if !entry.is_mutable {
        return Err(format!(
            "Interpreter Error: Cannot move immutable variable '{}'.",
            name
        ));
    }
    let moved = entry.data.clone();
    entry.data = RuntimeVal::Moved;
    Ok(moved)
}

fn update_nested_field(
    current: &mut RuntimeVal,
    mut path: Vec<String>,
    value: RuntimeVal,
) -> Result<(), String> {
    let field = path.pop().ok_or("Invalid path")?;
    if path.is_empty() {
        match current {
            RuntimeVal::Struct(_, fields) => {
                fields.insert(field, value);
                Ok(())
            }
            _ => Err("Target is not a struct".to_string()),
        }
    } else {
        match current {
            RuntimeVal::Struct(_, fields) => {
                let next = fields
                    .get_mut(&field)
                    .ok_or_else(|| format!("Field '{}' not found", field))?;
                update_nested_field(next, path, value)
            }
            _ => Err("Cannot access field on non-struct".to_string()),
        }
    }
}

#[allow(dead_code)]
fn _duration_from_millis(ms: Option<u64>) -> Option<Duration> {
    ms.map(Duration::from_millis)
}
