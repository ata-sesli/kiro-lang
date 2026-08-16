use crate::grammar::grammar;
use std::collections::{HashMap, HashSet};

pub mod eir;
pub mod types;

#[derive(Clone, Debug)]
pub struct FunctionInfo {
    pub is_pure: bool,
    pub can_error: bool,
    pub params: Vec<grammar::KiroType>,
    pub return_type: Option<grammar::KiroType>,
    pub doc: Option<String>,
}

impl Compiler {
    pub fn validate_semantics(
        &mut self,
        program: &grammar::Program,
        module: &str,
        source: &str,
    ) -> Result<(), crate::errors::KiroError> {
        crate::analysis::validate_program_semantics(self, program, module, source)
    }
}

pub struct Compiler {
    pub functions: HashMap<String, FunctionInfo>,
    pub module_functions: HashMap<(String, String), FunctionInfo>,
    pub known_modules: HashSet<String>,
    pub current_module: String,
}

const EFFECTFUL_RECURSION_MESSAGE: &str = "Recursive calls are only supported in pure fn. Effectful recursive functions are not supported yet; use a loop or split pure recursion from effects.";

fn find_cycle_from(
    start: &str,
    current: &str,
    graph: &HashMap<String, HashSet<String>>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    path.push(current.to_string());
    if let Some(nexts) = graph.get(current) {
        for next in nexts {
            if next == start {
                let mut cycle = path.clone();
                cycle.push(start.to_string());
                path.pop();
                return Some(cycle);
            }
            if !path.contains(next)
                && let Some(cycle) = find_cycle_from(start, next, graph, path)
            {
                path.pop();
                return Some(cycle);
            }
        }
    }
    path.pop();
    None
}

fn collect_calls_from_block(
    block: &grammar::Block,
    local_functions: &HashSet<String>,
    calls: &mut HashSet<String>,
) {
    for stmt in &block.statements {
        collect_calls_from_statement(stmt, local_functions, calls);
    }
}

fn collect_calls_from_statement(
    stmt: &grammar::Statement,
    local_functions: &HashSet<String>,
    calls: &mut HashSet<String>,
) {
    match stmt {
        grammar::Statement::VarDecl { value, .. } => {
            collect_calls_from_expr(value, local_functions, calls)
        }
        grammar::Statement::AssignStmt { lhs, rhs, .. } => {
            collect_calls_from_expr(lhs, local_functions, calls);
            collect_calls_from_expr(rhs, local_functions, calls);
        }
        grammar::Statement::On {
            condition,
            body,
            else_clause,
            error_clauses,
            ..
        } => {
            collect_calls_from_expr(condition, local_functions, calls);
            collect_calls_from_block(body, local_functions, calls);
            if let Some(off) = else_clause {
                collect_calls_from_block(&off.body, local_functions, calls);
            }
            if let Some(errors) = error_clauses {
                collect_calls_from_error_clauses(errors, local_functions, calls);
            }
        }
        grammar::Statement::LoopOn {
            condition, body, ..
        } => {
            collect_calls_from_expr(condition, local_functions, calls);
            collect_calls_from_block(body, local_functions, calls);
        }
        grammar::Statement::LoopIter {
            iterable,
            step,
            filter,
            body,
            else_clause,
            ..
        } => {
            collect_calls_from_expr(iterable, local_functions, calls);
            if let Some(step) = step {
                collect_calls_from_expr(&step.value, local_functions, calls);
            }
            if let Some(filter) = filter {
                collect_calls_from_expr(&filter.condition, local_functions, calls);
            }
            collect_calls_from_block(body, local_functions, calls);
            if let Some(off) = else_clause {
                collect_calls_from_block(&off.body, local_functions, calls);
            }
        }
        grammar::Statement::Give(_, ch, val) => {
            collect_calls_from_expr(ch, local_functions, calls);
            collect_calls_from_expr(val, local_functions, calls);
        }
        grammar::Statement::Close(_, ch) => collect_calls_from_expr(ch, local_functions, calls),
        grammar::Statement::Return(_, expr) => {
            if let Some(expr) = expr {
                collect_calls_from_expr(expr, local_functions, calls);
            }
        }
        grammar::Statement::Check(_, condition, _) => {
            collect_calls_from_expr(condition, local_functions, calls);
        }
        grammar::Statement::ExprStmt(expr) => collect_calls_from_expr(expr, local_functions, calls),
        grammar::Statement::Documented { item, .. } => {
            if let grammar::AnnotatableItem::FunctionDef(def) = item {
                collect_calls_from_block(&def.body, local_functions, calls);
            }
        }
        grammar::Statement::StructDef(_)
        | grammar::Statement::HandleDef(_)
        | grammar::Statement::ErrorDef { .. }
        | grammar::Statement::FunctionDef(_)
        | grammar::Statement::RustFnDecl(_)
        | grammar::Statement::Break(_)
        | grammar::Statement::Continue(_)
        | grammar::Statement::Rest(_)
        | grammar::Statement::Import { .. } => {}
    }
}

