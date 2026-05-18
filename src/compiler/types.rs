use crate::grammar::grammar::{KiroType, StructNameVal};

pub fn compile_type(t: &KiroType) -> String {
    match t {
        KiroType::Num => compile_num(),
        KiroType::Str => compile_str(),
        KiroType::Bool => compile_bool(),
        KiroType::Void => compile_void(),
        KiroType::Adr(_, inner) => compile_adr(inner),
        KiroType::Pipe(_, inner) => compile_pipe(inner),
        KiroType::List(_, inner) => compile_list(inner),
        KiroType::Map(_, k, v) => compile_map(k, v),
        KiroType::FnType(_, _, params, _, _, ret) => compile_fn_type(params, ret),
        KiroType::Custom(s) => compile_custom(s),
    }
}

pub fn compile_num() -> String {
    "f64".to_string()
}

pub fn compile_str() -> String {
    "String".to_string()
}

pub fn compile_bool() -> String {
    "bool".to_string()
}

pub fn compile_void() -> String {
    "()".to_string()
}

pub fn compile_adr(inner: &KiroType) -> String {
    // Adr<void> is an opaque managed handle.
    if let KiroType::Void = inner {
        "KiroAdrVoid".to_string()
    } else {
        // Otherwise, it's a lazy pointer: Option<Arc<Mutex<T>>>
        format!(
            "Option<std::sync::Arc<std::sync::Mutex<{}>>>",
            compile_type(inner)
        )
    }
}

pub fn compile_pipe(inner: &KiroType) -> String {
    format!("KiroPipe<{}>", compile_type(inner))
}

pub fn compile_custom(name: &StructNameVal) -> String {
    name.value.clone()
}

pub fn compile_list(inner: &KiroType) -> String {
    format!("Vec<{}>", compile_type(inner))
}

pub fn compile_map(key: &KiroType, value: &KiroType) -> String {
    format!(
        "std::collections::HashMap<{}, {}>",
        compile_type(key),
        compile_type(value)
    )
}

pub fn compile_fn_type(params: &[KiroType], ret: &KiroType) -> String {
    let args = params
        .iter()
        .map(compile_type)
        .collect::<Vec<_>>()
        .join(", ");
    let ret_ty = compile_type(ret);
    format!("fn({}) -> {}", args, ret_ty)
}
