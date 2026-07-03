use super::Compiler;

use crate::grammar::grammar::{self, Expression};

fn std_io_display_call(func: &Expression) -> Option<(&str, &str)> {
    if let Expression::FieldAccess(target, _, field) = func
        && let Expression::Variable(module) = &**target
        && crate::is_std_io_module_name(crate::grammar::variable_name(module))
        && crate::is_std_io_display_function(crate::grammar::field_name(field))
    {
        return Some((
            crate::grammar::variable_name(module),
            crate::grammar::field_name(field),
        ));
    }
    None
}

impl Compiler {
    fn compile_std_io_display_call(
        &mut self,
        module: &str,
        function: &str,
        args: &[Expression],
    ) -> String {
        if !self.imported_modules.contains(module) {
            panic!("Compiler Error: Module '{}' is not imported.", module);
        }
        if self.in_pure_context {
            panic!(
                "Compiler Error: Pure function cannot call impure/async function '{}.{}' inside a pure function.",
                module, function
            );
        }
        if args.len() != 1 {
            panic!(
                "Compiler Error: Function '{}.{}' expects 1 argument, got {}.",
                module,
                function,
                args.len()
            );
        }
        let value = format!("({}).clone()", self.compile_expr(args[0].clone()));
        match function {
            "print" => format!("println!(\"{{}}\", {})", value),
            "write" => format!("print!(\"{{}}\", {})", value),
            "eprint" => format!("eprint!(\"{{}}\", {})", value),
            "eprintline" => format!("eprintln!(\"{{}}\", {})", value),
            _ => unreachable!("std io display function checked before codegen"),
        }
    }

