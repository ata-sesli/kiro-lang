use std::collections::BTreeMap;
use std::fmt;

use crate::hir::{
    Effects, FieldId, FunctionId, HirBinaryOp, HirCallKind, HirExpr, HirExprKind, HirFunction,
    HirModule, HirProgram, HirStmt, HirStmtKind, LocalId, ModuleId, SemType, Signature,
    SourceAnchor, TypeId, TypeTable,
};

use super::{
    BasicBlock, BlockId, ConstId, Constant, EirFunction, EirHostFunction, EirProgram, GlobalId,
    Instruction, InstructionKind, SlotId, Terminator, TerminatorKind, VerifyError, verify_program,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LowerError {
    pub anchor: SourceAnchor,
    pub kind: LowerErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerErrorKind {
    Unsupported { operation: &'static str },
    InvalidProgram(String),
    Verification(Vec<VerifyError>),
}

impl fmt::Display for LowerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "EIR lowering failed at {}:{}..{}: ",
            self.anchor.source().raw(),
            self.anchor.start(),
            self.anchor.end()
        )?;
        match &self.kind {
            LowerErrorKind::Unsupported { operation } => {
                write!(formatter, "unsupported {operation}")
            }
            LowerErrorKind::InvalidProgram(message) => formatter.write_str(message),
            LowerErrorKind::Verification(errors) => {
                write!(formatter, "{} verifier error(s)", errors.len())
            }
        }
    }
}

impl std::error::Error for LowerError {}

pub fn lower_program(hir: &HirProgram) -> Result<EirProgram, LowerError> {
    let fallback = fallback_anchor(hir);
    let mut constants = Vec::new();
    let mut globals = Vec::new();
    let mut global_ids = BTreeMap::new();
    for module in &hir.modules {
        for statement in &module.statements {
            let global = match &statement.kind {
                HirStmtKind::VarDecl { local, value } => Some((*local, value.ty)),
                HirStmtKind::Assign { target, value }
                    if matches!(target.kind, HirExprKind::Local(_)) =>
                {
                    let HirExprKind::Local(local) = target.kind else {
                        unreachable!();
                    };
                    Some((local, value.ty))
                }
                _ => None,
            };
            if let Some((local, ty)) = global
                && !global_ids.contains_key(&(module.id, local))
            {
                let id = GlobalId::try_from(globals.len())
                    .map_err(|error| invalid(statement.anchor, error))?;
                globals.push(ty);
                global_ids.insert((module.id, local), id);
            }
        }
    }
    let mut functions = hir
        .modules
        .iter()
        .flat_map(|module| {
            module
                .functions
                .iter()
                .map(move |function| (module, function))
        })
        .collect::<Vec<_>>();
    functions.sort_by_key(|(_, function)| function.id);

    let mut lowered = Vec::with_capacity(functions.len() + hir.modules.len());
    for (index, (module, function)) in functions.into_iter().enumerate() {
        let expected = FunctionId::try_from(index).map_err(|error| invalid(fallback, error))?;
        if function.id != expected {
            return Err(invalid(
                function.anchor,
                format!(
                    "function IDs must be dense: expected f{}, found f{}",
                    expected.raw(),
                    function.id.raw()
                ),
            ));
        }
        lowered.push(lower_function(
            module,
            function,
            &global_ids,
            &mut constants,
            &hir.types,
        )?);
    }

    let mut module_initializers = Vec::with_capacity(hir.modules.len());
    for module in &hir.modules {
        let id = FunctionId::try_from(lowered.len()).map_err(|error| invalid(fallback, error))?;
        lowered.push(lower_initializer(
            module,
            id,
            &global_ids,
            &mut constants,
            &hir.types,
        )?);
        module_initializers.push(id);
    }

    module_initializers = ordered_initializers(hir, &module_initializers);

    let mut host_functions = hir
        .modules
        .iter()
        .flat_map(|module| {
            module
                .host_functions
                .iter()
                .map(|function| EirHostFunction {
                    id: function.id,
                    module: module.name.clone(),
                    name: function.name.clone(),
                    signature: function.signature.clone(),
                    anchor: function.anchor,
                })
        })
        .collect::<Vec<_>>();
    host_functions.sort_by_key(|function| function.id);

    let program = EirProgram {
        types: hir.types.clone(),
        errors: hir.symbols.errors.clone(),
        globals,
        host_functions,
        constants,
        functions: lowered,
        module_initializers,
    };
    if let Err(errors) = verify_program(&program) {
        let anchor = errors.first().map_or(fallback, |error| error.anchor);
        return Err(LowerError {
            anchor,
            kind: LowerErrorKind::Verification(errors),
        });
    }
    Ok(program)
}

