use super::Compiler;
use super::types::compile_type;
use crate::grammar::grammar::{self, Statement};

impl Compiler {
    pub fn compile_statement(&mut self, statement: Statement) -> String {
        match statement {
            // Error Definition: error NotFound = "Description"
            Statement::ErrorDef {
                name, description, ..
            } => {
                // d.value.value contains the raw string WITH quotes, we need to strip them
                let desc = description
                    .map(|d| d.value.value.trim_matches('"').to_string())
                    .unwrap_or_else(|| name.clone());
                // Generate a helper function that creates an Arc<anyhow error>
                format!(
                    "fn kiro_error_{name}() -> std::sync::Arc<anyhow::Error> {{ std::sync::Arc::new(anyhow::anyhow!(\"{desc}\").context(\"{name}\")) }}"
                )
            }
            // 1. Compile Struct Definition
            // 1. Compile Struct Definition
            Statement::StructDef(def) => {
                let name = def.name;
                let fields = def.fields;
                let field_strs: Vec<String> = fields
                    .iter()
                    .map(|f| format!("pub {}: {}", f.name.value, compile_type(&f.field_type)))
                    .collect();

                let eq_checks: Vec<String> = fields
                    .iter()
                    .map(|f| format!("self.{0}.kiro_eq(&other.{0})", f.name.value))
                    .collect();
                let eq_body = if eq_checks.is_empty() {
                    "true".to_string()
                } else {
                    eq_checks.join(" && ")
                };

                // We add #[derive(Clone, Debug)] and impl KiroGet/KiroEq/AsKiroLoopVar
                format!(
                    "#[derive(Clone, Debug)]\npub struct {0} {{ {1} }}\nimpl KiroGet for {0} {{ type Inner = Self; fn kiro_get<R>(&self, f: impl FnOnce(&Self::Inner) -> R) -> R {{ f(self) }} }}\nimpl KiroEq for {0} {{ fn kiro_eq(&self, other: &Self) -> bool {{ {2} }} }}\nimpl AsKiroLoopVar for {0} {{ type Out = {0}; fn as_kiro(self) -> Self::Out {{ self }} }}",
                    name.value,
                    field_strs.join(", "),
                    eq_body
                )
            }
            // 6. Import Statement
            Statement::Import { module_name, .. } => {
                self.imported_modules.insert(module_name.clone());
                if self.options.skipped_module_imports.contains(&module_name) {
                    String::new()
                } else {
                    format!("pub mod {};", module_name)
                }
            }
            // 1. Variable Declaration
            Statement::VarDecl { ident, value, .. } => {
                if self.expr_yields_fn_ref(&value) {
                    self.fn_ref_vars.insert(ident.clone());
                } else {
                    self.fn_ref_vars.remove(&ident);
                }
                let val_str = self.compile_expr(value);
                self.known_vars
                    .insert(ident.clone(), super::VarInfo { is_mutable: true });
                // Track local vars in pure scope
                if self.in_pure_context {
                    self.pure_scope_params.insert(ident.clone());
                }
                // In Kiro, vars are mutable by default
                format!("let mut {} = {};", ident, val_str)
            }

            // ... (Middle assignments kept same, just copying context) ...
            Statement::AssignStmt { lhs, rhs, .. } => {
                if let grammar::Expression::Variable(v) = &lhs {
                    if self.expr_yields_fn_ref(&rhs) {
                        self.fn_ref_vars.insert(v.value.clone());
                    } else {
                        self.fn_ref_vars.remove(&v.value);
                    }
                }
                let rhs_str = self.compile_expr(rhs);

                match lhs {
                    grammar::Expression::Variable(v) => {
                        let name = v.value;
                        if let Some(info) = self.known_vars.get(&name) {
                            if info.is_mutable {
                                // Mutable Assignment
                                format!("{}.kiro_assign({});", name, rhs_str)
                            } else {
                                // Immutable Assignment -> Error
                                panic!(
                                    "Compiler Error: Cannot mutate immutable variable '{}'.",
                                    name
                                );
                            }
                        } else {
                            // Implicit Immutable Declaration (x = 10)
                            self.known_vars
                                .insert(name.clone(), super::VarInfo { is_mutable: false });
                            // Track local vars in pure scope
                            if self.in_pure_context {
                                self.pure_scope_params.insert(name.clone());
                            }
                            format!("let {} = {};", name, rhs_str)
                        }
                    }
                    grammar::Expression::Deref(_, target) => {
                        let ptr = self.compile_expr(*target);
                        format!(
                            "(*({}.as_ref().expect(\"Null Pointer\").lock().unwrap())).kiro_assign({});",
                            ptr, rhs_str
                        )
                    }
                    _ => {
                        // Complex LValue (e.g. x.y = 10)
                        let lhs_str = self.compile_lvalue(lhs);
                        format!("{}.kiro_assign({});", lhs_str, rhs_str)
                    }
                }
            }
            Statement::On {
                condition,
                body,
                else_clause,
                error_clauses,
                ..
            } => {
                let cond_str = self.compile_expr(condition.clone());
                let body_str = self.compile_block(body);

                // Helper to flatten ErrorClauseList into Vec<&ErrorClause>
                fn flatten_clauses(list: &grammar::ErrorClauseList) -> Vec<&grammar::ErrorClause> {
                    let mut result = vec![&list.first];
                    if let Some(ref rest) = list.rest {
                        result.extend(flatten_clauses(rest));
                    }
                    result
                }

                // If there are error clauses, generate a match on Result
                if let Some(ref error_list) = error_clauses {
                    let clauses = flatten_clauses(error_list);
                    let shadowing = if let grammar::Expression::Variable(v) = &condition {
                        format!("let {} = __kiro_val;", v.value)
                    } else {
                        String::new()
                    };

                    // Build chained if/else if for multiple error handlers
                    let mut err_branches = Vec::new();
                    let mut has_catch_all = false;

                    for clause in clauses.iter() {
                        let block_body = self.compile_block(clause.body.clone());
                        let clause_body = if self.in_failable_fn {
                            format!("{} return Err(__kiro_err);", block_body)
                        } else {
                            block_body
                        };

                        if let Some(ref err_type) = clause.error_type {
                            err_branches.push(format!(
                                "if __kiro_err.to_string().contains(\"{}\") {{ {} }}",
                                err_type.value, clause_body
                            ));
                        } else {
                            // Catch-all (must be last)
                            has_catch_all = true;
                            err_branches.push(format!("{{ {} }}", clause_body));
                        }
                    }

                    // If no catch-all, add propagation as final else
                    if !has_catch_all {
                        let propagation = if self.in_failable_fn {
                            "{ return Err(__kiro_err); }".to_string()
                        } else {
                            "{ panic!(\"Unhandled error: {}\", __kiro_err); }".to_string()
                        };
                        err_branches.push(propagation);
                    }

                    // Join with "else"
                    let err_branch = err_branches.join(" else ");

                    format!(
                        "match {} {{ Ok(__kiro_val) => {{ {} {} }} Err(__kiro_err) => {{ {} }} }}",
                        cond_str, shadowing, body_str, err_branch
                    )
                } else {
                    // Standard if/else
                    let else_str = match else_clause {
                        Some(clause) => format!("else {}", self.compile_block(clause.body)),
                        None => String::new(),
                    };
                    format!("if ({}).kiro_truthy() {} {}", cond_str, body_str, else_str)
                }
            }
            Statement::LoopOn {
                condition, body, ..
            } => {
                let cond_str = self.compile_expr(condition);
                let body_str = self.compile_block(body);
                format!("while {} {}", cond_str, body_str)
            }
            // 2. Iterator Loop -> Rust 'for' with injected logic
            Statement::LoopIter {
                iterator,
                iterable,
                step,
                filter,
                body,
                else_clause,
                ..
            } => {
                let range_str = self.compile_expr(iterable);

                // Handle "per 5" -> .step_by(5)
                let iter_call = if let Some(s) = step {
                    let step_val = self.compile_expr(s.value);
                    format!("{}.kiro_iter().step_by({} as usize)", range_str, step_val)
                } else {
                    format!("{}.kiro_iter()", range_str)
                };

                // Handle "on (cond)" -> Inject 'if/else' inside the loop body
                let inner_logic = if let Some(f) = filter {
                    let cond_str = self.compile_expr(f.condition);
                    let true_block = self.compile_block(body);
                    let false_block = match else_clause {
                        Some(off) => format!("else {}", self.compile_block(off.body)),
                        None => String::new(),
                    };

                    // The body of the loop becomes an IF statement
                    format!("{{ if {} {} {} }}", cond_str, true_block, false_block)
                } else {
                    // No filter? Just run the block normally.
                    self.compile_block(body)
                };

                // Implicit Mutability Rule:
                self.known_vars
                    .insert(iterator.clone(), super::VarInfo { is_mutable: false });

                format!(
                    "for {}_temp in {} {{ let {} = {}_temp.as_kiro(); {} }}",
                    iterator, iter_call, iterator, iterator, inner_logic
                )
            }
            Statement::FunctionDef(def) => {
                let name = def.name;
                let params = def.params;
                let return_type = def.return_type;
                let body = def.body;
                let pure_kw = def.pure_kw;
                let can_error = def.can_error;
                let is_pure = pure_kw.is_some();

                // Preserve existing doc if present (from pre-scan)
                let existing_doc = self.functions.get(&name).and_then(|f| f.doc.clone());

                self.functions.insert(
                    name.clone(),
                    super::FunctionInfo {
                        is_pure,
                        can_error: can_error.is_some(),
                        params: params.iter().map(|p| p.command_type.clone()).collect(),
                        return_type: return_type.clone(),
                        doc: existing_doc,
                    },
                );
                if matches!(
                    return_type,
                    Some(crate::grammar::grammar::KiroType::FnType(_, _, _, _, _, _))
                ) {
                    self.fn_returning_fn.insert(name.clone());
                }

                let old_context = self.in_pure_context;
                let old_pure_params = self.pure_scope_params.clone();
                let old_fn_ref_vars = self.fn_ref_vars.clone();
                if is_pure {
                    self.in_pure_context = true;
                    // Populate allowed params for pure scope
                    self.pure_scope_params.clear();
                    for p in &params {
                        self.pure_scope_params.insert(p.name.clone());
                    }
                }
                for p in &params {
                    if matches!(
                        p.command_type,
                        crate::grammar::grammar::KiroType::FnType(_, _, _, _, _, _)
                    ) {
                        self.fn_ref_vars.insert(p.name.clone());
                    }
                }

                let param_strs: Vec<String> = params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, compile_type(&p.command_type)))
                    .collect();

                let can_error = can_error.is_some();
                let old_in_failable = self.in_failable_fn;
                if can_error {
                    self.in_failable_fn = true;
                }

                let body_str = self.compile_block(body);

                self.in_pure_context = old_context;
                self.in_failable_fn = old_in_failable;
                self.pure_scope_params = old_pure_params; // Restore
                self.fn_ref_vars = old_fn_ref_vars;

                let ret_def = if let Some(rt) = return_type {
                    if let crate::grammar::grammar::KiroType::Void = rt {
                        "()".to_string()
                    } else {
                        compile_type(&rt)
                    }
                } else {
                    "()".to_string()
                };

                let (ret_type, final_body) = if can_error {
                    (
                        format!("KiroResult<{}>", ret_def),
                        format!("{{ let __kiro_res = {}; Ok(__kiro_res) }}", body_str),
                    )
                } else {
                    (ret_def, body_str)
                };

                let async_kw = if is_pure { "" } else { "async" };

                format!(
                    "pub {} fn {}({}) -> {} {}",
                    async_kw,
                    name,
                    param_strs.join(", "),
                    ret_type,
                    final_body
                )
            }

