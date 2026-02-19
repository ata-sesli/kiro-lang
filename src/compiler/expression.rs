use super::Compiler;

use crate::grammar::grammar::{self, Expression};

impl Compiler {
    pub fn compile_expr(&mut self, expr: Expression) -> String {
        match expr {
            Expression::MoveExpr(_, ident) => {
                let name = ident.value;

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
                let name = name_val.value;
                // Generate Err(kiro_error_Name())
                format!("Err(kiro_error_{}())", name)
            }

            Expression::Variable(v) => {
                // Strict Purity: Ban capturing external variables
                // EXCEPTION: Allow calling other global functions (which are technically "captured" but are code, not data)
                let is_known_fn = self.functions.contains_key(&v.value);

                if self.in_pure_context
                    && !self.pure_scope_params.contains(&v.value)
                    && !is_known_fn
                {
                    panic!(
                        "Compiler Error: Pure function cannot capture external variable '{}'. Only parameters and local variables are allowed.",
                        v.value
                    );
                }

                // Move Check: Ensure variable hasn't been moved
                if self.moved_vars.contains(&v.value) {
                    panic!(
                        "Compiler Error: Variable '{}' was moved and cannot be used.",
                        v.value
                    );
                }

                // Default Behavior: Clone variable access to ensure Copy Semantics
                format!("({}).clone()", v.value)
            }

            // 2. Compile Struct Init
            Expression::StructInit(name, _, fields, _) => {
                let init_strs: Vec<String> = fields
                    .iter()
                    .map(|f| format!("{}: {}", f.name.value, self.compile_expr(f.value.clone())))
                    .collect();

                format!("{} {{ {} }}", name.value, init_strs.join(", "))
            }

            // 3. Compile Field Access
            Expression::FieldAccess(target, _, field) => {
                // Check if the target is a known module (e.g., "math")
                if let Expression::Variable(v) = &*target {
                    if self.imported_modules.contains(&v.value) {
                        return format!("{}::{}", v.value, field.value);
                    }
                }

                format!(
                    "{}.kiro_get(|v| v.{}.clone())",
                    self.compile_expr(*target),
                    field.value
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
                grammar::BoolVal::True(_) => "true".to_string(),
                grammar::BoolVal::False(_) => "false".to_string(),
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
                format!("{}.rx.recv().await.unwrap()", ch)
            }

            Expression::Ref(_, target) => {
                if let Expression::Variable(v) = &*target
                    && let Some(info) = self.functions.get(&v.value)
                {
                    if !info.is_pure {
                        panic!(
                            "Compiler Error: Function references currently support pure functions only: '{}'.",
                            v.value
                        );
                    }
                    return v.value.clone();
                }
                let val = self.compile_expr(*target);
                format!("Some(std::sync::Arc::new(std::sync::Mutex::new({})))", val)
            }

            Expression::Deref(_, target) => {
                let ptr = self.compile_expr(*target);
                format!(
                    "*({}.as_ref().expect(\"Dereferencing Void/Null Pointer\").lock().unwrap())",
                    ptr
                )
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
                // Determine if we need .await (Access func by reference BEFORE move)
                let needs_await = if let Expression::Variable(v) = &*func {
                    if self.fn_ref_vars.contains(&v.value) {
                        false
                    } else if let Some(info) = self.functions.get(&v.value) {
                        !info.is_pure
                    } else {
                        true
                    }
                } else {
                    true
                };

                if let Expression::Variable(v) = &*func {
                    if let Some(info) = self.functions.get(&v.value) {
                        if self.in_pure_context && !info.is_pure {
                            panic!(
                                "Compiler Error: Pure function cannot call impure/async function '{}' inside a pure function.",
                                v.value
                            );
                        }

                        if info.is_pure {
                            for arg in &args {
                                let mut current = arg;
                                while let Expression::FieldAccess(target, _, _) = current {
                                    current = target;
                                }
                                if let Expression::Variable(arg_v) = current {
                                    if let Some(var_info) = self.known_vars.get(&arg_v.value) {
                                        if var_info.is_mutable {
                                            panic!(
                                                "Compiler Error: Cannot pass mutable variable '{}' to pure function '{}'.",
                                                arg_v.value, v.value
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    } else if self.in_pure_context && !self.fn_ref_vars.contains(&v.value) {
                        panic!(
                            "Compiler Error: Pure function cannot call unknown/impure function '{}'.",
                            v.value
                        );
                    }
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
                    // Check if target is pure (Sync)
                    let is_pure_target = if let Expression::Variable(v) = &*func {
                        if self.fn_ref_vars.contains(&v.value) {
                            true
                        } else {
                            self.functions
                                .get(&v.value)
                                .map(|i| i.is_pure)
                                .unwrap_or(false)
                        }
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
            Expression::Variable(v) => v.value,
            Expression::FieldAccess(target, _, field) => {
                format!("{}.{}", self.compile_lvalue(*target), field.value)
            }
            Expression::Deref(_, target) => {
                format!("*({}.lock().unwrap())", self.compile_expr(*target))
            }
            _ => panic!("Invalid lvalue: {:?}", expr),
        }
    }
}
