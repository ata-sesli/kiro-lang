use std::collections::{HashMap, HashSet};

use crate::errors::{ErrorCode, KiroError, SourceSpan};
use crate::grammar::grammar;

use super::{Compiler, FunctionInfo};

struct SourceIndex {
    file: String,
    source: String,
}

impl SourceIndex {
    fn new(file: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            file: file.into(),
            source: source.into(),
        }
    }

    fn attach_span(&self, err: KiroError, span: crate::grammar::AstSpan, label: &str) -> KiroError {
        err.with_byte_span(
            self.file.clone(),
            self.source.clone(),
            SourceSpan::new(span.0, span.1),
            label,
        )
    }
}

#[derive(Clone)]
struct Binding {
    ty: Option<grammar::KiroType>,
    is_mutable: bool,
}

struct SemanticCtx<'a> {
    module: &'a str,
    source: SourceIndex,
    functions: &'a HashMap<String, FunctionInfo>,
    module_functions: &'a HashMap<(String, String), FunctionInfo>,
    structs: HashMap<String, HashMap<String, grammar::KiroType>>,
    imports: HashSet<String>,
    scopes: Vec<HashMap<String, Binding>>,
    in_pure: bool,
    fn_name: Option<String>,
    return_type: Option<grammar::KiroType>,
}

impl<'a> SemanticCtx<'a> {
    fn new(
        module: &'a str,
        source: &'a str,
        functions: &'a HashMap<String, FunctionInfo>,
        module_functions: &'a HashMap<(String, String), FunctionInfo>,
    ) -> Self {
        Self {
            module,
            source: SourceIndex::new(module, source),
            functions,
            module_functions,
            structs: HashMap::new(),
            imports: HashSet::new(),
            scopes: vec![HashMap::new()],
            in_pure: false,
            fn_name: None,
            return_type: None,
        }
    }

    fn error(&self, code: ErrorCode, message: impl Into<String>) -> KiroError {
        KiroError::compile_error(self.module, code, message, None)
    }

    fn error_at_span(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        span: crate::grammar::AstSpan,
        label: &str,
    ) -> KiroError {
        let err = self.error(code, message);
        self.source.attach_span(err, span, label)
    }

    fn error_with_help(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> KiroError {
        KiroError::compile_error(self.module, code, message, Some(help.into()))
    }

    fn error_at_span_with_help(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        span: crate::grammar::AstSpan,
        label: &str,
        help: impl Into<String>,
    ) -> KiroError {
        let err = self.error_with_help(code, message, help);
        self.source.attach_span(err, span, label)
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn insert_binding(&mut self, name: String, ty: Option<grammar::KiroType>, is_mutable: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name, Binding { ty, is_mutable });
        }
    }

