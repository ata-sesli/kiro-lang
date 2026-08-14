use std::fmt::Write;

use crate::hir::{Effects, SemType, SourceAnchor, TypeId};

use super::{Constant, EirProgram, InstructionKind, TerminatorKind};

pub fn print_program(program: &EirProgram) -> String {
    let mut output = String::new();
    output.push_str("types:\n");
    for index in 0..program.types.len() {
        let id = TypeId::try_from(index).expect("type index must fit u32");
        let ty = program.types.get(id).expect("type table index must exist");
        writeln!(output, "  t{} = {}", id.raw(), type_name(ty)).unwrap();
    }
    output.push_str("constants:\n");
    for (index, constant) in program.constants.iter().enumerate() {
        writeln!(
            output,
            "  c{index}:t{} = {}",
            constant.ty().raw(),
            constant_name(constant)
        )
        .unwrap();
    }
    output.push_str("functions:\n");
    for function in &program.functions {
        let parameters = function
            .signature
            .params()
            .iter()
            .map(|ty| format!("t{}", ty.raw()))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            output,
            "fn f{} {}({}) -> t{} effects={} {{",
            function.id.raw(),
            function.name,
            parameters,
            function.signature.return_type().raw(),
            effects_name(function.signature.effects())
        )
        .unwrap();
        let slots = function
            .slots
            .iter()
            .enumerate()
            .map(|(index, ty)| format!("s{index}:t{}", ty.raw()))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, "  slots: {slots}").unwrap();
        for (block_index, block) in function.blocks.iter().enumerate() {
            writeln!(output, "  b{block_index}:").unwrap();
            for instruction in &block.instructions {
                writeln!(
                    output,
                    "    {} {}",
                    instruction_name(&instruction.kind),
                    anchor_name(instruction.anchor)
                )
                .unwrap();
            }
            writeln!(
                output,
                "    {} {}",
                terminator_name(&block.terminator.kind),
                anchor_name(block.terminator.anchor)
            )
            .unwrap();
        }
        output.push_str("}\n");
    }
    output.push_str("module_initializers:");
    if program.module_initializers.is_empty() {
        output.push('\n');
    } else {
        let ids = program
            .module_initializers
            .iter()
            .map(|id| format!("f{}", id.raw()))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(output, " {ids}").unwrap();
    }
    output
}

