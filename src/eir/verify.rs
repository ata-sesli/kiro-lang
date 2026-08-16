use std::collections::VecDeque;
use std::fmt;

use crate::hir::{
    Effects, FieldId, FunctionId, HostFunctionId, SemType, SourceAnchor, StructId, TypeId,
};

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
    InvalidHostFunction(HostFunctionId),
    InvalidStruct(StructId),
    StructOrder {
        expected: StructId,
        actual: StructId,
    },
    FieldOrder {
        expected: FieldId,
        actual: FieldId,
    },
    InvalidFieldType {
        field: FieldId,
        ty: TypeId,
    },
    InvalidBlock(BlockId),
    InvalidSlot(SlotId),
    InvalidType(TypeId),
    InvalidConstant(super::ConstId),
    InvalidGlobal(super::GlobalId),
    InvalidAggregateOperand(SlotId),
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
    for index in 0..program.types.len() {
        let ty = TypeId::try_from(index).expect("type index must fit u32");
        if let Some(SemType::Struct(id)) = program.types.get(ty)
            && program.struct_def(*id).is_none()
        {
            errors.push(program_error(VerifyErrorKind::InvalidStruct(*id)));
        }
    }

    let mut field_index = 0usize;
    for (index, record) in program.structs.iter().enumerate() {
        let expected = StructId::try_from(index).expect("struct index must fit u32");
        if record.id != expected {
            errors.push(program_error(VerifyErrorKind::StructOrder {
                expected,
                actual: record.id,
            }));
        }
        for field in &record.fields {
            let expected = FieldId::try_from(field_index).expect("field index must fit u32");
            if field.id != expected {
                errors.push(program_error(VerifyErrorKind::FieldOrder {
                    expected,
                    actual: field.id,
                }));
            }
            if program.types.get(field.ty).is_none() {
                errors.push(program_error(VerifyErrorKind::InvalidFieldType {
                    field: field.id,
                    ty: field.ty,
                }));
            }
            field_index += 1;
        }
    }

    for (index, function) in program.host_functions.iter().enumerate() {
        let expected = HostFunctionId::try_from(index).expect("host function index must fit u32");
        if function.id != expected {
            errors.push(VerifyError {
                function: FunctionId::new(0),
                block: BlockId::new(0),
                instruction: None,
                anchor: function.anchor,
                kind: VerifyErrorKind::InvalidHostFunction(function.id),
            });
        }
    }
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

