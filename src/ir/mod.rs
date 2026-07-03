use std::collections::HashMap;

use crate::grammar::{self, grammar as ast};

pub type IrSpan = Option<grammar::AstSpan>;

#[derive(Debug, Clone)]
pub struct IrModule {
    pub name: String,
    pub statements: Vec<IrStmt>,
    pub functions: HashMap<String, IrFunction>,
    pub rust_functions: HashMap<String, IrRustFunction>,
}

#[derive(Debug, Clone)]
pub struct IrSignature {
    pub params: Vec<IrParam>,
    pub return_type: Option<ast::KiroType>,
    pub can_error: bool,
    pub is_pure: bool,
}

#[derive(Debug, Clone)]
pub struct IrParam {
    pub name: String,
    pub ty: ast::KiroType,
    pub span: IrSpan,
}

#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub signature: IrSignature,
    pub body: Vec<IrStmt>,
    pub span: IrSpan,
}

#[derive(Debug, Clone)]
pub struct IrRustFunction {
    pub name: String,
    pub signature: IrSignature,
    pub span: IrSpan,
}

#[derive(Debug, Clone)]
pub struct IrStructField {
    pub name: String,
    pub ty: ast::KiroType,
    pub span: IrSpan,
}

#[derive(Debug, Clone)]
pub struct IrFieldInit {
    pub name: String,
    pub value: IrExpr,
    pub span: IrSpan,
}

#[derive(Debug, Clone)]
pub struct IrMapPair {
    pub key: IrExpr,
    pub value: IrExpr,
}

#[derive(Debug, Clone)]
pub struct IrErrorClause {
    pub error_type: Option<String>,
    pub body: Vec<IrStmt>,
}

#[derive(Debug, Clone)]
pub enum IrStmt {
    ErrorDef {
        name: String,
        description: Option<String>,
        span: IrSpan,
    },
    StructDef {
        name: String,
        fields: Vec<IrStructField>,
        span: IrSpan,
    },
    VarDecl {
        name: String,
        value: IrExpr,
        span: IrSpan,
    },
    Assign {
        lhs: IrExpr,
        rhs: IrExpr,
        span: IrSpan,
    },
    On {
        condition: IrExpr,
        body: Vec<IrStmt>,
        else_body: Option<Vec<IrStmt>>,
        error_clauses: Vec<IrErrorClause>,
        span: IrSpan,
    },
    LoopOn {
        condition: IrExpr,
        body: Vec<IrStmt>,
        span: IrSpan,
    },
    LoopIter {
        iterator: String,
        iterable: IrExpr,
        step: Option<IrExpr>,
        filter: Option<IrExpr>,
        body: Vec<IrStmt>,
        else_body: Option<Vec<IrStmt>>,
        span: IrSpan,
    },
    FunctionDef(IrFunction),
    RustFnDecl(IrRustFunction),
    Give {
        channel: IrExpr,
        value: IrExpr,
        span: IrSpan,
    },
    Close {
        channel: IrExpr,
        span: IrSpan,
    },
    Return {
        value: Option<IrExpr>,
        span: IrSpan,
    },
    Break {
        span: IrSpan,
    },
    Continue {
        span: IrSpan,
    },
    Rest {
        span: IrSpan,
    },
    Check {
        condition: IrExpr,
        message: Option<String>,
        span: IrSpan,
    },
    Import {
        module_name: String,
        span: IrSpan,
    },
    Expr(IrExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Eq,
    Neq,
    Gt,
    Lt,
    Geq,
    Leq,
    Range,
}

#[derive(Debug, Clone)]
pub enum IrExpr {
    StructInit {
        name: String,
        fields: Vec<IrFieldInit>,
        span: IrSpan,
    },
    ListInit {
        item_type: ast::KiroType,
        items: Vec<IrExpr>,
        span: IrSpan,
    },
    MapInit {
        key_type: ast::KiroType,
        value_type: ast::KiroType,
        pairs: Vec<IrMapPair>,
        span: IrSpan,
    },
    FieldAccess {
        target: Box<IrExpr>,
        field: String,
        span: IrSpan,
    },
    At {
        collection: Box<IrExpr>,
        key: Box<IrExpr>,
        span: IrSpan,
    },
    Push {
        collection: Box<IrExpr>,
        value: Box<IrExpr>,
        span: IrSpan,
    },
    Bool(bool, IrSpan),
    Number(f64, IrSpan),
    String(String, IrSpan),
    Variable(String, IrSpan),
    Move {
        name: String,
        span: IrSpan,
    },
    ErrorRef(String, IrSpan),
    AdrInit {
        ty: ast::KiroType,
        span: IrSpan,
    },
    PipeInit {
        ty: ast::KiroType,
        capacity: Option<usize>,
        span: IrSpan,
    },
    Take {
        target: Box<IrExpr>,
        span: IrSpan,
    },
    Len {
        target: Box<IrExpr>,
        span: IrSpan,
    },
    Ref {
        target: Box<IrExpr>,
        span: IrSpan,
    },
    Deref {
        target: Box<IrExpr>,
        span: IrSpan,
    },
    Call {
        target: Box<IrExpr>,
        args: Vec<IrExpr>,
        span: IrSpan,
    },
    RunCall {
        target: Box<IrExpr>,
        span: IrSpan,
    },
    Binary {
        op: IrBinaryOp,
        lhs: Box<IrExpr>,
        rhs: Box<IrExpr>,
        span: IrSpan,
    },
}

impl IrModule {
    pub fn lower(name: impl Into<String>, program: ast::Program) -> Self {
        let name = name.into();
        let statements = lower_statements(program.statements);
        let mut functions = HashMap::new();
        let mut rust_functions = HashMap::new();

        for stmt in &statements {
            match stmt {
                IrStmt::FunctionDef(func) => {
                    functions.insert(func.name.clone(), func.clone());
                }
                IrStmt::RustFnDecl(func) => {
                    rust_functions.insert(func.name.clone(), func.clone());
                }
                _ => {}
            }
        }

        Self {
            name,
            statements,
            functions,
            rust_functions,
        }
    }