fn instruction_name(instruction: &InstructionKind) -> String {
    match instruction {
        InstructionKind::Const { dst, constant } => {
            format!("s{} = const c{}", dst.raw(), constant.raw())
        }
        InstructionKind::Copy { dst, src } => {
            format!("s{} = copy s{}", dst.raw(), src.raw())
        }
        InstructionKind::Move { dst, src } => {
            format!("s{} = move s{}", dst.raw(), src.raw())
        }
        InstructionKind::MoveGlobal { dst, global } => {
            format!("s{} = move_global g{}", dst.raw(), global.raw())
        }
        InstructionKind::LoadGlobal { dst, global } => {
            format!("s{} = load_global g{}", dst.raw(), global.raw())
        }
        InstructionKind::StoreGlobal { global, src } => {
            format!("store_global g{}, s{}", global.raw(), src.raw())
        }
        InstructionKind::MakeError { dst, error } => {
            format!("s{} = make_error error{}", dst.raw(), error.raw())
        }
        InstructionKind::MakeFunction { dst, function } => {
            format!("s{} = function f{}", dst.raw(), function.raw())
        }
        InstructionKind::MakeHostFunction { dst, function } => {
            format!("s{} = host_function h{}", dst.raw(), function.raw())
        }
        InstructionKind::IsError { dst, value } => {
            format!("s{} = is_error s{}", dst.raw(), value.raw())
        }
        InstructionKind::ErrorMatches { dst, value, error } => format!(
            "s{} = error_matches s{}, error{}",
            dst.raw(),
            value.raw(),
            error.raw()
        ),
        InstructionKind::IsTruthy { dst, value } => {
            format!("s{} = is_truthy s{}", dst.raw(), value.raw())
        }
        InstructionKind::Check { condition, message } => match message {
            Some(message) => format!("check s{}, c{}", condition.raw(), message.raw()),
            None => format!("check s{}", condition.raw()),
        },
        InstructionKind::MakeAddress { dst } => format!("s{} = make_address", dst.raw()),
        InstructionKind::MakeRef { dst, value } => {
            format!("s{} = make_ref s{}", dst.raw(), value.raw())
        }
        InstructionKind::Deref { dst, address } => {
            format!("s{} = deref s{}", dst.raw(), address.raw())
        }
        InstructionKind::StoreDeref { address, src } => {
            format!("store_deref s{}, s{}", address.raw(), src.raw())
        }
        InstructionKind::MakeList { dst, items } => {
            format!("s{} = make_list [{}]", dst.raw(), slot_list(items))
        }
        InstructionKind::MakeMap { dst, entries } => format!(
            "s{} = make_map [{}]",
            dst.raw(),
            entries
                .iter()
                .map(|(key, value)| format!("s{}: s{}", key.raw(), value.raw()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        InstructionKind::MakeStruct {
            dst,
            structure,
            fields,
        } => format!(
            "s{} = make_struct struct{} [{}]",
            dst.raw(),
            structure.raw(),
            fields
                .iter()
                .map(|(field, value)| format!("field{}: s{}", field.raw(), value.raw()))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        InstructionKind::GetField { dst, target, field } => format!(
            "s{} = get_field s{}, field{}",
            dst.raw(),
            target.raw(),
            field.raw()
        ),
        InstructionKind::SetField {
            target,
            fields,
            src,
        } => format!(
            "set_field s{}, [{}], s{}",
            target.raw(),
            fields
                .iter()
                .map(|field| format!("field{}", field.raw()))
                .collect::<Vec<_>>()
                .join("."),
            src.raw()
        ),
        InstructionKind::GetIndex {
            dst,
            collection,
            key,
        } => format!(
            "s{} = get_index s{}, s{}",
            dst.raw(),
            collection.raw(),
            key.raw()
        ),
        InstructionKind::Push { collection, value } => {
            format!("push s{}, s{}", collection.raw(), value.raw())
        }
        InstructionKind::Len { dst, collection } => {
            format!("s{} = len s{}", dst.raw(), collection.raw())
        }
        InstructionKind::MakeRange { dst, start, end } => binary("make_range", *dst, *start, *end),
        InstructionKind::IterInit { dst, iterable } => {
            format!("s{} = iter_init s{}", dst.raw(), iterable.raw())
        }
        InstructionKind::IterHasNext {
            dst,
            iterable,
            index,
        } => binary("iter_has_next", *dst, *iterable, *index),
        InstructionKind::IterGet {
            dst,
            iterable,
            index,
        } => binary("iter_get", *dst, *iterable, *index),
        InstructionKind::AddNum { dst, lhs, rhs } => binary("add_num", *dst, *lhs, *rhs),
        InstructionKind::ConcatString { dst, lhs, rhs } => {
            binary("concat_string", *dst, *lhs, *rhs)
        }
        InstructionKind::SubNum { dst, lhs, rhs } => binary("sub_num", *dst, *lhs, *rhs),
        InstructionKind::MulNum { dst, lhs, rhs } => binary("mul_num", *dst, *lhs, *rhs),
        InstructionKind::DivNum { dst, lhs, rhs } => binary("div_num", *dst, *lhs, *rhs),
        InstructionKind::EqNum { dst, lhs, rhs } => binary("eq_num", *dst, *lhs, *rhs),
        InstructionKind::EqString { dst, lhs, rhs } => binary("eq_string", *dst, *lhs, *rhs),
        InstructionKind::EqBool { dst, lhs, rhs } => binary("eq_bool", *dst, *lhs, *rhs),
        InstructionKind::NeNum { dst, lhs, rhs } => binary("ne_num", *dst, *lhs, *rhs),
        InstructionKind::NeString { dst, lhs, rhs } => binary("ne_string", *dst, *lhs, *rhs),
        InstructionKind::NeBool { dst, lhs, rhs } => binary("ne_bool", *dst, *lhs, *rhs),
        InstructionKind::GtNum { dst, lhs, rhs } => binary("gt_num", *dst, *lhs, *rhs),
        InstructionKind::LtNum { dst, lhs, rhs } => binary("lt_num", *dst, *lhs, *rhs),
        InstructionKind::GeNum { dst, lhs, rhs } => binary("ge_num", *dst, *lhs, *rhs),
        InstructionKind::LeNum { dst, lhs, rhs } => binary("le_num", *dst, *lhs, *rhs),
        InstructionKind::CallDirect {
            dst,
            function,
            args,
        } => {
            let args = args
                .iter()
                .map(|slot| format!("s{}", slot.raw()))
                .collect::<Vec<_>>()
                .join(", ");
            match dst {
                Some(dst) => format!("s{} = call f{}({args})", dst.raw(), function.raw()),
                None => format!("call f{}({args})", function.raw()),
            }
        }
        InstructionKind::CallHost {
            dst,
            function,
            args,
        } => call_name("call_host", *dst, format!("h{}", function.raw()), args),
        InstructionKind::CallIndirect { dst, callee, args } => {
            call_name("call_indirect", *dst, format!("s{}", callee.raw()), args)
        }
        InstructionKind::MakePipe { dst, capacity } => {
            format!("s{} = make_pipe {:?}", dst.raw(), capacity)
        }
        InstructionKind::Give { channel, value } => {
            format!("give s{}, s{}", channel.raw(), value.raw())
        }
        InstructionKind::Take { dst, channel } => {
            format!("s{} = take s{}", dst.raw(), channel.raw())
        }
        InstructionKind::Close { channel } => format!("close s{}", channel.raw()),
        InstructionKind::Rest => "rest".to_string(),
        InstructionKind::Spawn { function, args } => {
            call_name("spawn", None, format!("f{}", function.raw()), args)
        }
    }
}

fn call_name(
    name: &str,
    dst: Option<super::SlotId>,
    target: String,
    args: &[super::SlotId],
) -> String {
    let call = format!("{name} {target}({})", slot_list(args));
    dst.map_or(call.clone(), |dst| format!("s{} = {call}", dst.raw()))
}

fn slot_list(slots: &[super::SlotId]) -> String {
    slots
        .iter()
        .map(|slot| format!("s{}", slot.raw()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn terminator_name(terminator: &TerminatorKind) -> String {
    match terminator {
        TerminatorKind::Jump(target) => format!("jump b{}", target.raw()),
        TerminatorKind::Branch {
            condition,
            then_block,
            else_block,
        } => format!(
            "branch s{}, b{}, b{}",
            condition.raw(),
            then_block.raw(),
            else_block.raw()
        ),
        TerminatorKind::Return(Some(value)) => format!("return s{}", value.raw()),
        TerminatorKind::Return(None) => "return".to_string(),
        TerminatorKind::Throw(value) => format!("throw s{}", value.raw()),
        TerminatorKind::Unreachable => "unreachable".to_string(),
    }
}

fn binary(name: &str, dst: super::SlotId, lhs: super::SlotId, rhs: super::SlotId) -> String {
    format!("s{} = {name} s{}, s{}", dst.raw(), lhs.raw(), rhs.raw())
}

fn type_name(ty: &SemType) -> String {
    match ty {
        SemType::Unknown => "unknown".to_string(),
        SemType::Void => "void".to_string(),
        SemType::Bool => "bool".to_string(),
        SemType::Num => "num".to_string(),
        SemType::Str => "str".to_string(),
        SemType::Range => "range".to_string(),
        SemType::Address(inner) => format!("address t{}", inner.raw()),
        SemType::Pipe(inner) => format!("pipe t{}", inner.raw()),
        SemType::List(inner) => format!("list t{}", inner.raw()),
        SemType::Map(key, value) => format!("map t{} t{}", key.raw(), value.raw()),
        SemType::Function {
            params,
            return_type,
        } => format!(
            "fn({}) -> t{}",
            params
                .iter()
                .map(|ty| format!("t{}", ty.raw()))
                .collect::<Vec<_>>()
                .join(", "),
            return_type.raw()
        ),
        SemType::Struct(id) => format!("struct {}", id.raw()),
        SemType::Handle(id) => format!("handle {}", id.raw()),
        SemType::Error(id) => format!("error {}", id.raw()),
    }
}

fn constant_name(constant: &Constant) -> String {
    match constant {
        Constant::Bool(value) => value.to_string(),
        Constant::Num(value) => value.to_string(),
        Constant::Str(value) => format!("{value:?}"),
    }
}

fn effects_name(effects: Effects) -> String {
    format!("{effects:?}")
        .strip_prefix("Effects(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or("NONE")
        .to_string()
}

fn anchor_name(anchor: SourceAnchor) -> String {
    format!(
        "@{}:{}..{}",
        anchor.source().raw(),
        anchor.start(),
        anchor.end()
    )
}