            // Rust-backed function (external glue)
            Statement::RustFnDecl(def) => {
                let name = def.name;
                let params = def.params;
                let return_type = def.return_type;
                let can_error = def.can_error;
                let existing_doc = self.functions.get(&name).and_then(|f| f.doc.clone());
                self.functions.insert(
                    name.clone(),
                    super::FunctionInfo {
                        is_pure: false,
                        can_error: can_error.is_some(),
                        params: params.iter().map(|p| p.command_type.clone()).collect(),
                        return_type: Some(return_type.clone()),
                        doc: existing_doc,
                    },
                );

                let param_strs: Vec<String> = params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, compile_type(&p.command_type)))
                    .collect();

                let can_error = can_error.is_some();

                let ret_def = if let crate::grammar::grammar::KiroType::Void = return_type {
                    "()".to_string()
                } else {
                    compile_type(&return_type)
                };

                // Generate call to header glue
                let arg_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
                let args_vec = if arg_names.is_empty() {
                    "vec![]".to_string()
                } else {
                    format!(
                        "vec![{}]",
                        arg_names
                            .iter()
                            .map(|a| format!("kiro_runtime::RuntimeVal::from({}.clone())", a))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };

                let final_body = if can_error {
                    format!(
                        "{{ match header::{}({}).await {{ Ok(v) => match v.try_into() {{ Ok(val) => Ok(val), Err(e) => Err(std::sync::Arc::new(anyhow::anyhow!(e))) }}, Err(e) => Err(std::sync::Arc::new(anyhow::anyhow!(e.to_string()).context(e.name))) }} }}",
                        name, args_vec
                    )
                } else {
                    format!(
                        "{{ match header::{}({}).await {{ Ok(v) => v.try_into().unwrap(), Err(e) => kiro_runtime_error(\"KIRO3007\", &format!(\"Host function '{}' failed: {{}}.\", e)) }} }}",
                        name, args_vec, name
                    )
                };

                let ret_type = if can_error {
                    format!("KiroResult<{}>", ret_def)
                } else {
                    ret_def
                };

                format!(
                    "pub async fn {}({}) -> {} {}",
                    name,
                    param_strs.join(", "),
                    ret_type,
                    final_body
                )
            }

            // 2. Expression Statement (Standard Call on its own line)
            Statement::ExprStmt(expr) => {
                let val = self.compile_expr(expr);
                format!("{};", val)
            }
            Statement::Give(_, channel, value) => {
                if self.in_pure_context {
                    panic!("Pure Function Error: 'give' is forbidden.");
                }
                let ch = self.compile_expr(channel);
                let val = self.compile_expr(value);
                format!(
                    "if let Err(_) = {}.tx.send({}).await {{ kiro_runtime_error(\"KIRO3003\", \"Pipe receiver is closed; cannot give a value.\"); }}",
                    ch, val
                )
            }

            // 4. Close -> .tx.close()
            Statement::Close(_, channel) => {
                let ch = self.compile_expr(channel);
                format!("{}.tx.close();", ch)
            }
            Statement::Rest(_) => {
                if self.in_pure_context {
                    panic!("Pure Function Error: 'rest' is forbidden.");
                }
                "tokio::task::yield_now().await;".to_string()
            }
            Statement::Check(_, condition, message) => {
                let cond = self.compile_expr(condition);
                let msg = message
                    .map(|m| m.value.value)
                    .unwrap_or_else(|| "\"check failed\"".to_string());
                format!("if !({}) {{ kiro_check_failed({}); }}", cond, msg)
            }
            // 3. Return -> return ...
            Statement::Return(_, expr) => {
                if let Some(e) = expr {
                    let val = self.compile_expr(e);
                    if self.in_failable_fn && !val.starts_with("Err(") {
                        // In failable context, wrap non-error returns in Ok(...)
                        // Unless it's already an Err(...) creation
                        format!("return Ok({});", val)
                    } else {
                        format!("return {};", val)
                    }
                } else if self.in_failable_fn {
                    "return Ok(());".to_string()
                } else {
                    "return;".to_string()
                }
            }
            // 4. Break -> break
            Statement::Break(_) => "break;".to_string(),
            // 5. Continue -> continue
            Statement::Continue(_) => "continue;".to_string(),
            Statement::Documented { item, .. } => {
                let stmt = match item {
                    grammar::AnnotatableItem::StructDef(s) => Statement::StructDef(s),
                    grammar::AnnotatableItem::FunctionDef(f) => Statement::FunctionDef(f),
                    grammar::AnnotatableItem::RustFnDecl(r) => Statement::RustFnDecl(r),
                };
                self.compile_statement(stmt)
            }
        }
    }

    pub fn compile_block(&mut self, block: grammar::Block) -> String {
        let len = block.statements.len();
        let mut lines = Vec::new();

        for (i, stmt) in block.statements.iter().enumerate() {
            let mut line = self.compile_statement(stmt.clone());

            // LOGIC: If this is the LAST statement...
            if i == len - 1 {
                // Check if it's an ExprStmt (standalone expression)
                if let grammar::Statement::ExprStmt(expr) = stmt {
                    // Special Case: RunCall returns JoinHandle, but blocks usually expect value or void.
                    // If it's a RunCall at end of block, we PROBABLY want to discard the handle to return void,
                    // unless the function is explicitly typed to return JoinHandle (which Kiro doesn't support yet).
                    // So we force a semicolon for RunCall to return ().
                    if let grammar::Expression::RunCall(_, _) = expr {
                        if !line.trim().ends_with(';') {
                            line.push(';');
                        }
                    } else {
                        // For other expressions, strip trailing semicolon to return the value
                        if line.ends_with(';') {
                            line.pop();
                        }
                    }
                }
            }
            lines.push(line);
        }
        format!("{{\n{}\n}}", lines.join("\n"))
    }
}
