use std::fmt;
use std::time::Instant;

use crate::eir::{
    BlockId, Constant, EirProgram, Instruction, InstructionKind, SlotId, Terminator,
    TerminatorKind, VerifyError, verify_program,
};
use crate::hir::{FunctionId, SemType, SourceAnchor, SourceId, TypeId};

use super::values::RuntimeVal;
use super::{CancellationToken, InterpreterLimits};

#[derive(Debug)]
pub struct EirRuntimeError {
    pub anchor: SourceAnchor,
    pub kind: EirRuntimeErrorKind,
}

#[derive(Debug)]
pub enum EirRuntimeErrorKind {
    Verification(Vec<VerifyError>),
    InvalidFunction(FunctionId),
    ArgumentCount {
        expected: usize,
        actual: usize,
    },
    TypeMismatch {
        expected: String,
        actual: &'static str,
    },
    UninitializedSlot(SlotId),
    StepLimitExceeded {
        steps: u64,
        limit: u64,
    },
    CallDepthExceeded {
        depth: usize,
        limit: usize,
    },
    TimeoutExceeded {
        milliseconds: u128,
    },
    Cancelled,
    ReachedUnreachable,
    Thrown(Box<RuntimeVal>),
    ReentrantExecution,
}

impl fmt::Display for EirRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "EIR runtime error at {}:{}..{}: ",
            self.anchor.source().raw(),
            self.anchor.start(),
            self.anchor.end()
        )?;
        match &self.kind {
            EirRuntimeErrorKind::Verification(errors) => {
                write!(formatter, "{} verifier error(s)", errors.len())
            }
            EirRuntimeErrorKind::InvalidFunction(function) => {
                write!(formatter, "invalid function f{}", function.raw())
            }
            EirRuntimeErrorKind::ArgumentCount { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected} argument(s), received {actual}"
                )
            }
            EirRuntimeErrorKind::TypeMismatch { expected, actual } => {
                write!(formatter, "expected {expected}, received {actual}")
            }
            EirRuntimeErrorKind::UninitializedSlot(slot) => {
                write!(formatter, "read uninitialized slot s{}", slot.raw())
            }
            EirRuntimeErrorKind::StepLimitExceeded { steps, limit } => {
                write!(formatter, "step limit exceeded ({steps} > {limit})")
            }
            EirRuntimeErrorKind::CallDepthExceeded { depth, limit } => {
                write!(formatter, "call depth exceeded ({depth} > {limit})")
            }
            EirRuntimeErrorKind::TimeoutExceeded { milliseconds } => {
                write!(formatter, "timeout exceeded (>{milliseconds} ms)")
            }
            EirRuntimeErrorKind::Cancelled => formatter.write_str("execution cancelled"),
            EirRuntimeErrorKind::ReachedUnreachable => {
                formatter.write_str("reached an unreachable terminator")
            }
            EirRuntimeErrorKind::Thrown(value) => write!(formatter, "uncaught throw: {value}"),
            EirRuntimeErrorKind::ReentrantExecution => {
                formatter.write_str("runtime already has active frames")
            }
        }
    }
}

impl std::error::Error for EirRuntimeError {}

struct Frame {
    function: FunctionId,
    block: BlockId,
    instruction: usize,
    slots: Vec<Option<RuntimeVal>>,
    return_destination: Option<SlotId>,
}

pub struct EirRuntime<'program> {
    program: &'program EirProgram,
    frames: Vec<Frame>,
    limits: InterpreterLimits,
    cancellation: Option<CancellationToken>,
    step_count: u64,
    started_at: Option<Instant>,
}

impl<'program> EirRuntime<'program> {
    pub fn new(program: &'program EirProgram) -> Result<Self, EirRuntimeError> {
        if let Err(errors) = verify_program(program) {
            let anchor = errors
                .first()
                .map_or_else(|| fallback_anchor(program), |error| error.anchor);
            return Err(EirRuntimeError {
                anchor,
                kind: EirRuntimeErrorKind::Verification(errors),
            });
        }
        Ok(Self {
            program,
            frames: Vec::new(),
            limits: InterpreterLimits::default(),
            cancellation: None,
            step_count: 0,
            started_at: None,
        })
    }

    pub fn set_limits(&mut self, limits: InterpreterLimits) {
        self.limits = limits;
    }

    pub fn set_cancellation_token(&mut self, cancellation: CancellationToken) {
        self.cancellation = Some(cancellation);
    }

