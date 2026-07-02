use super::values::RuntimeVal;
use super::{HostCallCtx, HostMode, Interpreter};
use crate::grammar::grammar::{self, Expression, Statement};
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

fn matches_kiro_type(value: &RuntimeVal, ty: &grammar::KiroType) -> bool {
    match ty {
        grammar::KiroType::Num => matches!(value, RuntimeVal::Float(_)),
        grammar::KiroType::Str => matches!(value, RuntimeVal::String(_)),
        grammar::KiroType::Bool => matches!(value, RuntimeVal::Bool(_)),
        grammar::KiroType::List(_, _) => matches!(value, RuntimeVal::List(_)),
        grammar::KiroType::Map(_, _, _) => matches!(value, RuntimeVal::Map(_)),
        grammar::KiroType::Void => matches!(value, RuntimeVal::Void),
        // Keep complex/runtime-only types permissive for now in interpreter mode.
        _ => true,
    }
}

fn std_io_display_call(func: &Expression) -> Option<(&str, &str)> {
    if let Expression::FieldAccess(target, _, field) = func
        && let Expression::Variable(module) = &**target
        && crate::is_std_io_module_name(&module.value)
        && crate::is_std_io_display_function(&field.value)
    {
        return Some((&module.value, &field.value));
    }
    None
}

