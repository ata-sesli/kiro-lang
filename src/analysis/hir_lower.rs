use std::collections::{HashMap, HashSet};

use crate::grammar::{self, grammar as ast};

use crate::hir::{
    Effects, ErrorId, FieldId, FunctionId, HandleId, HirBinaryOp, HirCallKind, HirErrorClause,
    HirExpr, HirExprKind, HirFieldInit, HirFunction, HirHostFunction, HirMapPair, HirModule,
    HirParam, HirProgram, HirStmt, HirStmtKind, HirStruct, HirStructField, HirSymbols,
    HostFunctionId, LocalId, LocalSymbol, ModuleId, SemType, Signature, SourceAnchor, SourceId,
    StructId, TypeId, TypeTable,
};

pub(crate) struct HirModuleInput<'a> {
    pub name: &'a str,
    pub program: &'a ast::Program,
    pub source: SourceId,
}

#[derive(Clone)]
struct FunctionDecl {
    id: FunctionId,
    params: Vec<TypeId>,
    return_type: TypeId,
    effects: Effects,
}

#[derive(Clone)]
struct HostFunctionDecl {
    id: HostFunctionId,
    params: Vec<TypeId>,
    return_type: TypeId,
    effects: Effects,
}

#[derive(Default)]
struct Declarations {
    modules: HashMap<String, ModuleId>,
    functions: HashMap<(String, String), FunctionDecl>,
    host_functions: HashMap<(String, String), HostFunctionDecl>,
    structs: HashMap<(String, String), StructId>,
    handles: HashMap<(String, String), HandleId>,
    errors: HashMap<(String, String), ErrorId>,
    fields: HashMap<(StructId, String), (FieldId, TypeId)>,
}

pub(crate) fn lower_modules(inputs: &[HirModuleInput<'_>]) -> Result<HirProgram, String> {
    let mut ordered = inputs.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.name.cmp(right.name));

    let mut types = TypeTable::new();
    let mut symbols = HirSymbols::default();
    let mut declarations = Declarations::default();

    for (index, input) in ordered.iter().enumerate() {
        let id = ModuleId::try_from(index).map_err(|error| error.to_string())?;
        declarations.modules.insert(input.name.to_string(), id);
        symbols.modules.push(input.name.to_string());
    }

    predeclare_types(&ordered, &mut declarations, &mut symbols)?;
    predeclare_fields(&ordered, &mut declarations, &mut types, &mut symbols)?;
    predeclare_functions(&ordered, &mut declarations, &mut types, &mut symbols)?;

    let known_modules = declarations.modules.keys().cloned().collect::<HashSet<_>>();
    let mut modules = Vec::with_capacity(ordered.len());
    for input in ordered {
        let id = declarations.modules[input.name];
        let mut lowerer = ModuleLowerer::new(
            input.name,
            input.source,
            &known_modules,
            &declarations,
            &mut types,
            &mut symbols,
        );
        modules.push(lowerer.lower_module(id, input.program)?);
    }

    Ok(HirProgram {
        types,
        modules,
        symbols,
    })
}

