use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::eir::{
    BlockId, Constant, EirProgram, Instruction, InstructionKind, SlotId, Terminator,
    TerminatorKind, VerifyError, verify_program,
};
use crate::hir::{FunctionId, SemType, SourceAnchor, SourceId, TypeId};

use super::values::RuntimeVal;
use super::{
    CancellationToken, HostCallCtx, HostFnHandler, HostMode, HostRegistry, InterpreterLimits,
};

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
    UninitializedGlobal(crate::eir::GlobalId),
    PoisonedGlobals,
    EmptyAddress,
    PoisonedAddress,
    MissingField(crate::hir::FieldId),
    ListIndexOutOfBounds {
        index: usize,
        length: usize,
    },
    InvalidListRange {
        start: f64,
        end: f64,
        length: usize,
    },
    BytesIndexOutOfBounds {
        index: usize,
        length: usize,
    },
    MapKeyNotFound(String),
    CheckFailed(String),
    InvalidCallable(String),
    HostCallDenied {
        module: String,
        function: String,
    },
    HostFunctionNotRegistered {
        module: String,
        function: String,
    },
    HostCallFailed(String),
    PipeClosed,
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
            EirRuntimeErrorKind::UninitializedGlobal(global) => {
                write!(formatter, "read uninitialized global g{}", global.raw())
            }
            EirRuntimeErrorKind::PoisonedGlobals => {
                formatter.write_str("module global storage is poisoned")
            }
            EirRuntimeErrorKind::EmptyAddress => {
                formatter.write_str("dereferenced an empty address")
            }
            EirRuntimeErrorKind::PoisonedAddress => {
                formatter.write_str("address storage is poisoned")
            }
            EirRuntimeErrorKind::MissingField(field) => {
                write!(formatter, "missing struct field field{}", field.raw())
            }
            EirRuntimeErrorKind::ListIndexOutOfBounds { index, length } => {
                write!(
                    formatter,
                    "list index {index} out of bounds for length {length}"
                )
            }
            EirRuntimeErrorKind::InvalidListRange { start, end, length } => write!(
                formatter,
                "invalid list range {start}..{end} for length {length}"
            ),
            EirRuntimeErrorKind::BytesIndexOutOfBounds { index, length } => {
                write!(
                    formatter,
                    "byte index {index} out of bounds for length {length}"
                )
            }
            EirRuntimeErrorKind::MapKeyNotFound(key) => {
                write!(formatter, "map key not found: {key}")
            }
            EirRuntimeErrorKind::CheckFailed(message) => {
                write!(formatter, "check failed: {message}")
            }
            EirRuntimeErrorKind::InvalidCallable(value) => {
                write!(formatter, "invalid callable {value}")
            }
            EirRuntimeErrorKind::HostCallDenied { module, function } => {
                write!(formatter, "host call denied for '{module}.{function}'")
            }
            EirRuntimeErrorKind::HostFunctionNotRegistered { module, function } => {
                write!(
                    formatter,
                    "host function '{module}.{function}' is not registered"
                )
            }
            EirRuntimeErrorKind::HostCallFailed(message) => formatter.write_str(message),
            EirRuntimeErrorKind::PipeClosed => formatter.write_str("pipe is closed"),
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EirRuntimeStats {
    pub steps_executed: u64,
    pub frames_pushed: u64,
    pub peak_frame_depth: usize,
    pub peak_live_slots: usize,
}

