use std::collections::HashSet;
use std::fmt::{self, Write};

use crate::eir::{
    Constant, EirFunction, EirHostFunction, EirProgram, InstructionKind, SlotId, TerminatorKind,
};
use crate::hir::{SemType, TypeId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EirCodegenError {
    operation: &'static str,
}

impl fmt::Display for EirCodegenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "EIR Rust backend does not support {}",
            self.operation
        )
    }
}

impl std::error::Error for EirCodegenError {}

pub fn program_uses_pipes(program: &EirProgram) -> bool {
    program.functions.iter().any(|function| {
        function.blocks.iter().any(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction.kind,
                    InstructionKind::MakePipe { .. }
                        | InstructionKind::Give { .. }
                        | InstructionKind::Take { .. }
                        | InstructionKind::Close { .. }
                )
            })
        })
    })
}

pub fn compile_program(program: &EirProgram) -> Result<String, EirCodegenError> {
    validate_supported(program)?;

    let mut output = String::new();
    let uses_pipes = program_uses_pipes(program);
    output.push_str(
        r#"#![allow(unused)]
mod header;
pub use kiro_runtime::*;
use std::future::Future;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
enum __KiroValue {
    Num(f64),
    Str(String),
    Bytes(Arc<[u8]>),
    Bool(bool),
    Range(i64, i64),
    Void,
    List(Vec<__KiroValue>),
    Map(HashMap<String, __KiroValue>),
    Struct(u32, HashMap<u32, __KiroValue>),
    Error(u32),
    Address(Arc<Mutex<Option<__KiroValue>>>),
    Function { host: bool, id: u32 },
"#,
    );
    if uses_pipes {
        output.push_str("    Pipe(__KiroPipe),\n");
    }
    output.push_str(
        r#"    Host(RuntimeVal),
}

"#,
    );
    if uses_pipes {
        output.push_str(
            r#"#[derive(Clone, Debug)]
struct __KiroPipe {
    sender: async_channel::Sender<__KiroValue>,
    receiver: async_channel::Receiver<__KiroValue>,
    rendezvous_ack_sender: Option<async_channel::Sender<()>>,
    rendezvous_ack_receiver: Option<async_channel::Receiver<()>>,
}

"#,
        );
    }
    output.push_str(
        r#"
type __KiroGlobals = Arc<Mutex<Vec<Option<__KiroValue>>>>;
type __KiroFuture = Pin<Box<dyn Future<Output = __KiroValue> + Send>>;

fn __kiro_num(value: &__KiroValue) -> f64 {
    match value { __KiroValue::Num(value) => *value, other => panic!("expected num, got {other:?}") }
}
fn __kiro_bool(value: &__KiroValue) -> bool {
    match value { __KiroValue::Bool(value) => *value, other => panic!("expected bool, got {other:?}") }
}
fn __kiro_str(value: &__KiroValue) -> &str {
    match value { __KiroValue::Str(value) => value, other => panic!("expected str, got {other:?}") }
}
fn __kiro_truthy(value: &__KiroValue) -> bool {
    match value {
        __KiroValue::Num(value) => *value != 0.0,
        __KiroValue::Str(value) => !value.is_empty(),
        __KiroValue::Bytes(value) => !value.is_empty(),
        __KiroValue::Bool(value) => *value,
        __KiroValue::Void => false,
        _ => true,
    }
}
fn __kiro_display(value: &__KiroValue) -> String {
    match value {
        __KiroValue::Num(value) => value.to_string(),
        __KiroValue::Str(value) => value.clone(),
        __KiroValue::Bytes(value) => format!("<Bytes len={}>", value.len()),
        __KiroValue::Bool(value) => value.to_string(),
        __KiroValue::Range(start, end) => format!("{start}..{end}"),
        __KiroValue::Void => "void".to_string(),
        __KiroValue::List(values) => format!("<List len={}>", values.len()),
        __KiroValue::Map(values) => format!("<Map len={}>", values.len()),
        __KiroValue::Struct(structure, _) => format!("<Struct struct{structure}>") ,
        __KiroValue::Error(error) => format!("Error(error{error}): "),
        __KiroValue::Address(_) => "<Pointer>".to_string(),
        __KiroValue::Function { host, id } => {
            let prefix = if *host { "h" } else { "f" };
            format!("<FnRef {prefix}{id}>")
        }
"#,
    );
    if uses_pipes {
        output.push_str("        __KiroValue::Pipe(_) => \"<Pipe>\".to_string(),\n");
    }
    output.push_str(
        r#"
        __KiroValue::Host(RuntimeVal::Handle(handle)) => handle.to_string(),
        __KiroValue::Host(other) => format!("{other:?}"),
    }
}
fn __kiro_map_key(value: &__KiroValue) -> String { __kiro_display(value) }
fn __kiro_to_host(value: &__KiroValue) -> RuntimeVal {
    match value {
        __KiroValue::Num(value) => RuntimeVal::Num(*value),
        __KiroValue::Str(value) => RuntimeVal::Str(value.clone()),
        __KiroValue::Bytes(value) => RuntimeVal::Bytes(value.clone()),
        __KiroValue::Bool(value) => RuntimeVal::Bool(*value),
        __KiroValue::Void => RuntimeVal::Void,
        __KiroValue::List(values) => RuntimeVal::List(values.iter().map(__kiro_to_host).collect()),
        __KiroValue::Map(values) => RuntimeVal::Map(values.iter().map(|(key, value)| (key.clone(), __kiro_to_host(value))).collect()),
        __KiroValue::Host(value) => value.clone(),
        other => panic!("cannot pass EIR value to host: {other:?}"),
    }
}
fn __kiro_from_host(value: RuntimeVal) -> __KiroValue {
    match value {
        RuntimeVal::Num(value) => __KiroValue::Num(value),
        RuntimeVal::Str(value) => __KiroValue::Str(value),
        RuntimeVal::Bytes(value) => __KiroValue::Bytes(value),
        RuntimeVal::Bool(value) => __KiroValue::Bool(value),
        RuntimeVal::Void => __KiroValue::Void,
        RuntimeVal::List(values) => __KiroValue::List(values.into_iter().map(__kiro_from_host).collect()),
        RuntimeVal::Map(values) => __KiroValue::Map(values.into_iter().map(|(key, value)| (key, __kiro_from_host(value))).collect()),
        value @ RuntimeVal::Struct { .. } => __KiroValue::Host(value),
        value @ RuntimeVal::Handle(_) => __KiroValue::Host(value),
    }
}
fn __kiro_set_field(target: &mut __KiroValue, path: &[u32], value: __KiroValue) {
    let (field, rest) = path.split_first().expect("verified field path");
    let __KiroValue::Struct(_, fields) = target else { panic!("expected struct") };
    let target = fields.get_mut(field).expect("verified struct field");
    if rest.is_empty() { *target = value; } else { __kiro_set_field(target, rest, value); }
}

"#,
    );
    emit_host_type_helpers(program, &mut output);
    output.push_str("fn __kiro_from_host_error(error: KiroError) -> __KiroValue {\n    match error.name.as_str() {\n");
    let mut emitted_error_names = HashSet::new();
    for (index, symbol) in program.errors.iter().enumerate() {
        let name = symbol
            .rsplit_once('.')
            .map_or(symbol.as_str(), |(_, name)| name);
        if emitted_error_names.insert(name) {
            writeln!(output, "        {name:?} => __KiroValue::Error({index}),").unwrap();
        }
    }
    output.push_str(
        "        _ => kiro_runtime_error(\"KIRO3007\", &format!(\"Host function failed: {error}\")),\n    }\n}\n\n",
    );
    let indirect_host_functions = program
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.kind {
            InstructionKind::MakeHostFunction { function, .. } => Some(function.raw()),
            _ => None,
        })
        .collect::<HashSet<_>>();
    for function in &program.functions {
        compile_function(program, function, &indirect_host_functions, &mut output)?;
    }
    writeln!(output, "#[tokio::main]").unwrap();
    writeln!(output, "async fn main() {{").unwrap();
    writeln!(
        output,
        "    let __globals: __KiroGlobals = Arc::new(Mutex::new(vec![None; {}]));",
        program.globals.len()
    )
    .unwrap();
    for initializer in &program.module_initializers {
        writeln!(
            output,
            "    let _ = __kiro_eir_f{}(Vec::new(), __globals.clone()).await;",
            initializer.raw()
        )
        .unwrap();
    }
    output.push_str("}\n");
    Ok(output)
}