impl Interpreter {
    pub fn eval_expr(&mut self, expr: Expression) -> Result<RuntimeVal, String> {
        self.tick()?;
        match expr {
            Expression::MoveExpr(_, ident) => {
                let name = ident.value;
                if self.in_pure_mode {
                    return Err(
                        "Interpreter Error: 'move' is forbidden in pure functions.".to_string()
                    );
                }

                // We need to modify the env, so we need mutable access.
                if let Some(val) = self.env.get_mut(&name) {
                    if !val.is_mutable {
                        return Err(format!(
                            "Interpreter Error: Cannot move immutable variable '{}'.",
                            name
                        ));
                    }
                    // Take the value, replace with Moved
                    let moved_val = val.data.clone();
                    // We can't actually "take" out of HashMap easily without replacing.
                    // But we want to invalidate the source.
                    val.data = RuntimeVal::Moved;
                    Ok(moved_val)
                } else {
                    Err(format!("Interpreter Error: Variable '{}' not found.", name))
                }
            }

            Expression::ErrorRef(name_val) => {
                let name = name_val.value;
                if let Some(desc) = self.error_types.get(&name) {
                    Ok(RuntimeVal::Error(name.clone(), desc.clone()))
                } else {
                    Err(format!("Interpreter Error: Unknown error type '{}'.", name))
                }
            }

            Expression::StructInit(name, _, fields, _) => {
                // 1. Evaluate all fields
                let mut data = HashMap::new();
                for f in fields {
                    let val = self.eval_expr(f.value)?;
                    data.insert(f.name.value, val);
                }
                // 2. Return Struct Value
                Ok(RuntimeVal::Struct(name.value, data))
            }

            Expression::FieldAccess(target, _, field) => {
                let val = self.eval_expr(*target)?;

                // AUTO-DEREF LOGIC
                // Check if it's a struct directly OR a pointer to a struct
                match val {
                    RuntimeVal::Struct(_, fields) => fields
                        .get(&field.value)
                        .cloned()
                        .ok_or_else(|| format!("Field '{}' not found", field.value)),

                    // NEW: Handle Module Access
                    // NEW: Handle Module Access
                    RuntimeVal::Module(exports, _) => exports
                        .get(&field.value)
                        .cloned()
                        .ok_or_else(|| format!("Export '{}' not found in module", field.value)),

                    // Handle Pointer to Struct (Auto-Deref)
                    RuntimeVal::Pointer(ptr) => {
                        let guard = ptr.lock().unwrap();
                        match &*guard {
                            RuntimeVal::Struct(_, fields) => {
                                fields.get(&field.value).cloned().ok_or_else(|| {
                                    format!("Field '{}' not found in struct pointer", field.value)
                                })
                            }
                            _ => Err(format!(
                                "Cannot access field '{}' on pointer to non-struct",
                                field.value
                            )),
                        }
                    }
                    RuntimeVal::AdrHandle(Some(ptr)) => {
                        let guard = ptr.lock().unwrap();
                        match &*guard {
                            RuntimeVal::Struct(_, fields) => {
                                fields.get(&field.value).cloned().ok_or_else(|| {
                                    format!("Field '{}' not found in address handle", field.value)
                                })
                            }
                            _ => Err(format!(
                                "Cannot access field '{}' on handle to non-struct",
                                field.value
                            )),
                        }
                    }

                    _ => Err(format!(
                        "Cannot access field '{}' on this type {:?}",
                        field.value, val
                    )),
                }
            }

            Expression::Variable(v) => {
                // Check if this is an error type
                if let Some(desc) = self.error_types.get(&v.value) {
                    return Ok(RuntimeVal::Error(v.value.clone(), desc.clone()));
                }

                // Strict Purity: Ban capturing external variables
                if self.in_pure_mode && !self.pure_scope_params.contains(&v.value) {
                    return Err(format!(
                        "Compiler Error: Pure function cannot capture external variable '{}'. Only parameters and local variables are allowed.",
                        v.value
                    ));
                }

                // Otherwise look up as regular variable
                let val = self
                    .env
                    .get(&v.value)
                    .map(|val| val.data.clone())
                    .ok_or_else(|| format!("ERROR: Variable '{}' not found.", v.value))?;

                // Check for Moved
                if let RuntimeVal::Moved = val {
                    return Err(format!(
                        "Interpreter Error: Variable '{}' was moved and cannot be used.",
                        v.value
                    ));
                }

                Ok(val)
            }

            Expression::Number(num_val) => {
                let n: f64 = num_val.value.parse().map_err(|_| "Invalid number")?;
                Ok(RuntimeVal::Float(n))
            }

            // FIXED: Unwrap StringVal and strip quotes
            Expression::StringLit(s) => {
                let content = &s.value[1..s.value.len() - 1];
                Ok(RuntimeVal::String(content.to_string()))
            }
            Expression::BoolLit(b) => match b {
                grammar::BoolVal::True(_) => Ok(RuntimeVal::Bool(true)),
                grammar::BoolVal::False(_) => Ok(RuntimeVal::Bool(false)),
            },
            // 3. Pipe Init (unbounded or bounded)
            Expression::PipeInit(_, _, cap) => {
                if let Some(cap) = cap {
                    let cap: usize = cap.value.parse().map_err(|_| "Invalid pipe capacity")?;
                    let (tx, rx) = mpsc::sync_channel(cap);
                    Ok(RuntimeVal::Pipe(
                        super::values::PipeSender::Bounded(tx),
                        Arc::new(Mutex::new(rx)),
                    ))
                } else {
                    let (tx, rx) = mpsc::channel();
                    Ok(RuntimeVal::Pipe(
                        super::values::PipeSender::Unbounded(tx),
                        Arc::new(Mutex::new(rx)),
                    ))
                }
            }

            // Adr Init
            Expression::AdrInit(_, inner) => {
                if matches!(inner, crate::grammar::grammar::KiroType::Void) {
                    Ok(RuntimeVal::AdrHandle(None))
                } else {
                    // Typed adr init remains a null-like placeholder in interpreter mode.
                    Ok(RuntimeVal::Void)
                }
            }

            // 4. Take (Sync Receive)
            // 4. Take (Sync Receive)
            Expression::Take(_, channel_expr) => {
                if self.in_pure_mode {
                    return Err("Pure Function Error: 'take' is forbidden.".to_string());
                }
                let chan = self.eval_expr(*channel_expr)?;

                if let RuntimeVal::Pipe(_, rx_mutex) = chan {
                    let rx = rx_mutex.lock().unwrap();
                    let val = rx
                        .recv()
                        .map_err(|_| "Pipe Error: Channel empty or closed".to_string())?;
                    Ok(val)
                } else {
                    Err("Runtime Error: 'take' expects a pipe.".to_string())
                }
            }
            // 5. Ref (Create Pointer)
            Expression::Ref(_, target) => {
                // Function reference mode: ref foo
                if let Expression::Variable(v) = &*target {
                    if let Some(stmt) = self.functions.get(&v.value) {
                        if let Statement::FunctionDef(def) = stmt {
                            if def.pure_kw.is_none() {
                                return Err(format!(
                                    "Function reference supports pure functions only: '{}'",
                                    v.value
                                ));
                            }
                            return Ok(RuntimeVal::FunctionRef(v.value.clone()));
                        }
                    }
                }

                let val = self.eval_expr(*target)?;
                // Create a shared pointer to this value
                Ok(RuntimeVal::Pointer(Arc::new(Mutex::new(val))))
            }

            // 6. Deref (Read Pointer)
            Expression::Deref(_, target) => {
                let val = self.eval_expr(*target)?;
                match val {
                    RuntimeVal::Pointer(ptr) => {
                        let guard = ptr.lock().unwrap();
                        Ok(guard.clone())
                    }
                    RuntimeVal::AdrHandle(Some(ptr)) => {
                        let guard = ptr.lock().unwrap();
                        Ok(guard.clone())
                    }
                    RuntimeVal::AdrHandle(None) => {
                        Err("Runtime Error: 'deref' on null address handle.".to_string())
                    }
                    _ => Err("Runtime Error: 'deref' expects a pointer.".to_string()),
                }
            }

            // 2. List Init
            Expression::ListInit(_, _, _, items, _) => {
                let mut vec = Vec::new();
                for i in items {
                    vec.push(self.eval_expr(i)?);
                }
                Ok(RuntimeVal::List(vec))
            }

            // 3. Map Init
            Expression::MapInit(_, _, _, _, pairs, _) => {
                let mut map = HashMap::new();
                for p in pairs {
                    let k = self.eval_expr(p.key)?.to_string();
                    let v = self.eval_expr(p.value)?;
                    map.insert(k, v);
                }
                Ok(RuntimeVal::Map(map))
            }

            // 4. AT Command
            Expression::At(col, _, key_expr) => {
                let collection = self.eval_expr(*col)?;
                let key = self.eval_expr(*key_expr)?;

                match collection {
                    RuntimeVal::List(vec) => {
                        let idx = key.as_float()? as usize;
                        vec.get(idx)
                            .cloned()
                            .ok_or_else(|| "Index out of bounds".to_string())
                    }
                    RuntimeVal::Map(map) => {
                        let k_str = key.to_string();
                        map.get(&k_str)
                            .cloned()
                            .ok_or_else(|| "Key not found".to_string())
                    }
                    _ => Err("Cannot use 'at' on this type".to_string()),
                }
            }

            // 5. PUSH Command (Interpreter Warning)
            Expression::Push(col_expr, _, val_expr) => {
                println!("⚠️ Interpreter: 'push' ignored (compile to Rust for mutation).");
                let _ = self.eval_expr(*col_expr)?;
                let _ = self.eval_expr(*val_expr)?;
                Ok(RuntimeVal::Void)
            }
            Expression::Range(start, _, end) => {
                let s = self.eval_expr(*start)?.as_float()? as i64;
                let e = self.eval_expr(*end)?.as_float()? as i64;
                Ok(RuntimeVal::Range(s, e))
            }
            Expression::Add(lhs, _, rhs) => {
                let l = self.eval_expr(*lhs)?;
                let r = self.eval_expr(*rhs)?;
                match (l, r) {
                    (RuntimeVal::Float(a), RuntimeVal::Float(b)) => Ok(RuntimeVal::Float(a + b)),
                    (RuntimeVal::String(a), b) => Ok(RuntimeVal::String(format!("{}{}", a, b))),
                    (a, RuntimeVal::String(b)) => Ok(RuntimeVal::String(format!("{}{}", a, b))),
                    _ => Err("Runtime Error: Can only ADD numbers or strings".to_string()),
                }
            }
            Expression::Len(_, expr) => match self.eval_expr(*expr)? {
                RuntimeVal::String(s) => Ok(RuntimeVal::Float(s.len() as f64)),
                RuntimeVal::List(l) => Ok(RuntimeVal::Float(l.len() as f64)),
                RuntimeVal::Map(m) => Ok(RuntimeVal::Float(m.len() as f64)),
                _ => Err("Runtime Error: 'len' only supports string, list, map.".to_string()),
            },
            Expression::Sub(lhs, _, rhs) => {
                let l = self.eval_expr(*lhs)?;
                let r = self.eval_expr(*rhs)?;
                match (l, r) {
                    (RuntimeVal::Float(a), RuntimeVal::Float(b)) => Ok(RuntimeVal::Float(a - b)),
                    _ => Err("Runtime Error: Can only SUBTRACT numbers".to_string()),
                }
            }
            Expression::Mul(lhs, _, rhs) => {
                let l = self.eval_expr(*lhs)?;
                let r = self.eval_expr(*rhs)?;
                match (l, r) {
                    (RuntimeVal::Float(a), RuntimeVal::Float(b)) => Ok(RuntimeVal::Float(a * b)),
                    _ => Err("Runtime Error: Can only MULTIPLY numbers".to_string()),
                }
            }
            Expression::Div(lhs, _, rhs) => {
                let l = self.eval_expr(*lhs)?;
                let r = self.eval_expr(*rhs)?;
                match (l, r) {
                    (RuntimeVal::Float(a), RuntimeVal::Float(b)) => Ok(RuntimeVal::Float(a / b)),
                    _ => Err("Runtime Error: Can only DIVIDE numbers".to_string()),
                }
            }
            Expression::Gt(lhs, _, rhs) => {
                let val = self.eval_expr(*lhs)? > self.eval_expr(*rhs)?;
                Ok(RuntimeVal::Bool(val))
            }
            Expression::Lt(lhs, _, rhs) => {
                let val = self.eval_expr(*lhs)? < self.eval_expr(*rhs)?;
                Ok(RuntimeVal::Bool(val))
            }
            Expression::Eq(lhs, _, rhs) => {
                let val = self.eval_expr(*lhs)? == self.eval_expr(*rhs)?;
                Ok(RuntimeVal::Bool(val))
            }
            Expression::Neq(lhs, _, rhs) => {
                let val = self.eval_expr(*lhs)? != self.eval_expr(*rhs)?;
                Ok(RuntimeVal::Bool(val))
            }
            Expression::Geq(lhs, _, rhs) => {
                let val = self.eval_expr(*lhs)? >= self.eval_expr(*rhs)?;
                Ok(RuntimeVal::Bool(val))
            }
            Expression::Leq(lhs, _, rhs) => {
                let val = self.eval_expr(*lhs)? <= self.eval_expr(*rhs)?;
                Ok(RuntimeVal::Bool(val))
            }

            // 1. Handle Standard Calls
            Expression::Call(func_var, _, args, _) => {
                if let Some((module, function)) = std_io_display_call(&func_var)
                    && self.env.contains_key(module)
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
                    match function {
                        "print" => println!("{}", value),
                        "write" => print!("{}", value),
                        "eprint" => eprint!("{}", value),
                        "eprintline" => eprintln!("{}", value),
                        _ => unreachable!("std io display function checked before execution"),
                    }
                    return Ok(RuntimeVal::Void);
                }

                self.enter_call()?;
                let result = (|| {
                    // A. Resolve the function
                    // It could be a simple Variable (global function)
                    // OR a FieldAccess (module function)
                    let (func_stmt, func_debug_name, host_module_hint) = match *func_var {
                        Expression::Variable(v) => {
                            let mut f = self.functions.get(&v.value).cloned();
                            let mut debug_name = v.value.clone();
                            if f.is_none() {
                                if let Some(entry) = self.env.get(&v.value) {
                                    if let RuntimeVal::FunctionRef(name) = &entry.data {
                                        f = self.functions.get(name).cloned();
                                        debug_name = format!("{} -> {}", v.value, name);
                                    }
                                }
                            }
                            (f, debug_name, Some(self.current_module.clone()))
                        }
                        Expression::FieldAccess(target, _, field) => {
                            // Evaluate target to find the Module
                            let module_hint = match &*target {
                                Expression::Variable(v) => Some(v.value.clone()),
                                _ => None,
                            };
                            let val = self.eval_expr(*target)?;
                            if let RuntimeVal::Module(_, funcs) = &val {
                                let f = funcs.get(&field.value).cloned();
                                let debug_name = if let Some(ref m) = module_hint {
                                    format!("{}.{}", m, field.value)
                                } else {
                                    format!("{}.{}", val, field.value)
                                };
                                (f, debug_name, module_hint)
                            } else {
                                return Err("Target of field access is not a module.".to_string());
                            }
                        }
                        _ => return Err("Expected function name or module access".to_string()),
                    };

                    let func_stmt = func_stmt
                        .ok_or_else(|| format!("Undefined function: '{}'", func_debug_name))?;

                    if let Statement::FunctionDef(def) = func_stmt {
                        let params = def.params.clone();
                        let body = def.body.clone();
                        let pure_kw = def.pure_kw;
                        let return_type = def.return_type.clone();
                        let can_error = def.can_error.is_some();

                        // SAVE STATE
                        let old_mode = self.in_pure_mode;
                        let old_params = self.pure_scope_params.clone();
                        let old_failable = self.in_failable_fn;

                        // C. Purity Check (The "Sandbox")
                        if pure_kw.is_some() {
                            // Check Argument Safety (Must be Immutable)
                            for arg_expr in &args {
                                let mut current = arg_expr;
                                // Unwrap FieldAccess to find root
                                while let Expression::FieldAccess(target, _, _) = current {
                                    current = target;
                                }

                                if let Expression::Variable(v) = current
                                    && let Some(entry) = self.env.get(&v.value)
                                    && entry.is_mutable
                                {
                                    return Err(format!(
                                        "Pure Function Error: Argument '{}' is mutable. Pure functions only accept immutable values.",
                                        v.value
                                    ));
                                }
                            }
                        }

                        // D. Evaluate Arguments *in the current scope*
                        let mut arg_values = Vec::new();
                        for arg in args {
                            arg_values.push(self.eval_expr(arg)?);
                        }

                        // E. Create the "Stack Frame" (Local Scope)
                        let old_env = self.env.clone();
                        let mut fn_env = self.env.clone();

                        // F. Bind Arguments to Parameters
                        if params.len() != arg_values.len() {
                            return Err(format!(
                                "Function '{}' expects {} args, got {}.",
                                func_debug_name,
                                params.len(),
                                arg_values.len()
                            ));
                        }

                        for (i, param) in params.clone().into_iter().enumerate() {
                            fn_env.insert(
                                param.name,
                                super::values::Value {
                                    data: arg_values[i].clone(),
                                    is_mutable: pure_kw.is_none(),
                                },
                            );
                        }

                        // H. Run the Body
                        if pure_kw.is_some() {
                            self.in_pure_mode = true;
                            self.pure_scope_params.clear();
                            for p in &params {
                                self.pure_scope_params.insert(p.name.clone());
                            }
                        }
                        self.in_failable_fn = can_error;

                        // G. Context Switch!
                        self.env = fn_env;

                        let result_sig = self.execute_block(body);

                        // I. Restore the Old World
                        self.env = old_env;
                        self.in_pure_mode = old_mode;
                        self.pure_scope_params = old_params;
                        self.in_failable_fn = old_failable;

                        let result_sig = result_sig?;

                        // Return the result of the function
                        let out = match result_sig {
                            super::StatementResult::Normal(v) => Ok(v),
                            super::StatementResult::Return(v) => Ok(v),
                            super::StatementResult::Break | super::StatementResult::Continue => {
                                Err("Error: 'break' or 'continue' leaked from function body."
                                    .to_string())
                            }
                        }?;

                        // Enforce function return contracts in interpreter for parity with compiler.
                        let expects_void = match return_type {
                            None => true, // Omitted `->` defaults to void
                            Some(crate::grammar::grammar::KiroType::Void) => true,
                            Some(_) => false,
                        };
                        if expects_void && !matches!(out, RuntimeVal::Void) {
                            return Err(format!(
                                "Type Error: Function '{}' has void return type but returned a value. Add an explicit return type (e.g. -> num).",
                                func_debug_name
                            ));
                        }
                        if !expects_void && matches!(out, RuntimeVal::Void) {
                            return Err(format!(
                                "Type Error: Function '{}' expects a return value but returned void.",
                                func_debug_name
                            ));
                        }

                        Ok(out)
                    } else if let Statement::RustFnDecl(def) = func_stmt {
                        let params = &def.params;
                        let return_type = &def.return_type;
                        let host_module_name =
                            host_module_hint.unwrap_or_else(|| self.current_module.clone());

                        // 1. Evaluate arguments (to ensure side-effects happen or checks pass)
                        let mut arg_values = Vec::new();
                        for arg in args {
                            arg_values.push(self.eval_expr(arg)?);
                        }

                        if params.len() != arg_values.len() {
                            return Err(format!(
                                "Function '{}' expects {} args, got {}.",
                                func_debug_name,
                                params.len(),
                                arg_values.len()
                            ));
                        }

                        for (idx, (param, arg)) in params.iter().zip(arg_values.iter()).enumerate()
                        {
                            if !matches_kiro_type(arg, &param.command_type) {
                                return Err(format!(
                                    "Type Error: Argument {} for '{}' does not match declared type.",
                                    idx + 1,
                                    func_debug_name
                                ));
                            }
                        }

                        match self.host_mode {
                            HostMode::Deny => Err(format!(
                                "Interpreter Error: Host call denied for '{}'.",
                                func_debug_name
                            )),
                            HostMode::Simulate => {
                                println!(
                                    "ℹ️ [Interpreter] Simulator: Calling host function '{}' (MOCK)",
                                    func_debug_name
                                );

                                // Return Mock Value based on return_type
                                match return_type {
                                    crate::grammar::grammar::KiroType::Num => {
                                        Ok(RuntimeVal::Float(0.0))
                                    }
                                    crate::grammar::grammar::KiroType::Str => {
                                        Ok(RuntimeVal::String("MOCK_STRING".to_string()))
                                    }
                                    crate::grammar::grammar::KiroType::Bool => {
                                        Ok(RuntimeVal::Bool(false))
                                    }
                                    crate::grammar::grammar::KiroType::List(_, _) => {
                                        Ok(RuntimeVal::List(vec![]))
                                    }
                                    crate::grammar::grammar::KiroType::Map(_, _, _) => {
                                        Ok(RuntimeVal::Map(std::collections::HashMap::new()))
                                    }
                                    crate::grammar::grammar::KiroType::FnType(_, _, _, _, _, _) => {
                                        Ok(RuntimeVal::Void)
                                    }
                                    crate::grammar::grammar::KiroType::Void => Ok(RuntimeVal::Void),
                                    _ => {
                                        // For complex types (Custom, Pipe, Adr), return Void or simple fallback
                                        // to avoid complex construction logic in interpreter.
                                        Ok(RuntimeVal::Void)
                                    }
                                }
                            }
                            HostMode::Execute => {
                                let handler = self
                                    .host_registry
                                    .get(&host_module_name, &def.name)
                                    .ok_or_else(|| {
                                        format!(
                                            "Interpreter Error: Host function '{}.{}' is not registered.",
                                            host_module_name, def.name
                                        )
                                    })?;

                                let mut host_args = Vec::with_capacity(arg_values.len());
                                for arg in &arg_values {
                                    host_args.push(arg.to_host_runtime()?);
                                }

                                let host_ctx = HostCallCtx {
                                    module_name: host_module_name,
                                    function_name: def.name.clone(),
                                    step_count: self.step_count,
                                };

                                match handler(host_ctx, host_args) {
                                    Ok(value) => {
                                        let out = RuntimeVal::from_host_runtime(value)?;
                                        if !matches_kiro_type(&out, return_type) {
                                            return Err(format!(
                                                "Type Error: Host function '{}' returned a value that does not match declared type.",
                                                func_debug_name
                                            ));
                                        }
                                        Ok(out)
                                    }
                                    Err(host_err) => {
                                        if def.can_error.is_some() {
                                            Ok(RuntimeVal::Error(
                                                host_err.name.clone(),
                                                host_err.to_string(),
                                            ))
                                        } else {
                                            Err(format!(
                                                "Host Error: '{}' failed with '{}', but function is not declared failable.",
                                                func_debug_name, host_err
                                            ))
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        Err(format!("'{}' is not a function.", func_debug_name))
                    }
                })();
                self.exit_call();
                result
            }

            // 2. Handle 'Run' Calls
            // For the Interpreter (Test Bench), we run this Synchronously.
            // Why? Because implementing true threading in a tree-walker is overkill.
            // The Compiler will handle the real async/parallelism.
            Expression::RunCall(_, call_expr) => {
                println!("⚠️ [Interpreter] Note: 'run' executed synchronously in test mode.");
                self.eval_expr(*call_expr)
            }
        }
    }
}