    pub const fn step_count(&self) -> u64 {
        self.step_count
    }

    pub fn run_initializers(&mut self) -> Result<(), EirRuntimeError> {
        for index in 0..self.program.module_initializers.len() {
            let function = self.program.module_initializers[index];
            self.call_function(function, Vec::new())?;
        }
        Ok(())
    }

    pub fn call_function(
        &mut self,
        function: FunctionId,
        args: Vec<RuntimeVal>,
    ) -> Result<RuntimeVal, EirRuntimeError> {
        if !self.frames.is_empty() {
            return Err(runtime_error(
                fallback_anchor(self.program),
                EirRuntimeErrorKind::ReentrantExecution,
            ));
        }
        if self.program.function(function).is_none() {
            return Err(runtime_error(
                fallback_anchor(self.program),
                EirRuntimeErrorKind::InvalidFunction(function),
            ));
        }
        let anchor = function_anchor(self.program, function);
        self.push_frame(function, args, None, anchor)?;
        let result = self.run_frames();
        if result.is_err() {
            self.frames.clear();
        }
        result
    }

    fn push_frame(
        &mut self,
        function: FunctionId,
        args: Vec<RuntimeVal>,
        return_destination: Option<SlotId>,
        anchor: SourceAnchor,
    ) -> Result<(), EirRuntimeError> {
        let function_index = usize::try_from(function).expect("verified function ID fits usize");
        let metadata = &self.program.functions[function_index];
        if args.len() != metadata.signature.params().len() {
            return Err(runtime_error(
                anchor,
                EirRuntimeErrorKind::ArgumentCount {
                    expected: metadata.signature.params().len(),
                    actual: args.len(),
                },
            ));
        }
        for (value, ty) in args.iter().zip(metadata.signature.params()) {
            if !value_matches_type(value, *ty, self.program) {
                return Err(runtime_error(
                    anchor,
                    EirRuntimeErrorKind::TypeMismatch {
                        expected: type_name(*ty, self.program),
                        actual: runtime_value_name(value),
                    },
                ));
            }
        }
        let depth = self.frames.len() + 1;
        if let Some(limit) = self.limits.max_call_depth
            && depth > limit
        {
            return Err(runtime_error(
                anchor,
                EirRuntimeErrorKind::CallDepthExceeded { depth, limit },
            ));
        }

        let mut slots = vec![None; metadata.slots.len()];
        for (slot, value) in slots.iter_mut().zip(args) {
            *slot = Some(value);
        }
        self.frames.push(Frame {
            function,
            block: BlockId::new(0),
            instruction: 0,
            slots,
            return_destination,
        });
        Ok(())
    }

    fn run_frames(&mut self) -> Result<RuntimeVal, EirRuntimeError> {
        loop {
            let anchor = {
                let frame = self.frames.last().expect("execution has a root frame");
                active_anchor(self.program, frame)
            };
            self.tick(anchor)?;
            let action = {
                let frame = self.frames.last_mut().expect("execution has a root frame");
                step_frame(self.program, frame)?
            };
            match action {
                StepAction::Continue => {}
                StepAction::Call {
                    function,
                    args,
                    destination,
                } => self.push_frame(function, args, destination, anchor)?,
                StepAction::Return(value) => {
                    let completed = self.frames.pop().expect("return has an active frame");
                    let Some(caller) = self.frames.last_mut() else {
                        return Ok(value);
                    };
                    if let Some(destination) = completed.return_destination {
                        write_slot(caller, destination, value, anchor)?;
                    }
                }
                StepAction::Throw(value) => {
                    self.frames.clear();
                    return Err(runtime_error(
                        anchor,
                        EirRuntimeErrorKind::Thrown(Box::new(value)),
                    ));
                }
            }
        }
    }

    fn tick(&mut self, anchor: SourceAnchor) -> Result<(), EirRuntimeError> {
        let started_at = *self.started_at.get_or_insert_with(Instant::now);
        self.step_count = self.step_count.saturating_add(1);
        if self
            .cancellation
            .as_ref()
            .is_some_and(CancellationToken::is_cancelled)
        {
            self.frames.clear();
            return Err(runtime_error(anchor, EirRuntimeErrorKind::Cancelled));
        }
        if let Some(limit) = self.limits.max_steps
            && self.step_count > limit
        {
            self.frames.clear();
            return Err(runtime_error(
                anchor,
                EirRuntimeErrorKind::StepLimitExceeded {
                    steps: self.step_count,
                    limit,
                },
            ));
        }
        if let Some(timeout) = self.limits.timeout
            && started_at.elapsed() > timeout
        {
            self.frames.clear();
            return Err(runtime_error(
                anchor,
                EirRuntimeErrorKind::TimeoutExceeded {
                    milliseconds: timeout.as_millis(),
                },
            ));
        }
        Ok(())
    }
}