fn collect_calls_from_error_clauses(
    clauses: &grammar::ErrorClauseList,
    local_functions: &HashSet<String>,
    calls: &mut HashSet<String>,
) {
    collect_calls_from_block(&clauses.first.body, local_functions, calls);
    if let Some(rest) = &clauses.rest {
        collect_calls_from_error_clauses(rest, local_functions, calls);
    }
}

fn collect_calls_from_expr(
    expr: &grammar::Expression,
    local_functions: &HashSet<String>,
    calls: &mut HashSet<String>,
) {
    match expr {
        grammar::Expression::FieldAccess(target, _, _)
        | grammar::Expression::At(target, _, _)
        | grammar::Expression::Push(target, _, _)
        | grammar::Expression::Ref(_, target)
        | grammar::Expression::Deref(_, target)
        | grammar::Expression::Take(_, target)
        | grammar::Expression::Len(_, target)
        | grammar::Expression::RunCall(_, target) => {
            collect_calls_from_expr(target, local_functions, calls);
        }
        grammar::Expression::StructInit(_, _, fields, _) => {
            for field in fields {
                collect_calls_from_expr(&field.value, local_functions, calls);
            }
        }
        grammar::Expression::ListInit(_, _, _, items, _) => {
            for item in items {
                collect_calls_from_expr(item, local_functions, calls);
            }
        }
        grammar::Expression::MapInit(_, _, _, _, pairs, _) => {
            for pair in pairs {
                collect_calls_from_expr(&pair.key, local_functions, calls);
                collect_calls_from_expr(&pair.value, local_functions, calls);
            }
        }
        grammar::Expression::Call(func, _, args, _) => {
            if let grammar::Expression::Variable(v) = &**func
                && local_functions.contains(crate::grammar::variable_name(v))
            {
                calls.insert(crate::grammar::variable_name(v).to_string());
            }
            collect_calls_from_expr(func, local_functions, calls);
            for arg in args {
                collect_calls_from_expr(arg, local_functions, calls);
            }
        }
        grammar::Expression::Add(lhs, _, rhs)
        | grammar::Expression::Sub(lhs, _, rhs)
        | grammar::Expression::Mul(lhs, _, rhs)
        | grammar::Expression::Div(lhs, _, rhs)
        | grammar::Expression::Eq(lhs, _, rhs)
        | grammar::Expression::Neq(lhs, _, rhs)
        | grammar::Expression::Gt(lhs, _, rhs)
        | grammar::Expression::Lt(lhs, _, rhs)
        | grammar::Expression::Geq(lhs, _, rhs)
        | grammar::Expression::Leq(lhs, _, rhs)
        | grammar::Expression::Range(lhs, _, rhs) => {
            collect_calls_from_expr(lhs, local_functions, calls);
            collect_calls_from_expr(rhs, local_functions, calls);
        }
        grammar::Expression::Number(_)
        | grammar::Expression::StringLit(_)
        | grammar::Expression::BoolLit(_)
        | grammar::Expression::Variable(_)
        | grammar::Expression::MoveExpr(_, _)
        | grammar::Expression::ErrorRef(_)
        | grammar::Expression::AdrInit(_, _)
        | grammar::Expression::PipeInit(_, _, _) => {}
    }
}

pub fn program_uses_pipes(program: &grammar::Program) -> bool {
    program.statements.iter().any(statement_uses_pipes)
}