fn lower_function(
    module: &HirModule,
    function: &HirFunction,
    globals: &BTreeMap<(ModuleId, LocalId), GlobalId>,
    constants: &mut Vec<Constant>,
    types: &TypeTable,
) -> Result<EirFunction, LowerError> {
    let mut builder = FunctionBuilder::new(
        module.id,
        function.id,
        function.name.clone(),
        function.signature.clone(),
        function.anchor,
        globals,
        constants,
        types,
    );
    for parameter in &function.params {
        builder.declare_parameter(parameter.local, parameter.ty, parameter.anchor)?;
    }
    builder.lower_statements(&function.body)?;
    builder.finish(module, function.anchor)
}

fn lower_initializer(
    module: &HirModule,
    id: FunctionId,
    globals: &BTreeMap<(ModuleId, LocalId), GlobalId>,
    constants: &mut Vec<Constant>,
    types: &TypeTable,
) -> Result<EirFunction, LowerError> {
    let anchor = module.statements.first().map_or_else(
        || fallback_module_anchor(module),
        |statement| statement.anchor,
    );
    let mut builder = FunctionBuilder::new(
        module.id,
        id,
        format!("{}::$init", module.name),
        Signature::new(
            [],
            TypeId::VOID,
            Effects::MAY_FAIL
                | Effects::MAY_BLOCK
                | Effects::MAY_SPAWN
                | Effects::HOST_CALL
                | Effects::INDIRECT_CALL,
        ),
        anchor,
        globals,
        constants,
        types,
    );
    for statement in &module.statements {
        match statement.kind {
            HirStmtKind::FunctionDef(_)
            | HirStmtKind::HostFunctionDecl(_)
            | HirStmtKind::StructDef(_)
            | HirStmtKind::HandleDef(_)
            | HirStmtKind::ErrorDef { .. }
            | HirStmtKind::Import(_) => {}
            _ => builder.lower_statement(statement)?,
        }
        if builder.is_terminated() {
            break;
        }
    }
    builder.finish(module, anchor)
}

struct DraftBlock {
    instructions: Vec<Instruction>,
    terminator: Option<Terminator>,
}

struct FunctionBuilder<'a> {
    module: ModuleId,
    id: FunctionId,
    name: String,
    signature: Signature,
    anchor: SourceAnchor,
    globals: &'a BTreeMap<(ModuleId, LocalId), GlobalId>,
    constants: &'a mut Vec<Constant>,
    types: &'a TypeTable,
    slots: Vec<TypeId>,
    locals: BTreeMap<LocalId, SlotId>,
    parameter_count: u32,
    blocks: Vec<DraftBlock>,
    current: BlockId,
    loops: Vec<(BlockId, BlockId)>,
}