    pub fn compile_expr(&mut self, expr: Expression) -> String {
        match expr {
            Expression::MoveExpr(_, ident) => {
                let name = crate::grammar::variable_name(&ident).to_string();

                // 1. Purity Check
                if self.in_pure_context {
                    panic!("Compiler Error: 'move' is forbidden in pure functions.");
                }

                // 2. Mutable Check
                if let Some(info) = self.known_vars.get(&name) {
                    if !info.is_mutable {
                        panic!("Compiler Error: Cannot move immutable variable '{}'.", name);
                    }
                } else {
                    panic!("Compiler Error: Variable '{}' not found.", name);
                }

                // 3. Mark as Moved
                self.moved_vars.insert(name.clone());

                // 4. Generate Rust: std::mem::take(&mut var)
                // This swaps the value with default (void/empty)
                format!("std::mem::take(&mut {})", name)
            }

            Expression::ErrorRef(name_val) => {
                let name = crate::grammar::struct_name(&name_val);
                // Generate Err(kiro_error_Name())
                format!("Err(kiro_error_{}())", name)
            }

            Expression::Variable(v) => {
                let name = crate::grammar::variable_name(&v);
                // Strict Purity: Ban capturing external variables
                // EXCEPTION: Allow calling other global functions (which are technically "captured" but are code, not data)
                let is_known_fn = self.functions.contains_key(name);

                if self.in_pure_context && !self.pure_scope_params.contains(name) && !is_known_fn {
                    panic!(
                        "Compiler Error: Pure function cannot capture external variable '{}'. Only parameters and local variables are allowed.",
                        name
                    );
                }

                // Move Check: Ensure variable hasn't been moved
                if self.moved_vars.contains(name) {
                    panic!(
                        "Compiler Error: Variable '{}' was moved and cannot be used.",
                        name
                    );
                }

                // Default Behavior: Clone variable access to ensure Copy Semantics
                format!("({}).clone()", name)
            }

            // 2. Compile Struct Init
            Expression::StructInit(name, _, fields, _) => {
                let init_strs: Vec<String> = fields
                    .iter()
                    .map(|f| {
                        format!(
                            "{}: {}",
                            crate::grammar::field_name(&f.name),
                            self.compile_expr(f.value.clone())
                        )
                    })
                    .collect();

                format!(
                    "{} {{ {} }}",
                    crate::grammar::struct_name(&name),
                    init_strs.join(", ")
                )
            }

            // 3. Compile Field Access
            Expression::FieldAccess(target, _, field) => {
                // Check if the target is a known module (e.g., "math")
                if let Expression::Variable(v) = &*target {
                    let module_name = crate::grammar::variable_name(v);
                    if self.imported_modules.contains(module_name) {
                        return format!("{}::{}", module_name, crate::grammar::field_name(&field));
                    }
                }

                format!(
                    "{}.kiro_get(|v| v.{}.clone())",
                    self.compile_expr(*target),
                    crate::grammar::field_name(&field)
                )
            }

            Expression::Number(num_val) => {
                let n: f64 = num_val.value.parse().unwrap();
                if n.fract() == 0.0 {
                    format!("{:.1}", n)
                } else {
                    n.to_string()
                }
            }

            Expression::StringLit(s) => format!("String::from({})", s.value),
            Expression::BoolLit(b) => match b {
                rust_sitter::Spanned {
                    value: grammar::BoolVal::True(_),
                    ..
                } => "true".to_string(),
                rust_sitter::Spanned {
                    value: grammar::BoolVal::False(_),
                    ..
                } => "false".to_string(),
            },

            // Adr Init (Lazy / Void)
            Expression::AdrInit(_, inner) => {
                if let grammar::KiroType::Void = inner {
                    "KiroAdrVoid::default()".to_string()
                } else {
                    let type_str = crate::compiler::types::compile_type(&inner);
                    format!(
                        "Option::<std::sync::Arc<std::sync::Mutex<{}>>>::None",
                        type_str
                    )
                }
            }

            // Pipe Init (unbounded or bounded)
            Expression::PipeInit(_, pipe_type, cap) => {
                let inner_type = crate::compiler::types::compile_type(&pipe_type);
                let channel = if let Some(cap) = cap {
                    let cap: usize = cap.value.parse().unwrap_or(0);
                    format!("async_channel::bounded({})", cap)
                } else {
                    "async_channel::unbounded()".to_string()
                };
                if let grammar::KiroType::Void = pipe_type {
                    format!(
                        "{{ let (tx, rx) = {}; KiroPipe::<()> {{ tx, rx }} }}",
                        channel
                    )
                } else {
                    format!(
                        "{{ let (tx, rx) = {}; KiroPipe::<{}> {{ tx, rx }} }}",
                        channel, inner_type
                    )
                }
            }

            Expression::Take(_, channel) => {
                if self.in_pure_context {
                    panic!("Pure Function Error: 'take' is forbidden.");
                }
                let ch = self.compile_expr(*channel);
                format!(
                    "match {}.rx.recv().await {{ Ok(__kiro_val) => __kiro_val, Err(_) => kiro_runtime_error(\"KIRO3002\", \"Pipe is closed; cannot take a value.\") }}",
                    ch
                )
            }

            Expression::Ref(_, target) => {
                if let Expression::Variable(v) = &*target
                    && let Some(info) = self.functions.get(crate::grammar::variable_name(v))
                {
                    if !info.is_pure {
                        panic!(
                            "Compiler Error: Function references currently support pure functions only: '{}'.",
                            crate::grammar::variable_name(v)
                        );
                    }
                    return crate::grammar::variable_name(v).to_string();
                }
                let val = self.compile_expr(*target);
                format!("Some(std::sync::Arc::new(std::sync::Mutex::new({})))", val)
            }

            Expression::Deref(_, target) => {
                let ptr = self.compile_expr(*target);
                format!("(*kiro_adr_or_fail(&{}).lock().unwrap())", ptr)
            }

            Expression::ListInit(_, _, _, items, _) => {
                let elems: Vec<String> =
                    items.iter().map(|e| self.compile_expr(e.clone())).collect();
                format!("vec![{}]", elems.join(", "))
            }

            Expression::MapInit(_, _, _, _, pairs, _) => {
                let entries: Vec<String> = pairs
                    .iter()
                    .map(|p| {
                        format!(
                            "({}, {})",
                            self.compile_expr(p.key.clone()),
                            self.compile_expr(p.value.clone())
                        )
                    })
                    .collect();
                format!("std::collections::HashMap::from([{}])", entries.join(", "))
            }

            Expression::At(col, _, key) => {
                let col_str = self.compile_expr(*col);
                let key_str = self.compile_expr(*key);
                format!("{}.kiro_at({})", col_str, key_str)
            }

            Expression::Push(list, _, val) => {
                let list_str = self.compile_expr(*list);
                let val_str = self.compile_expr(*val);
                format!("{}.push({})", list_str, val_str)
            }

            Expression::Add(lhs, _, rhs) => format!(
                "({}.kiro_add({}))",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Len(_, expr) => {
                format!("{}.kiro_len()", self.compile_expr(*expr))
            }
            Expression::Sub(lhs, _, rhs) => format!(
                "({} - {})",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Mul(lhs, _, rhs) => format!(
                "({} * {})",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Div(lhs, _, rhs) => format!(
                "({} / {})",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Eq(lhs, _, rhs) => format!(
                "({}.kiro_eq(&{}))",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Neq(lhs, _, rhs) => format!(
                "(!{}.kiro_eq(&{}))",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Gt(lhs, _, rhs) => format!(
                "({} > {})",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Lt(lhs, _, rhs) => format!(
                "({} < {})",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Geq(lhs, _, rhs) => format!(
                "({} >= {})",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Leq(lhs, _, rhs) => format!(
                "({} <= {})",
                self.compile_expr(*lhs),
                self.compile_expr(*rhs)
            ),
            Expression::Range(start, _, end) => {
                let start_str = self.compile_expr(*start);
                let end_str = self.compile_expr(*end);
                format!("(({} as i64)..({} as i64))", start_str, end_str)
            }
            Expression::Call(func, _, args, _) => {
                if let Some((module, function)) = std_io_display_call(&func) {
                    return self.compile_std_io_display_call(module, function, &args);
                }

                let call_info = self.call_function_info(&func);
                let call_name = self.call_name(&func);
                // Determine if we need .await (Access func by reference BEFORE move)
                let needs_await = if let Expression::Variable(v) = &*func {
                    if self.fn_ref_vars.contains(crate::grammar::variable_name(v)) {
                        false
                    } else if let Some(info) = &call_info {
                        !info.is_pure
                    } else {
                        true
                    }
                } else if let Some(info) = &call_info {
                    !info.is_pure
                } else {
                    true
                };

                if let Some(info) = &call_info {
                    if self.in_pure_context && !info.is_pure {
                        panic!(
                            "Compiler Error: Pure function cannot call impure/async function '{}' inside a pure function.",
                            call_name
                        );
                    }

                    if info.is_pure {
                        for arg in &args {
                            let mut current = arg;
                            while let Expression::FieldAccess(target, _, _) = current {
                                current = target;
                            }
                            if let Expression::Variable(arg_v) = current {
                                let arg_name = crate::grammar::variable_name(arg_v);
                                if let Some(var_info) = self.known_vars.get(arg_name) {
                                    if var_info.is_mutable {
                                        panic!(
                                            "Compiler Error: Cannot pass mutable variable '{}' to pure function '{}'.",
                                            arg_name, call_name
                                        );
                                    }
                                }
                            }
                        }
                    }
                } else if let Expression::Variable(v) = &*func {
                    if self.in_pure_context
                        && !self.fn_ref_vars.contains(crate::grammar::variable_name(v))
                    {
                        panic!(
                            "Compiler Error: Pure function cannot call unknown/impure function '{}'.",
                            crate::grammar::variable_name(v)
                        );
                    }
                } else if self.in_pure_context {
                    panic!(
                        "Compiler Error: Pure function cannot call unknown/impure function '{}'.",
                        call_name
                    );
                }

                let func_name = self.compile_expr(*func);
                let arg_strs: Vec<String> = args
                    .iter()
                    .map(|a| format!("({}).clone()", self.compile_expr(a.clone())))
                    .collect();

                if needs_await {
                    format!("{}({}).await", func_name, arg_strs.join(", "))
                } else {
                    format!("{}({})", func_name, arg_strs.join(", "))
                }
            }

            // 4. Run Call -> tokio::spawn
            Expression::RunCall(_, call_expr) => {
                // call_expr is the "foo(x)" part.
                // We need to strip the ".await" that compile_expr normally adds to calls!
                // This is a bit tricky. Let's handle it manually:

                if let Expression::Call(func, _, args, _) = *call_expr {
                    if let Some((module, function)) = std_io_display_call(&func) {
                        let call = self.compile_std_io_display_call(module, function, &args);
                        return format!("tokio::spawn(async move {{ {} }})", call);
                    }

                    let call_info = self.call_function_info(&func);
                    // Check if target is pure (Sync)
                    let is_pure_target = if let Expression::Variable(v) = &*func {
                        if self.fn_ref_vars.contains(crate::grammar::variable_name(v)) {
                            true
                        } else if let Some(info) = &call_info {
                            info.is_pure
                        } else {
                            false
                        }
                    } else if let Some(info) = &call_info {
                        info.is_pure
                    } else {
                        false
                    };

                    let func_name = self.compile_expr(*func);
                    let arg_strs: Vec<String> = args
                        .iter()
                        .map(|a| format!("({}).clone()", self.compile_expr(a.clone())))
                        .collect();

                    // Spawn logic:
                    if is_pure_target {
                        // Sync function: Wrap in async block
                        // tokio::spawn(async move { foo(args) })
                        format!(
                            "tokio::spawn(async move {{ {}({}) }})",
                            func_name,
                            arg_strs.join(", ")
                        )
                    } else {
                        // Async function: Call directly (returns Future)
                        // tokio::spawn(foo(args))
                        format!("tokio::spawn({}({}))", func_name, arg_strs.join(", "))
                    }
                } else {
                    "/* Error: run must be followed by a function call */".to_string()
                }
            }
        }
    }

    pub fn compile_lvalue(&mut self, expr: Expression) -> String {
        match expr {
            Expression::Variable(v) => crate::grammar::variable_name(&v).to_string(),
            Expression::FieldAccess(target, _, field) => {
                format!(
                    "{}.{}",
                    self.compile_lvalue(*target),
                    crate::grammar::field_name(&field)
                )
            }
            Expression::Deref(_, target) => {
                format!(
                    "*kiro_adr_or_fail(&{}).lock().unwrap()",
                    self.compile_expr(*target)
                )
            }
            _ => panic!("Invalid lvalue: {:?}", expr),
        }
    }
}