pub fn program_uses_anyhow(program: &grammar::Program) -> bool {
    program.statements.iter().any(statement_uses_anyhow)
}

pub fn program_uses_std_io_module(program: &grammar::Program) -> bool {
    program.statements.iter().any(statement_uses_std_io_module)
}

fn block_uses_pipes(block: &grammar::Block) -> bool {
    block.statements.iter().any(statement_uses_pipes)
}

fn block_uses_anyhow(block: &grammar::Block) -> bool {
    block.statements.iter().any(statement_uses_anyhow)
}

fn block_uses_std_io_module(block: &grammar::Block) -> bool {
    block.statements.iter().any(statement_uses_std_io_module)
}

fn error_clauses_use_pipes(clauses: &grammar::ErrorClauseList) -> bool {
    block_uses_pipes(&clauses.first.body)
        || clauses
            .rest
            .as_deref()
            .map(error_clauses_use_pipes)
            .unwrap_or(false)
}

fn error_clauses_use_anyhow(clauses: &grammar::ErrorClauseList) -> bool {
    block_uses_anyhow(&clauses.first.body)
        || clauses
            .rest
            .as_deref()
            .map(error_clauses_use_anyhow)
            .unwrap_or(false)
}

fn error_clauses_use_std_io_module(clauses: &grammar::ErrorClauseList) -> bool {
    block_uses_std_io_module(&clauses.first.body)
        || clauses
            .rest
            .as_deref()
            .map(error_clauses_use_std_io_module)
            .unwrap_or(false)
}

fn item_uses_pipes(item: &grammar::AnnotatableItem) -> bool {
    match item {
        grammar::AnnotatableItem::StructDef(def) => struct_uses_pipes(def),
        grammar::AnnotatableItem::HandleDef(_) => false,
        grammar::AnnotatableItem::FunctionDef(def) => function_uses_pipes(def),
        grammar::AnnotatableItem::RustFnDecl(def) => rust_fn_uses_pipes(def),
    }
}

fn item_uses_anyhow(item: &grammar::AnnotatableItem) -> bool {
    match item {
        grammar::AnnotatableItem::StructDef(_) | grammar::AnnotatableItem::HandleDef(_) => false,
        grammar::AnnotatableItem::FunctionDef(def) => function_uses_anyhow(def),
        grammar::AnnotatableItem::RustFnDecl(def) => rust_fn_uses_anyhow(def),
    }
}

fn item_uses_std_io_module(item: &grammar::AnnotatableItem) -> bool {
    match item {
        grammar::AnnotatableItem::StructDef(_)
        | grammar::AnnotatableItem::HandleDef(_)
        | grammar::AnnotatableItem::RustFnDecl(_) => false,
        grammar::AnnotatableItem::FunctionDef(def) => block_uses_std_io_module(&def.body),
    }
}

fn statement_uses_pipes(stmt: &grammar::Statement) -> bool {
    match stmt {
        grammar::Statement::StructDef(def) => struct_uses_pipes(def),
        grammar::Statement::HandleDef(_) => false,
        grammar::Statement::FunctionDef(def) => function_uses_pipes(def),
        grammar::Statement::RustFnDecl(def) => rust_fn_uses_pipes(def),
        grammar::Statement::VarDecl { value, .. } => expr_uses_pipes(value),
        grammar::Statement::AssignStmt { lhs, rhs, .. } => {
            expr_uses_pipes(lhs) || expr_uses_pipes(rhs)
        }
        grammar::Statement::On {
            condition,
            body,
            else_clause,
            error_clauses,
            ..
        } => {
            expr_uses_pipes(condition)
                || block_uses_pipes(body)
                || else_clause
                    .as_ref()
                    .map(|clause| block_uses_pipes(&clause.body))
                    .unwrap_or(false)
                || error_clauses
                    .as_ref()
                    .map(error_clauses_use_pipes)
                    .unwrap_or(false)
        }
        grammar::Statement::LoopOn {
            condition, body, ..
        } => expr_uses_pipes(condition) || block_uses_pipes(body),
        grammar::Statement::LoopIter {
            iterable,
            step,
            filter,
            body,
            else_clause,
            ..
        } => {
            expr_uses_pipes(iterable)
                || step
                    .as_ref()
                    .map(|step| expr_uses_pipes(&step.value))
                    .unwrap_or(false)
                || filter
                    .as_ref()
                    .map(|filter| expr_uses_pipes(&filter.condition))
                    .unwrap_or(false)
                || block_uses_pipes(body)
                || else_clause
                    .as_ref()
                    .map(|clause| block_uses_pipes(&clause.body))
                    .unwrap_or(false)
        }
        grammar::Statement::Give(_, _, _) | grammar::Statement::Close(_, _) => true,
        grammar::Statement::Return(_, expr) => expr.as_ref().map(expr_uses_pipes).unwrap_or(false),
        grammar::Statement::Check(_, condition, _) => expr_uses_pipes(condition),
        grammar::Statement::ExprStmt(expr) => expr_uses_pipes(expr),
        grammar::Statement::Documented { item, .. } => item_uses_pipes(item),
        grammar::Statement::ErrorDef { .. }
        | grammar::Statement::Break(_)
        | grammar::Statement::Continue(_)
        | grammar::Statement::Rest(_)
        | grammar::Statement::Import { .. } => false,
    }
}