impl<'a> FunctionBuilder<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        module: ModuleId,
        id: FunctionId,
        name: String,
        signature: Signature,
        anchor: SourceAnchor,
        globals: &'a BTreeMap<(ModuleId, LocalId), GlobalId>,
        constants: &'a mut Vec<Constant>,
        types: &'a TypeTable,
    ) -> Self {
        Self {
            module,
            id,
            name,
            signature,
            anchor,
            globals,
            constants,
            types,
            slots: Vec::new(),
            locals: BTreeMap::new(),
            parameter_count: 0,
            blocks: vec![DraftBlock {
                instructions: Vec::new(),
                terminator: None,
            }],
            current: BlockId::new(0),
            loops: Vec::new(),
        }
    }

    fn declare_parameter(
        &mut self,
        local: LocalId,
        ty: TypeId,
        anchor: SourceAnchor,
    ) -> Result<(), LowerError> {
        let slot = self.allocate_slot(ty, anchor)?;
        self.locals.insert(local, slot);
        self.parameter_count = self
            .parameter_count
            .checked_add(1)
            .ok_or_else(|| invalid(anchor, "parameter count exceeds u32"))?;
        Ok(())
    }

    fn lower_statements(&mut self, statements: &[HirStmt]) -> Result<(), LowerError> {
        for statement in statements {
            if self.is_terminated() {
                break;
            }
            self.lower_statement(statement)?;
        }
        Ok(())
    }

    fn lower_statement(&mut self, statement: &HirStmt) -> Result<(), LowerError> {
        match &statement.kind {
            HirStmtKind::VarDecl { local, value } => {
                let value_slot = self.require_value(value)?;
                if let Some(global) = self.global(*local) {
                    self.emit(
                        InstructionKind::StoreGlobal {
                            global,
                            src: value_slot,
                        },
                        statement.anchor,
                    );
                } else {
                    let local_slot = self.allocate_slot(value.ty, statement.anchor)?;
                    self.locals.insert(*local, local_slot);
                    self.emit(
                        InstructionKind::Copy {
                            dst: local_slot,
                            src: value_slot,
                        },
                        statement.anchor,
                    );
                }
            }
            HirStmtKind::Assign { target, value } => {
                let value_slot = self.require_value(value)?;
                if let HirExprKind::Local(local) = target.kind {
                    if let Some(destination) = self.locals.get(&local).copied() {
                        self.emit(
                            InstructionKind::Copy {
                                dst: destination,
                                src: value_slot,
                            },
                            statement.anchor,
                        );
                    } else if let Some(global) = self.global(local) {
                        self.emit(
                            InstructionKind::StoreGlobal {
                                global,
                                src: value_slot,
                            },
                            statement.anchor,
                        );
                    } else {
                        return Err(unsupported(target.anchor, "unresolved assignment local"));
                    }
                } else if let HirExprKind::Deref(address) = &target.kind {
                    let address = self.require_value(address)?;
                    self.emit(
                        InstructionKind::StoreDeref {
                            address,
                            src: value_slot,
                        },
                        statement.anchor,
                    );
                } else if let Some((local, root_type, fields)) = field_assignment_path(target) {
                    let local_slot = self.locals.get(&local).copied();
                    let target = self.read_local(local, root_type, target.anchor)?;
                    self.emit(
                        InstructionKind::SetField {
                            target,
                            fields: fields.into_boxed_slice(),
                            src: value_slot,
                        },
                        statement.anchor,
                    );
                    if local_slot.is_none()
                        && let Some(global) = self.global(local)
                    {
                        self.emit(
                            InstructionKind::StoreGlobal {
                                global,
                                src: target,
                            },
                            statement.anchor,
                        );
                    }
                } else {
                    return Err(unsupported(statement.anchor, "assignment target"));
                }
            }
            HirStmtKind::On {
                condition,
                body,
                else_body,
                error_clauses,
            } => {
                if error_clauses.is_empty() {
                    self.lower_on(condition, body, else_body.as_deref(), statement.anchor)?;
                } else {
                    self.lower_on_with_errors(
                        condition,
                        body,
                        else_body.as_deref(),
                        error_clauses,
                        statement.anchor,
                    )?;
                }
            }
            HirStmtKind::LoopOn { condition, body } => {
                self.lower_loop_on(condition, body, statement.anchor)?;
            }
            HirStmtKind::Return(value) => {
                let value = value
                    .as_ref()
                    .map(|value| self.require_value(value))
                    .transpose()?;
                self.terminate(TerminatorKind::Return(value), statement.anchor);
            }
            HirStmtKind::Break => {
                let Some((_, exit)) = self.loops.last().copied() else {
                    return Err(invalid(statement.anchor, "break outside loop"));
                };
                self.terminate(TerminatorKind::Jump(exit), statement.anchor);
            }
            HirStmtKind::Continue => {
                let Some((condition, _)) = self.loops.last().copied() else {
                    return Err(invalid(statement.anchor, "continue outside loop"));
                };
                self.terminate(TerminatorKind::Jump(condition), statement.anchor);
            }
            HirStmtKind::Expr(expression) => {
                self.lower_expr(expression)?;
            }
            HirStmtKind::FunctionDef(_)
            | HirStmtKind::HostFunctionDecl(_)
            | HirStmtKind::StructDef(_)
            | HirStmtKind::HandleDef(_)
            | HirStmtKind::ErrorDef { .. }
            | HirStmtKind::Import(_) => {}
            HirStmtKind::LoopIter {
                iterator,
                iterable,
                step,
                filter,
                body,
                else_body,
            } => self.lower_loop_iter(
                *iterator,
                iterable,
                step.as_ref(),
                filter.as_ref(),
                body,
                else_body.as_deref(),
                statement.anchor,
            )?,
            HirStmtKind::Give { channel, value } => {
                let channel = self.require_value(channel)?;
                let value = self.require_value(value)?;
                self.emit(InstructionKind::Give { channel, value }, statement.anchor);
            }
            HirStmtKind::Close(channel) => {
                let channel = self.require_value(channel)?;
                self.emit(InstructionKind::Close { channel }, statement.anchor);
            }
            HirStmtKind::Rest => self.emit(InstructionKind::Rest, statement.anchor),
            HirStmtKind::Check { condition, message } => {
                let condition = self.require_value(condition)?;
                let message = message
                    .as_ref()
                    .map(|message| {
                        self.intern_constant(Constant::Str(message.clone()), statement.anchor)
                    })
                    .transpose()?;
                self.emit(
                    InstructionKind::Check { condition, message },
                    statement.anchor,
                );
            }
        }
        Ok(())
    }

    fn lower_on(
        &mut self,
        condition: &HirExpr,
        body: &[HirStmt],
        else_body: Option<&[HirStmt]>,
        anchor: SourceAnchor,
    ) -> Result<(), LowerError> {
        let condition = self.require_value(condition)?;
        let then_block = self.new_block(anchor)?;
        let else_block = self.new_block(anchor)?;
        let join_block = self.new_block(anchor)?;
        self.terminate(
            TerminatorKind::Branch {
                condition,
                then_block,
                else_block,
            },
            anchor,
        );

        self.current = then_block;
        self.lower_statements(body)?;
        let then_falls_through = !self.is_terminated();
        if then_falls_through {
            self.terminate(TerminatorKind::Jump(join_block), anchor);
        }

        self.current = else_block;
        if let Some(else_body) = else_body {
            self.lower_statements(else_body)?;
        }
        let else_falls_through = !self.is_terminated();
        if else_falls_through {
            self.terminate(TerminatorKind::Jump(join_block), anchor);
        }
        self.current = join_block;
        if !then_falls_through && !else_falls_through {
            self.terminate(TerminatorKind::Unreachable, anchor);
        }
        Ok(())
    }

    fn lower_loop_on(
        &mut self,
        condition: &HirExpr,
        body: &[HirStmt],
        anchor: SourceAnchor,
    ) -> Result<(), LowerError> {
        let condition_block = self.new_block(anchor)?;
        let body_block = self.new_block(anchor)?;
        let exit_block = self.new_block(anchor)?;
        self.terminate(TerminatorKind::Jump(condition_block), anchor);

        self.current = condition_block;
        let condition = self.require_value(condition)?;
        self.terminate(
            TerminatorKind::Branch {
                condition,
                then_block: body_block,
                else_block: exit_block,
            },
            anchor,
        );

        self.current = body_block;
        self.loops.push((condition_block, exit_block));
        let result = self.lower_statements(body);
        self.loops.pop();
        result?;
        if !self.is_terminated() {
            self.terminate(TerminatorKind::Jump(condition_block), anchor);
        }
        self.current = exit_block;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_loop_iter(
        &mut self,
        iterator: LocalId,
        iterable: &HirExpr,
        step: Option<&HirExpr>,
        filter: Option<&HirExpr>,
        body: &[HirStmt],
        else_body: Option<&[HirStmt]>,
        anchor: SourceAnchor,
    ) -> Result<(), LowerError> {
        let iterable_slot = self.require_value(iterable)?;
        let step_slot = if let Some(step) = step {
            self.require_value(step)?
        } else {
            let constant = self.intern_constant(Constant::Num(1.0), anchor)?;
            let slot = self.allocate_slot(TypeId::NUM, anchor)?;
            self.emit(
                InstructionKind::Const {
                    dst: slot,
                    constant,
                },
                anchor,
            );
            slot
        };
        let index = self.allocate_slot(TypeId::NUM, anchor)?;
        self.emit(
            InstructionKind::IterInit {
                dst: index,
                iterable: iterable_slot,
            },
            anchor,
        );
        let item_type = match self.types.get(iterable.ty) {
            Some(SemType::Range) => TypeId::NUM,
            Some(SemType::List(inner)) => *inner,
            Some(SemType::Str) => TypeId::STR,
            _ => TypeId::UNKNOWN,
        };
        let item = self.allocate_slot(item_type, anchor)?;
        self.locals.insert(iterator, item);

        let condition_block = self.new_block(anchor)?;
        let dispatch_block = self.new_block(anchor)?;
        let body_block = self.new_block(anchor)?;
        let else_block = self.new_block(anchor)?;
        let increment_block = self.new_block(anchor)?;
        let exit_block = self.new_block(anchor)?;
        self.terminate(TerminatorKind::Jump(condition_block), anchor);

        self.current = condition_block;
        let has_next = self.allocate_slot(TypeId::BOOL, anchor)?;
        self.emit(
            InstructionKind::IterHasNext {
                dst: has_next,
                iterable: iterable_slot,
                index,
            },
            anchor,
        );
        self.terminate(
            TerminatorKind::Branch {
                condition: has_next,
                then_block: dispatch_block,
                else_block: exit_block,
            },
            anchor,
        );

        self.current = dispatch_block;
        self.emit(
            InstructionKind::IterGet {
                dst: item,
                iterable: iterable_slot,
                index,
            },
            anchor,
        );
        if let Some(filter) = filter {
            let condition = self.require_value(filter)?;
            self.terminate(
                TerminatorKind::Branch {
                    condition,
                    then_block: body_block,
                    else_block,
                },
                anchor,
            );
        } else {
            self.terminate(TerminatorKind::Jump(body_block), anchor);
        }

        self.current = body_block;
        self.loops.push((increment_block, exit_block));
        self.lower_statements(body)?;
        self.loops.pop();
        if !self.is_terminated() {
            self.terminate(TerminatorKind::Jump(increment_block), anchor);
        }

        self.current = else_block;
        if let Some(else_body) = else_body {
            self.lower_statements(else_body)?;
        }
        if !self.is_terminated() {
            self.terminate(TerminatorKind::Jump(increment_block), anchor);
        }

        self.current = increment_block;
        self.emit(
            InstructionKind::AddNum {
                dst: index,
                lhs: index,
                rhs: step_slot,
            },
            anchor,
        );
        self.terminate(TerminatorKind::Jump(condition_block), anchor);
        self.current = exit_block;
        Ok(())
    }

    fn lower_on_with_errors(
        &mut self,
        condition: &HirExpr,
        body: &[HirStmt],
        else_body: Option<&[HirStmt]>,
        error_clauses: &[crate::hir::HirErrorClause],
        anchor: SourceAnchor,
    ) -> Result<(), LowerError> {
        let condition_type = condition.ty;
        let condition = self.require_value(condition)?;
        let is_error = self.allocate_slot(TypeId::BOOL, anchor)?;
        self.emit(
            InstructionKind::IsError {
                dst: is_error,
                value: condition,
            },
            anchor,
        );
        let error_dispatch = self.new_block(anchor)?;
        let normal_dispatch = self.new_block(anchor)?;
        let then_block = self.new_block(anchor)?;
        let else_block = self.new_block(anchor)?;
        let join_block = self.new_block(anchor)?;
        self.terminate(
            TerminatorKind::Branch {
                condition: is_error,
                then_block: error_dispatch,
                else_block: normal_dispatch,
            },
            anchor,
        );

        self.current = normal_dispatch;
        if condition_type == TypeId::VOID {
            // A non-error result from `void!` is successful even though plain
            // Void is not generally truthy.
            self.terminate(TerminatorKind::Jump(then_block), anchor);
        } else {
            let is_truthy = self.allocate_slot(TypeId::BOOL, anchor)?;
            self.emit(
                InstructionKind::IsTruthy {
                    dst: is_truthy,
                    value: condition,
                },
                anchor,
            );
            self.terminate(
                TerminatorKind::Branch {
                    condition: is_truthy,
                    then_block,
                    else_block,
                },
                anchor,
            );
        }

        self.current = then_block;
        self.lower_statements(body)?;
        if !self.is_terminated() {
            self.terminate(TerminatorKind::Jump(join_block), anchor);
        }

        self.current = else_block;
        if let Some(else_body) = else_body {
            self.lower_statements(else_body)?;
        }
        if !self.is_terminated() {
            self.terminate(TerminatorKind::Jump(join_block), anchor);
        }

        self.current = error_dispatch;
        for clause in error_clauses {
            let clause_block = self.new_block(anchor)?;
            if let Some(error) = clause.error {
                let next = self.new_block(anchor)?;
                let matches = self.allocate_slot(TypeId::BOOL, anchor)?;
                self.emit(
                    InstructionKind::ErrorMatches {
                        dst: matches,
                        value: condition,
                        error,
                    },
                    anchor,
                );
                self.terminate(
                    TerminatorKind::Branch {
                        condition: matches,
                        then_block: clause_block,
                        else_block: next,
                    },
                    anchor,
                );
                self.current = next;
            } else {
                self.terminate(TerminatorKind::Jump(clause_block), anchor);
            }

            let next_dispatch = self.current;
            self.current = clause_block;
            self.lower_statements(&clause.body)?;
            if !self.is_terminated() {
                self.terminate(TerminatorKind::Jump(join_block), anchor);
            }
            self.current = next_dispatch;
            if clause.error.is_none() {
                break;
            }
        }
        if !self.is_terminated() {
            self.terminate(TerminatorKind::Throw(condition), anchor);
        }
        self.current = join_block;
        Ok(())
    }

    fn lower_expr(&mut self, expression: &HirExpr) -> Result<Option<SlotId>, LowerError> {
        let result = match &expression.kind {
            HirExprKind::Bool(value) => self.lower_constant(Constant::Bool(*value), expression)?,
            HirExprKind::Number(value) => self.lower_constant(Constant::Num(*value), expression)?,
            HirExprKind::String(value) => {
                self.lower_constant(Constant::Str(value.clone()), expression)?
            }
            HirExprKind::Local(local) => {
                self.read_local(*local, expression.ty, expression.anchor)?
            }
            HirExprKind::Move(local) => {
                let destination = self.allocate_slot(expression.ty, expression.anchor)?;
                if let Some(source) = self.locals.get(local).copied() {
                    self.emit(
                        InstructionKind::Move {
                            dst: destination,
                            src: source,
                        },
                        expression.anchor,
                    );
                } else if let Some(global) = self.global(*local) {
                    self.emit(
                        InstructionKind::MoveGlobal {
                            dst: destination,
                            global,
                        },
                        expression.anchor,
                    );
                } else {
                    return Err(unsupported(expression.anchor, "unresolved move"));
                }
                destination
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.require_value(lhs)?;
                let rhs = self.require_value(rhs)?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                let kind = if *op == HirBinaryOp::RangeNum {
                    InstructionKind::MakeRange {
                        dst,
                        start: lhs,
                        end: rhs,
                    }
                } else {
                    binary_instruction(*op, dst, lhs, rhs)
                        .ok_or_else(|| unsupported(expression.anchor, "binary operation"))?
                };
                self.emit(kind, expression.anchor);
                dst
            }
            HirExprKind::Call { kind, args } => {
                let args = args
                    .iter()
                    .map(|argument| self.require_value(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                // A failable `void!` call still produces a runtime value: either
                // Void or the declared error. Keep a destination for every call
                // so that value can be bound and inspected by an error clause.
                let dst = Some(self.allocate_slot(expression.ty, expression.anchor)?);
                let args = args.into_boxed_slice();
                let instruction = match kind {
                    HirCallKind::Direct(function) => InstructionKind::CallDirect {
                        dst,
                        function: *function,
                        args,
                    },
                    HirCallKind::Host(function) => InstructionKind::CallHost {
                        dst,
                        function: *function,
                        args,
                    },
                    HirCallKind::Indirect(callee) => InstructionKind::CallIndirect {
                        dst,
                        callee: self.require_value(callee)?,
                        args,
                    },
                };
                self.emit(instruction, expression.anchor);
                return Ok(dst);
            }
            HirExprKind::ListInit(items) => {
                let items = items
                    .iter()
                    .map(|item| self.require_value(item))
                    .collect::<Result<Vec<_>, _>>()?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::MakeList {
                        dst,
                        items: items.into_boxed_slice(),
                    },
                    expression.anchor,
                );
                dst
            }
            HirExprKind::MapInit(entries) => {
                let entries = entries
                    .iter()
                    .map(|entry| {
                        Ok((
                            self.require_value(&entry.key)?,
                            self.require_value(&entry.value)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, LowerError>>()?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::MakeMap {
                        dst,
                        entries: entries.into_boxed_slice(),
                    },
                    expression.anchor,
                );
                dst
            }
            HirExprKind::StructInit { structure, fields } => {
                let fields = fields
                    .iter()
                    .map(|field| Ok((field.field, self.require_value(&field.value)?)))
                    .collect::<Result<Vec<_>, LowerError>>()?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::MakeStruct {
                        dst,
                        structure: *structure,
                        fields: fields.into_boxed_slice(),
                    },
                    expression.anchor,
                );
                dst
            }
            HirExprKind::FieldAccess { target, field } => {
                let target = self.require_value(target)?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::GetField {
                        dst,
                        target,
                        field: *field,
                    },
                    expression.anchor,
                );
                dst
            }
            HirExprKind::At { collection, key } => {
                let collection = self.require_value(collection)?;
                let key = self.require_value(key)?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::GetIndex {
                        dst,
                        collection,
                        key,
                    },
                    expression.anchor,
                );
                dst
            }
            HirExprKind::Push { collection, value } => {
                let global = match collection.kind {
                    HirExprKind::Local(local) if !self.locals.contains_key(&local) => {
                        self.global(local)
                    }
                    _ => None,
                };
                let collection = self.require_value(collection)?;
                let value = self.require_value(value)?;
                self.emit(
                    InstructionKind::Push { collection, value },
                    expression.anchor,
                );
                if let Some(global) = global {
                    self.emit(
                        InstructionKind::StoreGlobal {
                            global,
                            src: collection,
                        },
                        expression.anchor,
                    );
                }
                return Ok(None);
            }
            HirExprKind::Module(_) => return Err(unsupported(expression.anchor, "module value")),
            HirExprKind::Function(function) => {
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::MakeFunction {
                        dst,
                        function: *function,
                    },
                    expression.anchor,
                );
                dst
            }
            HirExprKind::HostFunction(function) => {
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::MakeHostFunction {
                        dst,
                        function: *function,
                    },
                    expression.anchor,
                );
                dst
            }
            HirExprKind::Error(error) => {
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::MakeError { dst, error: *error },
                    expression.anchor,
                );
                dst
            }
            HirExprKind::AddressInit => {
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(InstructionKind::MakeAddress { dst }, expression.anchor);
                dst
            }
            HirExprKind::PipeInit { capacity } => {
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::MakePipe {
                        dst,
                        capacity: *capacity,
                    },
                    expression.anchor,
                );
                dst
            }
            HirExprKind::Take(channel) => {
                let channel = self.require_value(channel)?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(InstructionKind::Take { dst, channel }, expression.anchor);
                dst
            }
            HirExprKind::Len(collection) => {
                let collection = self.require_value(collection)?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(InstructionKind::Len { dst, collection }, expression.anchor);
                dst
            }
            HirExprKind::Ref(value) => {
                let value = self.require_value(value)?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(InstructionKind::MakeRef { dst, value }, expression.anchor);
                dst
            }
            HirExprKind::Deref(address) => {
                let address = self.require_value(address)?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(InstructionKind::Deref { dst, address }, expression.anchor);
                dst
            }
            HirExprKind::RunCall(call) => {
                let HirExprKind::Call {
                    kind: HirCallKind::Direct(function),
                    args,
                } = &call.kind
                else {
                    return Err(unsupported(expression.anchor, "non-direct run call"));
                };
                let args = args
                    .iter()
                    .map(|argument| self.require_value(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                self.emit(
                    InstructionKind::Spawn {
                        function: *function,
                        args: args.into_boxed_slice(),
                    },
                    expression.anchor,
                );
                return Ok(None);
            }
        };
        Ok(Some(result))
    }

    fn require_value(&mut self, expression: &HirExpr) -> Result<SlotId, LowerError> {
        self.lower_expr(expression)?
            .ok_or_else(|| invalid(expression.anchor, "void expression used as a value"))
    }

    fn lower_constant(
        &mut self,
        value: Constant,
        expression: &HirExpr,
    ) -> Result<SlotId, LowerError> {
        let constant = self.intern_constant(value, expression.anchor)?;
        let dst = self.allocate_slot(expression.ty, expression.anchor)?;
        self.emit(InstructionKind::Const { dst, constant }, expression.anchor);
        Ok(dst)
    }

    fn intern_constant(
        &mut self,
        value: Constant,
        anchor: SourceAnchor,
    ) -> Result<ConstId, LowerError> {
        let constant = if let Some(index) = self.constants.iter().position(|item| item == &value) {
            ConstId::try_from(index).map_err(|error| invalid(anchor, error))?
        } else {
            let id =
                ConstId::try_from(self.constants.len()).map_err(|error| invalid(anchor, error))?;
            self.constants.push(value);
            id
        };
        Ok(constant)
    }

    fn read_local(
        &mut self,
        local: LocalId,
        ty: TypeId,
        anchor: SourceAnchor,
    ) -> Result<SlotId, LowerError> {
        if let Some(slot) = self.locals.get(&local) {
            return Ok(*slot);
        }
        let Some(global) = self.global(local) else {
            return Err(unsupported(anchor, "unresolved local access"));
        };
        let dst = self.allocate_slot(ty, anchor)?;
        self.emit(InstructionKind::LoadGlobal { dst, global }, anchor);
        Ok(dst)
    }

    fn global(&self, local: LocalId) -> Option<GlobalId> {
        self.globals.get(&(self.module, local)).copied()
    }

    fn allocate_slot(&mut self, ty: TypeId, anchor: SourceAnchor) -> Result<SlotId, LowerError> {
        let slot = SlotId::try_from(self.slots.len()).map_err(|error| invalid(anchor, error))?;
        self.slots.push(ty);
        Ok(slot)
    }

    fn emit(&mut self, kind: InstructionKind, anchor: SourceAnchor) {
        self.current_block_mut()
            .instructions
            .push(Instruction { kind, anchor });
    }

    fn terminate(&mut self, kind: TerminatorKind, anchor: SourceAnchor) {
        let previous = self
            .current_block_mut()
            .terminator
            .replace(Terminator { kind, anchor });
        debug_assert!(previous.is_none(), "basic block terminated twice");
    }

    fn new_block(&mut self, anchor: SourceAnchor) -> Result<BlockId, LowerError> {
        let id = BlockId::try_from(self.blocks.len()).map_err(|error| invalid(anchor, error))?;
        self.blocks.push(DraftBlock {
            instructions: Vec::new(),
            terminator: None,
        });
        Ok(id)
    }

    fn current_block_mut(&mut self) -> &mut DraftBlock {
        let index = usize::try_from(self.current).expect("valid block ID must fit usize");
        &mut self.blocks[index]
    }

    fn is_terminated(&self) -> bool {
        let index = usize::try_from(self.current).expect("valid block ID must fit usize");
        self.blocks[index].terminator.is_some()
    }

    fn finish(
        mut self,
        _module: &HirModule,
        end_anchor: SourceAnchor,
    ) -> Result<EirFunction, LowerError> {
        if !self.is_terminated() {
            if self.signature.return_type() == TypeId::VOID {
                self.terminate(TerminatorKind::Return(None), end_anchor);
            } else {
                return Err(invalid(
                    self.anchor,
                    format!("function {} can reach its end without returning", self.name),
                ));
            }
        }
        let blocks = self
            .blocks
            .into_iter()
            .map(|block| BasicBlock {
                instructions: block.instructions,
                terminator: block.terminator.unwrap_or(Terminator {
                    kind: TerminatorKind::Unreachable,
                    anchor: end_anchor,
                }),
            })
            .collect();
        Ok(EirFunction {
            id: self.id,
            name: self.name,
            signature: self.signature,
            slots: self.slots,
            parameter_count: self.parameter_count,
            blocks,
        })
    }
}