    fn binding(&self, name: &str) -> Option<&Binding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn visible_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for scope in &self.scopes {
            names.extend(scope.keys().cloned());
        }
        names.extend(self.functions.keys().cloned());
        names.extend(self.imports.iter().cloned());
        names
    }

    fn visible_function_names(&self) -> Vec<String> {
        self.functions.keys().cloned().collect()
    }

    fn imported_function_names(&self, module: &str) -> Vec<String> {
        self.module_functions
            .keys()
            .filter_map(|(known_module, function)| {
                if known_module == module {
                    Some(format!("{}.{}", known_module, function))
                } else {
                    None
                }
            })
            .collect()
    }

    fn suggest_name(&self, name: &str, candidates: Vec<String>) -> Option<String> {
        suggest_name(name, candidates)
    }

    fn required_call_target_span(&self, func: &grammar::Expression) -> crate::grammar::AstSpan {
        crate::grammar::call_target_span(func)
            .or_else(|| crate::grammar::expr_span(func))
            .expect("parsed Kiro call target should carry a source span")
    }

    fn analyze_block(&mut self, block: &grammar::Block) -> Result<(), KiroError> {
        self.push_scope();
        for stmt in &block.statements {
            self.analyze_statement(stmt)?;
        }
        self.pop_scope();
        Ok(())
    }

    fn analyze_statement(&mut self, stmt: &grammar::Statement) -> Result<(), KiroError> {
        match stmt {
            grammar::Statement::ErrorDef { .. }
            | grammar::Statement::RustFnDecl(_)
            | grammar::Statement::Break(_)
            | grammar::Statement::Continue(_) => Ok(()),
            grammar::Statement::StructDef(def) => {
                self.structs.insert(
                    crate::grammar::struct_def_name(def).to_string(),
                    def.fields
                        .iter()
                        .map(|field| {
                            (
                                crate::grammar::field_def_name(field).to_string(),
                                field.field_type.clone(),
                            )
                        })
                        .collect(),
                );
                Ok(())
            }
            grammar::Statement::Import { module_name, .. } => {
                self.imports
                    .insert(crate::grammar::variable_name(module_name).to_string());
                Ok(())
            }
            grammar::Statement::VarDecl { ident, value, .. } => {
                let ty = self.infer_expr(value)?;
                self.insert_binding(crate::grammar::variable_name(ident).to_string(), ty, true);
                Ok(())
            }
            grammar::Statement::AssignStmt { lhs, rhs, .. } => {
                let rhs_ty = self.infer_expr(rhs)?;
                match lhs {
                    grammar::Expression::Variable(v) => {
                        let name = crate::grammar::variable_name(v);
                        if let Some(binding) = self.binding(name) {
                            if !binding.is_mutable {
                                return Err(self.error_at_span(
                                    ErrorCode::MutabilityError,
                                    format!("Cannot mutate immutable variable '{}'.", name),
                                    crate::grammar::variable_span(v),
                                    "immutable variable",
                                ));
                            }
                            if let (Some(expected), Some(actual)) = (&binding.ty, &rhs_ty)
                                && !same_type(expected, actual)
                            {
                                return Err(self.error_at_span(
                                    ErrorCode::TypeError,
                                    format!(
                                        "Cannot assign {} to {} variable '{}'.",
                                        type_name(actual),
                                        type_name(expected),
                                        name
                                    ),
                                    crate::grammar::variable_span(v),
                                    "wrong assignment type",
                                ));
                            }
                        } else {
                            self.insert_binding(name.to_string(), rhs_ty, false);
                        }
                        Ok(())
                    }
                    other => {
                        self.infer_expr(other)?;
                        Ok(())
                    }
                }
            }
            grammar::Statement::Check(check_kw, condition, _) => {
                let ty = self.infer_expr(condition)?;
                if !matches!(ty, Some(grammar::KiroType::Bool)) {
                    return Err(self.error_at_span(
                        ErrorCode::TypeError,
                        "Check condition must be bool.",
                        crate::grammar::token_span(check_kw),
                        "check condition",
                    ));
                }
                Ok(())
            }
            grammar::Statement::Rest(rest_kw) => {
                if self.in_pure {
                    return Err(self.error_at_span(
                        ErrorCode::PureViolation,
                        "Pure Function Error: 'rest' is forbidden.",
                        crate::grammar::token_span(rest_kw),
                        "forbidden in pure fn",
                    ));
                }
                Ok(())
            }
            grammar::Statement::Give(give_kw, channel, value) => {
                if self.in_pure {
                    return Err(self.error_at_span(
                        ErrorCode::PureViolation,
                        "Pure Function Error: 'give' is forbidden.",
                        crate::grammar::token_span(give_kw),
                        "forbidden in pure fn",
                    ));
                }
                let ch_ty = self.infer_expr(channel)?;
                let value_ty = self.infer_expr(value)?;
                match ch_ty {
                    Some(grammar::KiroType::Pipe(_, inner)) => {
                        if let Some(actual) = value_ty
                            && !same_type(&inner, &actual)
                        {
                            return Err(self.error_at_span_with_help(
                                ErrorCode::TypeError,
                                format!(
                                    "'give' value must be {}, got {}.",
                                    type_name(&inner),
                                    type_name(&actual)
                                ),
                                crate::grammar::token_span(give_kw),
                                "wrong give value",
                                "Send a value whose type matches the pipe element type.",
                            ));
                        }
                    }
                    Some(_) => {
                        return Err(self.error_at_span_with_help(
                            ErrorCode::BadUse,
                            "'give' expects a pipe.",
                            crate::grammar::token_span(give_kw),
                            "bad give",
                            "Use `give pipe value` where the first expression is a pipe.",
                        ));
                    }
                    None => {}
                }
                Ok(())
            }
            grammar::Statement::Close(close_kw, channel) => {
                let ch_ty = self.infer_expr(channel)?;
                if !matches!(ch_ty, Some(grammar::KiroType::Pipe(_, _))) {
                    return Err(self.error_at_span_with_help(
                        ErrorCode::BadUse,
                        "'close' expects a pipe.",
                        crate::grammar::token_span(close_kw),
                        "bad close",
                        "Use `close pipe` where the expression is a pipe.",
                    ));
                }
                Ok(())
            }
            grammar::Statement::Return(return_kw, expr) => {
                let returned = if let Some(expr) = expr {
                    self.infer_expr(expr)?
                } else {
                    Some(grammar::KiroType::Void)
                };
                if matches!(self.return_type, Some(grammar::KiroType::Void))
                    && !matches!(returned, Some(grammar::KiroType::Void))
                {
                    let actual = returned
                        .as_ref()
                        .map(type_name)
                        .unwrap_or_else(|| "value".to_string());
                    return Err(self.error_at_span_with_help(
                        ErrorCode::TypeError,
                        format!(
                            "Function '{}' returns void but returned a value.",
                            self.fn_name.as_deref().unwrap_or("<function>")
                        ),
                        crate::grammar::token_span(return_kw),
                        "return value",
                        format!("Add `-> {}` or remove the returned value.", actual),
                    ));
                }
                if let (Some(expected), Some(actual)) = (&self.return_type, &returned)
                    && !same_type(expected, actual)
                {
                    return Err(self.error_at_span(
                        ErrorCode::TypeError,
                        format!(
                            "Wrong return type: expected {}, got {}.",
                            type_name(expected),
                            type_name(actual)
                        ),
                        crate::grammar::token_span(return_kw),
                        "wrong return type",
                    ));
                }
                Ok(())
            }
            grammar::Statement::On {
                condition,
                body,
                else_clause,
                error_clauses,
                ..
            } => {
                self.infer_expr(condition)?;
                self.analyze_block(body)?;
                if let Some(off) = else_clause {
                    self.analyze_block(&off.body)?;
                }
                if let Some(errors) = error_clauses {
                    self.analyze_error_clauses(errors)?;
                }
                Ok(())
            }
            grammar::Statement::LoopOn {
                condition, body, ..
            } => {
                self.infer_expr(condition)?;
                self.analyze_block(body)
            }
            grammar::Statement::LoopIter {
                iterator,
                iterable,
                step,
                filter,
                body,
                else_clause,
                ..
            } => {
                self.infer_expr(iterable)?;
                if let Some(step) = step {
                    self.infer_expr(&step.value)?;
                }
                self.push_scope();
                self.insert_binding(
                    crate::grammar::variable_name(iterator).to_string(),
                    None,
                    false,
                );
                if let Some(filter) = filter {
                    self.infer_expr(&filter.condition)?;
                }
                for stmt in &body.statements {
                    self.analyze_statement(stmt)?;
                }
                if let Some(off) = else_clause {
                    self.analyze_block(&off.body)?;
                }
                self.pop_scope();
                Ok(())
            }
            grammar::Statement::FunctionDef(def) => self.analyze_function(def),
            grammar::Statement::ExprStmt(expr) => {
                self.infer_expr(expr)?;
                Ok(())
            }
            grammar::Statement::Documented { item, .. } => match item {
                grammar::AnnotatableItem::RustFnDecl(_) => Ok(()),
                grammar::AnnotatableItem::StructDef(def) => {
                    self.structs.insert(
                        crate::grammar::struct_def_name(def).to_string(),
                        def.fields
                            .iter()
                            .map(|field| {
                                (
                                    crate::grammar::field_def_name(field).to_string(),
                                    field.field_type.clone(),
                                )
                            })
                            .collect(),
                    );
                    Ok(())
                }
                grammar::AnnotatableItem::FunctionDef(def) => self.analyze_function(def),
            },
        }
    }

    fn analyze_error_clauses(
        &mut self,
        clauses: &grammar::ErrorClauseList,
    ) -> Result<(), KiroError> {
        self.analyze_block(&clauses.first.body)?;
        if let Some(rest) = &clauses.rest {
            self.analyze_error_clauses(rest)?;
        }
        Ok(())
    }

    fn analyze_function(&mut self, def: &grammar::FunctionDef) -> Result<(), KiroError> {
        let old_pure = self.in_pure;
        let old_return = self.return_type.clone();
        let old_fn_name = self.fn_name.clone();
        self.in_pure = def.pure_kw.is_some();
        let declared_return = def.return_type.clone().unwrap_or(grammar::KiroType::Void);
        self.return_type = Some(declared_return.clone());
        let fn_name = crate::grammar::function_name(&def.name);
        self.fn_name = Some(fn_name.to_string());
        self.push_scope();
        for param in &def.params {
            self.insert_binding(
                crate::grammar::param_name(param).to_string(),
                Some(param.command_type.clone()),
                false,
            );
        }
        for stmt in &def.body.statements {
            self.analyze_statement(stmt)?;
        }
        if !matches!(declared_return, grammar::KiroType::Void)
            && let Some(grammar::Statement::ExprStmt(expr)) = def.body.statements.last()
        {
            let actual = self.infer_expr(expr)?;
            if let Some(actual) = actual
                && !same_type(&declared_return, &actual)
            {
                let span = crate::grammar::expr_span(expr)
                    .unwrap_or_else(|| crate::grammar::function_span(&def.name));
                return Err(self.error_at_span(
                    ErrorCode::TypeError,
                    format!(
                        "Wrong return type: expected {}, got {}.",
                        type_name(&declared_return),
                        type_name(&actual)
                    ),
                    span,
                    "wrong return type",
                ));
            }
        }
        self.pop_scope();
        self.in_pure = old_pure;
        self.return_type = old_return;
        self.fn_name = old_fn_name;
        if !matches!(declared_return, grammar::KiroType::Void)
            && !block_guarantees_return(&def.body, &declared_return)
        {
            return Err(self.error_at_span(
                ErrorCode::TypeError,
                format!(
                    "Function '{}' must return {} on every path.",
                    fn_name,
                    type_name(&declared_return)
                ),
                crate::grammar::function_span(&def.name),
                "missing return",
            ));
        }
        Ok(())
    }

    fn infer_expr(
        &mut self,
        expr: &grammar::Expression,
    ) -> Result<Option<grammar::KiroType>, KiroError> {
        match expr {
            grammar::Expression::Number(_) => Ok(Some(grammar::KiroType::Num)),
            grammar::Expression::StringLit(_) => Ok(Some(grammar::KiroType::Str)),
            grammar::Expression::BoolLit(_) => Ok(Some(grammar::KiroType::Bool)),
            grammar::Expression::ErrorRef(_) => Ok(None),
            grammar::Expression::AdrInit(_, inner) => {
                Ok(Some(grammar::KiroType::Adr((), Box::new(inner.clone()))))
            }
            grammar::Expression::PipeInit(_, inner, _) => {
                Ok(Some(grammar::KiroType::Pipe((), Box::new(inner.clone()))))
            }
            grammar::Expression::Variable(v) => {
                let name = crate::grammar::variable_name(v);
                if let Some(binding) = self.binding(name) {
                    return Ok(binding.ty.clone());
                }
                if self.imports.contains(name) || self.functions.contains_key(name) {
                    return Ok(None);
                }
                let mut err = self.error_at_span(
                    ErrorCode::UnknownName,
                    format!("Unknown variable '{}'.", name),
                    crate::grammar::variable_span(v),
                    "unknown variable",
                );
                if let Some(suggestion) = self.suggest_name(name, self.visible_names()) {
                    err = err.with_suggestion(suggestion);
                }
                Err(err)
            }
            grammar::Expression::MoveExpr(move_kw, v) => {
                if self.in_pure {
                    return Err(self.error_at_span(
                        ErrorCode::PureViolation,
                        "Compiler Error: 'move' is forbidden in pure functions.",
                        crate::grammar::token_span(move_kw),
                        "forbidden in pure fn",
                    ));
                }
                let name = crate::grammar::variable_name(v);
                let binding = self.binding(name).ok_or_else(|| {
                    let mut err = self.error_at_span(
                        ErrorCode::UnknownName,
                        format!("Unknown variable '{}'.", name),
                        crate::grammar::variable_span(v),
                        "unknown variable",
                    );
                    if let Some(suggestion) = self.suggest_name(name, self.visible_names()) {
                        err = err.with_suggestion(suggestion);
                    }
                    err
                })?;
                if !binding.is_mutable {
                    return Err(self.error_at_span(
                        ErrorCode::MutabilityError,
                        format!("Cannot move immutable variable '{}'.", name),
                        crate::grammar::variable_span(v),
                        "immutable variable",
                    ));
                }
                Ok(binding.ty.clone())
            }
            grammar::Expression::StructInit(name, _, fields, _) => {
                for field in fields {
                    let struct_name = crate::grammar::struct_name(name);
                    let field_name = crate::grammar::field_name(&field.name);
                    if let Some(known_fields) = self.structs.get(struct_name)
                        && !known_fields.contains_key(field_name)
                    {
                        return Err(self.error_at_span(
                            ErrorCode::TypeError,
                            format!("Type {} has no field '{}'.", struct_name, field_name),
                            crate::grammar::field_span(&field.name),
                            "unknown field",
                        ));
                    }
                    self.infer_expr(&field.value)?;
                }
                Ok(Some(grammar::KiroType::Custom(name.value.clone())))
            }
            grammar::Expression::FieldAccess(target, _, field) => {
                if let grammar::Expression::Variable(module) = &**target
                    && self.imports.contains(crate::grammar::variable_name(module))
                {
                    let module_name = crate::grammar::variable_name(module);
                    let member_name = crate::grammar::field_name(field);
                    if self
                        .module_functions
                        .contains_key(&(module_name.to_string(), member_name.to_string()))
                    {
                        return Ok(None);
                    }
                    let call_name = format!("{}.{}", module_name, member_name);
                    let mut err = self.error_at_span(
                        ErrorCode::ImportError,
                        format!("Unknown function '{}'.", call_name),
                        crate::grammar::expr_span(expr)
                            .unwrap_or_else(|| crate::grammar::field_span(field)),
                        "unknown imported function",
                    );
                    if let Some(suggestion) =
                        self.suggest_name(&call_name, self.imported_function_names(module_name))
                    {
                        err = err.with_suggestion(suggestion);
                    }
                    return Err(err);
                }
                let target_ty = self.infer_expr(target)?;
                if let Some(grammar::KiroType::Custom(name)) = target_ty
                    && let Some(fields) = self.structs.get(&name.value)
                {
                    return fields.get(field.name()).cloned().map(Some).ok_or_else(|| {
                        self.error_at_span(
                            ErrorCode::TypeError,
                            format!("Type {} has no field '{}'.", name.value, field.name()),
                            crate::grammar::field_span(field),
                            "unknown field",
                        )
                    });
                }
                Ok(None)
            }
            grammar::Expression::ListInit(_, inner, _, items, _) => {
                for item in items {
                    self.infer_expr(item)?;
                }
                Ok(Some(grammar::KiroType::List((), Box::new(inner.clone()))))
            }
            grammar::Expression::MapInit(_, key, val, _, pairs, _) => {
                for pair in pairs {
                    self.infer_expr(&pair.key)?;
                    self.infer_expr(&pair.value)?;
                }
                Ok(Some(grammar::KiroType::Map(
                    (),
                    Box::new(key.clone()),
                    Box::new(val.clone()),
                )))
            }
            grammar::Expression::At(collection, at_kw, key) => {
                let col_ty = self.infer_expr(collection)?;
                let key_ty = self.infer_expr(key)?;
                match col_ty {
                    Some(grammar::KiroType::List(_, inner)) => {
                        if let Some(actual) = key_ty
                            && !same_type(&grammar::KiroType::Num, &actual)
                        {
                            return Err(self.error_at_span_with_help(
                                ErrorCode::TypeError,
                                format!("List index must be num, got {}.", type_name(&actual)),
                                crate::grammar::token_span(at_kw),
                                "wrong index type",
                                "Lists are indexed with numeric positions.",
                            ));
                        }
                        Ok(Some(*inner))
                    }
                    Some(grammar::KiroType::Map(_, key, val)) => {
                        if let Some(actual) = key_ty
                            && !same_type(&key, &actual)
                        {
                            return Err(self.error_at_span_with_help(
                                ErrorCode::TypeError,
                                format!(
                                    "Map key must be {}, got {}.",
                                    type_name(&key),
                                    type_name(&actual)
                                ),
                                crate::grammar::token_span(at_kw),
                                "wrong key type",
                                "Use a key whose type matches the map declaration.",
                            ));
                        }
                        Ok(Some(*val))
                    }
                    Some(_) => Err(self.error_at_span_with_help(
                        ErrorCode::BadUse,
                        "'at' expects a list or map.",
                        crate::grammar::token_span(at_kw),
                        "bad access",
                        "Use `list at index` or `map at key`.",
                    )),
                    None => Ok(None),
                }
            }
            grammar::Expression::Push(list, push_kw, value) => {
                let list_ty = self.infer_expr(list)?;
                let value_ty = self.infer_expr(value)?;
                match list_ty {
                    Some(grammar::KiroType::List(_, inner)) => {
                        if let Some(actual) = value_ty
                            && !same_type(&inner, &actual)
                        {
                            return Err(self.error_at_span_with_help(
                                ErrorCode::TypeError,
                                format!(
                                    "'push' value must be {}, got {}.",
                                    type_name(&inner),
                                    type_name(&actual)
                                ),
                                crate::grammar::token_span(push_kw),
                                "wrong push value",
                                "Push a value whose type matches the list element type.",
                            ));
                        }
                    }
                    Some(_) => {
                        return Err(self.error_at_span_with_help(
                            ErrorCode::BadUse,
                            "'push' expects a list.",
                            crate::grammar::token_span(push_kw),
                            "bad push",
                            "Use `list push value` where the left expression is a list.",
                        ));
                    }
                    None => {}
                }
                Ok(Some(grammar::KiroType::Void))
            }
            grammar::Expression::Take(take_kw, channel) => {
                if self.in_pure {
                    return Err(self.error_at_span(
                        ErrorCode::PureViolation,
                        "Pure Function Error: 'take' is forbidden.",
                        crate::grammar::token_span(take_kw),
                        "forbidden in pure fn",
                    ));
                }
                match self.infer_expr(channel)? {
                    Some(grammar::KiroType::Pipe(_, inner)) => Ok(Some(*inner)),
                    Some(_) => Err(self.error_at_span_with_help(
                        ErrorCode::BadUse,
                        "'take' expects a pipe.",
                        crate::grammar::token_span(take_kw),
                        "bad take",
                        "Use `take pipe` where the expression is a pipe.",
                    )),
                    None => Ok(None),
                }
            }
            grammar::Expression::Ref(_, target) => {
                self.infer_expr(target)?;
                Ok(None)
            }
            grammar::Expression::Deref(deref_kw, target) => {
                let target_ty = self.infer_expr(target)?;
                if matches!(
                    target_ty,
                    Some(grammar::KiroType::Adr(_, inner)) if matches!(*inner, grammar::KiroType::Void)
                ) {
                    return Err(self.error_at_span_with_help(
                        ErrorCode::BadUse,
                        "Cannot deref adr void.",
                        crate::grammar::token_span(deref_kw),
                        "bad deref",
                        "Use a typed address like `adr num`, or pass the opaque address to host code.",
                    ));
                }
                Ok(None)
            }
            grammar::Expression::Len(_, target) => {
                self.infer_expr(target)?;
                Ok(Some(grammar::KiroType::Num))
            }
            grammar::Expression::Add(lhs, _, rhs)
            | grammar::Expression::Sub(lhs, _, rhs)
            | grammar::Expression::Mul(lhs, _, rhs)
            | grammar::Expression::Div(lhs, _, rhs) => {
                let lhs_ty = self.infer_expr(lhs)?;
                let rhs_ty = self.infer_expr(rhs)?;
                match (lhs_ty, rhs_ty) {
                    (Some(grammar::KiroType::Str), _) | (_, Some(grammar::KiroType::Str)) => {
                        Ok(Some(grammar::KiroType::Str))
                    }
                    _ => Ok(Some(grammar::KiroType::Num)),
                }
            }
            grammar::Expression::Eq(lhs, _, rhs)
            | grammar::Expression::Neq(lhs, _, rhs)
            | grammar::Expression::Gt(lhs, _, rhs)
            | grammar::Expression::Lt(lhs, _, rhs)
            | grammar::Expression::Geq(lhs, _, rhs)
            | grammar::Expression::Leq(lhs, _, rhs) => {
                self.infer_expr(lhs)?;
                self.infer_expr(rhs)?;
                Ok(Some(grammar::KiroType::Bool))
            }
            grammar::Expression::Range(lhs, _, rhs) => {
                self.infer_expr(lhs)?;
                self.infer_expr(rhs)?;
                Ok(None)
            }
            grammar::Expression::Call(func, _, args, _) => {
                if let Some((module, function)) = std_io_display_call(func)
                    && self.imports.contains(module)
                {
                    let call_name = format!("{}.{}", module, function);
                    if args.len() != 1 {
                        return Err(self.error_at_span_with_help(
                            ErrorCode::WrongArgumentCount,
                            format!(
                                "Wrong argument count for '{}': expected 1, got {}.",
                                call_name,
                                args.len()
                            ),
                            self.required_call_target_span(func),
                            "wrong argument count",
                            format!("{} expects (value)", call_name),
                        ));
                    }
                    if self.in_pure {
                        return Err(self.error_at_span(
                            ErrorCode::PureViolation,
                            format!(
                                "Pure function cannot call impure/async function '{}' inside a pure function.",
                                call_name
                            ),
                            self.required_call_target_span(func),
                            "impure call",
                        ));
                    }
                    self.infer_expr(&args[0])?;
                    return Ok(Some(grammar::KiroType::Void));
                }

                let (call_name, info) = self.lookup_call(func)?;
                if args.len() != info.params.len() {
                    let help = if info.params.is_empty() {
                        format!("{} expects no arguments", call_name)
                    } else {
                        format!(
                            "{} expects ({})",
                            call_name,
                            info.params
                                .iter()
                                .map(type_name)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    };
                    return Err(self.error_at_span_with_help(
                        ErrorCode::WrongArgumentCount,
                        format!(
                            "Wrong argument count for '{}': expected {}, got {}.",
                            call_name,
                            info.params.len(),
                            args.len()
                        ),
                        self.required_call_target_span(func),
                        "wrong argument count",
                        help,
                    ));
                }
                if self.in_pure && !info.is_pure {
                    return Err(self.error_at_span(
                        ErrorCode::PureViolation,
                        format!(
                            "Pure function cannot call impure/async function '{}' inside a pure function.",
                            call_name
                        ),
                        self.required_call_target_span(func),
                        "impure call",
                    ));
                }
                for (idx, arg) in args.iter().enumerate() {
                    let actual = self.infer_expr(arg)?;
                    if let Some(actual) = actual {
                        let expected = &info.params[idx];
                        if !same_type(expected, &actual) {
                            return Err(self.error_at_span(
                                ErrorCode::TypeError,
                                format!(
                                    "Argument {} for '{}' must be {}, got {}.",
                                    idx + 1,
                                    call_name,
                                    type_name(expected),
                                    type_name(&actual)
                                ),
                                self.required_call_target_span(func),
                                "wrong argument type",
                            ));
                        }
                    }
                }
                Ok(info.return_type.clone())
            }
            grammar::Expression::RunCall(run_kw, call) => {
                if let grammar::Expression::Call(func, _, args, _) = &**call {
                    let (_, info) = self.lookup_call(func)?;
                    if args.len() != info.params.len() {
                        return Err(self.error_at_span(
                            ErrorCode::WrongArgumentCount,
                            format!(
                                "Wrong argument count for '{}': expected {}, got {}.",
                                self.call_name(func),
                                info.params.len(),
                                args.len()
                            ),
                            crate::grammar::token_span(run_kw),
                            "wrong argument count",
                        ));
                    }
                    for arg in args {
                        self.infer_expr(arg)?;
                    }
                } else {
                    return Err(self.error_at_span_with_help(
                        ErrorCode::BadUse,
                        "'run' expects a function call.",
                        crate::grammar::token_span(run_kw),
                        "bad run",
                        "Use `run worker()` instead of `run worker`.",
                    ));
                }
                Ok(None)
            }
        }
    }

    fn lookup_call(&self, func: &grammar::Expression) -> Result<(String, FunctionInfo), KiroError> {
        match func {
            grammar::Expression::Variable(v) => {
                let name = crate::grammar::variable_name(v);
                self.functions
                    .get(name)
                    .cloned()
                    .map(|info| (name.to_string(), info))
                    .ok_or_else(|| {
                        let mut err = self.error_at_span(
                            ErrorCode::UnknownName,
                            format!("Unknown function '{}'.", name),
                            crate::grammar::variable_span(v),
                            "unknown function",
                        );
                        if let Some(suggestion) =
                            self.suggest_name(name, self.visible_function_names())
                        {
                            err = err.with_suggestion(suggestion);
                        }
                        err
                    })
            }
            grammar::Expression::FieldAccess(target, _, field) => {
                if let grammar::Expression::Variable(module) = &**target
                    && self.imports.contains(crate::grammar::variable_name(module))
                {
                    let module_name = crate::grammar::variable_name(module);
                    let member_name = crate::grammar::field_name(field);
                    return self
                        .module_functions
                        .get(&(module_name.to_string(), member_name.to_string()))
                        .cloned()
                        .map(|info| (format!("{}.{}", module_name, member_name), info))
                        .ok_or_else(|| {
                            let call_name = format!("{}.{}", module_name, member_name);
                            let mut err = self.error_at_span(
                                ErrorCode::ImportError,
                                format!("Unknown function '{}'.", call_name),
                                crate::grammar::call_target_span(func)
                                    .unwrap_or_else(|| crate::grammar::field_span(field)),
                                "unknown imported function",
                            );
                            if let Some(suggestion) = self
                                .suggest_name(&call_name, self.imported_function_names(module_name))
                            {
                                err = err.with_suggestion(suggestion);
                            }
                            err
                        });
                }
                Err(self.error_at_span(
                    ErrorCode::UnknownName,
                    "Unknown function target.",
                    self.required_call_target_span(func),
                    "unknown function target",
                ))
            }
            _ => Err(self.error_at_span(
                ErrorCode::UnknownName,
                "Unknown function target.",
                self.required_call_target_span(func),
                "unknown function target",
            )),
        }
    }

    fn call_name(&self, func: &grammar::Expression) -> String {
        match func {
            grammar::Expression::Variable(v) => crate::grammar::variable_name(v).to_string(),
            grammar::Expression::FieldAccess(target, _, field) => {
                if let grammar::Expression::Variable(module) = &**target {
                    format!(
                        "{}.{}",
                        crate::grammar::variable_name(module),
                        crate::grammar::field_name(field)
                    )
                } else {
                    "<computed function>".to_string()
                }
            }
            _ => "<computed function>".to_string(),
        }
    }
}

