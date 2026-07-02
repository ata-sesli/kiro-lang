use std::collections::{HashMap, HashSet};

use crate::errors::{ErrorCode, KiroError};
use crate::grammar::grammar;

use super::{Compiler, FunctionInfo};

struct SourceLocation {
    line: usize,
    column: usize,
}

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

    fn locate(&self, needle: &str) -> Option<SourceLocation> {
        self.source
            .find(needle)
            .and_then(|offset| self.location_at(offset))
    }

    fn locate_last(&self, needle: &str) -> Option<SourceLocation> {
        self.source
            .rfind(needle)
            .and_then(|offset| self.location_at(offset))
    }

    fn location_at(&self, offset: usize) -> Option<SourceLocation> {
        let mut line_start = 0;
        for (idx, line) in self.source.split_inclusive('\n').enumerate() {
            let line_end = line_start + line.len();
            if offset < line_end {
                return Some(SourceLocation {
                    line: idx + 1,
                    column: offset.saturating_sub(line_start) + 1,
                });
            }
            line_start = line_end;
        }
        None
    }

    fn attach(&self, err: KiroError, token: &str, label: &str, prefer_last: bool) -> KiroError {
        let located = if prefer_last {
            self.locate_last(token)
        } else {
            self.locate(token)
        };
        if let Some(loc) = located {
            err.with_source_span(
                self.file.clone(),
                self.source.clone(),
                loc.line,
                loc.column,
                token.len().max(1),
                label,
            )
        } else {
            err
        }
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

    fn error_at(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        token: &str,
        label: &str,
    ) -> KiroError {
        let err = self.error(code, message);
        self.source.attach(err, token, label, false)
    }

    fn error_at_last(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        token: &str,
        label: &str,
    ) -> KiroError {
        let err = self.error(code, message);
        self.source.attach(err, token, label, true)
    }

    fn error_with_help(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> KiroError {
        KiroError::compile_error(self.module, code, message, Some(help.into()))
    }

    fn error_at_with_help(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        token: &str,
        label: &str,
        help: impl Into<String>,
    ) -> KiroError {
        let err = self.error_with_help(code, message, help);
        self.source.attach(err, token, label, false)
    }

    fn error_at_last_with_help(
        &self,
        code: ErrorCode,
        message: impl Into<String>,
        token: &str,
        label: &str,
        help: impl Into<String>,
    ) -> KiroError {
        let err = self.error_with_help(code, message, help);
        self.source.attach(err, token, label, true)
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
                    def.name.value.clone(),
                    def.fields
                        .iter()
                        .map(|field| (field.name.value.clone(), field.field_type.clone()))
                        .collect(),
                );
                Ok(())
            }
            grammar::Statement::Import { module_name, .. } => {
                self.imports.insert(module_name.clone());
                Ok(())
            }
            grammar::Statement::VarDecl { ident, value, .. } => {
                let ty = self.infer_expr(value)?;
                self.insert_binding(ident.clone(), ty, true);
                Ok(())
            }
            grammar::Statement::AssignStmt { lhs, rhs, .. } => {
                let rhs_ty = self.infer_expr(rhs)?;
                match lhs {
                    grammar::Expression::Variable(v) => {
                        if let Some(binding) = self.binding(&v.value) {
                            if !binding.is_mutable {
                                return Err(self.error_at_last(
                                    ErrorCode::MutabilityError,
                                    format!("Cannot mutate immutable variable '{}'.", v.value),
                                    &v.value,
                                    "immutable variable",
                                ));
                            }
                            if let (Some(expected), Some(actual)) = (&binding.ty, &rhs_ty)
                                && !same_type(expected, actual)
                            {
                                return Err(self.error(
                                    ErrorCode::TypeError,
                                    format!(
                                        "Cannot assign {} to {} variable '{}'.",
                                        type_name(actual),
                                        type_name(expected),
                                        v.value
                                    ),
                                ));
                            }
                        } else {
                            self.insert_binding(v.value.clone(), rhs_ty, false);
                        }
                        Ok(())
                    }
                    other => {
                        self.infer_expr(other)?;
                        Ok(())
                    }
                }
            }
            grammar::Statement::Check(_, condition, _) => {
                let ty = self.infer_expr(condition)?;
                if !matches!(ty, Some(grammar::KiroType::Bool)) {
                    return Err(self.error_at(
                        ErrorCode::TypeError,
                        "Check condition must be bool.",
                        "check",
                        "check condition",
                    ));
                }
                Ok(())
            }
            grammar::Statement::Rest(_) => {
                if self.in_pure {
                    return Err(self.error_at(
                        ErrorCode::PureViolation,
                        "Pure Function Error: 'rest' is forbidden.",
                        "rest",
                        "forbidden in pure fn",
                    ));
                }
                Ok(())
            }
            grammar::Statement::Give(_, channel, value) => {
                if self.in_pure {
                    return Err(self.error_at(
                        ErrorCode::PureViolation,
                        "Pure Function Error: 'give' is forbidden.",
                        "give",
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
                            return Err(self.error_at_with_help(
                                ErrorCode::TypeError,
                                format!(
                                    "'give' value must be {}, got {}.",
                                    type_name(&inner),
                                    type_name(&actual)
                                ),
                                "give",
                                "wrong give value",
                                "Send a value whose type matches the pipe element type.",
                            ));
                        }
                    }
                    Some(_) => {
                        return Err(self.error_at_with_help(
                            ErrorCode::BadUse,
                            "'give' expects a pipe.",
                            "give",
                            "bad give",
                            "Use `give pipe value` where the first expression is a pipe.",
                        ));
                    }
                    None => {}
                }
                Ok(())
            }
            grammar::Statement::Close(_, channel) => {
                let ch_ty = self.infer_expr(channel)?;
                if !matches!(ch_ty, Some(grammar::KiroType::Pipe(_, _))) {
                    return Err(self.error_at_with_help(
                        ErrorCode::BadUse,
                        "'close' expects a pipe.",
                        "close",
                        "bad close",
                        "Use `close pipe` where the expression is a pipe.",
                    ));
                }
                Ok(())
            }
            grammar::Statement::Return(_, expr) => {
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
                    return Err(self.error_at_last_with_help(
                        ErrorCode::TypeError,
                        format!(
                            "Function '{}' returns void but returned a value.",
                            self.fn_name.as_deref().unwrap_or("<function>")
                        ),
                        "return",
                        "return value",
                        format!("Add `-> {}` or remove the returned value.", actual),
                    ));
                }
                if let (Some(expected), Some(actual)) = (&self.return_type, &returned)
                    && !same_type(expected, actual)
                {
                    return Err(self.error_at_last(
                        ErrorCode::TypeError,
                        format!(
                            "Wrong return type: expected {}, got {}.",
                            type_name(expected),
                            type_name(actual)
                        ),
                        "return",
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
                self.insert_binding(iterator.clone(), None, false);
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
                        def.name.value.clone(),
                        def.fields
                            .iter()
                            .map(|field| (field.name.value.clone(), field.field_type.clone()))
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
        self.fn_name = Some(def.name.clone());
        self.push_scope();
        for param in &def.params {
            self.insert_binding(param.name.clone(), Some(param.command_type.clone()), false);
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
                return Err(self.error_at_last(
                    ErrorCode::TypeError,
                    format!(
                        "Wrong return type: expected {}, got {}.",
                        type_name(&declared_return),
                        type_name(&actual)
                    ),
                    &def.name,
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
            return Err(self.error_at(
                ErrorCode::TypeError,
                format!(
                    "Function '{}' must return {} on every path.",
                    def.name,
                    type_name(&declared_return)
                ),
                &def.name,
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
                if let Some(binding) = self.binding(&v.value) {
                    return Ok(binding.ty.clone());
                }
                if self.imports.contains(&v.value) || self.functions.contains_key(&v.value) {
                    return Ok(None);
                }
                let mut err = self.error_at(
                    ErrorCode::UnknownName,
                    format!("Unknown variable '{}'.", v.value),
                    &v.value,
                    "unknown variable",
                );
                if let Some(suggestion) = self.suggest_name(&v.value, self.visible_names()) {
                    err = err.with_suggestion(suggestion);
                }
                Err(err)
            }
            grammar::Expression::MoveExpr(_, v) => {
                if self.in_pure {
                    return Err(self.error_at(
                        ErrorCode::PureViolation,
                        "Compiler Error: 'move' is forbidden in pure functions.",
                        "move",
                        "forbidden in pure fn",
                    ));
                }
                let binding = self.binding(&v.value).ok_or_else(|| {
                    let mut err = self.error_at(
                        ErrorCode::UnknownName,
                        format!("Unknown variable '{}'.", v.value),
                        &v.value,
                        "unknown variable",
                    );
                    if let Some(suggestion) = self.suggest_name(&v.value, self.visible_names()) {
                        err = err.with_suggestion(suggestion);
                    }
                    err
                })?;
                if !binding.is_mutable {
                    return Err(self.error_at(
                        ErrorCode::MutabilityError,
                        format!("Cannot move immutable variable '{}'.", v.value),
                        &v.value,
                        "immutable variable",
                    ));
                }
                Ok(binding.ty.clone())
            }
            grammar::Expression::StructInit(name, _, fields, _) => {
                for field in fields {
                    if let Some(known_fields) = self.structs.get(&name.value)
                        && !known_fields.contains_key(&field.name.value)
                    {
                        return Err(self.error_at(
                            ErrorCode::TypeError,
                            format!("Type {} has no field '{}'.", name.value, field.name.value),
                            &field.name.value,
                            "unknown field",
                        ));
                    }
                    self.infer_expr(&field.value)?;
                }
                Ok(Some(grammar::KiroType::Custom(name.clone())))
            }
            grammar::Expression::FieldAccess(target, _, field) => {
                if let grammar::Expression::Variable(module) = &**target
                    && self.imports.contains(&module.value)
                {
                    if self
                        .module_functions
                        .contains_key(&(module.value.clone(), field.value.clone()))
                    {
                        return Ok(None);
                    }
                    let call_name = format!("{}.{}", module.value, field.value);
                    let mut err = self.error_at(
                        ErrorCode::ImportError,
                        format!("Unknown function '{}'.", call_name),
                        &call_name,
                        "unknown imported function",
                    );
                    if let Some(suggestion) =
                        self.suggest_name(&call_name, self.imported_function_names(&module.value))
                    {
                        err = err.with_suggestion(suggestion);
                    }
                    return Err(err);
                }
                let target_ty = self.infer_expr(target)?;
                if let Some(grammar::KiroType::Custom(name)) = target_ty
                    && let Some(fields) = self.structs.get(&name.value)
                {
                    return fields.get(&field.value).cloned().map(Some).ok_or_else(|| {
                        self.error_at(
                            ErrorCode::TypeError,
                            format!("Type {} has no field '{}'.", name.value, field.value),
                            &field.value,
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
            grammar::Expression::At(collection, _, key) => {
                let col_ty = self.infer_expr(collection)?;
                let key_ty = self.infer_expr(key)?;
                match col_ty {
                    Some(grammar::KiroType::List(_, inner)) => {
                        if let Some(actual) = key_ty
                            && !same_type(&grammar::KiroType::Num, &actual)
                        {
                            return Err(self.error_at_with_help(
                                ErrorCode::TypeError,
                                format!("List index must be num, got {}.", type_name(&actual)),
                                "at",
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
                            return Err(self.error_at_with_help(
                                ErrorCode::TypeError,
                                format!(
                                    "Map key must be {}, got {}.",
                                    type_name(&key),
                                    type_name(&actual)
                                ),
                                "at",
                                "wrong key type",
                                "Use a key whose type matches the map declaration.",
                            ));
                        }
                        Ok(Some(*val))
                    }
                    Some(_) => Err(self.error_at_with_help(
                        ErrorCode::BadUse,
                        "'at' expects a list or map.",
                        "at",
                        "bad access",
                        "Use `list at index` or `map at key`.",
                    )),
                    None => Ok(None),
                }
            }
            grammar::Expression::Push(list, _, value) => {
                let list_ty = self.infer_expr(list)?;
                let value_ty = self.infer_expr(value)?;
                match list_ty {
                    Some(grammar::KiroType::List(_, inner)) => {
                        if let Some(actual) = value_ty
                            && !same_type(&inner, &actual)
                        {
                            return Err(self.error_at_with_help(
                                ErrorCode::TypeError,
                                format!(
                                    "'push' value must be {}, got {}.",
                                    type_name(&inner),
                                    type_name(&actual)
                                ),
                                "push",
                                "wrong push value",
                                "Push a value whose type matches the list element type.",
                            ));
                        }
                    }
                    Some(_) => {
                        return Err(self.error_at_with_help(
                            ErrorCode::BadUse,
                            "'push' expects a list.",
                            "push",
                            "bad push",
                            "Use `list push value` where the left expression is a list.",
                        ));
                    }
                    None => {}
                }
                Ok(Some(grammar::KiroType::Void))
            }
            grammar::Expression::Take(_, channel) => {
                if self.in_pure {
                    return Err(self.error_at(
                        ErrorCode::PureViolation,
                        "Pure Function Error: 'take' is forbidden.",
                        "take",
                        "forbidden in pure fn",
                    ));
                }
                match self.infer_expr(channel)? {
                    Some(grammar::KiroType::Pipe(_, inner)) => Ok(Some(*inner)),
                    Some(_) => Err(self.error_at_with_help(
                        ErrorCode::BadUse,
                        "'take' expects a pipe.",
                        "take",
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
            grammar::Expression::Deref(_, target) => {
                let target_ty = self.infer_expr(target)?;
                if matches!(
                    target_ty,
                    Some(grammar::KiroType::Adr(_, inner)) if matches!(*inner, grammar::KiroType::Void)
                ) {
                    return Err(self.error_at_with_help(
                        ErrorCode::BadUse,
                        "Cannot deref adr void.",
                        "deref",
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
                        return Err(self.error_at_last_with_help(
                            ErrorCode::WrongArgumentCount,
                            format!(
                                "Wrong argument count for '{}': expected 1, got {}.",
                                call_name,
                                args.len()
                            ),
                            &format!("{}(", call_name),
                            "wrong argument count",
                            format!("{} expects (value)", call_name),
                        ));
                    }
                    if self.in_pure {
                        return Err(self.error_at_last(
                            ErrorCode::PureViolation,
                            format!(
                                "Pure function cannot call impure/async function '{}' inside a pure function.",
                                call_name
                            ),
                            &format!("{}(", call_name),
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
                    return Err(self.error_at_last_with_help(
                        ErrorCode::WrongArgumentCount,
                        format!(
                            "Wrong argument count for '{}': expected {}, got {}.",
                            call_name,
                            info.params.len(),
                            args.len()
                        ),
                        &format!("{}(", call_name),
                        "wrong argument count",
                        help,
                    ));
                }
                if self.in_pure && !info.is_pure {
                    return Err(self.error_at_last(
                        ErrorCode::PureViolation,
                        format!(
                            "Pure function cannot call impure/async function '{}' inside a pure function.",
                            call_name
                        ),
                        &format!("{}(", call_name),
                        "impure call",
                    ));
                }
                for (idx, arg) in args.iter().enumerate() {
                    let actual = self.infer_expr(arg)?;
                    if let Some(actual) = actual {
                        let expected = &info.params[idx];
                        if !same_type(expected, &actual) {
                            return Err(self.error_at_last(
                                ErrorCode::TypeError,
                                format!(
                                    "Argument {} for '{}' must be {}, got {}.",
                                    idx + 1,
                                    call_name,
                                    type_name(expected),
                                    type_name(&actual)
                                ),
                                &format!("{}(", call_name),
                                "wrong argument type",
                            ));
                        }
                    }
                }
                Ok(info.return_type.clone())
            }
            grammar::Expression::RunCall(_, call) => {
                if let grammar::Expression::Call(func, _, args, _) = &**call {
                    let (_, info) = self.lookup_call(func)?;
                    if args.len() != info.params.len() {
                        return Err(self.error_at_last(
                            ErrorCode::WrongArgumentCount,
                            format!(
                                "Wrong argument count for '{}': expected {}, got {}.",
                                self.call_name(func),
                                info.params.len(),
                                args.len()
                            ),
                            "run",
                            "wrong argument count",
                        ));
                    }
                    for arg in args {
                        self.infer_expr(arg)?;
                    }
                } else {
                    return Err(self.error_at_last_with_help(
                        ErrorCode::BadUse,
                        "'run' expects a function call.",
                        "run",
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
            grammar::Expression::Variable(v) => self
                .functions
                .get(&v.value)
                .cloned()
                .map(|info| (v.value.clone(), info))
                .ok_or_else(|| {
                    let mut err = self.error_at_last(
                        ErrorCode::UnknownName,
                        format!("Unknown function '{}'.", v.value),
                        &format!("{}(", v.value),
                        "unknown function",
                    );
                    if let Some(suggestion) =
                        self.suggest_name(&v.value, self.visible_function_names())
                    {
                        err = err.with_suggestion(suggestion);
                    }
                    err
                }),
            grammar::Expression::FieldAccess(target, _, field) => {
                if let grammar::Expression::Variable(module) = &**target
                    && self.imports.contains(&module.value)
                {
                    return self
                        .module_functions
                        .get(&(module.value.clone(), field.value.clone()))
                        .cloned()
                        .map(|info| (format!("{}.{}", module.value, field.value), info))
                        .ok_or_else(|| {
                            let call_name = format!("{}.{}", module.value, field.value);
                            let mut err = self.error_at_last(
                                ErrorCode::ImportError,
                                format!("Unknown function '{}'.", call_name),
                                &call_name,
                                "unknown imported function",
                            );
                            if let Some(suggestion) = self.suggest_name(
                                &call_name,
                                self.imported_function_names(&module.value),
                            ) {
                                err = err.with_suggestion(suggestion);
                            }
                            err
                        });
                }
                Err(self.error(ErrorCode::UnknownName, "Unknown function target."))
            }
            _ => Err(self.error(ErrorCode::UnknownName, "Unknown function target.")),
        }
    }

    fn call_name(&self, func: &grammar::Expression) -> String {
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
        self.validate_effectful_recursion(program, module)?;
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
            def.name.value.clone(),
            def.fields
                .iter()
                .map(|field| (field.name.value.clone(), field.field_type.clone()))
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
        && crate::is_std_io_module_name(&module.value)
        && crate::is_std_io_display_function(&field.value)
    {
        return Some((&module.value, &field.value));
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