fn field_assignment_path(target: &HirExpr) -> Option<(LocalId, TypeId, Vec<FieldId>)> {
    let mut fields = Vec::new();
    let mut current = target;
    loop {
        match &current.kind {
            HirExprKind::FieldAccess { target, field } => {
                fields.push(*field);
                current = target;
            }
            HirExprKind::Local(local) => {
                fields.reverse();
                return Some((*local, current.ty, fields));
            }
            _ => return None,
        }
    }
}

fn ordered_initializers(hir: &HirProgram, by_module: &[FunctionId]) -> Vec<FunctionId> {
    fn visit(
        index: usize,
        hir: &HirProgram,
        by_module: &[FunctionId],
        state: &mut [u8],
        output: &mut Vec<FunctionId>,
    ) {
        if state[index] == 2 {
            return;
        }
        if state[index] == 1 {
            return;
        }
        state[index] = 1;
        for statement in &hir.modules[index].statements {
            if let HirStmtKind::Import(module) = statement.kind
                && let Ok(dependency) = usize::try_from(module)
                && dependency < hir.modules.len()
            {
                visit(dependency, hir, by_module, state, output);
            }
        }
        state[index] = 2;
        output.push(by_module[index]);
    }

    let mut state = vec![0; hir.modules.len()];
    let mut output = Vec::with_capacity(by_module.len());
    for index in 0..hir.modules.len() {
        visit(index, hir, by_module, &mut state, &mut output);
    }
    output
}