fn statement_uses_anyhow(stmt: &grammar::Statement) -> bool {
    match stmt {
        grammar::Statement::ErrorDef { .. } => true,
        grammar::Statement::FunctionDef(def) => function_uses_anyhow(def),
        grammar::Statement::RustFnDecl(def) => rust_fn_uses_anyhow(def),
        grammar::Statement::VarDecl { value, .. } => expr_uses_anyhow(value),
        grammar::Statement::AssignStmt { lhs, rhs, .. } => {
            expr_uses_anyhow(lhs) || expr_uses_anyhow(rhs)
        }
        grammar::Statement::On {
            condition,
            body,
            else_clause,
            error_clauses,
            ..
        } => {
            expr_uses_anyhow(condition)
                || block_uses_anyhow(body)
                || else_clause
                    .as_ref()
                    .map(|clause| block_uses_anyhow(&clause.body))
                    .unwrap_or(false)
                || error_clauses
                    .as_ref()
                    .map(error_clauses_use_anyhow)
                    .unwrap_or(false)
        }
        grammar::Statement::LoopOn {
            condition, body, ..
        } => expr_uses_anyhow(condition) || block_uses_anyhow(body),
        grammar::Statement::LoopIter {
            iterable,
            step,
            filter,
            body,
            else_clause,
            ..
        } => {
            expr_uses_anyhow(iterable)
                || step
                    .as_ref()
                    .map(|step| expr_uses_anyhow(&step.value))
                    .unwrap_or(false)
                || filter
                    .as_ref()
                    .map(|filter| expr_uses_anyhow(&filter.condition))
                    .unwrap_or(false)
                || block_uses_anyhow(body)
                || else_clause
                    .as_ref()
                    .map(|clause| block_uses_anyhow(&clause.body))
                    .unwrap_or(false)
        }
        grammar::Statement::Give(_, ch, val) => expr_uses_anyhow(ch) || expr_uses_anyhow(val),
        grammar::Statement::Close(_, ch) => expr_uses_anyhow(ch),
        grammar::Statement::Return(_, expr) => expr.as_ref().map(expr_uses_anyhow).unwrap_or(false),
        grammar::Statement::Check(_, condition, _) => expr_uses_anyhow(condition),
        grammar::Statement::ExprStmt(expr) => expr_uses_anyhow(expr),
        grammar::Statement::Documented { item, .. } => item_uses_anyhow(item),
        grammar::Statement::StructDef(_)
        | grammar::Statement::HandleDef(_)
        | grammar::Statement::Break(_)
        | grammar::Statement::Continue(_)
        | grammar::Statement::Rest(_)
        | grammar::Statement::Import { .. } => false,
    }
}