fn emit_host_type_helpers(program: &EirProgram, output: &mut String) {
    for index in 0..program.types.len() {
        let ty = TypeId::new(index as u32);
        let to_host = host_to_expression(program, ty, "value");
        let from_host = host_from_expression(program, ty, "value");
        writeln!(
            output,
            "fn __kiro_to_host_t{index}(value: &__KiroValue) -> RuntimeVal {{ {to_host} }}"
        )
        .unwrap();
        writeln!(
            output,
            "fn __kiro_from_host_t{index}(value: RuntimeVal) -> __KiroValue {{ {from_host} }}"
        )
        .unwrap();
    }
    output.push('\n');
}

fn host_to_expression(program: &EirProgram, ty: TypeId, value: &str) -> String {
    match program.types.get(ty) {
        Some(SemType::List(inner)) => format!(
            "match {value} {{ __KiroValue::List(values) => RuntimeVal::List(values.iter().map(__kiro_to_host_t{}).collect()), other => panic!(\"expected list, got {{other:?}}\") }}",
            inner.raw()
        ),
        Some(SemType::Map(_, inner)) => format!(
            "match {value} {{ __KiroValue::Map(values) => RuntimeVal::Map(values.iter().map(|(key, value)| (key.clone(), __kiro_to_host_t{}(value))).collect()), other => panic!(\"expected map, got {{other:?}}\") }}",
            inner.raw()
        ),
        Some(SemType::Struct(id)) => {
            let record = program
                .struct_def(*id)
                .expect("EIR struct type must have metadata");
            let fields = record
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "({:?}.to_string(), __kiro_to_host_t{}(fields.get(&{}).expect(\"verified struct field\")))",
                        field.name,
                        field.ty.raw(),
                        field.id.raw()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "match {value} {{ __KiroValue::Struct(structure, fields) if *structure == {} => RuntimeVal::structure({:?}, [{fields}].into_iter().collect()), other => panic!(\"expected struct {}, got {{other:?}}\") }}",
                id.raw(),
                record.name,
                record.name
            )
        }
        _ => format!("__kiro_to_host({value})"),
    }
}