fn binary_instruction(
    op: HirBinaryOp,
    dst: SlotId,
    lhs: SlotId,
    rhs: SlotId,
) -> Option<InstructionKind> {
    let kind = match op {
        HirBinaryOp::AddNum => InstructionKind::AddNum { dst, lhs, rhs },
        HirBinaryOp::ConcatString => InstructionKind::ConcatString { dst, lhs, rhs },
        HirBinaryOp::SubNum => InstructionKind::SubNum { dst, lhs, rhs },
        HirBinaryOp::MulNum => InstructionKind::MulNum { dst, lhs, rhs },
        HirBinaryOp::DivNum => InstructionKind::DivNum { dst, lhs, rhs },
        HirBinaryOp::EqNum => InstructionKind::EqNum { dst, lhs, rhs },
        HirBinaryOp::EqString => InstructionKind::EqString { dst, lhs, rhs },
        HirBinaryOp::EqBool => InstructionKind::EqBool { dst, lhs, rhs },
        HirBinaryOp::NeNum => InstructionKind::NeNum { dst, lhs, rhs },
        HirBinaryOp::NeString => InstructionKind::NeString { dst, lhs, rhs },
        HirBinaryOp::NeBool => InstructionKind::NeBool { dst, lhs, rhs },
        HirBinaryOp::GtNum => InstructionKind::GtNum { dst, lhs, rhs },
        HirBinaryOp::LtNum => InstructionKind::LtNum { dst, lhs, rhs },
        HirBinaryOp::GeNum => InstructionKind::GeNum { dst, lhs, rhs },
        HirBinaryOp::LeNum => InstructionKind::LeNum { dst, lhs, rhs },
        HirBinaryOp::RangeNum => return None,
    };
    Some(kind)
}

fn unsupported(anchor: SourceAnchor, operation: &'static str) -> LowerError {
    LowerError {
        anchor,
        kind: LowerErrorKind::Unsupported { operation },
    }
}

fn invalid(anchor: SourceAnchor, message: impl fmt::Display) -> LowerError {
    LowerError {
        anchor,
        kind: LowerErrorKind::InvalidProgram(message.to_string()),
    }
}

fn fallback_anchor(program: &HirProgram) -> SourceAnchor {
    program
        .modules
        .first()
        .map(fallback_module_anchor)
        .unwrap_or_else(|| {
            SourceAnchor::try_from_offsets(crate::hir::SourceId::new(0), 0, 0)
                .expect("zero source anchor is valid")
        })
}

fn fallback_module_anchor(module: &HirModule) -> SourceAnchor {
    module.functions.first().map_or_else(
        || {
            SourceAnchor::try_from_offsets(crate::hir::SourceId::new(module.id.raw()), 0, 0)
                .expect("zero source anchor is valid")
        },
        |function| function.anchor,
    )
}