pub struct EirRuntime<'program> {
    program: &'program EirProgram,
    frames: Vec<Frame>,
    globals: Arc<Mutex<Vec<Option<RuntimeVal>>>>,
    limits: InterpreterLimits,
    cancellation: Option<CancellationToken>,
    step_count: u64,
    started_at: Option<Instant>,
    host_mode: HostMode,
    host_registry: HostRegistry,
    spawned: Vec<JoinHandle<Result<RuntimeVal, EirRuntimeError>>>,
    stats: EirRuntimeStats,
    live_slots: usize,
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
            globals: Arc::new(Mutex::new(vec![None; program.globals.len()])),
            limits: InterpreterLimits::default(),
            cancellation: None,
            step_count: 0,
            started_at: None,
            host_mode: HostMode::default(),
            host_registry: HostRegistry::default(),
            spawned: Vec::new(),
            stats: EirRuntimeStats::default(),
            live_slots: 0,
        })
    }

    pub fn set_limits(&mut self, limits: InterpreterLimits) {
        self.limits = limits;
    }

    pub fn set_cancellation_token(&mut self, cancellation: CancellationToken) {
        self.cancellation = Some(cancellation);
    }

    pub fn set_host_mode(&mut self, mode: HostMode) {
        self.host_mode = mode;
    }

    pub fn register_host_fn(
        &mut self,
        module: impl Into<String>,
        name: impl Into<String>,
        handler: HostFnHandler,
    ) {
        self.host_registry.register(module, name, handler);
    }

    pub const fn step_count(&self) -> u64 {
        self.step_count
    }

    pub const fn stats(&self) -> EirRuntimeStats {
        EirRuntimeStats {
            steps_executed: self.step_count,
            frames_pushed: self.stats.frames_pushed,
            peak_frame_depth: self.stats.peak_frame_depth,
            peak_live_slots: self.stats.peak_live_slots,
        }
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
            self.live_slots = 0;
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
        let argument_count = args.len();
        let slots = prepare_frame_slots(
            self.program,
            function,
            argument_count,
            args.into_iter().map(Ok),
            anchor,
        )?;
        self.push_prepared_frame(function, slots, return_destination, anchor)
    }

    fn push_prepared_frame(
        &mut self,
        function: FunctionId,
        slots: Vec<Option<RuntimeVal>>,
        return_destination: Option<SlotId>,
        anchor: SourceAnchor,
    ) -> Result<(), EirRuntimeError> {
        let depth = self.frames.len() + 1;
        if let Some(limit) = self.limits.max_call_depth
            && depth > limit
        {
            return Err(runtime_error(
                anchor,
                EirRuntimeErrorKind::CallDepthExceeded { depth, limit },
            ));
        }

        let slot_count = slots.len();
        self.frames.push(Frame {
            function,
            block: BlockId::new(0),
            instruction: 0,
            slots,
            return_destination,
        });
        self.live_slots += slot_count;
        self.stats.frames_pushed += 1;
        self.stats.peak_frame_depth = self.stats.peak_frame_depth.max(depth);
        self.stats.peak_live_slots = self.stats.peak_live_slots.max(self.live_slots);
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
                step_frame(self.program, &self.globals, frame)?
            };
            match action {
                StepAction::Continue => {}
                StepAction::Call {
                    function,
                    args,
                    destination,
                } => self.push_frame(function, args, destination, anchor)?,
                StepAction::PreparedCall {
                    function,
                    slots,
                    destination,
                } => self.push_prepared_frame(function, slots, destination, anchor)?,
                StepAction::HostCall {
                    function,
                    args,
                    destination,
                } => {
                    let value = self.call_host(function, args, anchor)?;
                    if let Some(destination) = destination {
                        let frame = self.frames.last_mut().expect("host call has caller frame");
                        write_slot(frame, destination, value, anchor)?;
                    }
                }
                StepAction::Spawn { function, args } => self.spawn(function, args, anchor)?,
                StepAction::Return(value) => {
                    let completed = self.frames.pop().expect("return has an active frame");
                    self.live_slots -= completed.slots.len();
                    let Some(caller) = self.frames.last_mut() else {
                        for task in self.spawned.drain(..) {
                            task.join().map_err(|_| {
                                runtime_error(
                                    anchor,
                                    EirRuntimeErrorKind::HostCallFailed(
                                        "spawned task panicked".to_string(),
                                    ),
                                )
                            })??;
                        }
                        return Ok(value);
                    };
                    if let Some(destination) = completed.return_destination {
                        write_slot(caller, destination, value, anchor)?;
                    }
                }
                StepAction::Throw(value) => {
                    self.frames.clear();
                    self.live_slots = 0;
                    return Err(runtime_error(
                        anchor,
                        EirRuntimeErrorKind::Thrown(Box::new(value)),
                    ));
                }
            }
        }
    }

    fn call_host(
        &self,
        function: crate::hir::HostFunctionId,
        args: Vec<RuntimeVal>,
        anchor: SourceAnchor,
    ) -> Result<RuntimeVal, EirRuntimeError> {
        let metadata =
            &self.program.host_functions[usize::try_from(function).expect("verified host ID")];
        match self.host_mode {
            HostMode::Deny => Err(runtime_error(
                anchor,
                EirRuntimeErrorKind::HostCallDenied {
                    module: metadata.module.clone(),
                    function: metadata.name.clone(),
                },
            )),
            HostMode::Simulate => Ok(mock_value(metadata.signature.return_type(), self.program)),
            HostMode::Execute => {
                if crate::is_std_io_module_name(&metadata.module)
                    && crate::is_std_io_display_function(&metadata.name)
                {
                    let value = args.first().ok_or_else(|| {
                        runtime_error(
                            anchor,
                            EirRuntimeErrorKind::ArgumentCount {
                                expected: 1,
                                actual: 0,
                            },
                        )
                    })?;
                    match metadata.name.as_str() {
                        "print" => println!("{value}"),
                        "write" => {
                            print!("{value}");
                            io::stdout().flush().map_err(|error| {
                                runtime_error(
                                    anchor,
                                    EirRuntimeErrorKind::HostCallFailed(error.to_string()),
                                )
                            })?;
                        }
                        "eprint" => {
                            eprint!("{value}");
                            io::stderr().flush().map_err(|error| {
                                runtime_error(
                                    anchor,
                                    EirRuntimeErrorKind::HostCallFailed(error.to_string()),
                                )
                            })?;
                        }
                        "eprintline" => eprintln!("{value}"),
                        _ => unreachable!("display helper checked above"),
                    }
                    return Ok(RuntimeVal::Void);
                }
                let handler = self
                    .host_registry
                    .get(&metadata.module, &metadata.name)
                    .ok_or_else(|| {
                        runtime_error(
                            anchor,
                            EirRuntimeErrorKind::HostFunctionNotRegistered {
                                module: metadata.module.clone(),
                                function: metadata.name.clone(),
                            },
                        )
                    })?;
                let args = args
                    .iter()
                    .zip(metadata.signature.params())
                    .map(|(value, ty)| self.to_host_runtime_typed(value, *ty))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|message| {
                        runtime_error(anchor, EirRuntimeErrorKind::HostCallFailed(message))
                    })?;
                let value = match handler(
                    HostCallCtx {
                        module_name: metadata.module.clone(),
                        function_name: metadata.name.clone(),
                        step_count: self.step_count,
                    },
                    args,
                ) {
                    Ok(value) => value,
                    Err(error) => {
                        if let Some(error_id) = self.program.error_id_by_name(&error.name) {
                            return Ok(RuntimeVal::Error(
                                format!("error{}", error_id.raw()),
                                error.message.unwrap_or_default(),
                            ));
                        }
                        return Err(runtime_error(
                            anchor,
                            EirRuntimeErrorKind::HostCallFailed(error.to_string()),
                        ));
                    }
                };
                self.decode_host_runtime_typed(value, metadata.signature.return_type())
                    .map_err(|message| {
                        runtime_error(anchor, EirRuntimeErrorKind::HostCallFailed(message))
                    })
            }
        }
    }

    fn to_host_runtime_typed(
        &self,
        value: &RuntimeVal,
        ty: TypeId,
    ) -> Result<kiro_runtime::RuntimeVal, String> {
        match self.program.types.get(ty) {
            Some(SemType::Struct(id)) => {
                let RuntimeVal::Struct(_, values) = value else {
                    return Err("Type Error: expected struct value".to_string());
                };
                let record = self
                    .program
                    .struct_def(*id)
                    .ok_or_else(|| format!("missing EIR struct {}", id.raw()))?;
                let mut fields = HashMap::with_capacity(record.fields.len());
                for field in &record.fields {
                    let key = format!("field{}", field.id.raw());
                    let value = values
                        .get(&key)
                        .ok_or_else(|| format!("missing field {}.{}", record.name, field.name))?;
                    fields.insert(
                        field.name.clone(),
                        self.to_host_runtime_typed(value, field.ty)?,
                    );
                }
                Ok(kiro_runtime::RuntimeVal::structure(&record.name, fields))
            }
            Some(SemType::List(inner)) => {
                let RuntimeVal::List(values) = value else {
                    return Err("Type Error: expected list value".to_string());
                };
                values
                    .iter()
                    .map(|value| self.to_host_runtime_typed(value, *inner))
                    .collect::<Result<Vec<_>, _>>()
                    .map(kiro_runtime::RuntimeVal::List)
            }
            Some(SemType::Map(_, value_ty)) => {
                let RuntimeVal::Map(values) = value else {
                    return Err("Type Error: expected map value".to_string());
                };
                values
                    .iter()
                    .map(|(key, value)| {
                        Ok((key.clone(), self.to_host_runtime_typed(value, *value_ty)?))
                    })
                    .collect::<Result<HashMap<_, _>, String>>()
                    .map(kiro_runtime::RuntimeVal::Map)
            }
            _ => value.to_host_runtime(),
        }
    }

    fn decode_host_runtime_typed(
        &self,
        value: kiro_runtime::RuntimeVal,
        ty: TypeId,
    ) -> Result<RuntimeVal, String> {
        match self.program.types.get(ty) {
            Some(SemType::Struct(id)) => {
                let record = self
                    .program
                    .struct_def(*id)
                    .ok_or_else(|| format!("missing EIR struct {}", id.raw()))?;
                let kiro_runtime::RuntimeVal::Struct {
                    type_name,
                    mut fields,
                } = value
                else {
                    return Err(format!("Type Error: expected struct {}", record.name));
                };
                if type_name != record.name {
                    return Err(format!(
                        "Type Error: expected struct {}, got {}",
                        record.name, type_name
                    ));
                }
                let mut values = HashMap::with_capacity(record.fields.len());
                for field in &record.fields {
                    let value = fields.remove(&field.name).ok_or_else(|| {
                        format!("Type Error: missing field {}.{}", record.name, field.name)
                    })?;
                    values.insert(
                        format!("field{}", field.id.raw()),
                        self.decode_host_runtime_typed(value, field.ty)?,
                    );
                }
                Ok(RuntimeVal::Struct(format!("struct{}", id.raw()), values))
            }
            Some(SemType::List(inner)) => {
                let kiro_runtime::RuntimeVal::List(values) = value else {
                    return Err("Type Error: expected list".to_string());
                };
                values
                    .into_iter()
                    .map(|value| self.decode_host_runtime_typed(value, *inner))
                    .collect::<Result<Vec<_>, _>>()
                    .map(RuntimeVal::List)
            }
            Some(SemType::Map(_, value_ty)) => {
                let kiro_runtime::RuntimeVal::Map(values) = value else {
                    return Err("Type Error: expected map".to_string());
                };
                values
                    .into_iter()
                    .map(|(key, value)| {
                        Ok((key, self.decode_host_runtime_typed(value, *value_ty)?))
                    })
                    .collect::<Result<HashMap<_, _>, String>>()
                    .map(RuntimeVal::Map)
            }
            _ => RuntimeVal::from_host_runtime(value),
        }
    }

    fn spawn(
        &mut self,
        function: FunctionId,
        args: Vec<RuntimeVal>,
        anchor: SourceAnchor,
    ) -> Result<(), EirRuntimeError> {
        let program = self.program.clone();
        let limits = self.limits.clone();
        let cancellation = self.cancellation.clone();
        let host_mode = self.host_mode;
        let host_registry = self.host_registry.clone();
        let globals = self.globals.clone();
        self.spawned.push(std::thread::spawn(move || {
            let mut runtime = EirRuntime::new(&program)?;
            runtime.globals = globals;
            runtime.set_limits(limits);
            runtime.host_mode = host_mode;
            runtime.host_registry = host_registry;
            if let Some(cancellation) = cancellation {
                runtime.set_cancellation_token(cancellation);
            }
            runtime.call_function(function, args)
        }));
        let _ = anchor;
        Ok(())
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
    PreparedCall {
        function: FunctionId,
        slots: Vec<Option<RuntimeVal>>,
        destination: Option<SlotId>,
    },
    HostCall {
        function: crate::hir::HostFunctionId,
        args: Vec<RuntimeVal>,
        destination: Option<SlotId>,
    },
    Spawn {
        function: FunctionId,
        args: Vec<RuntimeVal>,
    },
    Return(RuntimeVal),
    Throw(RuntimeVal),
}

