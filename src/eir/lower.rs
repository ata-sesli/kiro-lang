use std::collections::BTreeMap;
use std::fmt;

use crate::hir::{
    Effects, FunctionId, HirBinaryOp, HirCallKind, HirExpr, HirExprKind, HirFunction, HirModule,
    HirProgram, HirStmt, HirStmtKind, LocalId, Signature, SourceAnchor, TypeId,
};

use super::{
    BasicBlock, BlockId, ConstId, Constant, EirFunction, EirProgram, Instruction, InstructionKind,
    SlotId, Terminator, TerminatorKind, VerifyError, verify_program,
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
        lowered.push(lower_function(module, function, &mut constants)?);
    }

    let mut module_initializers = Vec::with_capacity(hir.modules.len());
    for module in &hir.modules {
        let id = FunctionId::try_from(lowered.len()).map_err(|error| invalid(fallback, error))?;
        lowered.push(lower_initializer(module, id, &mut constants)?);
        module_initializers.push(id);
    }

    let program = EirProgram {
        types: hir.types.clone(),
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
    constants: &mut Vec<Constant>,
) -> Result<EirFunction, LowerError> {
    let mut builder = FunctionBuilder::new(
        function.id,
        function.name.clone(),
        function.signature.clone(),
        function.anchor,
        constants,
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
    constants: &mut Vec<Constant>,
) -> Result<EirFunction, LowerError> {
    let anchor = module.statements.first().map_or_else(
        || fallback_module_anchor(module),
        |statement| statement.anchor,
    );
    let mut builder = FunctionBuilder::new(
        id,
        format!("{}::$init", module.name),
        Signature::new([], TypeId::VOID, Effects::NONE),
        anchor,
        constants,
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
    id: FunctionId,
    name: String,
    signature: Signature,
    anchor: SourceAnchor,
    constants: &'a mut Vec<Constant>,
    slots: Vec<TypeId>,
    locals: BTreeMap<LocalId, SlotId>,
    parameter_count: u32,
    blocks: Vec<DraftBlock>,
    current: BlockId,
    loops: Vec<(BlockId, BlockId)>,
}

impl<'a> FunctionBuilder<'a> {
    fn new(
        id: FunctionId,
        name: String,
        signature: Signature,
        anchor: SourceAnchor,
        constants: &'a mut Vec<Constant>,
    ) -> Self {
        Self {
            id,
            name,
            signature,
            anchor,
            constants,
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
            HirStmtKind::Assign { target, value } => {
                let HirExprKind::Local(local) = target.kind else {
                    return Err(unsupported(statement.anchor, "non-local assignment"));
                };
                let value_slot = self.require_value(value)?;
                let destination = self.local(local, target.anchor)?;
                self.emit(
                    InstructionKind::Copy {
                        dst: destination,
                        src: value_slot,
                    },
                    statement.anchor,
                );
            }
            HirStmtKind::On {
                condition,
                body,
                else_body,
                error_clauses,
            } => {
                if !error_clauses.is_empty() {
                    return Err(unsupported(statement.anchor, "conditional error clauses"));
                }
                self.lower_on(condition, body, else_body.as_deref(), statement.anchor)?;
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
            HirStmtKind::LoopIter { .. } => {
                return Err(unsupported(statement.anchor, "iterator loop"));
            }
            HirStmtKind::Give { .. } => return Err(unsupported(statement.anchor, "give")),
            HirStmtKind::Close(_) => return Err(unsupported(statement.anchor, "close")),
            HirStmtKind::Rest => return Err(unsupported(statement.anchor, "rest")),
            HirStmtKind::Check { .. } => return Err(unsupported(statement.anchor, "check")),
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

    fn lower_expr(&mut self, expression: &HirExpr) -> Result<Option<SlotId>, LowerError> {
        let result = match &expression.kind {
            HirExprKind::Bool(value) => self.lower_constant(Constant::Bool(*value), expression)?,
            HirExprKind::Number(value) => self.lower_constant(Constant::Num(*value), expression)?,
            HirExprKind::String(value) => {
                self.lower_constant(Constant::Str(value.clone()), expression)?
            }
            HirExprKind::Local(local) => self.local(*local, expression.anchor)?,
            HirExprKind::Move(local) => {
                let source = self.local(*local, expression.anchor)?;
                let destination = self.allocate_slot(expression.ty, expression.anchor)?;
                self.emit(
                    InstructionKind::Move {
                        dst: destination,
                        src: source,
                    },
                    expression.anchor,
                );
                destination
            }
            HirExprKind::Binary { op, lhs, rhs } => {
                let lhs = self.require_value(lhs)?;
                let rhs = self.require_value(rhs)?;
                let dst = self.allocate_slot(expression.ty, expression.anchor)?;
                let kind = binary_instruction(*op, dst, lhs, rhs)
                    .ok_or_else(|| unsupported(expression.anchor, "numeric range"))?;
                self.emit(kind, expression.anchor);
                dst
            }
            HirExprKind::Call { kind, args } => {
                let HirCallKind::Direct(function) = kind else {
                    let operation = match kind {
                        HirCallKind::Host(_) => "host call",
                        HirCallKind::Indirect(_) => "indirect call",
                        HirCallKind::Direct(_) => unreachable!(),
                    };
                    return Err(unsupported(expression.anchor, operation));
                };
                let args = args
                    .iter()
                    .map(|argument| self.require_value(argument))
                    .collect::<Result<Vec<_>, _>>()?;
                let dst = if expression.ty == TypeId::VOID {
                    None
                } else {
                    Some(self.allocate_slot(expression.ty, expression.anchor)?)
                };
                self.emit(
                    InstructionKind::CallDirect {
                        dst,
                        function: *function,
                        args: args.into_boxed_slice(),
                    },
                    expression.anchor,
                );
                return Ok(dst);
            }
            HirExprKind::ListInit(_) => {
                return Err(unsupported(expression.anchor, "list initialization"));
            }
            HirExprKind::MapInit(_) => {
                return Err(unsupported(expression.anchor, "map initialization"));
            }
            HirExprKind::StructInit { .. } => {
                return Err(unsupported(expression.anchor, "struct initialization"));
            }
            HirExprKind::FieldAccess { .. } => {
                return Err(unsupported(expression.anchor, "field access"));
            }
            HirExprKind::At { .. } => {
                return Err(unsupported(expression.anchor, "collection access"));
            }
            HirExprKind::Push { .. } => return Err(unsupported(expression.anchor, "push")),
            HirExprKind::Module(_) => return Err(unsupported(expression.anchor, "module value")),
            HirExprKind::Function(_) => {
                return Err(unsupported(expression.anchor, "function value"));
            }
            HirExprKind::HostFunction(_) => {
                return Err(unsupported(expression.anchor, "host function value"));
            }
            HirExprKind::Error(_) => return Err(unsupported(expression.anchor, "error value")),
            HirExprKind::AddressInit => {
                return Err(unsupported(expression.anchor, "address initialization"));
            }
            HirExprKind::PipeInit { .. } => {
                return Err(unsupported(expression.anchor, "pipe initialization"));
            }
            HirExprKind::Take(_) => return Err(unsupported(expression.anchor, "take")),
            HirExprKind::Len(_) => return Err(unsupported(expression.anchor, "length")),
            HirExprKind::Ref(_) => return Err(unsupported(expression.anchor, "reference")),
            HirExprKind::Deref(_) => return Err(unsupported(expression.anchor, "dereference")),
            HirExprKind::RunCall(_) => return Err(unsupported(expression.anchor, "run call")),
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
        let constant = if let Some(index) = self.constants.iter().position(|item| item == &value) {
            ConstId::try_from(index).map_err(|error| invalid(expression.anchor, error))?
        } else {
            let id = ConstId::try_from(self.constants.len())
                .map_err(|error| invalid(expression.anchor, error))?;
            self.constants.push(value);
            id
        };
        let dst = self.allocate_slot(expression.ty, expression.anchor)?;
        self.emit(InstructionKind::Const { dst, constant }, expression.anchor);
        Ok(dst)
    }

    fn local(&self, local: LocalId, anchor: SourceAnchor) -> Result<SlotId, LowerError> {
        self.locals
            .get(&local)
            .copied()
            .ok_or_else(|| unsupported(anchor, "module-global local access"))
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
