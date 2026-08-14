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
    }
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