enum StepAction {
    Continue,
    Call {
        function: FunctionId,
        args: Vec<RuntimeVal>,
        destination: Option<SlotId>,
    },
    Return(RuntimeVal),
    Throw(RuntimeVal),
}

fn step_frame(program: &EirProgram, frame: &mut Frame) -> Result<StepAction, EirRuntimeError> {
    let function_index = usize::try_from(frame.function).expect("verified function ID fits usize");
    let function = &program.functions[function_index];
    let block_index = usize::try_from(frame.block).expect("verified block ID fits usize");
    let block = &function.blocks[block_index];
    if let Some(instruction) = block.instructions.get(frame.instruction) {
        frame.instruction += 1;
        execute_instruction(program, frame, instruction)
    } else {
        execute_terminator(frame, &block.terminator)
    }
}

fn active_anchor(program: &EirProgram, frame: &Frame) -> SourceAnchor {
    let function_index = usize::try_from(frame.function).expect("verified function ID fits usize");
    let block_index = usize::try_from(frame.block).expect("verified block ID fits usize");
    let block = &program.functions[function_index].blocks[block_index];
    block
        .instructions
        .get(frame.instruction)
        .map_or(block.terminator.anchor, |instruction| instruction.anchor)
}

fn execute_instruction(
    program: &EirProgram,
    frame: &mut Frame,
    instruction: &Instruction,
) -> Result<StepAction, EirRuntimeError> {
    let anchor = instruction.anchor;
    match &instruction.kind {
        InstructionKind::Const { dst, constant } => {
            let index = usize::try_from(*constant).expect("verified constant ID fits usize");
            let value = match &program.constants[index] {
                Constant::Bool(value) => RuntimeVal::Bool(*value),
                Constant::Num(value) => RuntimeVal::Float(*value),
                Constant::Str(value) => RuntimeVal::String(value.clone()),
            };
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::Copy { dst, src } => {
            let value = read_slot(frame, *src, anchor)?.clone();
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::Move { dst, src } => {
            let value = take_slot(frame, *src, anchor)?;
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::AddNum { dst, lhs, rhs } => {
            execute_num_binary(frame, *dst, *lhs, *rhs, anchor, |left, right| left + right)?;
        }
        InstructionKind::SubNum { dst, lhs, rhs } => {
            execute_num_binary(frame, *dst, *lhs, *rhs, anchor, |left, right| left - right)?;
        }
        InstructionKind::MulNum { dst, lhs, rhs } => {
            execute_num_binary(frame, *dst, *lhs, *rhs, anchor, |left, right| left * right)?;
        }
        InstructionKind::DivNum { dst, lhs, rhs } => {
            execute_num_binary(frame, *dst, *lhs, *rhs, anchor, |left, right| left / right)?;
        }
        InstructionKind::ConcatString { dst, lhs, rhs } => {
            let left = read_string(frame, *lhs, anchor)?;
            let right = read_string(frame, *rhs, anchor)?;
            let mut value = String::with_capacity(left.len() + right.len());
            value.push_str(left);
            value.push_str(right);
            write_slot(frame, *dst, RuntimeVal::String(value), anchor)?;
        }
        InstructionKind::EqNum { dst, lhs, rhs } => {
            execute_num_compare(frame, *dst, *lhs, *rhs, anchor, |left, right| left == right)?;
        }
        InstructionKind::NeNum { dst, lhs, rhs } => {
            execute_num_compare(frame, *dst, *lhs, *rhs, anchor, |left, right| left != right)?;
        }
        InstructionKind::GtNum { dst, lhs, rhs } => {
            execute_num_compare(frame, *dst, *lhs, *rhs, anchor, |left, right| left > right)?;
        }
        InstructionKind::LtNum { dst, lhs, rhs } => {
            execute_num_compare(frame, *dst, *lhs, *rhs, anchor, |left, right| left < right)?;
        }
        InstructionKind::GeNum { dst, lhs, rhs } => {
            execute_num_compare(frame, *dst, *lhs, *rhs, anchor, |left, right| left >= right)?;
        }
        InstructionKind::LeNum { dst, lhs, rhs } => {
            execute_num_compare(frame, *dst, *lhs, *rhs, anchor, |left, right| left <= right)?;
        }
        InstructionKind::EqString { dst, lhs, rhs } => {
            let value = read_string(frame, *lhs, anchor)? == read_string(frame, *rhs, anchor)?;
            write_slot(frame, *dst, RuntimeVal::Bool(value), anchor)?;
        }
        InstructionKind::NeString { dst, lhs, rhs } => {
            let value = read_string(frame, *lhs, anchor)? != read_string(frame, *rhs, anchor)?;
            write_slot(frame, *dst, RuntimeVal::Bool(value), anchor)?;
        }
        InstructionKind::EqBool { dst, lhs, rhs } => {
            let value = read_bool(frame, *lhs, anchor)? == read_bool(frame, *rhs, anchor)?;
            write_slot(frame, *dst, RuntimeVal::Bool(value), anchor)?;
        }
        InstructionKind::NeBool { dst, lhs, rhs } => {
            let value = read_bool(frame, *lhs, anchor)? != read_bool(frame, *rhs, anchor)?;
            write_slot(frame, *dst, RuntimeVal::Bool(value), anchor)?;
        }
        InstructionKind::CallDirect {
            dst,
            function,
            args,
        } => {
            let values = args
                .iter()
                .map(|slot| read_slot(frame, *slot, anchor).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(StepAction::Call {
                function: *function,
                args: values,
                destination: *dst,
            });
        }
    }
    Ok(StepAction::Continue)
}

fn execute_terminator(
    frame: &mut Frame,
    terminator: &Terminator,
) -> Result<StepAction, EirRuntimeError> {
    let anchor = terminator.anchor;
    match terminator.kind {
        TerminatorKind::Jump(block) => {
            frame.block = block;
            frame.instruction = 0;
            Ok(StepAction::Continue)
        }
        TerminatorKind::Branch {
            condition,
            then_block,
            else_block,
        } => {
            frame.block = if read_bool(frame, condition, anchor)? {
                then_block
            } else {
                else_block
            };
            frame.instruction = 0;
            Ok(StepAction::Continue)
        }
        TerminatorKind::Return(value) => {
            let value = value
                .map(|slot| take_slot(frame, slot, anchor))
                .transpose()?
                .unwrap_or(RuntimeVal::Void);
            Ok(StepAction::Return(value))
        }
        TerminatorKind::Throw(value) => Ok(StepAction::Throw(take_slot(frame, value, anchor)?)),
        TerminatorKind::Unreachable => Err(runtime_error(
            anchor,
            EirRuntimeErrorKind::ReachedUnreachable,
        )),
    }
}

fn execute_num_binary(
    frame: &mut Frame,
    dst: SlotId,
    lhs: SlotId,
    rhs: SlotId,
    anchor: SourceAnchor,
    operation: impl FnOnce(f64, f64) -> f64,
) -> Result<(), EirRuntimeError> {
    let value = operation(
        read_number(frame, lhs, anchor)?,
        read_number(frame, rhs, anchor)?,
    );
    write_slot(frame, dst, RuntimeVal::Float(value), anchor)
}

fn execute_num_compare(
    frame: &mut Frame,
    dst: SlotId,
    lhs: SlotId,
    rhs: SlotId,
    anchor: SourceAnchor,
    operation: impl FnOnce(f64, f64) -> bool,
) -> Result<(), EirRuntimeError> {
    let value = operation(
        read_number(frame, lhs, anchor)?,
        read_number(frame, rhs, anchor)?,
    );
    write_slot(frame, dst, RuntimeVal::Bool(value), anchor)
}

fn read_number(frame: &Frame, slot: SlotId, anchor: SourceAnchor) -> Result<f64, EirRuntimeError> {
    match read_slot(frame, slot, anchor)? {
        RuntimeVal::Float(value) => Ok(*value),
        actual => Err(type_mismatch(anchor, "num", actual)),
    }
}

fn read_string(frame: &Frame, slot: SlotId, anchor: SourceAnchor) -> Result<&str, EirRuntimeError> {
    match read_slot(frame, slot, anchor)? {
        RuntimeVal::String(value) => Ok(value),
        actual => Err(type_mismatch(anchor, "str", actual)),
    }
}

fn read_bool(frame: &Frame, slot: SlotId, anchor: SourceAnchor) -> Result<bool, EirRuntimeError> {
    match read_slot(frame, slot, anchor)? {
        RuntimeVal::Bool(value) => Ok(*value),
        actual => Err(type_mismatch(anchor, "bool", actual)),
    }
}

fn read_slot(
    frame: &Frame,
    slot: SlotId,
    anchor: SourceAnchor,
) -> Result<&RuntimeVal, EirRuntimeError> {
    let index = usize::try_from(slot).expect("verified slot ID fits usize");
    frame.slots[index]
        .as_ref()
        .ok_or_else(|| runtime_error(anchor, EirRuntimeErrorKind::UninitializedSlot(slot)))
}

fn take_slot(
    frame: &mut Frame,
    slot: SlotId,
    anchor: SourceAnchor,
) -> Result<RuntimeVal, EirRuntimeError> {
    let index = usize::try_from(slot).expect("verified slot ID fits usize");
    frame.slots[index]
        .take()
        .ok_or_else(|| runtime_error(anchor, EirRuntimeErrorKind::UninitializedSlot(slot)))
}

fn write_slot(
    frame: &mut Frame,
    slot: SlotId,
    value: RuntimeVal,
    _anchor: SourceAnchor,
) -> Result<(), EirRuntimeError> {
    let index = usize::try_from(slot).expect("verified slot ID fits usize");
    frame.slots[index] = Some(value);
    Ok(())
}

fn value_matches_type(value: &RuntimeVal, ty: TypeId, program: &EirProgram) -> bool {
    matches!(
        (value, program.types.get(ty)),
        (RuntimeVal::Float(_), Some(SemType::Num))
            | (RuntimeVal::String(_), Some(SemType::Str))
            | (RuntimeVal::Bool(_), Some(SemType::Bool))
            | (RuntimeVal::Void, Some(SemType::Void))
    )
}

fn type_name(ty: TypeId, program: &EirProgram) -> String {
    program
        .types
        .get(ty)
        .map_or_else(|| format!("t{}", ty.raw()), |value| format!("{value:?}"))
}

const fn runtime_value_name(value: &RuntimeVal) -> &'static str {
    match value {
        RuntimeVal::Float(_) => "num",
        RuntimeVal::String(_) => "str",
        RuntimeVal::Bool(_) => "bool",
        RuntimeVal::Range(_, _) => "range",
        RuntimeVal::Void => "void",
        RuntimeVal::Pipe(_, _) => "pipe",
        RuntimeVal::Struct(_, _) => "struct",
        RuntimeVal::List(_) => "list",
        RuntimeVal::Map(_) => "map",
        RuntimeVal::Handle(_) => "handle",
        RuntimeVal::Module(_, _) => "module",
        RuntimeVal::FunctionRef(_) => "function",
        RuntimeVal::Error(_, _) => "error",
        RuntimeVal::Pointer(_) => "pointer",
        RuntimeVal::AdrHandle(_) => "address",
        RuntimeVal::Moved => "moved",
    }
}

fn type_mismatch(
    anchor: SourceAnchor,
    expected: impl Into<String>,
    actual: &RuntimeVal,
) -> EirRuntimeError {
    runtime_error(
        anchor,
        EirRuntimeErrorKind::TypeMismatch {
            expected: expected.into(),
            actual: runtime_value_name(actual),
        },
    )
}

fn runtime_error(anchor: SourceAnchor, kind: EirRuntimeErrorKind) -> EirRuntimeError {
    EirRuntimeError { anchor, kind }
}

fn function_anchor(program: &EirProgram, function: FunctionId) -> SourceAnchor {
    let index = usize::try_from(function).expect("verified function ID fits usize");
    program.functions[index]
        .blocks
        .first()
        .map_or_else(|| fallback_anchor(program), |block| block.terminator.anchor)
}

fn fallback_anchor(program: &EirProgram) -> SourceAnchor {
    program
        .functions
        .first()
        .and_then(|function| function.blocks.first())
        .map_or_else(
            || {
                SourceAnchor::try_from_offsets(SourceId::new(0), 0, 0)
                    .expect("zero source anchor is valid")
            },
            |block| block.terminator.anchor,
        )
}