fn program_error(kind: VerifyErrorKind) -> VerifyError {
    VerifyError {
        function: FunctionId::new(0),
        block: BlockId::new(0),
        instruction: None,
        anchor: fallback_anchor(),
        kind,
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
        InstructionKind::LoadGlobal { dst, global } => {
            let Some(global_type) = global_type(program, *global) else {
                errors.push(location(VerifyErrorKind::InvalidGlobal(*global)));
                return;
            };
            check_slot_type(function, *dst, global_type, &location, errors, false);
        }
        InstructionKind::MoveGlobal { dst, global } => {
            let Some(global_type) = global_type(program, *global) else {
                errors.push(location(VerifyErrorKind::InvalidGlobal(*global)));
                return;
            };
            check_slot_type(function, *dst, global_type, &location, errors, false);
        }
        InstructionKind::StoreGlobal { global, src } => {
            let Some(global_type) = global_type(program, *global) else {
                errors.push(location(VerifyErrorKind::InvalidGlobal(*global)));
                return;
            };
            check_slot_type(function, *src, global_type, &location, errors, false);
        }
        InstructionKind::MakeError { dst, error } => {
            let Some(destination_type) = slot_type(function, *dst, &location, errors) else {
                return;
            };
            if program.types.get(destination_type) != Some(&SemType::Error(*error)) {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
            }
        }
        InstructionKind::MakeFunction {
            dst,
            function: callee,
        } => {
            let Some(callee) = program.function(*callee) else {
                errors.push(location(VerifyErrorKind::InvalidFunction(*callee)));
                return;
            };
            check_function_value_type(
                program,
                function,
                *dst,
                &callee.signature,
                &location,
                errors,
            );
        }
        InstructionKind::MakeHostFunction {
            dst,
            function: callee,
        } => {
            let Some(callee) = program
                .host_functions
                .get(usize::try_from(*callee).unwrap_or(usize::MAX))
            else {
                errors.push(location(VerifyErrorKind::InvalidHostFunction(*callee)));
                return;
            };
            check_function_value_type(
                program,
                function,
                *dst,
                &callee.signature,
                &location,
                errors,
            );
        }
        InstructionKind::IsError { dst, value }
        | InstructionKind::ErrorMatches { dst, value, .. }
        | InstructionKind::IsTruthy { dst, value } => {
            slot_type(function, *value, &location, errors);
            check_slot_type(function, *dst, TypeId::BOOL, &location, errors, false);
        }
        InstructionKind::Check { condition, message } => {
            check_slot_type(function, *condition, TypeId::BOOL, &location, errors, false);
            if let Some(message) = message
                && !matches!(
                    constant_at(program, *message),
                    Some(super::Constant::Str(_))
                )
            {
                errors.push(location(VerifyErrorKind::InvalidConstant(*message)));
            }
            if !function.signature.effects().contains(Effects::MAY_FAIL) {
                errors.push(location(VerifyErrorKind::ThrowWithoutEffect));
            }
        }
        InstructionKind::MakeAddress { dst } => {
            let destination_type = slot_type(function, *dst, &location, errors);
            if !destination_type
                .and_then(|ty| program.types.get(ty))
                .is_some_and(|ty| matches!(ty, SemType::Address(_)))
            {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
            }
        }
        InstructionKind::MakeRef { dst, value } => {
            let Some(destination_type) = slot_type(function, *dst, &location, errors) else {
                return;
            };
            let Some(SemType::Address(inner)) = program.types.get(destination_type) else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
                return;
            };
            check_slot_type(function, *value, *inner, &location, errors, false);
        }
        InstructionKind::Deref { dst, address } => {
            let Some(address_type) = slot_type(function, *address, &location, errors) else {
                return;
            };
            let Some(SemType::Address(inner)) = program.types.get(address_type) else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*address)));
                return;
            };
            check_slot_type(function, *dst, *inner, &location, errors, false);
        }
        InstructionKind::StoreDeref { address, src } => {
            let Some(address_type) = slot_type(function, *address, &location, errors) else {
                return;
            };
            let Some(SemType::Address(inner)) = program.types.get(address_type) else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*address)));
                return;
            };
            check_slot_type(function, *src, *inner, &location, errors, false);
        }
        InstructionKind::MakeList { dst, items } => {
            let Some(destination_type) = slot_type(function, *dst, &location, errors) else {
                return;
            };
            let Some(SemType::List(item_type)) = program.types.get(destination_type) else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
                return;
            };
            for item in items {
                check_slot_type(function, *item, *item_type, &location, errors, false);
            }
        }
        InstructionKind::MakeMap { dst, entries } => {
            let Some(destination_type) = slot_type(function, *dst, &location, errors) else {
                return;
            };
            let Some(SemType::Map(key_type, value_type)) = program.types.get(destination_type)
            else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
                return;
            };
            for (key, value) in entries {
                check_slot_type(function, *key, *key_type, &location, errors, false);
                check_slot_type(function, *value, *value_type, &location, errors, false);
            }
        }
        InstructionKind::ListJoin { dst, left, right } => {
            let Some(dst_type) = slot_type(function, *dst, &location, errors) else {
                return;
            };
            if !matches!(program.types.get(dst_type), Some(SemType::List(_))) {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
                return;
            }
            check_slot_type(function, *left, dst_type, &location, errors, false);
            check_slot_type(function, *right, dst_type, &location, errors, false);
        }
        InstructionKind::ListSlice {
            dst,
            list,
            start,
            end,
        } => {
            let Some(dst_type) = slot_type(function, *dst, &location, errors) else {
                return;
            };
            if !matches!(program.types.get(dst_type), Some(SemType::List(_))) {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
                return;
            }
            check_slot_type(function, *list, dst_type, &location, errors, false);
            check_slot_type(function, *start, TypeId::NUM, &location, errors, false);
            check_slot_type(function, *end, TypeId::NUM, &location, errors, false);
        }
        InstructionKind::ListReverse { dst, list } => {
            let Some(dst_type) = slot_type(function, *dst, &location, errors) else {
                return;
            };
            if !matches!(program.types.get(dst_type), Some(SemType::List(_))) {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
                return;
            }
            check_slot_type(function, *list, dst_type, &location, errors, false);
        }
        InstructionKind::MapHas { dst, map, key } => {
            check_slot_type(function, *dst, TypeId::BOOL, &location, errors, false);
            let Some(map_type) = slot_type(function, *map, &location, errors) else {
                return;
            };
            let Some(SemType::Map(key_type, _)) = program.types.get(map_type) else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*map)));
                return;
            };
            check_slot_type(function, *key, *key_type, &location, errors, false);
        }
        InstructionKind::MapSet {
            dst,
            map,
            key,
            value,
        } => {
            let Some(map_type) = slot_type(function, *map, &location, errors) else {
                return;
            };
            check_slot_type(function, *dst, map_type, &location, errors, false);
            let Some(SemType::Map(key_type, value_type)) = program.types.get(map_type) else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*map)));
                return;
            };
            check_slot_type(function, *key, *key_type, &location, errors, false);
            check_slot_type(function, *value, *value_type, &location, errors, false);
        }
        InstructionKind::MapDelete { dst, map, key } => {
            let Some(map_type) = slot_type(function, *map, &location, errors) else {
                return;
            };
            check_slot_type(function, *dst, map_type, &location, errors, false);
            let Some(SemType::Map(key_type, _)) = program.types.get(map_type) else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*map)));
                return;
            };
            check_slot_type(function, *key, *key_type, &location, errors, false);
        }
        InstructionKind::MakeStruct {
            dst,
            structure,
            fields,
        } => {
            let Some(destination_type) = slot_type(function, *dst, &location, errors) else {
                return;
            };
            if program.types.get(destination_type) != Some(&SemType::Struct(*structure)) {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
            }
            for (_, value) in fields {
                slot_type(function, *value, &location, errors);
            }
        }
        InstructionKind::GetField { dst, target, .. } => {
            let target_type = slot_type(function, *target, &location, errors);
            slot_type(function, *dst, &location, errors);
            if !target_type
                .and_then(|ty| program.types.get(ty))
                .is_some_and(|ty| matches!(ty, SemType::Struct(_)))
            {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*target)));
            }
        }
        InstructionKind::SetField {
            target,
            fields,
            src,
        } => {
            let target_type = slot_type(function, *target, &location, errors);
            slot_type(function, *src, &location, errors);
            if fields.is_empty()
                || !target_type
                    .and_then(|ty| program.types.get(ty))
                    .is_some_and(|ty| matches!(ty, SemType::Struct(_)))
            {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*target)));
            }
        }
        InstructionKind::GetIndex {
            dst,
            collection,
            key,
        } => {
            let Some(collection_type) = slot_type(function, *collection, &location, errors) else {
                return;
            };
            match program.types.get(collection_type) {
                Some(SemType::List(item_type)) => {
                    check_slot_type(function, *key, TypeId::NUM, &location, errors, false);
                    check_slot_type(function, *dst, *item_type, &location, errors, false);
                }
                Some(SemType::Map(key_type, value_type)) => {
                    check_slot_type(function, *key, *key_type, &location, errors, false);
                    check_slot_type(function, *dst, *value_type, &location, errors, false);
                }
                Some(SemType::Bytes) => {
                    check_slot_type(function, *key, TypeId::NUM, &location, errors, false);
                    check_slot_type(function, *dst, TypeId::NUM, &location, errors, false);
                }
                _ => errors.push(location(VerifyErrorKind::InvalidAggregateOperand(
                    *collection,
                ))),
            }
        }
        InstructionKind::Push { collection, value } => {
            let Some(collection_type) = slot_type(function, *collection, &location, errors) else {
                return;
            };
            if let Some(SemType::List(item_type)) = program.types.get(collection_type) {
                check_slot_type(function, *value, *item_type, &location, errors, false);
            } else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(
                    *collection,
                )));
            }
        }
        InstructionKind::Len { dst, collection } => {
            check_slot_type(function, *dst, TypeId::NUM, &location, errors, false);
            let collection_type = slot_type(function, *collection, &location, errors);
            if !collection_type
                .and_then(|ty| program.types.get(ty))
                .is_some_and(|ty| {
                    matches!(
                        ty,
                        SemType::Str | SemType::Bytes | SemType::List(_) | SemType::Map(_, _)
                    )
                })
            {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(
                    *collection,
                )));
            }
        }
        InstructionKind::MakeRange { dst, start, end } => {
            check_slot_type(function, *start, TypeId::NUM, &location, errors, false);
            check_slot_type(function, *end, TypeId::NUM, &location, errors, false);
            let ty = slot_type(function, *dst, &location, errors);
            if !ty
                .and_then(|ty| program.types.get(ty))
                .is_some_and(|ty| *ty == SemType::Range)
            {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
            }
        }
        InstructionKind::IterInit { dst, iterable } => {
            check_slot_type(function, *dst, TypeId::NUM, &location, errors, false);
            check_iterable(program, function, *iterable, &location, errors);
        }
        InstructionKind::IterHasNext {
            dst,
            iterable,
            index,
        } => {
            check_slot_type(function, *dst, TypeId::BOOL, &location, errors, false);
            check_slot_type(function, *index, TypeId::NUM, &location, errors, false);
            check_iterable(program, function, *iterable, &location, errors);
        }
        InstructionKind::IterGet {
            dst,
            iterable,
            index,
        } => {
            check_slot_type(function, *index, TypeId::NUM, &location, errors, false);
            let Some(ty) = slot_type(function, *iterable, &location, errors) else {
                return;
            };
            let item = match program.types.get(ty) {
                Some(SemType::Range) => TypeId::NUM,
                Some(SemType::List(inner)) => *inner,
                Some(SemType::Str) => TypeId::STR,
                _ => {
                    errors.push(location(VerifyErrorKind::InvalidAggregateOperand(
                        *iterable,
                    )));
                    return;
                }
            };
            check_slot_type(function, *dst, item, &location, errors, false);
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
                (TypeId::VOID, Some(destination)) => check_slot_type(
                    function,
                    *destination,
                    TypeId::VOID,
                    &location,
                    errors,
                    false,
                ),
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
        InstructionKind::CallHost {
            dst,
            function: callee,
            args,
        } => {
            let Some(callee) = program
                .host_functions
                .get(usize::try_from(*callee).unwrap_or(usize::MAX))
            else {
                errors.push(location(VerifyErrorKind::InvalidHostFunction(*callee)));
                return;
            };
            check_call_signature(function, *dst, args, &callee.signature, &location, errors);
        }
        InstructionKind::CallIndirect { dst, callee, args } => {
            let Some(ty) = slot_type(function, *callee, &location, errors) else {
                return;
            };
            let Some(SemType::Function {
                params,
                return_type,
            }) = program.types.get(ty)
            else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*callee)));
                return;
            };
            let signature = crate::hir::Signature::new(params.clone(), *return_type, Effects::NONE);
            check_call_signature(function, *dst, args, &signature, &location, errors);
        }
        InstructionKind::MakePipe { dst, .. } => {
            let ty = slot_type(function, *dst, &location, errors);
            if !ty
                .and_then(|ty| program.types.get(ty))
                .is_some_and(|ty| matches!(ty, SemType::Pipe(_)))
            {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*dst)));
            }
        }
        InstructionKind::Give { channel, value } => {
            let Some(ty) = slot_type(function, *channel, &location, errors) else {
                return;
            };
            if let Some(SemType::Pipe(inner)) = program.types.get(ty) {
                check_slot_type(function, *value, *inner, &location, errors, false);
            } else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*channel)));
            }
        }
        InstructionKind::Take { dst, channel } => {
            let Some(ty) = slot_type(function, *channel, &location, errors) else {
                return;
            };
            if let Some(SemType::Pipe(inner)) = program.types.get(ty) {
                check_slot_type(function, *dst, *inner, &location, errors, false);
            } else {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*channel)));
            }
        }
        InstructionKind::Close { channel } => {
            let ty = slot_type(function, *channel, &location, errors);
            if !ty
                .and_then(|ty| program.types.get(ty))
                .is_some_and(|ty| matches!(ty, SemType::Pipe(_)))
            {
                errors.push(location(VerifyErrorKind::InvalidAggregateOperand(*channel)));
            }
        }
        InstructionKind::Rest => {}
        InstructionKind::Spawn {
            function: callee,
            args,
        } => {
            let Some(callee) = program.function(*callee) else {
                errors.push(location(VerifyErrorKind::InvalidFunction(*callee)));
                return;
            };
            check_call_signature(function, None, args, &callee.signature, &location, errors);
        }
    }
}