fn step_frame(
    program: &EirProgram,
    globals: &Arc<Mutex<Vec<Option<RuntimeVal>>>>,
    frame: &mut Frame,
) -> Result<StepAction, EirRuntimeError> {
    let function_index = usize::try_from(frame.function).expect("verified function ID fits usize");
    let function = &program.functions[function_index];
    let block_index = usize::try_from(frame.block).expect("verified block ID fits usize");
    let block = &function.blocks[block_index];
    if let Some(instruction) = block.instructions.get(frame.instruction) {
        frame.instruction += 1;
        execute_instruction(program, globals, frame, instruction)
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
    globals: &Arc<Mutex<Vec<Option<RuntimeVal>>>>,
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
        InstructionKind::MoveGlobal { dst, global } => {
            let index = usize::try_from(*global).expect("verified global ID fits usize");
            let mut globals = globals
                .lock()
                .map_err(|_| runtime_error(anchor, EirRuntimeErrorKind::PoisonedGlobals))?;
            let value = globals[index].take().ok_or_else(|| {
                runtime_error(anchor, EirRuntimeErrorKind::UninitializedGlobal(*global))
            })?;
            drop(globals);
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::LoadGlobal { dst, global } => {
            let index = usize::try_from(*global).expect("verified global ID fits usize");
            let globals = globals
                .lock()
                .map_err(|_| runtime_error(anchor, EirRuntimeErrorKind::PoisonedGlobals))?;
            let value = globals[index].as_ref().cloned().ok_or_else(|| {
                runtime_error(anchor, EirRuntimeErrorKind::UninitializedGlobal(*global))
            })?;
            drop(globals);
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::StoreGlobal { global, src } => {
            let index = usize::try_from(*global).expect("verified global ID fits usize");
            let value = read_slot(frame, *src, anchor)?.clone();
            globals
                .lock()
                .map_err(|_| runtime_error(anchor, EirRuntimeErrorKind::PoisonedGlobals))?[index] =
                Some(value);
        }
        InstructionKind::MakeError { dst, error } => {
            write_slot(
                frame,
                *dst,
                RuntimeVal::Error(format!("error{}", error.raw()), String::new()),
                anchor,
            )?;
        }
        InstructionKind::MakeFunction { dst, function } => {
            write_slot(
                frame,
                *dst,
                RuntimeVal::FunctionRef(format!("f{}", function.raw())),
                anchor,
            )?;
        }
        InstructionKind::MakeHostFunction { dst, function } => {
            write_slot(
                frame,
                *dst,
                RuntimeVal::FunctionRef(format!("h{}", function.raw())),
                anchor,
            )?;
        }
        InstructionKind::IsError { dst, value } => {
            let value = matches!(read_slot(frame, *value, anchor)?, RuntimeVal::Error(_, _));
            write_slot(frame, *dst, RuntimeVal::Bool(value), anchor)?;
        }
        InstructionKind::ErrorMatches { dst, value, error } => {
            let expected = format!("error{}", error.raw());
            let matches = matches!(
                read_slot(frame, *value, anchor)?,
                RuntimeVal::Error(actual, _) if actual == &expected
            );
            write_slot(frame, *dst, RuntimeVal::Bool(matches), anchor)?;
        }
        InstructionKind::IsTruthy { dst, value } => {
            let value = read_slot(frame, *value, anchor)?.is_truthy();
            write_slot(frame, *dst, RuntimeVal::Bool(value), anchor)?;
        }
        InstructionKind::Check { condition, message } => {
            if !read_bool(frame, *condition, anchor)? {
                let message = message
                    .and_then(|message| {
                        usize::try_from(message)
                            .ok()
                            .and_then(|index| program.constants.get(index))
                    })
                    .and_then(|constant| match constant {
                        Constant::Str(message) => Some(message.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| "check failed".to_string());
                return Err(runtime_error(
                    anchor,
                    EirRuntimeErrorKind::CheckFailed(message),
                ));
            }
        }
        InstructionKind::MakeAddress { dst } => {
            write_slot(frame, *dst, RuntimeVal::AdrHandle(None), anchor)?;
        }
        InstructionKind::MakeRef { dst, value } => {
            let value = read_slot(frame, *value, anchor)?.clone();
            write_slot(
                frame,
                *dst,
                RuntimeVal::Pointer(Arc::new(Mutex::new(value))),
                anchor,
            )?;
        }
        InstructionKind::Deref { dst, address } => {
            let target = address_target(read_slot(frame, *address, anchor)?, anchor)?;
            let value = target
                .lock()
                .map_err(|_| runtime_error(anchor, EirRuntimeErrorKind::PoisonedAddress))?
                .clone();
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::StoreDeref { address, src } => {
            let value = read_slot(frame, *src, anchor)?.clone();
            let target = address_target(read_slot(frame, *address, anchor)?, anchor)?;
            *target
                .lock()
                .map_err(|_| runtime_error(anchor, EirRuntimeErrorKind::PoisonedAddress))? = value;
        }
        InstructionKind::MakeList { dst, items } => {
            let values = items
                .iter()
                .map(|slot| read_slot(frame, *slot, anchor).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            write_slot(frame, *dst, RuntimeVal::List(values), anchor)?;
        }
        InstructionKind::MakeMap { dst, entries } => {
            let mut values = HashMap::with_capacity(entries.len());
            for (key, value) in entries {
                let key = map_key(read_slot(frame, *key, anchor)?);
                let value = read_slot(frame, *value, anchor)?.clone();
                values.insert(key, value);
            }
            write_slot(frame, *dst, RuntimeVal::Map(values), anchor)?;
        }
        InstructionKind::ListJoin { dst, left, right } => {
            let RuntimeVal::List(left) = read_slot(frame, *left, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "list",
                    read_slot(frame, *left, anchor)?,
                ));
            };
            let RuntimeVal::List(right) = read_slot(frame, *right, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "list",
                    read_slot(frame, *right, anchor)?,
                ));
            };
            let mut values = Vec::with_capacity(left.len() + right.len());
            values.extend_from_slice(left);
            values.extend_from_slice(right);
            write_slot(frame, *dst, RuntimeVal::List(values), anchor)?;
        }
        InstructionKind::ListSlice {
            dst,
            list,
            start,
            end,
        } => {
            let start = read_number(frame, *start, anchor)?;
            let end = read_number(frame, *end, anchor)?;
            let RuntimeVal::List(values) = read_slot(frame, *list, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "list",
                    read_slot(frame, *list, anchor)?,
                ));
            };
            let valid = start.is_finite()
                && end.is_finite()
                && start.fract() == 0.0
                && end.fract() == 0.0
                && start >= 0.0
                && start <= end
                && end <= values.len() as f64;
            if !valid {
                return Err(runtime_error(
                    anchor,
                    EirRuntimeErrorKind::InvalidListRange {
                        start,
                        end,
                        length: values.len(),
                    },
                ));
            }
            write_slot(
                frame,
                *dst,
                RuntimeVal::List(values[start as usize..end as usize].to_vec()),
                anchor,
            )?;
        }
        InstructionKind::ListReverse { dst, list } => {
            let RuntimeVal::List(values) = read_slot(frame, *list, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "list",
                    read_slot(frame, *list, anchor)?,
                ));
            };
            let mut values = values.clone();
            values.reverse();
            write_slot(frame, *dst, RuntimeVal::List(values), anchor)?;
        }
        InstructionKind::MapHas { dst, map, key } => {
            let key = map_key(read_slot(frame, *key, anchor)?);
            let RuntimeVal::Map(values) = read_slot(frame, *map, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "map",
                    read_slot(frame, *map, anchor)?,
                ));
            };
            write_slot(
                frame,
                *dst,
                RuntimeVal::Bool(values.contains_key(&key)),
                anchor,
            )?;
        }
        InstructionKind::MapSet {
            dst,
            map,
            key,
            value,
        } => {
            let key = map_key(read_slot(frame, *key, anchor)?);
            let value = read_slot(frame, *value, anchor)?.clone();
            let RuntimeVal::Map(values) = read_slot(frame, *map, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "map",
                    read_slot(frame, *map, anchor)?,
                ));
            };
            let mut values = values.clone();
            values.insert(key, value);
            write_slot(frame, *dst, RuntimeVal::Map(values), anchor)?;
        }
        InstructionKind::MapDelete { dst, map, key } => {
            let key = map_key(read_slot(frame, *key, anchor)?);
            let RuntimeVal::Map(values) = read_slot(frame, *map, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "map",
                    read_slot(frame, *map, anchor)?,
                ));
            };
            let mut values = values.clone();
            values.remove(&key);
            write_slot(frame, *dst, RuntimeVal::Map(values), anchor)?;
        }
        InstructionKind::MakeStruct {
            dst,
            structure,
            fields,
        } => {
            let mut values = HashMap::with_capacity(fields.len());
            for (field, value) in fields {
                values.insert(field_key(*field), read_slot(frame, *value, anchor)?.clone());
            }
            write_slot(
                frame,
                *dst,
                RuntimeVal::Struct(format!("struct{}", structure.raw()), values),
                anchor,
            )?;
        }
        InstructionKind::GetField { dst, target, field } => {
            let RuntimeVal::Struct(_, fields) = read_slot(frame, *target, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "struct",
                    read_slot(frame, *target, anchor)?,
                ));
            };
            let value = fields
                .get(&field_key(*field))
                .cloned()
                .ok_or_else(|| runtime_error(anchor, EirRuntimeErrorKind::MissingField(*field)))?;
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::SetField {
            target,
            fields,
            src,
        } => {
            let value = read_slot(frame, *src, anchor)?.clone();
            let target = slot_mut(frame, *target, anchor)?;
            set_field_path(target, fields, value, anchor)?;
        }
        InstructionKind::GetIndex {
            dst,
            collection,
            key,
        } => {
            let key = read_slot(frame, *key, anchor)?.clone();
            let value = match read_slot(frame, *collection, anchor)? {
                RuntimeVal::List(items) => {
                    let RuntimeVal::Float(index) = key else {
                        return Err(type_mismatch(anchor, "num", &key));
                    };
                    let index = index as usize;
                    items.get(index).cloned().ok_or_else(|| {
                        runtime_error(
                            anchor,
                            EirRuntimeErrorKind::ListIndexOutOfBounds {
                                index,
                                length: items.len(),
                            },
                        )
                    })?
                }
                RuntimeVal::Map(entries) => {
                    let key = map_key(&key);
                    entries.get(&key).cloned().ok_or_else(|| {
                        runtime_error(anchor, EirRuntimeErrorKind::MapKeyNotFound(key))
                    })?
                }
                RuntimeVal::Bytes(bytes) => {
                    let RuntimeVal::Float(index) = key else {
                        return Err(type_mismatch(anchor, "num", &key));
                    };
                    let index = index as usize;
                    RuntimeVal::Float(*bytes.get(index).ok_or_else(|| {
                        runtime_error(
                            anchor,
                            EirRuntimeErrorKind::BytesIndexOutOfBounds {
                                index,
                                length: bytes.len(),
                            },
                        )
                    })? as f64)
                }
                actual => return Err(type_mismatch(anchor, "bytes, list, or map", actual)),
            };
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::Push { collection, value } => {
            let value = read_slot(frame, *value, anchor)?.clone();
            match slot_mut(frame, *collection, anchor)? {
                RuntimeVal::List(items) => items.push(value),
                actual => return Err(type_mismatch(anchor, "list", actual)),
            }
        }
        InstructionKind::Len { dst, collection } => {
            let length = match read_slot(frame, *collection, anchor)? {
                RuntimeVal::String(value) => value.len(),
                RuntimeVal::Bytes(value) => value.len(),
                RuntimeVal::List(value) => value.len(),
                RuntimeVal::Map(value) => value.len(),
                actual => return Err(type_mismatch(anchor, "str, bytes, list, or map", actual)),
            };
            write_slot(frame, *dst, RuntimeVal::Float(length as f64), anchor)?;
        }
        InstructionKind::MakeRange { dst, start, end } => {
            let start = read_number(frame, *start, anchor)? as i64;
            let end = read_number(frame, *end, anchor)? as i64;
            write_slot(frame, *dst, RuntimeVal::Range(start, end), anchor)?;
        }
        InstructionKind::IterInit { dst, iterable } => {
            let index = match read_slot(frame, *iterable, anchor)? {
                RuntimeVal::Range(start, _) => *start as f64,
                RuntimeVal::List(_) | RuntimeVal::String(_) => 0.0,
                actual => return Err(type_mismatch(anchor, "iterable", actual)),
            };
            write_slot(frame, *dst, RuntimeVal::Float(index), anchor)?;
        }
        InstructionKind::IterHasNext {
            dst,
            iterable,
            index,
        } => {
            let index = read_number(frame, *index, anchor)?;
            let has_next = match read_slot(frame, *iterable, anchor)? {
                RuntimeVal::Range(_, end) => index < *end as f64,
                RuntimeVal::List(items) => index >= 0.0 && (index as usize) < items.len(),
                RuntimeVal::String(text) => index >= 0.0 && (index as usize) < text.chars().count(),
                actual => return Err(type_mismatch(anchor, "iterable", actual)),
            };
            write_slot(frame, *dst, RuntimeVal::Bool(has_next), anchor)?;
        }
        InstructionKind::IterGet {
            dst,
            iterable,
            index,
        } => {
            let index = read_number(frame, *index, anchor)?;
            let value = match read_slot(frame, *iterable, anchor)? {
                RuntimeVal::Range(_, _) => RuntimeVal::Float(index),
                RuntimeVal::List(items) => items[index as usize].clone(),
                RuntimeVal::String(text) => RuntimeVal::String(
                    text.chars()
                        .nth(index as usize)
                        .expect("verified iterator bound")
                        .to_string(),
                ),
                actual => return Err(type_mismatch(anchor, "iterable", actual)),
            };
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
            let slots = prepare_frame_slots(
                program,
                *function,
                args.len(),
                args.iter()
                    .map(|slot| read_slot(frame, *slot, anchor).cloned()),
                anchor,
            )?;
            return Ok(StepAction::PreparedCall {
                function: *function,
                slots,
                destination: *dst,
            });
        }
        InstructionKind::CallHost {
            dst,
            function,
            args,
        } => {
            let values = args
                .iter()
                .map(|slot| read_slot(frame, *slot, anchor).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(StepAction::HostCall {
                function: *function,
                args: values,
                destination: *dst,
            });
        }
        InstructionKind::CallIndirect { dst, callee, args } => {
            let RuntimeVal::FunctionRef(callable) = read_slot(frame, *callee, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "function",
                    read_slot(frame, *callee, anchor)?,
                ));
            };
            let values = args
                .iter()
                .map(|slot| read_slot(frame, *slot, anchor).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(id) = callable
                .strip_prefix('f')
                .and_then(|id| id.parse::<u32>().ok())
            {
                return Ok(StepAction::Call {
                    function: FunctionId::new(id),
                    args: values,
                    destination: *dst,
                });
            }
            if let Some(id) = callable
                .strip_prefix('h')
                .and_then(|id| id.parse::<u32>().ok())
            {
                return Ok(StepAction::HostCall {
                    function: crate::hir::HostFunctionId::new(id),
                    args: values,
                    destination: *dst,
                });
            }
            return Err(runtime_error(
                anchor,
                EirRuntimeErrorKind::InvalidCallable(callable.clone()),
            ));
        }
        InstructionKind::MakePipe { dst, capacity } => {
            let value = if let Some(capacity) = capacity {
                let (sender, receiver) = std::sync::mpsc::sync_channel(*capacity);
                RuntimeVal::Pipe(
                    super::values::PipeSender::Bounded(sender),
                    Arc::new(Mutex::new(receiver)),
                    Arc::new(AtomicBool::new(false)),
                )
            } else {
                let (sender, receiver) = std::sync::mpsc::channel();
                RuntimeVal::Pipe(
                    super::values::PipeSender::Unbounded(sender),
                    Arc::new(Mutex::new(receiver)),
                    Arc::new(AtomicBool::new(false)),
                )
            };
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::Give { channel, value } => {
            let value = read_slot(frame, *value, anchor)?.clone();
            match read_slot(frame, *channel, anchor)? {
                RuntimeVal::Pipe(_, _, closed) if closed.load(Ordering::Acquire) => {
                    return Err(runtime_error(anchor, EirRuntimeErrorKind::PipeClosed));
                }
                RuntimeVal::Pipe(super::values::PipeSender::Unbounded(sender), _, _) => {
                    sender.send(value)
                }
                RuntimeVal::Pipe(super::values::PipeSender::Bounded(sender), _, _) => {
                    sender.send(value)
                }
                actual => return Err(type_mismatch(anchor, "pipe", actual)),
            }
            .map_err(|_| runtime_error(anchor, EirRuntimeErrorKind::PipeClosed))?;
        }
        InstructionKind::Take { dst, channel } => {
            let (receiver, closed) = match read_slot(frame, *channel, anchor)? {
                RuntimeVal::Pipe(_, receiver, closed) => (Arc::clone(receiver), Arc::clone(closed)),
                actual => return Err(type_mismatch(anchor, "pipe", actual)),
            };
            let receiver = receiver
                .lock()
                .map_err(|_| runtime_error(anchor, EirRuntimeErrorKind::PipeClosed))?;
            let value = loop {
                match receiver.recv_timeout(Duration::from_millis(1)) {
                    Ok(value) => break value,
                    Err(RecvTimeoutError::Timeout) if closed.load(Ordering::Acquire) => {
                        return Err(runtime_error(anchor, EirRuntimeErrorKind::PipeClosed));
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        return Err(runtime_error(anchor, EirRuntimeErrorKind::PipeClosed));
                    }
                }
            };
            drop(receiver);
            write_slot(frame, *dst, value, anchor)?;
        }
        InstructionKind::Close { channel } => {
            let RuntimeVal::Pipe(_, _, closed) = read_slot(frame, *channel, anchor)? else {
                return Err(type_mismatch(
                    anchor,
                    "pipe",
                    read_slot(frame, *channel, anchor)?,
                ));
            };
            closed.store(true, Ordering::Release);
        }
        InstructionKind::Rest => std::thread::yield_now(),
        InstructionKind::Spawn { function, args } => {
            let values = args
                .iter()
                .map(|slot| read_slot(frame, *slot, anchor).cloned())
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(StepAction::Spawn {
                function: *function,
                args: values,
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

fn slot_mut(
    frame: &mut Frame,
    slot: SlotId,
    anchor: SourceAnchor,
) -> Result<&mut RuntimeVal, EirRuntimeError> {
    let index = usize::try_from(slot).expect("verified slot ID fits usize");
    frame.slots[index]
        .as_mut()
        .ok_or_else(|| runtime_error(anchor, EirRuntimeErrorKind::UninitializedSlot(slot)))
}

fn set_field_path(
    target: &mut RuntimeVal,
    fields: &[crate::hir::FieldId],
    value: RuntimeVal,
    anchor: SourceAnchor,
) -> Result<(), EirRuntimeError> {
    let Some((field, rest)) = fields.split_first() else {
        return Err(type_mismatch(anchor, "struct field path", target));
    };
    let RuntimeVal::Struct(_, values) = target else {
        return Err(type_mismatch(anchor, "struct", target));
    };
    let target = values
        .get_mut(&field_key(*field))
        .ok_or_else(|| runtime_error(anchor, EirRuntimeErrorKind::MissingField(*field)))?;
    if rest.is_empty() {
        *target = value;
        Ok(())
    } else {
        set_field_path(target, rest, value, anchor)
    }
}

fn field_key(field: crate::hir::FieldId) -> String {
    format!("field{}", field.raw())
}

fn map_key(value: &RuntimeVal) -> String {
    value.to_string()
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

fn prepare_frame_slots(
    program: &EirProgram,
    function: FunctionId,
    argument_count: usize,
    arguments: impl IntoIterator<Item = Result<RuntimeVal, EirRuntimeError>>,
    anchor: SourceAnchor,
) -> Result<Vec<Option<RuntimeVal>>, EirRuntimeError> {
    let function_index = usize::try_from(function).expect("verified function ID fits usize");
    let metadata = &program.functions[function_index];
    if argument_count != metadata.signature.params().len() {
        return Err(runtime_error(
            anchor,
            EirRuntimeErrorKind::ArgumentCount {
                expected: metadata.signature.params().len(),
                actual: argument_count,
            },
        ));
    }

    let mut slots = vec![None; metadata.slots.len()];
    for ((slot, value), ty) in slots
        .iter_mut()
        .zip(arguments)
        .zip(metadata.signature.params())
    {
        let value = value?;
        if !value_matches_type(&value, *ty, program) {
            return Err(runtime_error(
                anchor,
                EirRuntimeErrorKind::TypeMismatch {
                    expected: type_name(*ty, program),
                    actual: runtime_value_name(&value),
                },
            ));
        }
        *slot = Some(value);
    }
    Ok(slots)
}

fn value_matches_type(value: &RuntimeVal, ty: TypeId, program: &EirProgram) -> bool {
    matches!(
        (value, program.types.get(ty)),
        (RuntimeVal::Float(_), Some(SemType::Num))
            | (RuntimeVal::String(_), Some(SemType::Str))
            | (RuntimeVal::Bytes(_), Some(SemType::Bytes))
            | (RuntimeVal::Bool(_), Some(SemType::Bool))
            | (RuntimeVal::Range(_, _), Some(SemType::Range))
            | (RuntimeVal::Void, Some(SemType::Void))
            | (RuntimeVal::List(_), Some(SemType::List(_)))
            | (RuntimeVal::Map(_), Some(SemType::Map(_, _)))
            | (RuntimeVal::Struct(_, _), Some(SemType::Struct(_)))
            | (RuntimeVal::FunctionRef(_), Some(SemType::Function { .. }))
            | (RuntimeVal::Pipe(_, _, _), Some(SemType::Pipe(_)))
            | (RuntimeVal::Pointer(_), Some(SemType::Address(_)))
            | (RuntimeVal::AdrHandle(_), Some(SemType::Address(_)))
    )
}

fn mock_value(ty: TypeId, program: &EirProgram) -> RuntimeVal {
    match program.types.get(ty) {
        Some(SemType::Num) => RuntimeVal::Float(0.0),
        Some(SemType::Str) => RuntimeVal::String(String::new()),
        Some(SemType::Bytes) => RuntimeVal::Bytes(Arc::from([])),
        Some(SemType::Bool) => RuntimeVal::Bool(false),
        Some(SemType::List(_)) => RuntimeVal::List(Vec::new()),
        Some(SemType::Map(_, _)) => RuntimeVal::Map(HashMap::new()),
        _ => RuntimeVal::Void,
    }
}

fn address_target(
    value: &RuntimeVal,
    anchor: SourceAnchor,
) -> Result<Arc<Mutex<RuntimeVal>>, EirRuntimeError> {
    match value {
        RuntimeVal::Pointer(target) => Ok(Arc::clone(target)),
        RuntimeVal::AdrHandle(Some(target)) => Ok(Arc::clone(target)),
        RuntimeVal::AdrHandle(None) => {
            Err(runtime_error(anchor, EirRuntimeErrorKind::EmptyAddress))
        }
        actual => Err(type_mismatch(anchor, "address", actual)),
    }
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
        RuntimeVal::Bytes(_) => "bytes",
        RuntimeVal::Bool(_) => "bool",
        RuntimeVal::Range(_, _) => "range",
        RuntimeVal::Void => "void",
        RuntimeVal::Pipe(_, _, _) => "pipe",
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
