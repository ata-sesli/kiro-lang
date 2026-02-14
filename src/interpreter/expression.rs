use super::Interpreter;
use super::values::RuntimeVal;
use crate::grammar::grammar::{self, Expression, Statement};
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

impl Interpreter {
    pub fn eval_expr(&mut self, expr: Expression) -> Result<RuntimeVal, String> {
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

                    // Handle Pointer to Struct (Auto-Deref) could go here
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
                    // (Purity check logic from previous task)
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
            // 3. Pipe Init
            Expression::PipeInit(_, _) => {
                let (tx, rx) = mpsc::channel();
                Ok(RuntimeVal::Pipe(tx, Arc::new(Mutex::new(rx))))
            }

            // Adr Init
            Expression::AdrInit(_, _) => {
                // For interpreter, we can treat lazy pointers as Void until assigned?
                // Or implementing a special "None" value?
                // Currently returning Void which matches Option::None behavior loosely in untyped interpreter.
                Ok(RuntimeVal::Void)
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
            // Pointer Logic (Interpreter Stub)
            // Implementing true shared memory in a tree-walker is hard.
            // For now, we just pass the value through (Copy semantics) to fix the build.
            Expression::Ref(_, target) => {
                let val = self.eval_expr(*target)?;
                Ok(val) // "Fake" reference
            }
            Expression::Deref(_, target) => {
                let val = self.eval_expr(*target)?;
                Ok(val) // "Fake" dereference
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
                // A. Resolve the function
                // It could be a simple Variable (global function)
                // OR a FieldAccess (module function)

                // We'll extract the FunctionDef statement
                let (func_stmt, func_debug_name) = match *func_var {
                    Expression::Variable(v) => {
                        let f = self.functions.get(&v.value).cloned();
                        (f, v.value)
                    }
                    Expression::FieldAccess(target, _, field) => {
                        // Evaluate target to find the Module
                        let val = self.eval_expr(*target)?;
                        if let RuntimeVal::Module(_, funcs) = &val {
                            let f = funcs.get(&field.value).cloned();
                            (f, format!("{}.{}", val, field.value)) // Note: val display might be <Module>
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
                    // C. Purity Check (The "Sandbox")
                    if pure_kw.is_some() {
                        // Check Argument Safety (Must be Immutable)
                        for arg_expr in &args {
                            let mut current = arg_expr;
                            // Unwrap FieldAccess to find root
                            while let Expression::FieldAccess(target, _, _) = current {
                                current = target;
                            }

                            if let Expression::Variable(v) = current {
                                if let Some(entry) = self.env.get(&v.value) {
                                    if entry.is_mutable {
                                        return Err(format!(
                                            "Pure Function Error: Argument '{}' is mutable. Pure functions only accept immutable values.",
                                            v.value
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    // D. Evaluate Arguments *in the current scope*
                    let mut arg_values = Vec::new();
                    for arg in args {
                        arg_values.push(self.eval_expr(arg)?);
                    }

                    // E. Create the "Stack Frame" (Local Scope)
                    // We save the old environment to restore it later
                    let old_env = self.env.clone();

                    // We create a fresh environment.
                    // Note: For true "Lexical Scoping", we should copy global variables in,
                    // but for "Pure" functions, we might want an empty map!
                    // For now, let's clone the global scope so we can read globals.
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

                    for (i, param) in params.into_iter().enumerate() {
                        fn_env.insert(
                            param.name,
                            super::values::Value {
                                data: arg_values[i].clone(),
                                is_mutable: if pure_kw.is_some() { false } else { true },
                            },
                        );
                    }

                    // H. Run the Body
                    let old_mode = self.in_pure_mode;
                    if pure_kw.is_some() {
                        self.in_pure_mode = true;
                        // Args checked above before move
                    }

                    // G. Context Switch!
                    self.env = fn_env;

                    let result_sig = self.execute_block(body);

                    // I. Restore the Old World
                    self.env = old_env;
                    self.in_pure_mode = old_mode;

                    let result_sig = result_sig?; // Propagate error now

                    // Return the result of the function
                    match result_sig {
                        super::StatementResult::Normal(v) => Ok(v),
                        super::StatementResult::Return(v) => Ok(v),
                        super::StatementResult::Break | super::StatementResult::Continue => {
                            Err("Error: 'break' or 'continue' leaked from function body."
                                .to_string())
                        }
                    }
                } else if let Statement::RustFnDecl(def) = func_stmt {
                    let params = &def.params;
                    let return_type = &def.return_type;
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

                    println!(
                        "ℹ️ [Interpreter] Simulator: Calling host function '{}' (MOCK)",
                        func_debug_name
                    );

                    // 2. Return Mock Value based on return_type
                    match return_type {
                        crate::grammar::grammar::KiroType::Num => Ok(RuntimeVal::Float(0.0)),
                        crate::grammar::grammar::KiroType::Str => {
                            Ok(RuntimeVal::String("MOCK_STRING".to_string()))
                        }
                        crate::grammar::grammar::KiroType::Bool => Ok(RuntimeVal::Bool(false)),
                        crate::grammar::grammar::KiroType::List(_, _) => {
                            Ok(RuntimeVal::List(vec![]))
                        }
                        crate::grammar::grammar::KiroType::Map(_, _, _) => {
                            Ok(RuntimeVal::Map(std::collections::HashMap::new()))
                        }
                        crate::grammar::grammar::KiroType::Void => Ok(RuntimeVal::Void),
                        _ => {
                            // For complex types (Custom, Pipe, Adr), return Void or simple fallback
                            // to avoid complex construction logic in interpreter.
                            Ok(RuntimeVal::Void)
                        }
                    }
                } else {
                    Err(format!("'{}' is not a function.", func_debug_name))
                }
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