fn check_iterable(
    program: &EirProgram,
    function: &EirFunction,
    slot: SlotId,
    location: &impl Fn(VerifyErrorKind) -> VerifyError,
    errors: &mut Vec<VerifyError>,
) {
    let ty = slot_type(function, slot, location, errors);
    if !ty
        .and_then(|ty| program.types.get(ty))
        .is_some_and(|ty| matches!(ty, SemType::Range | SemType::List(_) | SemType::Str))
    {
        errors.push(location(VerifyErrorKind::InvalidAggregateOperand(slot)));
    }
}

fn check_function_value_type(
    program: &EirProgram,
    owner: &EirFunction,
    slot: SlotId,
    signature: &crate::hir::Signature,
    location: &impl Fn(VerifyErrorKind) -> VerifyError,
    errors: &mut Vec<VerifyError>,
) {
    let Some(ty) = slot_type(owner, slot, location, errors) else {
        return;
    };
    let expected = SemType::Function {
        params: signature.params().into(),
        return_type: signature.return_type(),
    };
    if program.types.get(ty) != Some(&expected) {
        errors.push(location(VerifyErrorKind::InvalidAggregateOperand(slot)));
    }
}

fn check_call_signature(
    owner: &EirFunction,
    dst: Option<SlotId>,
    args: &[SlotId],
    signature: &crate::hir::Signature,
    location: &impl Fn(VerifyErrorKind) -> VerifyError,
    errors: &mut Vec<VerifyError>,
) {
    for (argument, expected) in args.iter().zip(signature.params()) {
        if *expected != TypeId::UNKNOWN {
            check_slot_type(owner, *argument, *expected, location, errors, false);
        }
    }
    match (signature.return_type(), dst) {
        (TypeId::VOID, None) => {}
        (TypeId::VOID, Some(slot)) => {
            check_slot_type(owner, slot, TypeId::VOID, location, errors, false)
        }
        (return_type, Some(slot)) => {
            check_slot_type(owner, slot, return_type, location, errors, false)
        }
        (_, None) => {}
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
                let Some(actual) = slot_type(function, slot, &location, errors) else {
                    return;
                };
                let returns_declared_error =
                    function.signature.effects().contains(Effects::MAY_FAIL)
                        && matches!(_program.types.get(actual), Some(SemType::Error(_)));
                if actual != expected && !returns_declared_error {
                    errors.push(location(VerifyErrorKind::SlotType {
                        slot,
                        expected,
                        actual,
                    }));
                }
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

fn global_type(program: &EirProgram, id: super::GlobalId) -> Option<TypeId> {
    let index = usize::try_from(id).ok()?;
    program.globals.get(index).copied()
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
