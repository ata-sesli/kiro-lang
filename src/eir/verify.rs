use std::collections::VecDeque;
use std::fmt;

use crate::hir::{Effects, FunctionId, SourceAnchor, TypeId};

use super::{
    BlockId, EirFunction, EirProgram, Instruction, InstructionKind, SlotId, Terminator,
    TerminatorKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyError {
    pub function: FunctionId,
    pub block: BlockId,
    pub instruction: Option<usize>,
    pub anchor: SourceAnchor,
    pub kind: VerifyErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyErrorKind {
    InvalidFunction(FunctionId),
    InvalidBlock(BlockId),
    InvalidSlot(SlotId),
    InvalidType(TypeId),
    InvalidConstant(super::ConstId),
    FunctionOrder {
        expected: FunctionId,
        actual: FunctionId,
    },
    EmptyFunction,
    ParameterCount {
        signature: usize,
        declared: u32,
        slots: usize,
    },
    SlotType {
        slot: SlotId,
        expected: TypeId,
        actual: TypeId,
    },
    ConstantType {
        slot: SlotId,
        expected: TypeId,
        actual: TypeId,
    },
    CallArgumentCount {
        function: FunctionId,
        expected: usize,
        actual: usize,
    },
    MissingCallDestination {
        function: FunctionId,
        return_type: TypeId,
    },
    UnexpectedCallDestination {
        function: FunctionId,
    },
    MissingReturnValue(TypeId),
    UnexpectedReturnValue,
    UninitializedRead(SlotId),
    EffectViolation {
        callee: FunctionId,
    },
    ThrowWithoutEffect,
    InvalidInitializer(FunctionId),
}

impl fmt::Display for VerifyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "EIR verification failed in f{} b{}",
            self.function.raw(),
            self.block.raw()
        )?;
        if let Some(instruction) = self.instruction {
            write!(formatter, " i{instruction}")?;
        } else {
            formatter.write_str(" terminator")?;
        }
        write!(
            formatter,
            " at {}:{}..{}: {:?}",
            self.anchor.source().raw(),
            self.anchor.start(),
            self.anchor.end(),
            self.kind
        )
    }
}

impl std::error::Error for VerifyError {}