fn host_from_expression(program: &EirProgram, ty: TypeId, value: &str) -> String {
    match program.types.get(ty) {
        Some(SemType::List(inner)) => format!(
            "match {value} {{ RuntimeVal::List(values) => __KiroValue::List(values.into_iter().map(__kiro_from_host_t{}).collect()), other => panic!(\"expected host list, got {{other:?}}\") }}",
            inner.raw()
        ),
        Some(SemType::Map(_, inner)) => format!(
            "match {value} {{ RuntimeVal::Map(values) => __KiroValue::Map(values.into_iter().map(|(key, value)| (key, __kiro_from_host_t{}(value))).collect()), other => panic!(\"expected host map, got {{other:?}}\") }}",
            inner.raw()
        ),
        Some(SemType::Struct(id)) => {
            let record = program
                .struct_def(*id)
                .expect("EIR struct type must have metadata");
            let fields = record
                .fields
                .iter()
                .map(|field| {
                    format!(
                        "({}, __kiro_from_host_t{}(fields.remove({:?}).expect(\"missing host struct field\")))",
                        field.id.raw(),
                        field.ty.raw(),
                        field.name
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "match {value} {{ RuntimeVal::Struct {{ type_name, mut fields }} if type_name == {:?} => __KiroValue::Struct({}, [{fields}].into_iter().collect()), other => panic!(\"expected host struct {}, got {{other:?}}\") }}",
                record.name,
                id.raw(),
                record.name
            )
        }
        _ => format!("__kiro_from_host({value})"),
    }
}

fn compile_function(
    program: &EirProgram,
    function: &EirFunction,
    indirect_host_functions: &HashSet<u32>,
    output: &mut String,
) -> Result<(), EirCodegenError> {
    writeln!(
        output,
        "fn __kiro_eir_f{}(__args: Vec<__KiroValue>, __globals: __KiroGlobals) -> __KiroFuture {{",
        function.id.raw()
    )
    .unwrap();
    output.push_str("    Box::pin(async move {\n");
    writeln!(
        output,
        "        let mut __slots: Vec<Option<__KiroValue>> = vec![None; {}];",
        function.slots.len()
    )
    .unwrap();
    writeln!(
        output,
        "        for (__slot, __arg) in __slots.iter_mut().take({}).zip(__args) {{ *__slot = Some(__arg); }}",
        function.parameter_count
    )
    .unwrap();
    output.push_str(
        "        let mut __block: u32 = 0;\n        loop {\n            match __block {\n",
    );
    for (index, block) in function.blocks.iter().enumerate() {
        writeln!(output, "                {index} => {{").unwrap();
        for instruction in &block.instructions {
            compile_instruction(program, &instruction.kind, indirect_host_functions, output)?;
        }
        compile_terminator(&block.terminator.kind, output)?;
        output.push_str("                }\n");
    }
    output.push_str("                _ => panic!(\"invalid verified EIR block {__block}\"),\n");
    output.push_str("            }\n        }\n    })\n}\n\n");
    Ok(())
}

fn compile_instruction(
    program: &EirProgram,
    instruction: &InstructionKind,
    indirect_host_functions: &HashSet<u32>,
    output: &mut String,
) -> Result<(), EirCodegenError> {
    let line = match instruction {
        InstructionKind::Const { dst, constant } => {
            let value =
                match &program.constants[usize::try_from(*constant).expect("verified constant")] {
                    Constant::Bool(value) => format!("__KiroValue::Bool({value})"),
                    Constant::Num(value) => format!("__KiroValue::Num({value:?})"),
                    Constant::Str(value) => format!("__KiroValue::Str({value:?}.to_string())"),
                };
            assign(*dst, value)
        }
        InstructionKind::Copy { dst, src } => assign(*dst, clone_slot(*src)),
        InstructionKind::Move { dst, src } => assign(*dst, take_slot(*src)),
        InstructionKind::LoadGlobal { dst, global } => assign(
            *dst,
            format!(
                "__globals.lock().unwrap()[{}].clone().expect(\"initialized global\")",
                global.raw()
            ),
        ),
        InstructionKind::MoveGlobal { dst, global } => assign(
            *dst,
            format!(
                "__globals.lock().unwrap()[{}].take().expect(\"initialized global\")",
                global.raw()
            ),
        ),
        InstructionKind::StoreGlobal { global, src } => format!(
            "*__globals.lock().unwrap().get_mut({}).expect(\"verified global\") = Some({});",
            global.raw(),
            clone_slot(*src)
        ),
        InstructionKind::MakeError { dst, error } => {
            assign(*dst, format!("__KiroValue::Error({})", error.raw()))
        }
        InstructionKind::IsError { dst, value } => assign(
            *dst,
            format!(
                "__KiroValue::Bool(matches!({}, __KiroValue::Error(_)))",
                slot(*value)
            ),
        ),
        InstructionKind::ErrorMatches { dst, value, error } => assign(
            *dst,
            format!(
                "__KiroValue::Bool(matches!({}, __KiroValue::Error(id) if *id == {}))",
                slot(*value),
                error.raw()
            ),
        ),
        InstructionKind::IsTruthy { dst, value } => assign(
            *dst,
            format!("__KiroValue::Bool(__kiro_truthy(&{}))", slot(*value)),
        ),
        InstructionKind::Check { condition, message } => {
            let message = message
                .and_then(|id| program.constants.get(usize::try_from(id).ok()?))
                .and_then(|constant| match constant {
                    Constant::Str(value) => Some(value),
                    _ => None,
                })
                .map_or("check failed", String::as_str);
            format!(
                "if !__kiro_bool(&{}) {{ kiro_check_failed({:?}); }}",
                slot(*condition),
                message
            )
        }
        InstructionKind::MakeFunction { dst, function } => assign(
            *dst,
            format!(
                "__KiroValue::Function {{ host: false, id: {} }}",
                function.raw()
            ),
        ),
        InstructionKind::MakeHostFunction { dst, function } => assign(
            *dst,
            format!(
                "__KiroValue::Function {{ host: true, id: {} }}",
                function.raw()
            ),
        ),
        InstructionKind::MakeAddress { dst } => assign(
            *dst,
            "__KiroValue::Address(Arc::new(Mutex::new(None)))".to_string(),
        ),
        InstructionKind::MakeRef { dst, value } => assign(
            *dst,
            format!(
                "__KiroValue::Address(Arc::new(Mutex::new(Some({}))))",
                clone_slot(*value)
            ),
        ),
        InstructionKind::Deref { dst, address } => assign(
            *dst,
            format!(
                "match {} {{ __KiroValue::Address(value) => match value.lock().unwrap().clone() {{ Some(value) => value, None => kiro_runtime_error_help(\"KIRO3006\", \"Cannot deref an empty address.\", \"Assign it with `ref value` before using `deref`.\") }}, other => panic!(\"expected address, got {{other:?}}\") }}",
                slot(*address)
            ),
        ),
        InstructionKind::StoreDeref { address, src } => format!(
            "match {} {{ __KiroValue::Address(value) => *value.lock().unwrap() = Some({}), other => panic!(\"expected address, got {{other:?}}\") }};",
            slot(*address),
            clone_slot(*src)
        ),
        InstructionKind::MakeList { dst, items } => assign(
            *dst,
            format!(
                "__KiroValue::List(vec![{}])",
                items
                    .iter()
                    .map(|item| clone_slot(*item))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
        InstructionKind::MakeMap { dst, entries } => {
            let entries = entries
                .iter()
                .map(|(key, value)| {
                    format!(
                        "__values.insert(__kiro_map_key(&{}), {});",
                        slot(*key),
                        clone_slot(*value)
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            assign(
                *dst,
                format!(
                    "{{ let mut __values = HashMap::new(); {entries} __KiroValue::Map(__values) }}"
                ),
            )
        }
        InstructionKind::ListJoin { dst, left, right } => assign(
            *dst,
            format!(
                "{{ let mut __joined = match {} {{ __KiroValue::List(values) => values.clone(), other => panic!(\"expected list, got {{other:?}}\") }}; match {} {{ __KiroValue::List(values) => __joined.extend_from_slice(values), other => panic!(\"expected list, got {{other:?}}\") }}; __KiroValue::List(__joined) }}",
                slot(*left),
                slot(*right)
            ),
        ),
        InstructionKind::ListSlice {
            dst,
            list,
            start,
            end,
        } => assign(
            *dst,
            format!(
                "{{ let __start = __kiro_num(&{}); let __end = __kiro_num(&{}); match {} {{ __KiroValue::List(values) => {{ if !__start.is_finite() || !__end.is_finite() || __start.fract() != 0.0 || __end.fract() != 0.0 || __start < 0.0 || __start > __end || __end > values.len() as f64 {{ kiro_runtime_error(\"KIRO3004\", &format!(\"Invalid list range {{__start}}..{{__end}} for length {{}}.\", values.len())) }} __KiroValue::List(values[__start as usize..__end as usize].to_vec()) }}, other => panic!(\"expected list, got {{other:?}}\") }} }}",
                slot(*start),
                slot(*end),
                slot(*list)
            ),
        ),
        InstructionKind::ListReverse { dst, list } => assign(
            *dst,
            format!(
                "match {} {{ __KiroValue::List(values) => {{ let mut values = values.clone(); values.reverse(); __KiroValue::List(values) }}, other => panic!(\"expected list, got {{other:?}}\") }}",
                slot(*list)
            ),
        ),
        InstructionKind::MapHas { dst, map, key } => assign(
            *dst,
            format!(
                "{{ let __key = __kiro_map_key(&{}); __KiroValue::Bool(match {} {{ __KiroValue::Map(values) => values.contains_key(&__key), other => panic!(\"expected map, got {{other:?}}\") }}) }}",
                slot(*key),
                slot(*map)
            ),
        ),
        InstructionKind::MapSet {
            dst,
            map,
            key,
            value,
        } => assign(
            *dst,
            format!(
                "{{ let __key = __kiro_map_key(&{}); let __value = {}; match {} {{ __KiroValue::Map(values) => {{ let mut values = values.clone(); values.insert(__key, __value); __KiroValue::Map(values) }}, other => panic!(\"expected map, got {{other:?}}\") }} }}",
                slot(*key),
                clone_slot(*value),
                slot(*map)
            ),
        ),
        InstructionKind::MapDelete { dst, map, key } => assign(
            *dst,
            format!(
                "{{ let __key = __kiro_map_key(&{}); match {} {{ __KiroValue::Map(values) => {{ let mut values = values.clone(); values.remove(&__key); __KiroValue::Map(values) }}, other => panic!(\"expected map, got {{other:?}}\") }} }}",
                slot(*key),
                slot(*map)
            ),
        ),
        InstructionKind::MakeStruct {
            dst,
            structure,
            fields,
        } => {
            let fields = fields
                .iter()
                .map(|(field, value)| {
                    format!("__fields.insert({}, {});", field.raw(), clone_slot(*value))
                })
                .collect::<Vec<_>>()
                .join(" ");
            assign(
                *dst,
                format!(
                    "{{ let mut __fields = HashMap::new(); {fields} __KiroValue::Struct({}, __fields) }}",
                    structure.raw()
                ),
            )
        }
        InstructionKind::GetField { dst, target, field } => assign(
            *dst,
            format!(
                "match {} {{ __KiroValue::Struct(_, fields) => fields.get(&{}).cloned().expect(\"verified field\"), other => panic!(\"expected struct, got {{other:?}}\") }}",
                slot(*target),
                field.raw()
            ),
        ),
        InstructionKind::SetField {
            target,
            fields,
            src,
        } => format!(
            "{{ let __value = {}; __kiro_set_field(__slots[{}].as_mut().expect(\"initialized struct\"), &[{}], __value); }}",
            clone_slot(*src),
            target.raw(),
            fields
                .iter()
                .map(|field| field.raw().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        InstructionKind::GetIndex {
            dst,
            collection,
            key,
        } => assign(
            *dst,
            format!(
                "match {} {{ __KiroValue::Bytes(values) => {{ let index = __kiro_num(&{}) as usize; __KiroValue::Num(*values.get(index).unwrap_or_else(|| kiro_runtime_error(\"KIRO3004\", &format!(\"Bytes index out of bounds: index {{index}}, length {{}}.\", values.len()))) as f64) }}, __KiroValue::List(values) => {{ let index = __kiro_num(&{}) as usize; values.get(index).cloned().unwrap_or_else(|| kiro_runtime_error(\"KIRO3004\", &format!(\"List index out of bounds: index {{index}}, length {{}}.\", values.len()))) }}, __KiroValue::Map(values) => {{ let key = __kiro_map_key(&{}); values.get(&key).cloned().unwrap_or_else(|| kiro_runtime_error(\"KIRO3005\", &format!(\"Map key not found: {{key:?}}.\"))) }}, other => panic!(\"expected bytes, list, or map, got {{other:?}}\") }}",
                slot(*collection),
                slot(*key),
                slot(*key),
                slot(*key)
            ),
        ),
        InstructionKind::Push { collection, value } => format!(
            "{{ let __value = {}; match __slots[{}].as_mut().expect(\"initialized list\") {{ __KiroValue::List(values) => values.push(__value), other => panic!(\"expected list, got {{other:?}}\") }} }}",
            clone_slot(*value),
            collection.raw()
        ),
        InstructionKind::Len { dst, collection } => assign(
            *dst,
            format!(
                "__KiroValue::Num(match {} {{ __KiroValue::Str(value) => value.len(), __KiroValue::Bytes(value) => value.len(), __KiroValue::List(value) => value.len(), __KiroValue::Map(value) => value.len(), other => panic!(\"expected collection, got {{other:?}}\") }} as f64)",
                slot(*collection)
            ),
        ),
        InstructionKind::MakeRange { dst, start, end } => assign(
            *dst,
            format!(
                "__KiroValue::Range(__kiro_num(&{}) as i64, __kiro_num(&{}) as i64)",
                slot(*start),
                slot(*end)
            ),
        ),
        InstructionKind::IterInit { dst, iterable } => assign(
            *dst,
            format!(
                "__KiroValue::Num(match {} {{ __KiroValue::Range(start, _) => *start as f64, __KiroValue::List(_) | __KiroValue::Str(_) => 0.0, other => panic!(\"expected iterable, got {{other:?}}\") }})",
                slot(*iterable)
            ),
        ),
        InstructionKind::IterHasNext {
            dst,
            iterable,
            index,
        } => assign(
            *dst,
            format!(
                "__KiroValue::Bool({{ let index = __kiro_num(&{}); match {} {{ __KiroValue::Range(_, end) => index < *end as f64, __KiroValue::List(values) => (index as usize) < values.len(), __KiroValue::Str(value) => (index as usize) < value.chars().count(), other => panic!(\"expected iterable, got {{other:?}}\") }} }})",
                slot(*index),
                slot(*iterable)
            ),
        ),
        InstructionKind::IterGet {
            dst,
            iterable,
            index,
        } => assign(
            *dst,
            format!(
                "{{ let index = __kiro_num(&{}); match {} {{ __KiroValue::Range(_, _) => __KiroValue::Num(index), __KiroValue::List(values) => values[index as usize].clone(), __KiroValue::Str(value) => __KiroValue::Str(value.chars().nth(index as usize).unwrap().to_string()), other => panic!(\"expected iterable, got {{other:?}}\") }} }}",
                slot(*index),
                slot(*iterable)
            ),
        ),
        InstructionKind::AddNum { dst, lhs, rhs } => num_binary(*dst, *lhs, *rhs, "+"),
        InstructionKind::SubNum { dst, lhs, rhs } => num_binary(*dst, *lhs, *rhs, "-"),
        InstructionKind::MulNum { dst, lhs, rhs } => num_binary(*dst, *lhs, *rhs, "*"),
        InstructionKind::DivNum { dst, lhs, rhs } => num_binary(*dst, *lhs, *rhs, "/"),
        InstructionKind::ConcatString { dst, lhs, rhs } => assign(
            *dst,
            format!(
                "__KiroValue::Str(format!(\"{{}}{{}}\", __kiro_str(&{}), __kiro_str(&{})))",
                slot(*lhs),
                slot(*rhs)
            ),
        ),
        InstructionKind::EqNum { dst, lhs, rhs } => num_compare(*dst, *lhs, *rhs, "=="),
        InstructionKind::NeNum { dst, lhs, rhs } => num_compare(*dst, *lhs, *rhs, "!="),
        InstructionKind::GtNum { dst, lhs, rhs } => num_compare(*dst, *lhs, *rhs, ">"),
        InstructionKind::LtNum { dst, lhs, rhs } => num_compare(*dst, *lhs, *rhs, "<"),
        InstructionKind::GeNum { dst, lhs, rhs } => num_compare(*dst, *lhs, *rhs, ">="),
        InstructionKind::LeNum { dst, lhs, rhs } => num_compare(*dst, *lhs, *rhs, "<="),
        InstructionKind::EqString { dst, lhs, rhs } => string_compare(*dst, *lhs, *rhs, "=="),
        InstructionKind::NeString { dst, lhs, rhs } => string_compare(*dst, *lhs, *rhs, "!="),
        InstructionKind::EqBool { dst, lhs, rhs } => bool_compare(*dst, *lhs, *rhs, "=="),
        InstructionKind::NeBool { dst, lhs, rhs } => bool_compare(*dst, *lhs, *rhs, "!="),
        InstructionKind::CallDirect {
            dst,
            function,
            args,
        } => {
            let call = format!(
                "__kiro_eir_f{}(vec![{}], __globals.clone()).await",
                function.raw(),
                args.iter()
                    .map(|slot| clone_slot(*slot))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            match dst {
                Some(dst) => assign(*dst, call),
                None => format!("let _ = {call};"),
            }
        }
        InstructionKind::CallHost {
            dst,
            function,
            args,
        } => {
            let metadata =
                &program.host_functions[usize::try_from(*function).expect("verified host")];
            let values = args
                .iter()
                .map(|argument| clone_slot(*argument))
                .collect::<Vec<_>>();
            let call = host_call_expression(metadata, &format!("vec![{}]", values.join(", ")));
            match dst {
                Some(dst) => assign(*dst, call),
                None => format!("let _ = {call};"),
            }
        }
        InstructionKind::CallIndirect { dst, callee, args } => {
            let args = format!(
                "vec![{}]",
                args.iter()
                    .map(|argument| clone_slot(*argument))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let kiro_arms = program
                .functions
                .iter()
                .map(|function| {
                    format!(
                        "{} => __kiro_eir_f{}(__call_args, __globals.clone()).await,",
                        function.id.raw(),
                        function.id.raw()
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            let host_arms = program
                .host_functions
                .iter()
                .filter(|function| indirect_host_functions.contains(&function.id.raw()))
                .map(|function| {
                    format!(
                        "{} => {},",
                        function.id.raw(),
                        host_call_expression(function, "__call_args")
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            let call = format!(
                "{{ let __call_args = {args}; match {} {{ __KiroValue::Function {{ host: false, id }} => match id {{ {kiro_arms} _ => panic!(\"invalid Kiro function id\") }}, __KiroValue::Function {{ host: true, id }} => match id {{ {host_arms} _ => panic!(\"invalid host function id\") }}, other => panic!(\"expected function, got {{other:?}}\") }} }}",
                clone_slot(*callee)
            );
            match dst {
                Some(dst) => assign(*dst, call),
                None => format!("let _ = {call};"),
            }
        }
        InstructionKind::MakePipe { dst, capacity } => {
            let channel = match capacity {
                Some(0) => {
                    "{ let (sender, receiver) = async_channel::bounded(1); let (__rendezvous_ack_sender, __rendezvous_ack_receiver) = async_channel::bounded(1); __KiroPipe { sender, receiver, rendezvous_ack_sender: Some(__rendezvous_ack_sender), rendezvous_ack_receiver: Some(__rendezvous_ack_receiver) } }".to_string()
                }
                Some(capacity) => format!(
                    "{{ let (sender, receiver) = async_channel::bounded({capacity}); __KiroPipe {{ sender, receiver, rendezvous_ack_sender: None, rendezvous_ack_receiver: None }} }}"
                ),
                None => {
                    "{ let (sender, receiver) = async_channel::unbounded(); __KiroPipe { sender, receiver, rendezvous_ack_sender: None, rendezvous_ack_receiver: None } }".to_string()
                }
            };
            assign(*dst, format!("__KiroValue::Pipe({channel})"))
        }
        InstructionKind::Give { channel, value } => format!(
            "{{ let __value = {}; match {} {{ __KiroValue::Pipe(pipe) => {{ if pipe.sender.send(__value).await.is_err() {{ kiro_runtime_error(\"KIRO3003\", \"Pipe receiver is closed; cannot give a value.\") }} if let Some(__rendezvous_ack) = &pipe.rendezvous_ack_receiver {{ if __rendezvous_ack.recv().await.is_err() {{ kiro_runtime_error(\"KIRO3003\", \"Pipe receiver is closed; cannot give a value.\") }} }} }}, other => panic!(\"expected pipe, got {{other:?}}\") }} }}",
            clone_slot(*value),
            slot(*channel),
        ),
        InstructionKind::Take { dst, channel } => assign(
            *dst,
            format!(
                "match {} {{ __KiroValue::Pipe(pipe) => {{ let __value = pipe.receiver.recv().await.unwrap_or_else(|_| kiro_runtime_error(\"KIRO3002\", \"Pipe is closed; cannot take a value.\")); if let Some(__rendezvous_ack) = &pipe.rendezvous_ack_sender {{ let _ = __rendezvous_ack.send(()).await; }} __value }}, other => panic!(\"expected pipe, got {{other:?}}\") }}",
                slot(*channel)
            ),
        ),
        InstructionKind::Close { channel } => format!(
            "match {} {{ __KiroValue::Pipe(pipe) => {{ pipe.sender.close(); if let Some(__rendezvous_ack) = &pipe.rendezvous_ack_sender {{ __rendezvous_ack.close(); }} }}, other => panic!(\"expected pipe, got {{other:?}}\") }};",
            slot(*channel)
        ),
        InstructionKind::Rest => "tokio::task::yield_now().await;".to_string(),
        InstructionKind::Spawn { function, args } => format!(
            "{{ let __task_args = vec![{}]; let __task_globals = __globals.clone(); tokio::spawn(async move {{ let _ = __kiro_eir_f{}(__task_args, __task_globals).await; }}); }}",
            args.iter()
                .map(|argument| clone_slot(*argument))
                .collect::<Vec<_>>()
                .join(", "),
            function.raw()
        ),
    };
    writeln!(output, "                    {line}").unwrap();
    Ok(())
}

fn compile_terminator(
    terminator: &TerminatorKind,
    output: &mut String,
) -> Result<(), EirCodegenError> {
    let line = match terminator {
        TerminatorKind::Jump(block) => format!("__block = {}; continue;", block.raw()),
        TerminatorKind::Branch {
            condition,
            then_block,
            else_block,
        } => format!(
            "__block = if __kiro_bool(&{}) {{ {} }} else {{ {} }}; continue;",
            slot(*condition),
            then_block.raw(),
            else_block.raw()
        ),
        TerminatorKind::Return(Some(value)) => format!("return {};", take_slot(*value)),
        TerminatorKind::Return(None) => "return __KiroValue::Void;".to_string(),
        TerminatorKind::Unreachable => "panic!(\"reached verified unreachable EIR\");".to_string(),
        TerminatorKind::Throw(value) => format!(
            "panic!(\"uncaught Kiro error: {{:?}}\", {});",
            take_slot(*value)
        ),
    };
    writeln!(output, "                    {line}").unwrap();
    Ok(())
}

fn validate_supported(_program: &EirProgram) -> Result<(), EirCodegenError> {
    Ok(())
}

fn assign(dst: SlotId, value: String) -> String {
    format!("__slots[{}] = Some({value});", dst.raw())
}

fn slot(slot: SlotId) -> String {
    format!(
        "__slots[{}].as_ref().expect(\"verified initialized slot\")",
        slot.raw()
    )
}

fn clone_slot(slot: SlotId) -> String {
    format!("{}.clone()", self::slot(slot))
}

fn take_slot(slot: SlotId) -> String {
    format!(
        "__slots[{}].take().expect(\"verified initialized slot\")",
        slot.raw()
    )
}

fn num_binary(dst: SlotId, lhs: SlotId, rhs: SlotId, operator: &str) -> String {
    assign(
        dst,
        format!(
            "__KiroValue::Num(__kiro_num(&{}) {operator} __kiro_num(&{}))",
            slot(lhs),
            slot(rhs)
        ),
    )
}

fn num_compare(dst: SlotId, lhs: SlotId, rhs: SlotId, operator: &str) -> String {
    assign(
        dst,
        format!(
            "__KiroValue::Bool(__kiro_num(&{}) {operator} __kiro_num(&{}))",
            slot(lhs),
            slot(rhs)
        ),
    )
}

fn string_compare(dst: SlotId, lhs: SlotId, rhs: SlotId, operator: &str) -> String {
    assign(
        dst,
        format!(
            "__KiroValue::Bool(__kiro_str(&{}) {operator} __kiro_str(&{}))",
            slot(lhs),
            slot(rhs)
        ),
    )
}

fn bool_compare(dst: SlotId, lhs: SlotId, rhs: SlotId, operator: &str) -> String {
    assign(
        dst,
        format!(
            "__KiroValue::Bool(__kiro_bool(&{}) {operator} __kiro_bool(&{}))",
            slot(lhs),
            slot(rhs)
        ),
    )
}

fn host_call_expression(function: &EirHostFunction, args: &str) -> String {
    if crate::is_std_io_module_name(&function.module)
        && crate::is_std_io_display_function(&function.name)
    {
        let macro_name = match function.name.as_str() {
            "print" => "println",
            "write" => "print",
            "eprint" => "eprint",
            "eprintline" => "eprintln",
            _ => unreachable!("display helper checked above"),
        };
        return format!(
            "{{ let __display_args = {args}; {macro_name}!(\"{{}}\", __kiro_display(&__display_args[0])); __KiroValue::Void }}"
        );
    }
    let await_suffix = if function
        .signature
        .effects()
        .contains(crate::hir::Effects::PURE)
    {
        ""
    } else {
        ".await"
    };
    let error = if function
        .signature
        .effects()
        .contains(crate::hir::Effects::MAY_FAIL)
    {
        "__kiro_from_host_error(error)".to_string()
    } else {
        format!(
            "kiro_runtime_error(\"KIRO3007\", &format!(\"Host function '{}' failed: {{error}}.\"))",
            function.name
        )
    };
    let host_args = function
        .signature
        .params()
        .iter()
        .enumerate()
        .map(|(index, ty)| format!("__kiro_to_host_t{}(&__host_args[{index}])", ty.raw()))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = function.signature.return_type().raw();
    format!(
        "{{ let __host_args: Vec<__KiroValue> = {args}; match header::{}(vec![{host_args}]){await_suffix} {{ Ok(value) => __kiro_from_host_t{return_type}(value), Err(error) => {error} }} }}",
        function.name
    )
}