fn statement_uses_std_io_module(stmt: &grammar::Statement) -> bool {
    match stmt {
        grammar::Statement::FunctionDef(def) => block_uses_std_io_module(&def.body),
        grammar::Statement::VarDecl { value, .. } => expr_uses_std_io_module(value),
        grammar::Statement::AssignStmt { lhs, rhs, .. } => {
            expr_uses_std_io_module(lhs) || expr_uses_std_io_module(rhs)
        }
        grammar::Statement::On {
            condition,
            body,
            else_clause,
            error_clauses,
            ..
        } => {
            expr_uses_std_io_module(condition)
                || block_uses_std_io_module(body)
                || else_clause
                    .as_ref()
                    .map(|clause| block_uses_std_io_module(&clause.body))
                    .unwrap_or(false)
                || error_clauses
                    .as_ref()
                    .map(error_clauses_use_std_io_module)
                    .unwrap_or(false)
        }
        grammar::Statement::LoopOn {
            condition, body, ..
        } => expr_uses_std_io_module(condition) || block_uses_std_io_module(body),
        grammar::Statement::LoopIter {
            iterable,
            step,
            filter,
            body,
            else_clause,
            ..
        } => {
            expr_uses_std_io_module(iterable)
                || step
                    .as_ref()
                    .map(|step| expr_uses_std_io_module(&step.value))
                    .unwrap_or(false)
                || filter
                    .as_ref()
                    .map(|filter| expr_uses_std_io_module(&filter.condition))
                    .unwrap_or(false)
                || block_uses_std_io_module(body)
                || else_clause
                    .as_ref()
                    .map(|clause| block_uses_std_io_module(&clause.body))
                    .unwrap_or(false)
        }
        grammar::Statement::Give(_, ch, val) => {
            expr_uses_std_io_module(ch) || expr_uses_std_io_module(val)
        }
        grammar::Statement::Close(_, ch) => expr_uses_std_io_module(ch),
        grammar::Statement::Return(_, expr) => {
            expr.as_ref().map(expr_uses_std_io_module).unwrap_or(false)
        }
        grammar::Statement::Check(_, condition, _) => expr_uses_std_io_module(condition),
        grammar::Statement::ExprStmt(expr) => expr_uses_std_io_module(expr),
        grammar::Statement::Documented { item, .. } => item_uses_std_io_module(item),
        grammar::Statement::StructDef(_)
        | grammar::Statement::HandleDef(_)
        | grammar::Statement::RustFnDecl(_)
        | grammar::Statement::ErrorDef { .. }
        | grammar::Statement::Break(_)
        | grammar::Statement::Continue(_)
        | grammar::Statement::Rest(_)
        | grammar::Statement::Import { .. } => false,
    }
}

fn struct_uses_pipes(def: &grammar::StructDef) -> bool {
    def.fields
        .iter()
        .any(|field| type_uses_pipes(&field.field_type))
}

fn function_uses_pipes(def: &grammar::FunctionDef) -> bool {
    def.params
        .iter()
        .any(|param| type_uses_pipes(&param.command_type))
        || def
            .return_type
            .as_ref()
            .map(type_uses_pipes)
            .unwrap_or(false)
        || block_uses_pipes(&def.body)
}

fn function_uses_anyhow(def: &grammar::FunctionDef) -> bool {
    block_uses_anyhow(&def.body)
}

fn rust_fn_uses_pipes(def: &grammar::RustFnDecl) -> bool {
    def.params
        .iter()
        .any(|param| type_uses_pipes(&param.command_type))
        || type_uses_pipes(&def.return_type)
}

fn rust_fn_uses_anyhow(def: &grammar::RustFnDecl) -> bool {
    def.can_error.is_some()
}