fn block_guarantees_return(block: &grammar::Block, expected: &grammar::KiroType) -> bool {
    block
        .statements
        .last()
        .map(|stmt| statement_guarantees_return(stmt, expected))
        .unwrap_or(false)
}

fn statement_guarantees_return(stmt: &grammar::Statement, expected: &grammar::KiroType) -> bool {
    match stmt {
        grammar::Statement::Return(_, _) => true,
        grammar::Statement::ExprStmt(_) => !matches!(expected, grammar::KiroType::Void),
        grammar::Statement::On {
            body,
            else_clause: Some(off),
            ..
        } => {
            block_guarantees_return(body, expected) && block_guarantees_return(&off.body, expected)
        }
        _ => false,
    }
}

impl Compiler {
    pub fn validate_semantics(
        &mut self,
        program: &grammar::Program,
        module: &str,
        source: &str,
    ) -> Result<(), KiroError> {
        self.functions = Self::collect_program_functions(program);
        self.validate_effectful_recursion(program, module, Some(source))?;
        let mut ctx = SemanticCtx::new(module, source, &self.functions, &self.module_functions);
        ctx.collect_program_structs(program);
        for stmt in &program.statements {
            ctx.analyze_statement(stmt)?;
        }
        Ok(())
    }
}

impl SemanticCtx<'_> {
    fn collect_program_structs(&mut self, program: &grammar::Program) {
        for stmt in &program.statements {
            match stmt {
                grammar::Statement::StructDef(def) => self.insert_struct(def),
                grammar::Statement::Documented {
                    item: grammar::AnnotatableItem::StructDef(def),
                    ..
                } => self.insert_struct(def),
                _ => {}
            }
        }
    }

    fn insert_struct(&mut self, def: &grammar::StructDef) {
        self.structs.insert(
            crate::grammar::struct_def_name(def).to_string(),
            def.fields
                .iter()
                .map(|field| {
                    (
                        crate::grammar::field_def_name(field).to_string(),
                        field.field_type.clone(),
                    )
                })
                .collect(),
        );
    }
}