    pub fn function(&self, name: &str) -> Option<&IrFunction> {
        self.functions.get(name)
    }

    pub fn rust_function(&self, name: &str) -> Option<&IrRustFunction> {
        self.rust_functions.get(name)
    }
}

fn lower_statements(statements: Vec<ast::Statement>) -> Vec<IrStmt> {
    statements.into_iter().map(lower_statement).collect()
}

fn lower_statement(stmt: ast::Statement) -> IrStmt {
    match stmt {
        ast::Statement::ErrorDef {
            name, description, ..
        } => IrStmt::ErrorDef {
            name: grammar::struct_name(&name).to_string(),
            description: description.map(|d| strip_quotes(&d.value.value)),
            span: Some(name.span),
        },
        ast::Statement::StructDef(def) => lower_struct(def),
        ast::Statement::VarDecl { ident, value, .. } => IrStmt::VarDecl {
            name: grammar::variable_name(&ident).to_string(),
            value: lower_expr(value),
            span: Some(ident.span),
        },
        ast::Statement::AssignStmt { lhs, rhs, .. } => {
            let span = merge_span(grammar::expr_span(&lhs), grammar::expr_span(&rhs));
            IrStmt::Assign {
                lhs: lower_expr(lhs),
                rhs: lower_expr(rhs),
                span,
            }
        }
        ast::Statement::On {
            condition,
            body,
            else_clause,
            error_clauses,
            ..
        } => IrStmt::On {
            condition: lower_expr(condition),
            body: lower_statements(body.statements),
            else_body: else_clause.map(|off| lower_statements(off.body.statements)),
            error_clauses: error_clauses.map(lower_error_clauses).unwrap_or_default(),
            span: None,
        },
        ast::Statement::LoopOn {
            condition, body, ..
        } => IrStmt::LoopOn {
            condition: lower_expr(condition),
            body: lower_statements(body.statements),
            span: None,
        },
        ast::Statement::LoopIter {
            iterator,
            iterable,
            step,
            filter,
            body,
            else_clause,
            ..
        } => IrStmt::LoopIter {
            iterator: grammar::variable_name(&iterator).to_string(),
            iterable: lower_expr(iterable),
            step: step.map(|s| lower_expr(s.value)),
            filter: filter.map(|f| lower_expr(f.condition)),
            body: lower_statements(body.statements),
            else_body: else_clause.map(|off| lower_statements(off.body.statements)),
            span: Some(iterator.span),
        },
        ast::Statement::FunctionDef(def) => IrStmt::FunctionDef(lower_function(def)),
        ast::Statement::RustFnDecl(def) => IrStmt::RustFnDecl(lower_rust_function(def)),
        ast::Statement::Give(keyword, channel, value) => IrStmt::Give {
            channel: lower_expr(channel),
            value: lower_expr(value),
            span: Some(keyword.span),
        },
        ast::Statement::Close(keyword, channel) => IrStmt::Close {
            channel: lower_expr(channel),
            span: Some(keyword.span),
        },
        ast::Statement::Return(keyword, value) => IrStmt::Return {
            value: value.map(lower_expr),
            span: Some(keyword.span),
        },
        ast::Statement::Break(keyword) => IrStmt::Break {
            span: Some(keyword.span),
        },
        ast::Statement::Continue(keyword) => IrStmt::Continue {
            span: Some(keyword.span),
        },
        ast::Statement::Rest(keyword) => IrStmt::Rest {
            span: Some(keyword.span),
        },
        ast::Statement::Check(keyword, condition, message) => IrStmt::Check {
            condition: lower_expr(condition),
            message: message.map(|m| strip_quotes(&m.value.value)),
            span: Some(keyword.span),
        },
        ast::Statement::Import { module_name, .. } => IrStmt::Import {
            module_name: grammar::variable_name(&module_name).to_string(),
            span: Some(module_name.span),
        },
        ast::Statement::ExprStmt(expr) => IrStmt::Expr(lower_expr(expr)),
        ast::Statement::Documented { item, .. } => match item {
            ast::AnnotatableItem::StructDef(def) => lower_struct(def),
            ast::AnnotatableItem::FunctionDef(def) => IrStmt::FunctionDef(lower_function(def)),
            ast::AnnotatableItem::RustFnDecl(def) => IrStmt::RustFnDecl(lower_rust_function(def)),
        },
    }
}

fn lower_struct(def: ast::StructDef) -> IrStmt {
    IrStmt::StructDef {
        name: grammar::struct_name(&def.name).to_string(),
        fields: def
            .fields
            .into_iter()
            .map(|field| IrStructField {
                name: grammar::field_name(&field.name).to_string(),
                ty: field.field_type,
                span: Some(field.name.span),
            })
            .collect(),
        span: Some(def.name.span),
    }
}

fn lower_function(def: ast::FunctionDef) -> IrFunction {
    let name = grammar::function_name(&def.name).to_string();
    let params = lower_params(def.params);
    let return_type = def.return_type;
    IrFunction {
        name: name.clone(),
        signature: IrSignature {
            params,
            return_type,
            can_error: def.can_error.is_some(),
            is_pure: def.pure_kw.is_some(),
        },
        body: lower_statements(def.body.statements),
        span: Some(def.name.span),
    }
}

fn lower_rust_function(def: ast::RustFnDecl) -> IrRustFunction {
    let name = grammar::function_name(&def.name).to_string();
    let span = grammar::rust_fn_decl_span(&def);
    IrRustFunction {
        name,
        signature: IrSignature {
            params: lower_params(def.params),
            return_type: Some(def.return_type),
            can_error: def.can_error.is_some(),
            is_pure: false,
        },
        span: Some(span),
    }
}

fn lower_params(params: Vec<ast::FuncParam>) -> Vec<IrParam> {
    params
        .into_iter()
        .map(|param| IrParam {
            name: grammar::param_name(&param).to_string(),
            ty: param.command_type,
            span: Some(param.name.span),
        })
        .collect()
}

fn lower_error_clauses(clauses: ast::ErrorClauseList) -> Vec<IrErrorClause> {
    let mut out = Vec::new();
    lower_error_clause_list(clauses, &mut out);
    out
}

fn lower_error_clause_list(clauses: ast::ErrorClauseList, out: &mut Vec<IrErrorClause>) {
    out.push(IrErrorClause {
        error_type: clauses.first.error_type.map(|ty| ty.value),
        body: lower_statements(clauses.first.body.statements),
    });
    if let Some(rest) = clauses.rest {
        lower_error_clause_list(*rest, out);
    }
}

fn lower_expr(expr: ast::Expression) -> IrExpr {
    let span = grammar::expr_span(&expr);
    match expr {
        ast::Expression::StructInit(name, _, fields, _) => IrExpr::StructInit {
            name: grammar::struct_name(&name).to_string(),
            fields: fields
                .into_iter()
                .map(|field| IrFieldInit {
                    name: grammar::field_name(&field.name).to_string(),
                    value: lower_expr(field.value),
                    span: Some(field.name.span),
                })
                .collect(),
            span,
        },
        ast::Expression::ListInit(_, item_type, _, items, _) => IrExpr::ListInit {
            item_type,
            items: items.into_iter().map(lower_expr).collect(),
            span,
        },
        ast::Expression::MapInit(_, key_type, value_type, _, pairs, _) => IrExpr::MapInit {
            key_type,
            value_type,
            pairs: pairs
                .into_iter()
                .map(|pair| IrMapPair {
                    key: lower_expr(pair.key),
                    value: lower_expr(pair.value),
                })
                .collect(),
            span,
        },
        ast::Expression::FieldAccess(target, _, field) => IrExpr::FieldAccess {
            target: Box::new(lower_expr(*target)),
            field: grammar::field_name(&field).to_string(),
            span,
        },
        ast::Expression::At(collection, _, key) => IrExpr::At {
            collection: Box::new(lower_expr(*collection)),
            key: Box::new(lower_expr(*key)),
            span,
        },
        ast::Expression::Push(collection, _, value) => IrExpr::Push {
            collection: Box::new(lower_expr(*collection)),
            value: Box::new(lower_expr(*value)),
            span,
        },
        ast::Expression::BoolLit(value) => match value.value {
            ast::BoolVal::True(_) => IrExpr::Bool(true, span),
            ast::BoolVal::False(_) => IrExpr::Bool(false, span),
        },
        ast::Expression::Number(value) => {
            IrExpr::Number(value.value.parse().unwrap_or(0.0), Some(value.span))
        }
        ast::Expression::StringLit(value) => {
            IrExpr::String(strip_quotes(&value.value), Some(value.span))
        }
        ast::Expression::Variable(value) => {
            IrExpr::Variable(grammar::variable_name(&value).to_string(), Some(value.span))
        }
        ast::Expression::MoveExpr(_, value) => IrExpr::Move {
            name: grammar::variable_name(&value).to_string(),
            span,
        },
        ast::Expression::ErrorRef(value) => {
            IrExpr::ErrorRef(grammar::struct_name(&value).to_string(), Some(value.span))
        }
        ast::Expression::AdrInit(_, ty) => IrExpr::AdrInit { ty, span },
        ast::Expression::PipeInit(_, ty, capacity) => IrExpr::PipeInit {
            ty,
            capacity: capacity.and_then(|n| n.value.parse().ok()),
            span,
        },
        ast::Expression::Take(_, target) => IrExpr::Take {
            target: Box::new(lower_expr(*target)),
            span,
        },
        ast::Expression::Len(_, target) => IrExpr::Len {
            target: Box::new(lower_expr(*target)),
            span,
        },
        ast::Expression::Ref(_, target) => IrExpr::Ref {
            target: Box::new(lower_expr(*target)),
            span,
        },
        ast::Expression::Deref(_, target) => IrExpr::Deref {
            target: Box::new(lower_expr(*target)),
            span,
        },
        ast::Expression::Call(target, _, args, _) => IrExpr::Call {
            target: Box::new(lower_expr(*target)),
            args: args.into_iter().map(lower_expr).collect(),
            span,
        },
        ast::Expression::RunCall(_, target) => IrExpr::RunCall {
            target: Box::new(lower_expr(*target)),
            span,
        },
        ast::Expression::Add(lhs, _, rhs) => binary(IrBinaryOp::Add, lhs, rhs, span),
        ast::Expression::Sub(lhs, _, rhs) => binary(IrBinaryOp::Sub, lhs, rhs, span),
        ast::Expression::Mul(lhs, _, rhs) => binary(IrBinaryOp::Mul, lhs, rhs, span),
        ast::Expression::Div(lhs, _, rhs) => binary(IrBinaryOp::Div, lhs, rhs, span),
        ast::Expression::Eq(lhs, _, rhs) => binary(IrBinaryOp::Eq, lhs, rhs, span),
        ast::Expression::Neq(lhs, _, rhs) => binary(IrBinaryOp::Neq, lhs, rhs, span),
        ast::Expression::Gt(lhs, _, rhs) => binary(IrBinaryOp::Gt, lhs, rhs, span),
        ast::Expression::Lt(lhs, _, rhs) => binary(IrBinaryOp::Lt, lhs, rhs, span),
        ast::Expression::Geq(lhs, _, rhs) => binary(IrBinaryOp::Geq, lhs, rhs, span),
        ast::Expression::Leq(lhs, _, rhs) => binary(IrBinaryOp::Leq, lhs, rhs, span),
        ast::Expression::Range(lhs, _, rhs) => binary(IrBinaryOp::Range, lhs, rhs, span),
    }
}

fn binary(
    op: IrBinaryOp,
    lhs: Box<ast::Expression>,
    rhs: Box<ast::Expression>,
    span: IrSpan,
) -> IrExpr {
    IrExpr::Binary {
        op,
        lhs: Box::new(lower_expr(*lhs)),
        rhs: Box::new(lower_expr(*rhs)),
        span,
    }
}

fn merge_span(a: IrSpan, b: IrSpan) -> IrSpan {
    match (a, b) {
        (Some(a), Some(b)) => Some((a.0, b.1)),
        (Some(span), None) | (None, Some(span)) => Some(span),
        (None, None) => None,
    }
}

fn strip_quotes(value: &str) -> String {
    value.trim_matches('"').to_string()
}