fn expr_uses_pipes(expr: &grammar::Expression) -> bool {
    match expr {
        grammar::Expression::PipeInit(_, _, _) | grammar::Expression::Take(_, _) => true,
        grammar::Expression::AdrInit(_, inner) => type_uses_pipes(inner),
        grammar::Expression::ListInit(_, inner, _, items, _) => {
            type_uses_pipes(inner) || items.iter().any(expr_uses_pipes)
        }
        grammar::Expression::MapInit(_, key, value, _, pairs, _) => {
            type_uses_pipes(key)
                || type_uses_pipes(value)
                || pairs
                    .iter()
                    .any(|pair| expr_uses_pipes(&pair.key) || expr_uses_pipes(&pair.value))
        }
        grammar::Expression::StructInit(_, _, fields, _) => {
            fields.iter().any(|field| expr_uses_pipes(&field.value))
        }
        grammar::Expression::FieldAccess(target, _, _)
        | grammar::Expression::Ref(_, target)
        | grammar::Expression::Deref(_, target)
        | grammar::Expression::Len(_, target)
        | grammar::Expression::RunCall(_, target) => expr_uses_pipes(target),
        grammar::Expression::At(target, _, index) | grammar::Expression::Push(target, _, index) => {
            expr_uses_pipes(target) || expr_uses_pipes(index)
        }
        grammar::Expression::Call(func, _, args, _) => {
            expr_uses_pipes(func) || args.iter().any(expr_uses_pipes)
        }
        grammar::Expression::Add(lhs, _, rhs)
        | grammar::Expression::Sub(lhs, _, rhs)
        | grammar::Expression::Mul(lhs, _, rhs)
        | grammar::Expression::Div(lhs, _, rhs)
        | grammar::Expression::Eq(lhs, _, rhs)
        | grammar::Expression::Neq(lhs, _, rhs)
        | grammar::Expression::Gt(lhs, _, rhs)
        | grammar::Expression::Lt(lhs, _, rhs)
        | grammar::Expression::Geq(lhs, _, rhs)
        | grammar::Expression::Leq(lhs, _, rhs)
        | grammar::Expression::Range(lhs, _, rhs) => expr_uses_pipes(lhs) || expr_uses_pipes(rhs),
        grammar::Expression::Number(_)
        | grammar::Expression::StringLit(_)
        | grammar::Expression::BoolLit(_)
        | grammar::Expression::Variable(_)
        | grammar::Expression::MoveExpr(_, _)
        | grammar::Expression::ErrorRef(_) => false,
    }
}

fn expr_uses_anyhow(expr: &grammar::Expression) -> bool {
    match expr {
        grammar::Expression::ErrorRef(_) => true,
        grammar::Expression::ListInit(_, _, _, items, _) => items.iter().any(expr_uses_anyhow),
        grammar::Expression::MapInit(_, _, _, _, pairs, _) => pairs
            .iter()
            .any(|pair| expr_uses_anyhow(&pair.key) || expr_uses_anyhow(&pair.value)),
        grammar::Expression::StructInit(_, _, fields, _) => {
            fields.iter().any(|field| expr_uses_anyhow(&field.value))
        }
        grammar::Expression::FieldAccess(target, _, _)
        | grammar::Expression::Ref(_, target)
        | grammar::Expression::Deref(_, target)
        | grammar::Expression::Len(_, target)
        | grammar::Expression::RunCall(_, target)
        | grammar::Expression::Take(_, target) => expr_uses_anyhow(target),
        grammar::Expression::At(target, _, index) | grammar::Expression::Push(target, _, index) => {
            expr_uses_anyhow(target) || expr_uses_anyhow(index)
        }
        grammar::Expression::Call(func, _, args, _) => {
            expr_uses_anyhow(func) || args.iter().any(expr_uses_anyhow)
        }
        grammar::Expression::Add(lhs, _, rhs)
        | grammar::Expression::Sub(lhs, _, rhs)
        | grammar::Expression::Mul(lhs, _, rhs)
        | grammar::Expression::Div(lhs, _, rhs)
        | grammar::Expression::Eq(lhs, _, rhs)
        | grammar::Expression::Neq(lhs, _, rhs)
        | grammar::Expression::Gt(lhs, _, rhs)
        | grammar::Expression::Lt(lhs, _, rhs)
        | grammar::Expression::Geq(lhs, _, rhs)
        | grammar::Expression::Leq(lhs, _, rhs)
        | grammar::Expression::Range(lhs, _, rhs) => expr_uses_anyhow(lhs) || expr_uses_anyhow(rhs),
        grammar::Expression::Variable(_)
        | grammar::Expression::Number(_)
        | grammar::Expression::StringLit(_)
        | grammar::Expression::BoolLit(_)
        | grammar::Expression::MoveExpr(_, _)
        | grammar::Expression::AdrInit(_, _)
        | grammar::Expression::PipeInit(_, _, _) => false,
    }
}