pub fn verify_program(program: &EirProgram) -> Result<(), Vec<VerifyError>> {
    let mut errors = Vec::new();
    for (index, function) in program.functions.iter().enumerate() {
        let expected = FunctionId::try_from(index).expect("EIR function index must fit u32");
        if function.id != expected {
            errors.push(function_error(
                function,
                VerifyErrorKind::FunctionOrder {
                    expected,
                    actual: function.id,
                },
            ));
        }
        verify_function(program, function, &mut errors);
    }

    for initializer in &program.module_initializers {
        match program.function(*initializer) {
            Some(function)
                if function.signature.params().is_empty()
                    && function.signature.return_type() == TypeId::VOID => {}
            _ => errors.push(VerifyError {
                function: *initializer,
                block: BlockId::new(0),
                instruction: None,
                anchor: fallback_anchor(),
                kind: VerifyErrorKind::InvalidInitializer(*initializer),
            }),
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn verify_function(program: &EirProgram, function: &EirFunction, errors: &mut Vec<VerifyError>) {
    if function.blocks.is_empty() {
        errors.push(function_error(function, VerifyErrorKind::EmptyFunction));
        return;
    }

    let parameter_count = usize::try_from(function.parameter_count).unwrap_or(usize::MAX);
    if parameter_count != function.signature.params().len()
        || parameter_count > function.slots.len()
    {
        errors.push(function_error(
            function,
            VerifyErrorKind::ParameterCount {
                signature: function.signature.params().len(),
                declared: function.parameter_count,
                slots: function.slots.len(),
            },
        ));
    }

    for ty in function
        .signature
        .params()
        .iter()
        .copied()
        .chain(std::iter::once(function.signature.return_type()))
    {
        if program.types.get(ty).is_none() {
            errors.push(VerifyError {
                function: function.id,
                block: BlockId::new(0),
                instruction: None,
                anchor: function_anchor(function),
                kind: VerifyErrorKind::InvalidType(ty),
            });
        }
    }

    for (index, ty) in function.slots.iter().enumerate() {
        if program.types.get(*ty).is_none() {
            errors.push(VerifyError {
                function: function.id,
                block: BlockId::new(0),
                instruction: None,
                anchor: function_anchor(function),
                kind: VerifyErrorKind::InvalidType(*ty),
            });
        }
        if index < function.signature.params().len()
            && function.signature.params().get(index) != Some(ty)
        {
            errors.push(VerifyError {
                function: function.id,
                block: BlockId::new(0),
                instruction: None,
                anchor: function_anchor(function),
                kind: VerifyErrorKind::SlotType {
                    slot: SlotId::try_from(index).expect("slot index must fit u32"),
                    expected: function.signature.params()[index],
                    actual: *ty,
                },
            });
        }
    }

    for (block_index, block) in function.blocks.iter().enumerate() {
        let block_id = BlockId::try_from(block_index).expect("block index must fit u32");
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            verify_instruction(
                program,
                function,
                block_id,
                instruction_index,
                instruction,
                errors,
            );
        }
        verify_terminator(program, function, block_id, &block.terminator, errors);
    }

    verify_initialized_reads(function, errors);
}

fn verify_instruction(
    program: &EirProgram,
    function: &EirFunction,
    block: BlockId,
    index: usize,
    instruction: &Instruction,
    errors: &mut Vec<VerifyError>,
) {
    let location = |kind| VerifyError {
        function: function.id,
        block,
        instruction: Some(index),
        anchor: instruction.anchor,
        kind,
    };

    match &instruction.kind {
        InstructionKind::Const { dst, constant } => {
            let Some(value) = constant_at(program, *constant) else {
                errors.push(location(VerifyErrorKind::InvalidConstant(*constant)));
                return;
            };
            check_slot_type(function, *dst, value.ty(), &location, errors, true);
        }
        InstructionKind::Copy { dst, src } | InstructionKind::Move { dst, src } => {
            let Some(source_ty) = slot_type(function, *src, &location, errors) else {
                return;
            };
            check_slot_type(function, *dst, source_ty, &location, errors, false);
        }
        InstructionKind::AddNum { dst, lhs, rhs }
        | InstructionKind::SubNum { dst, lhs, rhs }
        | InstructionKind::MulNum { dst, lhs, rhs }
        | InstructionKind::DivNum { dst, lhs, rhs } => {
            check_binary(
                function,
                *dst,
                *lhs,
                *rhs,
                TypeId::NUM,
                TypeId::NUM,
                &location,
                errors,
            );
        }
        InstructionKind::ConcatString { dst, lhs, rhs } => {
            check_binary(
                function,
                *dst,
                *lhs,
                *rhs,
                TypeId::STR,
                TypeId::STR,
                &location,
                errors,
            );
        }
        InstructionKind::EqNum { dst, lhs, rhs }
        | InstructionKind::NeNum { dst, lhs, rhs }
        | InstructionKind::GtNum { dst, lhs, rhs }
        | InstructionKind::LtNum { dst, lhs, rhs }
        | InstructionKind::GeNum { dst, lhs, rhs }
        | InstructionKind::LeNum { dst, lhs, rhs } => {
            check_binary(
                function,
                *dst,
                *lhs,
                *rhs,
                TypeId::NUM,
                TypeId::BOOL,
                &location,
                errors,
            );
        }
        InstructionKind::EqString { dst, lhs, rhs }
        | InstructionKind::NeString { dst, lhs, rhs } => {
            check_binary(
                function,
                *dst,
                *lhs,
                *rhs,
                TypeId::STR,
                TypeId::BOOL,
                &location,
                errors,
            );
        }
        InstructionKind::EqBool { dst, lhs, rhs } | InstructionKind::NeBool { dst, lhs, rhs } => {
            check_binary(
                function,
                *dst,
                *lhs,
                *rhs,
                TypeId::BOOL,
                TypeId::BOOL,
                &location,
                errors,
            );
        }
        InstructionKind::CallDirect {
            dst,
            function: callee_id,
            args,
        } => {
            let Some(callee) = program.function(*callee_id) else {
                errors.push(location(VerifyErrorKind::InvalidFunction(*callee_id)));
                return;
            };
            if args.len() != callee.signature.params().len() {
                errors.push(location(VerifyErrorKind::CallArgumentCount {
                    function: *callee_id,
                    expected: callee.signature.params().len(),
                    actual: args.len(),
                }));
            }
            for (argument, expected) in args.iter().zip(callee.signature.params()) {
                check_slot_type(function, *argument, *expected, &location, errors, false);
            }
            let return_type = callee.signature.return_type();
            match (return_type, dst) {
                (TypeId::VOID, None) => {}
                (TypeId::VOID, Some(_)) => {
                    errors.push(location(VerifyErrorKind::UnexpectedCallDestination {
                        function: *callee_id,
                    }))
                }
                (_, Some(destination)) => check_slot_type(
                    function,
                    *destination,
                    return_type,
                    &location,
                    errors,
                    false,
                ),
                (_, None) => errors.push(location(VerifyErrorKind::MissingCallDestination {
                    function: *callee_id,
                    return_type,
                })),
            }
            let caller_effects = function.signature.effects();
            let callee_effects = callee.signature.effects();
            let violates_purity =
                caller_effects.contains(Effects::PURE) && !callee_effects.contains(Effects::PURE);
            let misses_effect = [
                Effects::MAY_FAIL,
                Effects::MAY_BLOCK,
                Effects::MAY_SPAWN,
                Effects::HOST_CALL,
                Effects::INDIRECT_CALL,
            ]
            .into_iter()
            .any(|effect| callee_effects.contains(effect) && !caller_effects.contains(effect));
            if violates_purity || misses_effect {
                errors.push(location(VerifyErrorKind::EffectViolation {
                    callee: *callee_id,
                }));
            }
        }
    }
}

fn verify_terminator(
    _program: &EirProgram,
    function: &EirFunction,
    block: BlockId,
    terminator: &Terminator,
    errors: &mut Vec<VerifyError>,
) {
    let location = |kind| VerifyError {
        function: function.id,
        block,
        instruction: None,
        anchor: terminator.anchor,
        kind,
    };
    match terminator.kind {
        TerminatorKind::Jump(target) => check_block(function, target, &location, errors),
        TerminatorKind::Branch {
            condition,
            then_block,
            else_block,
        } => {
            check_slot_type(function, condition, TypeId::BOOL, &location, errors, false);
            check_block(function, then_block, &location, errors);
            check_block(function, else_block, &location, errors);
        }
        TerminatorKind::Return(value) => match (function.signature.return_type(), value) {
            (TypeId::VOID, None) => {}
            (TypeId::VOID, Some(_)) => {
                errors.push(location(VerifyErrorKind::UnexpectedReturnValue))
            }
            (expected, Some(slot)) => {
                check_slot_type(function, slot, expected, &location, errors, false)
            }
            (expected, None) => {
                errors.push(location(VerifyErrorKind::MissingReturnValue(expected)))
            }
        },
        TerminatorKind::Throw(slot) => {
            slot_type(function, slot, &location, errors);
            if !function.signature.effects().contains(Effects::MAY_FAIL) {
                errors.push(location(VerifyErrorKind::ThrowWithoutEffect));
            }
        }
        TerminatorKind::Unreachable => {}
    }
}

fn verify_initialized_reads(function: &EirFunction, errors: &mut Vec<VerifyError>) {
    let slot_count = function.slots.len();
    let block_count = function.blocks.len();
    let mut predecessors = vec![Vec::new(); block_count];
    for (index, block) in function.blocks.iter().enumerate() {
        for target in terminator_targets(&block.terminator.kind) {
            if let Ok(target_index) = usize::try_from(target)
                && target_index < block_count
            {
                predecessors[target_index].push(index);
            }
        }
    }

    let reachable = reachable_blocks(function);
    let mut incoming = vec![vec![true; slot_count]; block_count];
    incoming[0] = vec![false; slot_count];
    for slot in incoming[0]
        .iter_mut()
        .take(function.parameter_count as usize)
    {
        *slot = true;
    }
    for (index, is_reachable) in reachable.iter().enumerate().skip(1) {
        if !is_reachable {
            incoming[index].fill(false);
        }
    }
    let mut outgoing = Vec::with_capacity(block_count);
    for (block_index, block) in function.blocks.iter().enumerate() {
        let mut state = incoming[block_index].clone();
        for instruction in &block.instructions {
            apply_initialization_transfer(&instruction.kind, &mut state);
        }
        outgoing.push(state);
    }

    loop {
        let previous = outgoing.clone();
        for block_index in 0..block_count {
            if !reachable[block_index] {
                continue;
            }
            if block_index != 0 {
                let mut state = vec![true; slot_count];
                let mut has_predecessor = false;
                for predecessor in &predecessors[block_index] {
                    if reachable[*predecessor] {
                        has_predecessor = true;
                        for (slot, initialized) in state.iter_mut().zip(&previous[*predecessor]) {
                            *slot &= *initialized;
                        }
                    }
                }
                if !has_predecessor {
                    state.fill(false);
                }
                incoming[block_index] = state;
            }
            let mut state = incoming[block_index].clone();
            for instruction in &function.blocks[block_index].instructions {
                apply_initialization_transfer(&instruction.kind, &mut state);
            }
            outgoing[block_index] = state;
        }
        if outgoing == previous {
            break;
        }
    }

    for (block_index, block) in function.blocks.iter().enumerate() {
        if !reachable[block_index] {
            continue;
        }
        let block_id = BlockId::try_from(block_index).expect("block index must fit u32");
        let mut state = incoming[block_index].clone();
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            for slot in instruction.kind.read_slots() {
                check_initialized(
                    function,
                    block_id,
                    Some(instruction_index),
                    instruction.anchor,
                    slot,
                    &state,
                    errors,
                );
            }
            apply_initialization_transfer(&instruction.kind, &mut state);
        }
        for slot in terminator_reads(&block.terminator.kind) {
            check_initialized(
                function,
                block_id,
                None,
                block.terminator.anchor,
                slot,
                &state,
                errors,
            );
        }
    }
}

fn apply_initialization_transfer(instruction: &InstructionKind, state: &mut [bool]) {
    if let InstructionKind::Move { src, .. } = instruction
        && let Ok(slot) = usize::try_from(*src)
        && let Some(initialized) = state.get_mut(slot)
    {
        *initialized = false;
    }
    if let Some(destination) = instruction.destination()
        && let Ok(slot) = usize::try_from(destination)
        && let Some(initialized) = state.get_mut(slot)
    {
        *initialized = true;
    }
}

fn check_binary(
    function: &EirFunction,
    dst: SlotId,
    lhs: SlotId,
    rhs: SlotId,
    operand_type: TypeId,
    result_type: TypeId,
    location: &impl Fn(VerifyErrorKind) -> VerifyError,
    errors: &mut Vec<VerifyError>,
) {
    check_slot_type(function, lhs, operand_type, location, errors, false);
    check_slot_type(function, rhs, operand_type, location, errors, false);
    check_slot_type(function, dst, result_type, location, errors, false);
}

fn check_slot_type(
    function: &EirFunction,
    slot: SlotId,
    expected: TypeId,
    location: &impl Fn(VerifyErrorKind) -> VerifyError,
    errors: &mut Vec<VerifyError>,
    constant: bool,
) {
    let Some(actual) = slot_type(function, slot, location, errors) else {
        return;
    };
    if actual != expected {
        let kind = if constant {
            VerifyErrorKind::ConstantType {
                slot,
                expected,
                actual,
            }
        } else {
            VerifyErrorKind::SlotType {
                slot,
                expected,
                actual,
            }
        };
        errors.push(location(kind));
    }
}

fn slot_type(
    function: &EirFunction,
    slot: SlotId,
    location: &impl Fn(VerifyErrorKind) -> VerifyError,
    errors: &mut Vec<VerifyError>,
) -> Option<TypeId> {
    let index = usize::try_from(slot).ok()?;
    match function.slots.get(index) {
        Some(ty) => Some(*ty),
        None => {
            errors.push(location(VerifyErrorKind::InvalidSlot(slot)));
            None
        }
    }
}

fn check_block(
    function: &EirFunction,
    block: BlockId,
    location: &impl Fn(VerifyErrorKind) -> VerifyError,
    errors: &mut Vec<VerifyError>,
) {
    let valid = usize::try_from(block)
        .ok()
        .is_some_and(|index| index < function.blocks.len());
    if !valid {
        errors.push(location(VerifyErrorKind::InvalidBlock(block)));
    }
}

fn check_initialized(
    function: &EirFunction,
    block: BlockId,
    instruction: Option<usize>,
    anchor: SourceAnchor,
    slot: SlotId,
    state: &[bool],
    errors: &mut Vec<VerifyError>,
) {
    let initialized = usize::try_from(slot)
        .ok()
        .and_then(|index| state.get(index))
        .copied()
        .unwrap_or(true);
    if !initialized {
        errors.push(VerifyError {
            function: function.id,
            block,
            instruction,
            anchor,
            kind: VerifyErrorKind::UninitializedRead(slot),
        });
    }
}

fn constant_at(program: &EirProgram, id: super::ConstId) -> Option<&super::Constant> {
    let index = usize::try_from(id).ok()?;
    program.constants.get(index)
}

fn terminator_targets(terminator: &TerminatorKind) -> Vec<BlockId> {
    match terminator {
        TerminatorKind::Jump(target) => vec![*target],
        TerminatorKind::Branch {
            then_block,
            else_block,
            ..
        } => vec![*then_block, *else_block],
        TerminatorKind::Return(_) | TerminatorKind::Throw(_) | TerminatorKind::Unreachable => {
            Vec::new()
        }
    }
}

fn terminator_reads(terminator: &TerminatorKind) -> Vec<SlotId> {
    match terminator {
        TerminatorKind::Branch { condition, .. } => vec![*condition],
        TerminatorKind::Return(Some(value)) | TerminatorKind::Throw(value) => vec![*value],
        TerminatorKind::Jump(_) | TerminatorKind::Return(None) | TerminatorKind::Unreachable => {
            Vec::new()
        }
    }
}

fn reachable_blocks(function: &EirFunction) -> Vec<bool> {
    let mut reachable = vec![false; function.blocks.len()];
    let mut queue = VecDeque::from([0usize]);
    while let Some(index) = queue.pop_front() {
        if index >= function.blocks.len() || reachable[index] {
            continue;
        }
        reachable[index] = true;
        for target in terminator_targets(&function.blocks[index].terminator.kind) {
            if let Ok(target) = usize::try_from(target)
                && target < function.blocks.len()
            {
                queue.push_back(target);
            }
        }
    }
    reachable
}

fn function_anchor(function: &EirFunction) -> SourceAnchor {
    function
        .blocks
        .first()
        .map(|block| block.terminator.anchor)
        .unwrap_or_else(fallback_anchor)
}

fn function_error(function: &EirFunction, kind: VerifyErrorKind) -> VerifyError {
    VerifyError {
        function: function.id,
        block: BlockId::new(0),
        instruction: None,
        anchor: function_anchor(function),
        kind,
    }
}

fn fallback_anchor() -> SourceAnchor {
    SourceAnchor::try_from_offsets(crate::hir::SourceId::new(0), 0, 0)
        .expect("zero source anchor is valid")
}