fn predeclare_types(
    inputs: &[&HirModuleInput<'_>],
    declarations: &mut Declarations,
    symbols: &mut HirSymbols,
) -> Result<(), String> {
    for input in inputs {
        for statement in &input.program.statements {
            match declaration_item(statement) {
                Some(DeclarationItem::Struct(def)) => {
                    let name = grammar::struct_def_name(def).to_string();
                    let id = StructId::try_from(symbols.structs.len())
                        .map_err(|error| error.to_string())?;
                    declarations
                        .structs
                        .insert((input.name.to_string(), name.clone()), id);
                    symbols.structs.push(format!("{}.{}", input.name, name));
                }
                Some(DeclarationItem::Handle(def)) => {
                    let name = grammar::handle_name(def).to_string();
                    let id = HandleId::try_from(symbols.handles.len())
                        .map_err(|error| error.to_string())?;
                    declarations
                        .handles
                        .insert((input.name.to_string(), name.clone()), id);
                    symbols.handles.push(format!("{}.{}", input.name, name));
                }
                _ => {}
            }
            if let ast::Statement::ErrorDef { name, .. } = statement {
                let error_name = grammar::struct_name(name).to_string();
                let id =
                    ErrorId::try_from(symbols.errors.len()).map_err(|error| error.to_string())?;
                declarations
                    .errors
                    .insert((input.name.to_string(), error_name.clone()), id);
                symbols
                    .errors
                    .push(format!("{}.{}", input.name, error_name));
            }
        }
    }
    Ok(())
}

fn predeclare_fields(
    inputs: &[&HirModuleInput<'_>],
    declarations: &mut Declarations,
    types: &mut TypeTable,
    symbols: &mut HirSymbols,
) -> Result<(), String> {
    for input in inputs {
        for statement in &input.program.statements {
            let Some(DeclarationItem::Struct(def)) = declaration_item(statement) else {
                continue;
            };
            let struct_name = grammar::struct_def_name(def);
            let struct_id =
                declarations.structs[&(input.name.to_string(), struct_name.to_string())];
            for field in &def.fields {
                let name = grammar::field_def_name(field).to_string();
                let id =
                    FieldId::try_from(symbols.fields.len()).map_err(|error| error.to_string())?;
                let ty = lower_type(&field.field_type, input.name, declarations, types);
                declarations
                    .fields
                    .insert((struct_id, name.clone()), (id, ty));
                symbols
                    .fields
                    .push(format!("{}.{}.{}", input.name, struct_name, name));
            }
        }
    }
    Ok(())
}

fn predeclare_functions(
    inputs: &[&HirModuleInput<'_>],
    declarations: &mut Declarations,
    types: &mut TypeTable,
    symbols: &mut HirSymbols,
) -> Result<(), String> {
    for input in inputs {
        for statement in &input.program.statements {
            match declaration_item(statement) {
                Some(DeclarationItem::Function(def)) => {
                    let name = grammar::function_name(&def.name).to_string();
                    let id = FunctionId::try_from(symbols.functions.len())
                        .map_err(|error| error.to_string())?;
                    let params = def
                        .params
                        .iter()
                        .map(|param| {
                            lower_type(&param.command_type, input.name, declarations, types)
                        })
                        .collect();
                    let return_type = def
                        .return_type
                        .as_ref()
                        .map(|ty| lower_type(ty, input.name, declarations, types))
                        .unwrap_or(TypeId::VOID);
                    let mut effects = if def.pure_kw.is_some() {
                        Effects::PURE
                    } else {
                        Effects::NONE
                    };
                    if def.can_error.is_some() {
                        effects |= Effects::MAY_FAIL;
                    }
                    declarations.functions.insert(
                        (input.name.to_string(), name.clone()),
                        FunctionDecl {
                            id,
                            params,
                            return_type,
                            effects,
                        },
                    );
                    symbols.functions.push(format!("{}.{}", input.name, name));
                }
                Some(DeclarationItem::HostFunction(def)) => {
                    let name = grammar::function_name(&def.name).to_string();
                    let id = HostFunctionId::try_from(symbols.host_functions.len())
                        .map_err(|error| error.to_string())?;
                    let params = def
                        .params
                        .iter()
                        .map(|param| {
                            lower_type(&param.command_type, input.name, declarations, types)
                        })
                        .collect();
                    let return_type = lower_type(&def.return_type, input.name, declarations, types);
                    let mut effects = Effects::HOST_CALL;
                    if def.can_error.is_some() {
                        effects |= Effects::MAY_FAIL;
                    }
                    declarations.host_functions.insert(
                        (input.name.to_string(), name.clone()),
                        HostFunctionDecl {
                            id,
                            params,
                            return_type,
                            effects,
                        },
                    );
                    symbols
                        .host_functions
                        .push(format!("{}.{}", input.name, name));
                }
                _ => {}
            }
        }
        if crate::is_std_io_module_name(input.name) {
            for name in ["print", "write"] {
                let key = (input.name.to_string(), name.to_string());
                if declarations.host_functions.contains_key(&key) {
                    continue;
                }
                let id = HostFunctionId::try_from(symbols.host_functions.len())
                    .map_err(|error| error.to_string())?;
                declarations.host_functions.insert(
                    key,
                    HostFunctionDecl {
                        id,
                        params: vec![TypeId::UNKNOWN],
                        return_type: TypeId::VOID,
                        effects: Effects::HOST_CALL,
                    },
                );
                symbols
                    .host_functions
                    .push(format!("{}.{}", input.name, name));
            }
        }
    }
    Ok(())
}

enum DeclarationItem<'a> {
    Struct(&'a ast::StructDef),
    Handle(&'a ast::HandleDef),
    Function(&'a ast::FunctionDef),
    HostFunction(&'a ast::RustFnDecl),
}

fn declaration_item(statement: &ast::Statement) -> Option<DeclarationItem<'_>> {
    match statement {
        ast::Statement::StructDef(def) => Some(DeclarationItem::Struct(def)),
        ast::Statement::HandleDef(def) => Some(DeclarationItem::Handle(def)),
        ast::Statement::FunctionDef(def) => Some(DeclarationItem::Function(def)),
        ast::Statement::RustFnDecl(def) => Some(DeclarationItem::HostFunction(def)),
        ast::Statement::Documented { item, .. } => match item {
            ast::AnnotatableItem::StructDef(def) => Some(DeclarationItem::Struct(def)),
            ast::AnnotatableItem::HandleDef(def) => Some(DeclarationItem::Handle(def)),
            ast::AnnotatableItem::FunctionDef(def) => Some(DeclarationItem::Function(def)),
            ast::AnnotatableItem::RustFnDecl(def) => Some(DeclarationItem::HostFunction(def)),
        },
        _ => None,
    }
}

fn lower_type(
    ty: &ast::KiroType,
    module: &str,
    declarations: &Declarations,
    types: &mut TypeTable,
) -> TypeId {
    match ty {
        ast::KiroType::Num => TypeId::NUM,
        ast::KiroType::Str => TypeId::STR,
        ast::KiroType::Bool => TypeId::BOOL,
        ast::KiroType::Void => TypeId::VOID,
        ast::KiroType::Adr(_, inner) => {
            let inner = lower_type(inner, module, declarations, types);
            types.intern(SemType::Address(inner))
        }
        ast::KiroType::Pipe(_, inner) => {
            let inner = lower_type(inner, module, declarations, types);
            types.intern(SemType::Pipe(inner))
        }
        ast::KiroType::List(_, inner) => {
            let inner = lower_type(inner, module, declarations, types);
            types.intern(SemType::List(inner))
        }
        ast::KiroType::Map(_, key, value) => {
            let key = lower_type(key, module, declarations, types);
            let value = lower_type(value, module, declarations, types);
            types.intern(SemType::Map(key, value))
        }
        ast::KiroType::FnType(_, _, params, _, _, return_type) => {
            let params = params
                .iter()
                .map(|param| lower_type(param, module, declarations, types))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let return_type = lower_type(return_type, module, declarations, types);
            types.intern(SemType::Function {
                params,
                return_type,
            })
        }
        ast::KiroType::Custom(name) => {
            let key = (module.to_string(), name.value.clone());
            if let Some(id) = declarations.structs.get(&key) {
                types.intern(SemType::Struct(*id))
            } else if let Some(id) = declarations.handles.get(&key) {
                types.intern(SemType::Handle(*id))
            } else {
                TypeId::UNKNOWN
            }
        }
    }
}

struct ModuleLowerer<'a> {
    module: &'a str,
    source: SourceId,
    known_modules: &'a HashSet<String>,
    declarations: &'a Declarations,
    types: &'a mut TypeTable,
    symbols: &'a mut HirSymbols,
    imports: HashMap<String, String>,
    scopes: Vec<HashMap<String, (LocalId, TypeId)>>,
    next_local: u32,
    owner: Option<FunctionId>,
    effects: Effects,
}

impl<'a> ModuleLowerer<'a> {
    fn new(
        module: &'a str,
        source: SourceId,
        known_modules: &'a HashSet<String>,
        declarations: &'a Declarations,
        types: &'a mut TypeTable,
        symbols: &'a mut HirSymbols,
    ) -> Self {
        Self {
            module,
            source,
            known_modules,
            declarations,
            types,
            symbols,
            imports: HashMap::new(),
            scopes: vec![HashMap::new()],
            next_local: 0,
            owner: None,
            effects: Effects::NONE,
        }
    }

    fn lower_module(&mut self, id: ModuleId, program: &ast::Program) -> Result<HirModule, String> {
        self.collect_imports(program);
        self.owner = None;
        self.next_local = 0;
        self.scopes = vec![HashMap::new()];
        let statements = self.lower_statements(&program.statements)?;
        let mut functions = Vec::new();
        let mut host_functions = Vec::new();
        let mut structs = Vec::new();
        for statement in &program.statements {
            match declaration_item(statement) {
                Some(DeclarationItem::Function(def)) => functions.push(self.lower_function(def)?),
                Some(DeclarationItem::HostFunction(def)) => {
                    host_functions.push(self.lower_host_function(def)?)
                }
                Some(DeclarationItem::Struct(def)) => structs.push(self.lower_struct(def)),
                _ => {}
            }
        }
        if crate::is_std_io_module_name(self.module) {
            for name in ["print", "write"] {
                let declaration = self.declarations.host_functions
                    [&(self.module.to_string(), name.to_string())]
                    .clone();
                host_functions.push(HirHostFunction {
                    id: declaration.id,
                    name: name.to_string(),
                    params: Vec::new(),
                    signature: Signature::new(
                        declaration.params,
                        declaration.return_type,
                        declaration.effects,
                    ),
                    anchor: self.anchor((0, 0)),
                });
            }
            host_functions.sort_by_key(|function| function.id);
        }
        Ok(HirModule::new(
            id,
            self.module.to_string(),
            statements,
            functions,
            host_functions,
            structs,
        ))
    }

    fn collect_imports(&mut self, program: &ast::Program) {
        for statement in &program.statements {
            if let ast::Statement::Import { module_name, .. } = statement {
                let written = grammar::module_path_name(module_name).to_string();
                let canonical = grammar::resolve_relative_module_path(
                    &written,
                    self.module,
                    self.known_modules,
                );
                self.imports.insert(written, canonical);
            }
        }
    }

    fn lower_function(&mut self, def: &ast::FunctionDef) -> Result<HirFunction, String> {
        let name = grammar::function_name(&def.name).to_string();
        let declaration =
            self.declarations.functions[&(self.module.to_string(), name.clone())].clone();
        let global_scope = self.scopes.first().cloned().unwrap_or_default();
        let first_function_local = u32::try_from(global_scope.len())
            .map_err(|_| "module global ID space exhausted".to_string())?;
        let saved_scopes = std::mem::replace(&mut self.scopes, vec![global_scope, HashMap::new()]);
        let saved_next_local = std::mem::replace(&mut self.next_local, first_function_local);
        let saved_owner = self.owner.replace(declaration.id);
        let saved_effects = std::mem::replace(&mut self.effects, declaration.effects);

        let mut params = Vec::with_capacity(def.params.len());
        for (param, ty) in def.params.iter().zip(&declaration.params) {
            let local = self.declare_local(grammar::param_name(param), *ty)?;
            params.push(HirParam {
                local,
                ty: *ty,
                anchor: self.anchor(grammar::param_name_span(param)),
            });
        }
        let body = self.lower_statements(&def.body.statements)?;
        let effects = self.effects;

        self.scopes = saved_scopes;
        self.next_local = saved_next_local;
        self.owner = saved_owner;
        self.effects = saved_effects;

        Ok(HirFunction {
            id: declaration.id,
            name,
            params,
            signature: Signature::new(declaration.params, declaration.return_type, effects),
            body,
            anchor: self.anchor(grammar::function_span(&def.name)),
        })
    }

    fn lower_host_function(&mut self, def: &ast::RustFnDecl) -> Result<HirHostFunction, String> {
        let name = grammar::function_name(&def.name).to_string();
        let declaration =
            self.declarations.host_functions[&(self.module.to_string(), name.clone())].clone();
        let saved_owner = self.owner.take();
        let saved_next_local = std::mem::replace(&mut self.next_local, 0);
        let saved_scopes = std::mem::replace(&mut self.scopes, vec![HashMap::new()]);
        let mut params = Vec::with_capacity(def.params.len());
        for (param, ty) in def.params.iter().zip(&declaration.params) {
            let local = self.declare_local(grammar::param_name(param), *ty)?;
            params.push(HirParam {
                local,
                ty: *ty,
                anchor: self.anchor(grammar::param_name_span(param)),
            });
        }
        self.owner = saved_owner;
        self.next_local = saved_next_local;
        self.scopes = saved_scopes;
        Ok(HirHostFunction {
            id: declaration.id,
            name,
            params,
            signature: Signature::new(
                declaration.params,
                declaration.return_type,
                declaration.effects,
            ),
            anchor: self.anchor(grammar::rust_fn_decl_span(def)),
        })
    }

    fn lower_struct(&mut self, def: &ast::StructDef) -> HirStruct {
        let name = grammar::struct_def_name(def).to_string();
        let id = self.declarations.structs[&(self.module.to_string(), name)];
        let fields = def
            .fields
            .iter()
            .map(|field| {
                let field_name = grammar::field_def_name(field).to_string();
                let (field_id, ty) = self.declarations.fields[&(id, field_name)];
                HirStructField {
                    id: field_id,
                    ty,
                    anchor: self.anchor(grammar::field_def_span(field)),
                }
            })
            .collect();
        HirStruct {
            id,
            fields,
            anchor: self.anchor(grammar::struct_def_span(def)),
        }
    }

    fn lower_statements(&mut self, statements: &[ast::Statement]) -> Result<Vec<HirStmt>, String> {
        statements
            .iter()
            .map(|statement| self.lower_statement(statement))
            .collect()
    }

    fn lower_statement(&mut self, statement: &ast::Statement) -> Result<HirStmt, String> {
        let (kind, span) = match statement {
            ast::Statement::ErrorDef {
                name, description, ..
            } => {
                let name_text = grammar::struct_name(name).to_string();
                let id = self.declarations.errors[&(self.module.to_string(), name_text)];
                (
                    HirStmtKind::ErrorDef {
                        id,
                        description: description
                            .as_ref()
                            .map(|value| strip_quotes(&value.value.value)),
                    },
                    name.span,
                )
            }
            ast::Statement::StructDef(def) => {
                let name = grammar::struct_def_name(def).to_string();
                (
                    HirStmtKind::StructDef(
                        self.declarations.structs[&(self.module.to_string(), name)],
                    ),
                    grammar::struct_def_span(def),
                )
            }
            ast::Statement::HandleDef(def) => {
                let name = grammar::handle_name(def).to_string();
                (
                    HirStmtKind::HandleDef(
                        self.declarations.handles[&(self.module.to_string(), name)],
                    ),
                    grammar::handle_span(def),
                )
            }
            ast::Statement::VarDecl { ident, value, .. } => {
                let value = self.lower_expr(value)?;
                let local = self.declare_local(grammar::variable_name(ident), value.ty)?;
                (
                    HirStmtKind::VarDecl { local, value },
                    grammar::variable_span(ident),
                )
            }
            ast::Statement::AssignStmt { lhs, rhs, .. } => {
                let value = self.lower_expr(rhs)?;
                if let ast::Expression::Variable(variable) = lhs
                    && self
                        .resolve_local(grammar::variable_name(variable))
                        .is_none()
                {
                    self.declare_local(grammar::variable_name(variable), value.ty)?;
                }
                let target = self.lower_expr(lhs)?;
                let span = merged_span(grammar::expr_span(lhs), grammar::expr_span(rhs));
                (HirStmtKind::Assign { target, value }, span)
            }
            ast::Statement::On {
                condition,
                body,
                else_clause,
                error_clauses,
                ..
            } => {
                let condition = self.lower_expr(condition)?;
                let body = self.lower_block(&body.statements)?;
                let else_body = else_clause
                    .as_ref()
                    .map(|clause| self.lower_block(&clause.body.statements))
                    .transpose()?;
                let error_clauses = error_clauses
                    .as_ref()
                    .map(|clauses| self.lower_error_clauses(clauses))
                    .transpose()?
                    .unwrap_or_default();
                if !error_clauses.is_empty() {
                    self.effects |= Effects::MAY_FAIL;
                }
                let span = grammar::expr_span(condition_source(statement)).unwrap_or((0, 0));
                (
                    HirStmtKind::On {
                        condition,
                        body,
                        else_body,
                        error_clauses,
                    },
                    span,
                )
            }
            ast::Statement::LoopOn {
                condition, body, ..
            } => {
                let span = grammar::expr_span(condition).unwrap_or((0, 0));
                let condition = self.lower_expr(condition)?;
                let body = self.lower_block(&body.statements)?;
                (HirStmtKind::LoopOn { condition, body }, span)
            }
            ast::Statement::LoopIter {
                iterator,
                iterable,
                step,
                filter,
                body,
                else_clause,
                ..
            } => {
                let iterable = self.lower_expr(iterable)?;
                let iterator_ty = match self.types.get(iterable.ty) {
                    Some(SemType::List(inner)) => *inner,
                    Some(SemType::Map(key, _)) => *key,
                    Some(SemType::Range) => TypeId::NUM,
                    _ => TypeId::UNKNOWN,
                };
                self.push_scope();
                let iterator_id =
                    self.declare_local(grammar::variable_name(iterator), iterator_ty)?;
                let step = step
                    .as_ref()
                    .map(|step| self.lower_expr(&step.value))
                    .transpose()?;
                let filter = filter
                    .as_ref()
                    .map(|filter| self.lower_expr(&filter.condition))
                    .transpose()?;
                let body = self.lower_statements(&body.statements)?;
                let else_body = else_clause
                    .as_ref()
                    .map(|clause| self.lower_block(&clause.body.statements))
                    .transpose()?;
                self.pop_scope();
                (
                    HirStmtKind::LoopIter {
                        iterator: iterator_id,
                        iterable,
                        step,
                        filter,
                        body,
                        else_body,
                    },
                    grammar::variable_span(iterator),
                )
            }
            ast::Statement::FunctionDef(def) => {
                let name = grammar::function_name(&def.name).to_string();
                (
                    HirStmtKind::FunctionDef(
                        self.declarations.functions[&(self.module.to_string(), name)].id,
                    ),
                    grammar::function_span(&def.name),
                )
            }
            ast::Statement::RustFnDecl(def) => {
                let name = grammar::function_name(&def.name).to_string();
                (
                    HirStmtKind::HostFunctionDecl(
                        self.declarations.host_functions[&(self.module.to_string(), name)].id,
                    ),
                    grammar::rust_fn_decl_span(def),
                )
            }
            ast::Statement::Give(keyword, channel, value) => {
                self.effects |= Effects::MAY_BLOCK;
                (
                    HirStmtKind::Give {
                        channel: self.lower_expr(channel)?,
                        value: self.lower_expr(value)?,
                    },
                    keyword.span,
                )
            }
            ast::Statement::Close(keyword, channel) => {
                (HirStmtKind::Close(self.lower_expr(channel)?), keyword.span)
            }
            ast::Statement::Return(keyword, value) => (
                HirStmtKind::Return(
                    value
                        .as_ref()
                        .map(|value| self.lower_expr(value))
                        .transpose()?,
                ),
                keyword.span,
            ),
            ast::Statement::Break(keyword) => (HirStmtKind::Break, keyword.span),
            ast::Statement::Continue(keyword) => (HirStmtKind::Continue, keyword.span),
            ast::Statement::Rest(keyword) => {
                self.effects |= Effects::MAY_BLOCK;
                (HirStmtKind::Rest, keyword.span)
            }
            ast::Statement::Check(keyword, condition, message) => {
                self.effects |= Effects::MAY_FAIL;
                (
                    HirStmtKind::Check {
                        condition: self.lower_expr(condition)?,
                        message: message
                            .as_ref()
                            .map(|message| strip_quotes(&message.value.value)),
                    },
                    keyword.span,
                )
            }
            ast::Statement::Import { module_name, .. } => {
                let written = grammar::module_path_name(module_name);
                let canonical = self
                    .imports
                    .get(written)
                    .ok_or_else(|| format!("HIR import '{}' was not predeclared", written))?;
                (
                    HirStmtKind::Import(self.declarations.modules[canonical]),
                    grammar::module_path_span(module_name),
                )
            }
            ast::Statement::ExprStmt(expression) => {
                let span = grammar::expr_span(expression).unwrap_or((0, 0));
                (HirStmtKind::Expr(self.lower_expr(expression)?), span)
            }
            ast::Statement::Documented { item, .. } => {
                return self.lower_annotatable(item);
            }
        };
        Ok(HirStmt {
            kind,
            anchor: self.anchor(span),
        })
    }

    fn lower_annotatable(&mut self, item: &ast::AnnotatableItem) -> Result<HirStmt, String> {
        let (kind, span) = match item {
            ast::AnnotatableItem::HandleDef(def) => {
                let name = grammar::handle_name(def).to_string();
                (
                    HirStmtKind::HandleDef(
                        self.declarations.handles[&(self.module.to_string(), name)],
                    ),
                    grammar::handle_span(def),
                )
            }
            ast::AnnotatableItem::StructDef(def) => {
                let name = grammar::struct_def_name(def).to_string();
                (
                    HirStmtKind::StructDef(
                        self.declarations.structs[&(self.module.to_string(), name)],
                    ),
                    grammar::struct_def_span(def),
                )
            }
            ast::AnnotatableItem::FunctionDef(def) => {
                let name = grammar::function_name(&def.name).to_string();
                (
                    HirStmtKind::FunctionDef(
                        self.declarations.functions[&(self.module.to_string(), name)].id,
                    ),
                    grammar::function_span(&def.name),
                )
            }
            ast::AnnotatableItem::RustFnDecl(def) => {
                let name = grammar::function_name(&def.name).to_string();
                (
                    HirStmtKind::HostFunctionDecl(
                        self.declarations.host_functions[&(self.module.to_string(), name)].id,
                    ),
                    grammar::rust_fn_decl_span(def),
                )
            }
        };
        Ok(HirStmt {
            kind,
            anchor: self.anchor(span),
        })
    }

    fn lower_block(&mut self, statements: &[ast::Statement]) -> Result<Vec<HirStmt>, String> {
        self.push_scope();
        let result = self.lower_statements(statements);
        self.pop_scope();
        result
    }

    fn lower_error_clauses(
        &mut self,
        clauses: &ast::ErrorClauseList,
    ) -> Result<Vec<HirErrorClause>, String> {
        let mut output = Vec::new();
        self.append_error_clauses(clauses, &mut output)?;
        Ok(output)
    }

    fn append_error_clauses(
        &mut self,
        clauses: &ast::ErrorClauseList,
        output: &mut Vec<HirErrorClause>,
    ) -> Result<(), String> {
        let error = clauses.first.error_type.as_ref().and_then(|name| {
            self.declarations
                .errors
                .get(&(self.module.to_string(), name.value.clone()))
                .copied()
                .or_else(|| {
                    self.declarations
                        .errors
                        .iter()
                        .find(|((_, candidate), _)| candidate == &name.value)
                        .map(|(_, id)| *id)
                })
        });
        output.push(HirErrorClause {
            error,
            body: self.lower_block(&clauses.first.body.statements)?,
        });
        if let Some(rest) = &clauses.rest {
            self.append_error_clauses(rest, output)?;
        }
        Ok(())
    }

    fn lower_expr(&mut self, expression: &ast::Expression) -> Result<HirExpr, String> {
        let span = grammar::expr_span(expression).unwrap_or((0, 0));
        let anchor = self.anchor(span);
        let (kind, ty) = match expression {
            ast::Expression::BoolLit(value) => {
                let value = matches!(value.value, ast::BoolVal::True(_));
                (HirExprKind::Bool(value), TypeId::BOOL)
            }
            ast::Expression::Number(value) => (
                HirExprKind::Number(value.value.parse().unwrap_or(0.0)),
                TypeId::NUM,
            ),
            ast::Expression::StringLit(value) => {
                (HirExprKind::String(strip_quotes(&value.value)), TypeId::STR)
            }
            ast::Expression::Variable(variable) => {
                self.lower_name(grammar::variable_name(variable))?
            }
            ast::Expression::MoveExpr(_, variable) => {
                let name = grammar::variable_name(variable);
                let (local, ty) = self
                    .resolve_local(name)
                    .ok_or_else(|| format!("unresolved move local '{name}'"))?;
                (HirExprKind::Move(local), ty)
            }
            ast::Expression::ErrorRef(name) => {
                let text = grammar::struct_name(name).to_string();
                let id = self.declarations.errors[&(self.module.to_string(), text)];
                let ty = self.types.intern(SemType::Error(id));
                (HirExprKind::Error(id), ty)
            }
            ast::Expression::AdrInit(_, inner) => {
                let inner = lower_type(inner, self.module, self.declarations, self.types);
                (
                    HirExprKind::AddressInit,
                    self.types.intern(SemType::Address(inner)),
                )
            }
            ast::Expression::PipeInit(_, inner, capacity) => {
                let inner = lower_type(inner, self.module, self.declarations, self.types);
                (
                    HirExprKind::PipeInit {
                        capacity: capacity.as_ref().and_then(|value| value.value.parse().ok()),
                    },
                    self.types.intern(SemType::Pipe(inner)),
                )
            }
            ast::Expression::StructInit(name, _, fields, _) => {
                let name_text = grammar::struct_name(name).to_string();
                let structure = self.declarations.structs[&(self.module.to_string(), name_text)];
                let fields = fields
                    .iter()
                    .map(|field| {
                        let name = grammar::field_name(&field.name).to_string();
                        let field_id = self.declarations.fields[&(structure, name)].0;
                        Ok(HirFieldInit {
                            field: field_id,
                            value: self.lower_expr(&field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                (
                    HirExprKind::StructInit { structure, fields },
                    self.types.intern(SemType::Struct(structure)),
                )
            }
            ast::Expression::ListInit(_, inner, _, items, _) => {
                let inner = lower_type(inner, self.module, self.declarations, self.types);
                let items = items
                    .iter()
                    .map(|item| self.lower_expr(item))
                    .collect::<Result<Vec<_>, _>>()?;
                (
                    HirExprKind::ListInit(items),
                    self.types.intern(SemType::List(inner)),
                )
            }
            ast::Expression::MapInit(_, key, value, _, pairs, _) => {
                let key = lower_type(key, self.module, self.declarations, self.types);
                let value = lower_type(value, self.module, self.declarations, self.types);
                let pairs = pairs
                    .iter()
                    .map(|pair| {
                        Ok(HirMapPair {
                            key: self.lower_expr(&pair.key)?,
                            value: self.lower_expr(&pair.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                (
                    HirExprKind::MapInit(pairs),
                    self.types.intern(SemType::Map(key, value)),
                )
            }
            ast::Expression::FieldAccess(target, _, field) => {
                if let Some((kind, ty)) = self.resolve_qualified_function(expression) {
                    (kind, ty)
                } else {
                    let target = self.lower_expr(target)?;
                    let structure = match self.types.get(target.ty) {
                        Some(SemType::Struct(id)) => Some(*id),
                        Some(SemType::Address(inner)) => match self.types.get(*inner) {
                            Some(SemType::Struct(id)) => Some(*id),
                            _ => None,
                        },
                        _ => None,
                    };
                    let (field_id, field_ty) = structure
                        .and_then(|id| {
                            self.declarations
                                .fields
                                .get(&(id, grammar::field_name(field).to_string()))
                                .copied()
                        })
                        .unwrap_or((FieldId::new(0), TypeId::UNKNOWN));
                    (
                        HirExprKind::FieldAccess {
                            target: Box::new(target),
                            field: field_id,
                        },
                        field_ty,
                    )
                }
            }
            ast::Expression::At(collection, _, key) => {
                let collection = self.lower_expr(collection)?;
                let key = self.lower_expr(key)?;
                let ty = match self.types.get(collection.ty) {
                    Some(SemType::List(inner)) => *inner,
                    Some(SemType::Map(_, value)) => *value,
                    _ => TypeId::UNKNOWN,
                };
                (
                    HirExprKind::At {
                        collection: Box::new(collection),
                        key: Box::new(key),
                    },
                    ty,
                )
            }
            ast::Expression::Push(collection, _, value) => (
                HirExprKind::Push {
                    collection: Box::new(self.lower_expr(collection)?),
                    value: Box::new(self.lower_expr(value)?),
                },
                TypeId::VOID,
            ),
            ast::Expression::Take(_, target) => {
                self.effects |= Effects::MAY_BLOCK;
                let target = self.lower_expr(target)?;
                let ty = match self.types.get(target.ty) {
                    Some(SemType::Pipe(inner)) => *inner,
                    _ => TypeId::UNKNOWN,
                };
                (HirExprKind::Take(Box::new(target)), ty)
            }
            ast::Expression::Len(_, target) => (
                HirExprKind::Len(Box::new(self.lower_expr(target)?)),
                TypeId::NUM,
            ),
            ast::Expression::Ref(_, target) => {
                let target = self.lower_expr(target)?;
                if matches!(
                    target.kind,
                    HirExprKind::Function(_) | HirExprKind::HostFunction(_)
                ) {
                    (target.kind, target.ty)
                } else {
                    let ty = self.types.intern(SemType::Address(target.ty));
                    (HirExprKind::Ref(Box::new(target)), ty)
                }
            }
            ast::Expression::Deref(_, target) => {
                let target = self.lower_expr(target)?;
                let ty = match self.types.get(target.ty) {
                    Some(SemType::Address(inner)) => *inner,
                    _ => TypeId::UNKNOWN,
                };
                (HirExprKind::Deref(Box::new(target)), ty)
            }
            ast::Expression::Call(target, _, args, _) => {
                let target = self.lower_expr(target)?;
                let args = args
                    .iter()
                    .map(|argument| self.lower_expr(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                let (kind, return_type, effects) = match target.kind {
                    HirExprKind::Function(id) => {
                        let declaration = self
                            .declarations
                            .functions
                            .values()
                            .find(|declaration| declaration.id == id)
                            .expect("predeclared function ID");
                        (
                            HirCallKind::Direct(id),
                            declaration.return_type,
                            declaration.effects,
                        )
                    }
                    HirExprKind::HostFunction(id) => {
                        let declaration = self
                            .declarations
                            .host_functions
                            .values()
                            .find(|declaration| declaration.id == id)
                            .expect("predeclared host function ID");
                        (
                            HirCallKind::Host(id),
                            declaration.return_type,
                            declaration.effects,
                        )
                    }
                    _ => {
                        let return_type = match self.types.get(target.ty) {
                            Some(SemType::Function { return_type, .. }) => *return_type,
                            _ => TypeId::UNKNOWN,
                        };
                        (
                            HirCallKind::Indirect(Box::new(target)),
                            return_type,
                            Effects::INDIRECT_CALL,
                        )
                    }
                };
                self.effects |= effects;
                (HirExprKind::Call { kind, args }, return_type)
            }
            ast::Expression::RunCall(_, target) => {
                self.effects |= Effects::MAY_SPAWN;
                (
                    HirExprKind::RunCall(Box::new(self.lower_expr(target)?)),
                    TypeId::VOID,
                )
            }
            ast::Expression::Add(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Add)?,
            ast::Expression::Sub(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Sub)?,
            ast::Expression::Mul(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Mul)?,
            ast::Expression::Div(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Div)?,
            ast::Expression::Eq(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Eq)?,
            ast::Expression::Neq(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Ne)?,
            ast::Expression::Gt(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Gt)?,
            ast::Expression::Lt(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Lt)?,
            ast::Expression::Geq(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Ge)?,
            ast::Expression::Leq(lhs, _, rhs) => self.lower_binary(lhs, rhs, BinarySyntax::Le)?,
            ast::Expression::Range(lhs, _, rhs) => {
                self.lower_binary(lhs, rhs, BinarySyntax::Range)?
            }
        };
        Ok(HirExpr { kind, ty, anchor })
    }

    fn lower_binary(
        &mut self,
        lhs: &ast::Expression,
        rhs: &ast::Expression,
        syntax: BinarySyntax,
    ) -> Result<(HirExprKind, TypeId), String> {
        let lhs = self.lower_expr(lhs)?;
        let rhs = self.lower_expr(rhs)?;
        let (op, ty) = match syntax {
            BinarySyntax::Add if lhs.ty == TypeId::STR || rhs.ty == TypeId::STR => {
                (HirBinaryOp::ConcatString, TypeId::STR)
            }
            BinarySyntax::Add => (HirBinaryOp::AddNum, TypeId::NUM),
            BinarySyntax::Sub => (HirBinaryOp::SubNum, TypeId::NUM),
            BinarySyntax::Mul => (HirBinaryOp::MulNum, TypeId::NUM),
            BinarySyntax::Div => (HirBinaryOp::DivNum, TypeId::NUM),
            BinarySyntax::Eq => (equality_op(lhs.ty, false), TypeId::BOOL),
            BinarySyntax::Ne => (equality_op(lhs.ty, true), TypeId::BOOL),
            BinarySyntax::Gt => (HirBinaryOp::GtNum, TypeId::BOOL),
            BinarySyntax::Lt => (HirBinaryOp::LtNum, TypeId::BOOL),
            BinarySyntax::Ge => (HirBinaryOp::GeNum, TypeId::BOOL),
            BinarySyntax::Le => (HirBinaryOp::LeNum, TypeId::BOOL),
            BinarySyntax::Range => (HirBinaryOp::RangeNum, self.types.intern(SemType::Range)),
        };
        Ok((
            HirExprKind::Binary {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            },
            ty,
        ))
    }

    fn lower_name(&mut self, name: &str) -> Result<(HirExprKind, TypeId), String> {
        if let Some((local, ty)) = self.resolve_local(name) {
            return Ok((HirExprKind::Local(local), ty));
        }
        let key = (self.module.to_string(), name.to_string());
        if let Some(declaration) = self.declarations.functions.get(&key) {
            let ty = self.types.intern(SemType::Function {
                params: declaration.params.clone().into_boxed_slice(),
                return_type: declaration.return_type,
            });
            return Ok((HirExprKind::Function(declaration.id), ty));
        }
        if let Some(declaration) = self.declarations.host_functions.get(&key) {
            let ty = self.types.intern(SemType::Function {
                params: declaration.params.clone().into_boxed_slice(),
                return_type: declaration.return_type,
            });
            return Ok((HirExprKind::HostFunction(declaration.id), ty));
        }
        if let Some(module) = self.imports.get(name) {
            return Ok((
                HirExprKind::Module(self.declarations.modules[module]),
                TypeId::UNKNOWN,
            ));
        }
        Err(format!("unresolved HIR name '{name}'"))
    }

    fn resolve_qualified_function(
        &mut self,
        expression: &ast::Expression,
    ) -> Option<(HirExprKind, TypeId)> {
        let segments = grammar::expression_path_segments(expression)?;
        if segments.len() < 2 {
            return None;
        }
        let function = segments.last()?.clone();
        let written_module = segments[..segments.len() - 1].join(".");
        let module = self.imports.get(&written_module)?;
        let key = (module.clone(), function);
        if let Some(declaration) = self.declarations.functions.get(&key) {
            let ty = self.types.intern(SemType::Function {
                params: declaration.params.clone().into_boxed_slice(),
                return_type: declaration.return_type,
            });
            Some((HirExprKind::Function(declaration.id), ty))
        } else {
            let declaration = self.declarations.host_functions.get(&key)?;
            let ty = self.types.intern(SemType::Function {
                params: declaration.params.clone().into_boxed_slice(),
                return_type: declaration.return_type,
            });
            Some((HirExprKind::HostFunction(declaration.id), ty))
        }
    }

    fn declare_local(&mut self, name: &str, ty: TypeId) -> Result<LocalId, String> {
        let id = LocalId::new(self.next_local);
        self.next_local = self
            .next_local
            .checked_add(1)
            .ok_or_else(|| "local ID space exhausted".to_string())?;
        self.scopes
            .last_mut()
            .expect("lowerer always has a scope")
            .insert(name.to_string(), (id, ty));
        self.symbols.locals.push(LocalSymbol {
            owner: self.owner,
            id,
            name: name.to_string(),
        });
        Ok(id)
    }

    fn resolve_local(&self, name: &str) -> Option<(LocalId, TypeId)> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn anchor(&self, span: grammar::AstSpan) -> SourceAnchor {
        SourceAnchor::try_from_offsets(self.source, span.0, span.1)
            .expect("parser source offsets must fit the semantic anchor range")
    }
}

#[derive(Clone, Copy)]
enum BinarySyntax {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    Range,
}

fn equality_op(ty: TypeId, negated: bool) -> HirBinaryOp {
    match (ty, negated) {
        (TypeId::STR, false) => HirBinaryOp::EqString,
        (TypeId::STR, true) => HirBinaryOp::NeString,
        (TypeId::BOOL, false) => HirBinaryOp::EqBool,
        (TypeId::BOOL, true) => HirBinaryOp::NeBool,
        (_, false) => HirBinaryOp::EqNum,
        (_, true) => HirBinaryOp::NeNum,
    }
}

fn merged_span(
    left: Option<grammar::AstSpan>,
    right: Option<grammar::AstSpan>,
) -> grammar::AstSpan {
    match (left, right) {
        (Some(left), Some(right)) => (left.0, right.1),
        (Some(span), None) | (None, Some(span)) => span,
        (None, None) => (0, 0),
    }
}

fn condition_source(statement: &ast::Statement) -> &ast::Expression {
    match statement {
        ast::Statement::On { condition, .. } => condition,
        _ => unreachable!("condition_source only accepts on statements"),
    }
}

fn strip_quotes(value: &str) -> String {
    value.trim_matches('"').to_string()
}