fn expr_uses_std_io_module(expr: &grammar::Expression) -> bool {
    match expr {
        grammar::Expression::FieldAccess(target, _, field) => {
            if let grammar::Expression::Variable(module) = &**target
                && crate::is_std_io_module_name(&module.value)
            {
                return !crate::is_std_io_display_function(&field.value);
            }
            expr_uses_std_io_module(target)
        }
        grammar::Expression::ListInit(_, _, _, items, _) => {
            items.iter().any(expr_uses_std_io_module)
        }
        grammar::Expression::MapInit(_, _, _, _, pairs, _) => pairs
            .iter()
            .any(|pair| expr_uses_std_io_module(&pair.key) || expr_uses_std_io_module(&pair.value)),
        grammar::Expression::StructInit(_, _, fields, _) => fields
            .iter()
            .any(|field| expr_uses_std_io_module(&field.value)),
        grammar::Expression::Ref(_, target)
        | grammar::Expression::Deref(_, target)
        | grammar::Expression::Len(_, target)
        | grammar::Expression::RunCall(_, target)
        | grammar::Expression::Take(_, target) => expr_uses_std_io_module(target),
        grammar::Expression::At(target, _, index) | grammar::Expression::Push(target, _, index) => {
            expr_uses_std_io_module(target) || expr_uses_std_io_module(index)
        }
        grammar::Expression::Call(func, _, args, _) => {
            expr_uses_std_io_module(func) || args.iter().any(expr_uses_std_io_module)
        }
        grammar::Expression::Add(lhs, _, rhs)
        | grammar::Expression::Sub(lhs, _, rhs)
        | grammar::Expression::Mul(lhs, _, rhs)
        | grammar::Expression::Div(lhs, _, rhs)
        | grammar::Expression::Eq(lhs, _, rhs)
        | grammar::Expression::Neq(lhs, _, rhs)
        | grammar::Expression::Gt(lhs, _, rhs)
        | grammar::Expression::Lt(lhs, _, rhs)
        | grammar::Expression::Geq(lhs, _, rhs)
        | grammar::Expression::Leq(lhs, _, rhs)
        | grammar::Expression::Range(lhs, _, rhs) => {
            expr_uses_std_io_module(lhs) || expr_uses_std_io_module(rhs)
        }
        grammar::Expression::Variable(_)
        | grammar::Expression::Number(_)
        | grammar::Expression::StringLit(_)
        | grammar::Expression::BoolLit(_)
        | grammar::Expression::MoveExpr(_, _)
        | grammar::Expression::ErrorRef(_)
        | grammar::Expression::AdrInit(_, _)
        | grammar::Expression::PipeInit(_, _, _) => false,
    }
}