fn same_type(a: &grammar::KiroType, b: &grammar::KiroType) -> bool {
    match (a, b) {
        (grammar::KiroType::Num, grammar::KiroType::Num)
        | (grammar::KiroType::Str, grammar::KiroType::Str)
        | (grammar::KiroType::Bool, grammar::KiroType::Bool)
        | (grammar::KiroType::Void, grammar::KiroType::Void) => true,
        (grammar::KiroType::Adr(_, a), grammar::KiroType::Adr(_, b))
        | (grammar::KiroType::Pipe(_, a), grammar::KiroType::Pipe(_, b))
        | (grammar::KiroType::List(_, a), grammar::KiroType::List(_, b)) => same_type(a, b),
        (grammar::KiroType::Map(_, ak, av), grammar::KiroType::Map(_, bk, bv)) => {
            same_type(ak, bk) && same_type(av, bv)
        }
        (grammar::KiroType::Custom(a), grammar::KiroType::Custom(b)) => a.value == b.value,
        _ => false,
    }
}

fn type_name(ty: &grammar::KiroType) -> String {
    match ty {
        grammar::KiroType::Num => "num".to_string(),
        grammar::KiroType::Str => "str".to_string(),
        grammar::KiroType::Bool => "bool".to_string(),
        grammar::KiroType::Void => "void".to_string(),
        grammar::KiroType::Adr(_, inner) => format!("adr {}", type_name(inner)),
        grammar::KiroType::Pipe(_, inner) => format!("pipe {}", type_name(inner)),
        grammar::KiroType::List(_, inner) => format!("list {}", type_name(inner)),
        grammar::KiroType::Map(_, key, val) => {
            format!("map {} {}", type_name(key), type_name(val))
        }
        grammar::KiroType::FnType(_, _, args, _, _, ret) => {
            format!(
                "fn({}) -> {}",
                args.iter().map(type_name).collect::<Vec<_>>().join(", "),
                type_name(ret)
            )
        }
        grammar::KiroType::Custom(name) => name.value.clone(),
    }
}

fn std_io_display_call(func: &grammar::Expression) -> Option<(&str, &str)> {
    if let grammar::Expression::FieldAccess(target, _, field) = func
        && let grammar::Expression::Variable(module) = &**target
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

fn suggest_name(input: &str, candidates: Vec<String>) -> Option<String> {
    candidates
        .into_iter()
        .filter(|candidate| candidate != input)
        .map(|candidate| {
            let score = edit_distance(input, &candidate);
            (candidate, score)
        })
        .filter(|(candidate, score)| {
            *score <= 2
                || (*score <= 3 && candidate.len().abs_diff(input.len()) <= 2)
                || candidate.starts_with(input)
                || input.starts_with(candidate)
        })
        .min_by_key(|(_, score)| *score)
        .map(|(candidate, _)| candidate)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0; b.len() + 1];

    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[b.len()]
}