fn type_uses_pipes(ty: &grammar::KiroType) -> bool {
    match ty {
        grammar::KiroType::Pipe(_, _) => true,
        grammar::KiroType::Adr(_, inner) | grammar::KiroType::List(_, inner) => {
            type_uses_pipes(inner)
        }
        grammar::KiroType::Map(_, key, value) => type_uses_pipes(key) || type_uses_pipes(value),
        grammar::KiroType::FnType(_, _, params, _, _, ret) => {
            params.iter().any(type_uses_pipes) || type_uses_pipes(ret)
        }
        grammar::KiroType::Num
        | grammar::KiroType::Str
        | grammar::KiroType::Bytes
        | grammar::KiroType::Bool
        | grammar::KiroType::Void
        | grammar::KiroType::Custom(_) => false,
    }
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            module_functions: HashMap::new(),
            known_modules: HashSet::new(),
            current_module: "main".to_string(),
        }
    }

    pub fn with_module_functions(
        module_functions: HashMap<(String, String), FunctionInfo>,
    ) -> Self {
        let mut compiler = Self::new();
        compiler.module_functions = module_functions;
        compiler
    }

    pub fn collect_program_functions(program: &grammar::Program) -> HashMap<String, FunctionInfo> {
        let mut functions = HashMap::new();
        for stmt in &program.statements {
            match stmt {
                grammar::Statement::Documented { doc, item } => match item {
                    grammar::AnnotatableItem::HandleDef(_) => {}
                    grammar::AnnotatableItem::FunctionDef(def) => {
                        let doc_str = Some(
                            doc.iter()
                                .map(|d| d.content.trim_start_matches("///").trim().to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                        functions.insert(
                            crate::grammar::function_name(&def.name).to_string(),
                            FunctionInfo {
                                is_pure: def.pure_kw.is_some(),
                                can_error: def.can_error.is_some(),
                                params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                                return_type: def.return_type.clone(),
                                doc: doc_str,
                            },
                        );
                    }
                    grammar::AnnotatableItem::RustFnDecl(def) => {
                        let doc_str = Some(
                            doc.iter()
                                .map(|d| d.content.trim_start_matches("///").trim().to_string())
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                        functions.insert(
                            crate::grammar::function_name(&def.name).to_string(),
                            FunctionInfo {
                                is_pure: def.pure_kw.is_some(),
                                can_error: def.can_error.is_some(),
                                params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                                return_type: Some(def.return_type.clone()),
                                doc: doc_str,
                            },
                        );
                    }
                    _ => {}
                },
                grammar::Statement::FunctionDef(def) => {
                    functions.insert(
                        crate::grammar::function_name(&def.name).to_string(),
                        FunctionInfo {
                            is_pure: def.pure_kw.is_some(),
                            can_error: def.can_error.is_some(),
                            params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                            return_type: def.return_type.clone(),
                            doc: None,
                        },
                    );
                }
                grammar::Statement::RustFnDecl(def) => {
                    functions.insert(
                        crate::grammar::function_name(&def.name).to_string(),
                        FunctionInfo {
                            is_pure: def.pure_kw.is_some(),
                            can_error: def.can_error.is_some(),
                            params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                            return_type: Some(def.return_type.clone()),
                            doc: None,
                        },
                    );
                }
                grammar::Statement::HandleDef(_) => {}
                _ => {}
            }
        }
        functions
    }

    pub fn validate_effectful_recursion(
        &self,
        program: &grammar::Program,
        module: &str,
        source: Option<&str>,
    ) -> Result<(), crate::errors::KiroError> {
        let local_functions: HashSet<String> = self.functions.keys().cloned().collect();
        let mut graph: HashMap<String, HashSet<String>> = HashMap::new();
        let mut spans: HashMap<String, crate::grammar::AstSpan> = HashMap::new();

        for stmt in &program.statements {
            match stmt {
                grammar::Statement::FunctionDef(def) => {
                    let name = crate::grammar::function_name(&def.name).to_string();
                    let mut calls = HashSet::new();
                    collect_calls_from_block(&def.body, &local_functions, &mut calls);
                    spans.insert(name.clone(), crate::grammar::function_span(&def.name));
                    graph.insert(name, calls);
                }
                grammar::Statement::Documented { item, .. } => {
                    if let grammar::AnnotatableItem::FunctionDef(def) = item {
                        let name = crate::grammar::function_name(&def.name).to_string();
                        let mut calls = HashSet::new();
                        collect_calls_from_block(&def.body, &local_functions, &mut calls);
                        spans.insert(name.clone(), crate::grammar::function_span(&def.name));
                        graph.insert(name, calls);
                    }
                }
                _ => {}
            }
        }

        for name in graph.keys() {
            let mut path = Vec::new();
            if let Some(cycle) = find_cycle_from(name, name, &graph, &mut path)
                && cycle.iter().any(|n| {
                    self.functions
                        .get(n)
                        .map(|info| !info.is_pure)
                        .unwrap_or(false)
                })
            {
                let mut err = crate::errors::KiroError::compile_error(
                    module,
                    crate::errors::ErrorCode::PureViolation,
                    EFFECTFUL_RECURSION_MESSAGE,
                    None,
                );
                if let Some(source) = source
                    && let Some(offending) = cycle.iter().find(|n| {
                        self.functions
                            .get(*n)
                            .map(|info| !info.is_pure)
                            .unwrap_or(false)
                    })
                    && let Some(span) = spans.get(offending)
                {
                    err = err.with_byte_span(
                        module,
                        source,
                        crate::errors::SourceSpan::new(span.0, span.1),
                        "effectful recursion",
                    );
                }
                return Err(err);
            }
        }

        Ok(())
    }

    pub fn effectful_recursion_message() -> &'static str {
        EFFECTFUL_RECURSION_MESSAGE
    }
}
