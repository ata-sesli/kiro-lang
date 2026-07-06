use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::MetadataCommand;
use syn::{
    Attribute, FnArg, GenericArgument, ImplItem, Item, ItemFn, ItemImpl, ItemMod, Pat,
    PathArguments, ReturnType, Type, Visibility,
};

use crate::errors::{ErrorCode, ErrorPhase, KiroError};
use crate::project::{self, KiroProject};

const GENERATED_BEGIN: &str = "// kiro:generated begin";
const GENERATED_END: &str = "// kiro:generated end";

#[derive(Debug, Clone)]
pub struct HostGenOptions {
    pub crate_name: String,
    pub module_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HostGenResult {
    pub module_name: String,
    pub declarations: usize,
    pub skipped: Vec<String>,
    pub kiro_path: PathBuf,
    pub rust_path: PathBuf,
}

#[derive(Debug, Clone)]
struct Binding {
    exported_name: String,
    source: BindingSource,
    params: Vec<Param>,
    return_type: RustType,
    can_error: bool,
    error_name: Option<String>,
    pure: bool,
}

#[derive(Debug, Clone)]
enum BindingSource {
    CrateFunction {
        path: String,
    },
    Constructor {
        path: String,
    },
    Method {
        crate_ident: String,
        method_name: String,
    },
    ManualFunction {
        module: String,
        function: String,
    },
}

#[derive(Debug, Clone)]
struct Param {
    name: String,
    rust_type: RustType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RustType {
    Str,
    Num { rust: String },
    Bool,
    Void,
    List(Box<RustType>),
    Map(Box<RustType>),
    Handle(String),
}

#[derive(Debug, Default)]
struct Collector {
    bindings: Vec<Binding>,
    handles: BTreeSet<String>,
    skipped: Vec<String>,
    crate_ident: String,
    manual_module: String,
}

pub fn generate(
    project: &KiroProject,
    options: HostGenOptions,
) -> Result<HostGenResult, KiroError> {
    let Some(dep) = project
        .dependencies
        .iter()
        .find(|dep| dep.name == options.crate_name)
    else {
        return Err(KiroError::new(
            ErrorCode::BuildGraphFailed,
            ErrorPhase::Cli,
            format!(
                "Dependency '{}' is not declared in kiro.toml.",
                options.crate_name
            ),
        )
        .with_file(project.manifest_path.display().to_string())
        .with_help(format!(
            "run `kiro add {}@version` first",
            options.crate_name
        )));
    };

    let module_name = options
        .module_name
        .unwrap_or_else(|| options.crate_name.replace('-', "_"));
    if !project::is_valid_cargo_dependency_name(&module_name) {
        return Err(KiroError::new(
            ErrorCode::ParseFailed,
            ErrorPhase::Cli,
            format!("Invalid host module name '{}'.", module_name),
        ));
    }

    let resolved = resolve_crate(project, &options.crate_name, &dep.version)?;
    let crate_root = resolved.root;
    let crate_ident = options.crate_name.replace('-', "_");
    let manual_module = manual_module_name(&module_name);
    let mut collector = Collector {
        crate_ident: crate_ident.clone(),
        manual_module,
        ..Collector::default()
    };
    collect_crate(&crate_root, &mut collector)?;

    let rust_path = project.root.join(format!("{}.rs", module_name));
    if rust_path.exists() {
        collect_manual_exports(&rust_path, &mut collector)?;
    }

    if collector.bindings.is_empty() {
        return Err(KiroError::new(
            ErrorCode::BuildGraphFailed,
            ErrorPhase::Cli,
            format!(
                "No supported Kiro bindings found for '{}'.",
                options.crate_name
            ),
        )
        .with_help(format!(
            "write fallback exports inside `mod {}` with #[kiro_export]",
            collector.manual_module
        )));
    }

    collector
        .bindings
        .sort_by(|a, b| a.exported_name.cmp(&b.exported_name));

    let kiro_path = project.root.join(format!("{}.kiro", module_name));
    fs::write(&kiro_path, render_kiro_module(&collector)).map_err(|e| {
        KiroError::new(
            ErrorCode::BuildGraphFailed,
            ErrorPhase::Cli,
            format!("Failed to write '{}': {}", kiro_path.display(), e),
        )
    })?;

    let existing = if rust_path.exists() {
        fs::read_to_string(&rust_path).map_err(|e| {
            KiroError::new(
                ErrorCode::BuildGraphFailed,
                ErrorPhase::Cli,
                format!("Failed to read '{}': {}", rust_path.display(), e),
            )
        })?
    } else {
        initial_rust_module(&collector.manual_module)
    };
    let rust = replace_generated_region(&existing, &render_rust_glue(&collector));
    fs::write(&rust_path, rust).map_err(|e| {
        KiroError::new(
            ErrorCode::BuildGraphFailed,
            ErrorPhase::Cli,
            format!("Failed to write '{}': {}", rust_path.display(), e),
        )
    })?;

    Ok(HostGenResult {
        module_name,
        declarations: collector.bindings.len() + collector.handles.len(),
        skipped: collector.skipped,
        kiro_path,
        rust_path,
    })
}

#[derive(Debug, Clone)]
pub struct ResolvedCrate {
    pub root: PathBuf,
    pub version: String,
}

pub fn resolve_crate(
    project: &KiroProject,
    crate_name: &str,
    version: &str,
) -> Result<ResolvedCrate, KiroError> {
    let manifest_path = project.root.join("Cargo.toml");
    let metadata = if manifest_path.exists() {
        MetadataCommand::new()
            .manifest_path(&manifest_path)
            .exec()
            .map_err(metadata_error)?
    } else {
        let dir = project.root.join(".kiro/host_gen");
        fs::create_dir_all(&dir).map_err(|e| {
            KiroError::new(
                ErrorCode::BuildGraphFailed,
                ErrorPhase::Cli,
                format!("Failed to create '{}': {}", dir.display(), e),
            )
        })?;
        let src_dir = dir.join("src");
        fs::create_dir_all(&src_dir).map_err(|e| {
            KiroError::new(
                ErrorCode::BuildGraphFailed,
                ErrorPhase::Cli,
                format!("Failed to create '{}': {}", src_dir.display(), e),
            )
        })?;
        fs::write(src_dir.join("lib.rs"), "").map_err(|e| {
            KiroError::new(
                ErrorCode::BuildGraphFailed,
                ErrorPhase::Cli,
                format!("Failed to write metadata probe source: {}", e),
            )
        })?;
        let manifest = format!(
            r#"[package]
name = "kiro_host_gen_probe"
version = "0.1.0"
edition = "2021"

[dependencies]
{} = "{}"
"#,
            crate_name, version
        );
        let manifest_path = dir.join("Cargo.toml");
        fs::write(&manifest_path, manifest).map_err(|e| {
            KiroError::new(
                ErrorCode::BuildGraphFailed,
                ErrorPhase::Cli,
                format!("Failed to write '{}': {}", manifest_path.display(), e),
            )
        })?;
        MetadataCommand::new()
            .manifest_path(&manifest_path)
            .exec()
            .map_err(metadata_error)?
    };

    let package = metadata
        .packages
        .iter()
        .find(|package| package.name.as_str() == crate_name)
        .ok_or_else(|| {
            KiroError::new(
                ErrorCode::BuildGraphFailed,
                ErrorPhase::Cli,
                format!(
                    "Cargo metadata did not resolve dependency '{}'.",
                    crate_name
                ),
            )
        })?;
    let lib_target = package
        .targets
        .iter()
        .find(|target| {
            target
                .kind
                .iter()
                .any(|kind| matches!(kind, cargo_metadata::TargetKind::Lib))
        })
        .ok_or_else(|| {
            KiroError::new(
                ErrorCode::BuildGraphFailed,
                ErrorPhase::Cli,
                format!(
                    "Dependency '{}' does not expose a library target.",
                    crate_name
                ),
            )
        })?;
    Ok(ResolvedCrate {
        root: lib_target.src_path.as_std_path().to_path_buf(),
        version: package.version.to_string(),
    })
}

fn metadata_error(error: cargo_metadata::Error) -> KiroError {
    KiroError::new(
        ErrorCode::BuildGraphFailed,
        ErrorPhase::Cli,
        format!("Failed to inspect Cargo metadata: {}", error),
    )
}

fn collect_crate(root: &Path, collector: &mut Collector) -> Result<(), KiroError> {
    let source = fs::read_to_string(root).map_err(|e| {
        KiroError::new(
            ErrorCode::FileNotFound,
            ErrorPhase::Cli,
            format!("Failed to read '{}': {}", root.display(), e),
        )
    })?;
    let file = syn::parse_file(&source).map_err(|e| {
        KiroError::new(
            ErrorCode::ParseFailed,
            ErrorPhase::Cli,
            format!("Failed to parse Rust source '{}': {}", root.display(), e),
        )
    })?;

    let mut public_structs = BTreeSet::new();
    for item in &file.items {
        if let Item::Struct(item_struct) = item
            && is_public(&item_struct.vis)
        {
            public_structs.insert(item_struct.ident.to_string());
        }
    }

    for item in &file.items {
        match item {
            Item::Fn(item_fn) if is_public(&item_fn.vis) => {
                match binding_from_fn(
                    item_fn,
                    BindingSource::CrateFunction {
                        path: format!("{}::{}", collector.crate_ident, item_fn.sig.ident),
                    },
                    &public_structs,
                    false,
                ) {
                    Ok(binding) => collector.bindings.push(binding),
                    Err(reason) => collector
                        .skipped
                        .push(format!("{}: {}", item_fn.sig.ident, reason)),
                }
            }
            Item::Impl(item_impl) => collect_impl(item_impl, &public_structs, collector),
            _ => {}
        }
    }
    Ok(())
}

fn collect_impl(
    item_impl: &ItemImpl,
    public_structs: &BTreeSet<String>,
    collector: &mut Collector,
) {
    if item_impl.trait_.is_some() {
        return;
    }
    let Some(type_name) = impl_type_name(&item_impl.self_ty) else {
        return;
    };
    if !public_structs.contains(&type_name) {
        return;
    }
    for item in &item_impl.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        if !is_public(&method.vis) {
            continue;
        }
        if !method.sig.generics.params.is_empty() || method.sig.generics.lt_token.is_some() {
            collector.skipped.push(format!(
                "{}::{}: generics are unsupported",
                type_name, method.sig.ident
            ));
            continue;
        }

        if let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() {
            if receiver.reference.is_none() || receiver.mutability.is_some() {
                collector.skipped.push(format!(
                    "{}::{}: mutable or by-value receivers are unsupported",
                    type_name, method.sig.ident
                ));
                continue;
            }
            let mut params = vec![Param {
                name: to_snake_case(&type_name),
                rust_type: RustType::Handle(type_name.clone()),
            }];
            match params_from_signature(method.sig.inputs.iter().skip(1), public_structs) {
                Ok(mut rest) => params.append(&mut rest),
                Err(reason) => {
                    collector
                        .skipped
                        .push(format!("{}::{}: {}", type_name, method.sig.ident, reason));
                    continue;
                }
            }
            let Ok((return_type, can_error, error_name)) =
                return_type_from_signature(&method.sig.output, public_structs)
            else {
                collector.skipped.push(format!(
                    "{}::{}: unsupported return type",
                    type_name, method.sig.ident
                ));
                continue;
            };
            collector.handles.insert(type_name.clone());
            collector.bindings.push(Binding {
                exported_name: format!("{}_{}", to_snake_case(&type_name), method.sig.ident),
                source: BindingSource::Method {
                    crate_ident: collector.crate_ident.clone(),
                    method_name: method.sig.ident.to_string(),
                },
                params,
                return_type,
                can_error,
                error_name,
                pure: false,
            });
        } else if matches_type(&method.sig.output, &type_name) {
            match binding_from_fn(
                &ItemFn {
                    attrs: method.attrs.clone(),
                    vis: method.vis.clone(),
                    sig: method.sig.clone(),
                    block: Box::new(syn::Block {
                        brace_token: Default::default(),
                        stmts: Vec::new(),
                    }),
                },
                BindingSource::Constructor {
                    path: format!(
                        "{}::{}::{}",
                        collector.crate_ident, type_name, method.sig.ident
                    ),
                },
                public_structs,
                false,
            ) {
                Ok(mut binding) => {
                    collector.handles.insert(type_name.clone());
                    binding.exported_name =
                        format!("{}_{}", to_snake_case(&type_name), method.sig.ident);
                    collector.bindings.push(binding);
                }
                Err(reason) => collector
                    .skipped
                    .push(format!("{}::{}: {}", type_name, method.sig.ident, reason)),
            }
        }
    }
}

fn collect_manual_exports(path: &Path, collector: &mut Collector) -> Result<(), KiroError> {
    let source = fs::read_to_string(path).map_err(|e| {
        KiroError::new(
            ErrorCode::BuildGraphFailed,
            ErrorPhase::Cli,
            format!("Failed to read '{}': {}", path.display(), e),
        )
    })?;
    let file = syn::parse_file(&source).map_err(|e| {
        KiroError::new(
            ErrorCode::ParseFailed,
            ErrorPhase::Cli,
            format!("Failed to parse '{}': {}", path.display(), e),
        )
    })?;
    let mut handles = BTreeSet::new();
    for item in &file.items {
        if let Item::Mod(module) = item
            && module.ident == collector.manual_module
            && let Some((_, items)) = &module.content
        {
            collect_manual_items(module, items, &mut handles, collector);
        }
    }
    collector.handles.extend(handles);
    Ok(())
}

fn collect_manual_items(
    _module: &ItemMod,
    items: &[Item],
    handles: &mut BTreeSet<String>,
    collector: &mut Collector,
) {
    for item in items {
        match item {
            Item::Struct(item_struct) if has_attr(&item_struct.attrs, "kiro_handle") => {
                handles.insert(item_struct.ident.to_string());
            }
            Item::Fn(item_fn) if has_attr(&item_fn.attrs, "kiro_export") => {
                let pure = attr_is_pure(&item_fn.attrs);
                if pure && item_fn.sig.asyncness.is_some() {
                    collector.skipped.push(format!(
                        "{}::{}: pure export cannot be async",
                        collector.manual_module, item_fn.sig.ident
                    ));
                    continue;
                }
                match binding_from_fn(
                    item_fn,
                    BindingSource::ManualFunction {
                        module: collector.manual_module.clone(),
                        function: item_fn.sig.ident.to_string(),
                    },
                    handles,
                    pure,
                ) {
                    Ok(binding) => collector.bindings.push(binding),
                    Err(reason) => collector.skipped.push(format!(
                        "{}::{}: {}",
                        collector.manual_module, item_fn.sig.ident, reason
                    )),
                }
            }
            _ => {}
        }
    }
}

fn binding_from_fn(
    item_fn: &ItemFn,
    source: BindingSource,
    public_structs: &BTreeSet<String>,
    pure: bool,
) -> Result<Binding, String> {
    if !item_fn.sig.generics.params.is_empty() || item_fn.sig.generics.lt_token.is_some() {
        return Err("generics are unsupported".to_string());
    }
    if item_fn.sig.variadic.is_some() {
        return Err("variadic functions are unsupported".to_string());
    }
    let params = params_from_signature(item_fn.sig.inputs.iter(), public_structs)?;
    let (return_type, can_error, error_name) =
        return_type_from_signature(&item_fn.sig.output, public_structs)?;
    Ok(Binding {
        exported_name: item_fn.sig.ident.to_string(),
        source,
        params,
        return_type,
        can_error,
        error_name,
        pure,
    })
}

fn params_from_signature<'a>(
    inputs: impl Iterator<Item = &'a FnArg>,
    public_structs: &BTreeSet<String>,
) -> Result<Vec<Param>, String> {
    let mut params = Vec::new();
    for arg in inputs {
        let FnArg::Typed(arg) = arg else {
            return Err("method receivers are only supported on inherent methods".to_string());
        };
        let Pat::Ident(name) = arg.pat.as_ref() else {
            return Err("only named parameters are supported".to_string());
        };
        let rust_type = rust_type_from_syn(&arg.ty, public_structs)?;
        params.push(Param {
            name: name.ident.to_string(),
            rust_type,
        });
    }
    Ok(params)
}

fn return_type_from_signature(
    output: &ReturnType,
    public_structs: &BTreeSet<String>,
) -> Result<(RustType, bool, Option<String>), String> {
    match output {
        ReturnType::Default => Ok((RustType::Void, false, None)),
        ReturnType::Type(_, ty) => {
            if let Some((ok, err)) = result_type(ty, public_structs)? {
                Ok((ok, true, Some(err)))
            } else {
                Ok((rust_type_from_syn(ty, public_structs)?, false, None))
            }
        }
    }
}

fn result_type(
    ty: &Type,
    public_structs: &BTreeSet<String>,
) -> Result<Option<(RustType, String)>, String> {
    let Type::Path(type_path) = ty else {
        return Ok(None);
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Ok(None);
    };
    if segment.ident != "Result" {
        return Ok(None);
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err("Result must use explicit type arguments".to_string());
    };
    let mut args = args.args.iter();
    let Some(GenericArgument::Type(ok_ty)) = args.next() else {
        return Err("Result ok type is unsupported".to_string());
    };
    let Some(GenericArgument::Type(err_ty)) = args.next() else {
        return Err("Result error type is unsupported".to_string());
    };
    let ok = rust_type_from_syn(ok_ty, public_structs)?;
    let err = type_last_ident(err_ty).unwrap_or_else(|| "HostError".to_string());
    Ok(Some((ok, err)))
}

fn rust_type_from_syn(ty: &Type, public_structs: &BTreeSet<String>) -> Result<RustType, String> {
    match ty {
        Type::Reference(reference) => {
            if reference.mutability.is_some() {
                return Err("mutable references are unsupported".to_string());
            }
            match reference.elem.as_ref() {
                Type::Path(path) if path.path.is_ident("str") => Ok(RustType::Str),
                _ => Err("borrowed types are unsupported".to_string()),
            }
        }
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(RustType::Void),
        Type::Path(type_path) => {
            let Some(segment) = type_path.path.segments.last() else {
                return Err("unsupported path type".to_string());
            };
            let name = segment.ident.to_string();
            match name.as_str() {
                "String" => Ok(RustType::Str),
                "f64" | "f32" | "i64" | "i32" | "i16" | "i8" | "isize" | "u64" | "u32" | "u16"
                | "u8" | "usize" => Ok(RustType::Num { rust: name }),
                "bool" => Ok(RustType::Bool),
                "Vec" => {
                    let inner = one_generic_type(segment, "Vec")?;
                    Ok(RustType::List(Box::new(rust_type_from_syn(
                        inner,
                        public_structs,
                    )?)))
                }
                "HashMap" | "BTreeMap" => {
                    let (key, value) = two_generic_types(segment, &name)?;
                    let key_ty = rust_type_from_syn(key, public_structs)?;
                    if key_ty != RustType::Str {
                        return Err("map keys must be String/str".to_string());
                    }
                    Ok(RustType::Map(Box::new(rust_type_from_syn(
                        value,
                        public_structs,
                    )?)))
                }
                _ if public_structs.contains(&name) => Ok(RustType::Handle(name)),
                _ => Err(format!("unsupported type '{}'", name)),
            }
        }
        _ => Err("unsupported type".to_string()),
    }
}

fn one_generic_type<'a>(segment: &'a syn::PathSegment, name: &str) -> Result<&'a Type, String> {
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(format!("{} must use explicit type arguments", name));
    };
    let Some(GenericArgument::Type(ty)) = args.args.first() else {
        return Err(format!("{} type argument is unsupported", name));
    };
    Ok(ty)
}

fn two_generic_types<'a>(
    segment: &'a syn::PathSegment,
    name: &str,
) -> Result<(&'a Type, &'a Type), String> {
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(format!("{} must use explicit type arguments", name));
    };
    let mut iter = args.args.iter();
    let Some(GenericArgument::Type(key)) = iter.next() else {
        return Err(format!("{} key type is unsupported", name));
    };
    let Some(GenericArgument::Type(value)) = iter.next() else {
        return Err(format!("{} value type is unsupported", name));
    };
    Ok((key, value))
}

fn render_kiro_module(collector: &Collector) -> String {
    let mut lines = vec![
        "// Generated by `kiro host gen`; edit the Rust manual section for fallbacks.".to_string(),
    ];
    if !collector.handles.is_empty() {
        lines.push(String::new());
        for handle in &collector.handles {
            lines.push(format!("handle {}", handle));
        }
    }
    if !collector.bindings.is_empty() {
        lines.push(String::new());
        for binding in &collector.bindings {
            let pure = if binding.pure { "pure " } else { "" };
            let params = binding
                .params
                .iter()
                .map(|p| format!("{}: {}", p.name, kiro_type(&p.rust_type)))
                .collect::<Vec<_>>()
                .join(", ");
            let bang = if binding.can_error { "!" } else { "" };
            lines.push(format!(
                "{}rust fn {}({}) -> {}{}",
                pure,
                binding.exported_name,
                params,
                kiro_type(&binding.return_type),
                bang
            ));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

fn render_rust_glue(collector: &Collector) -> String {
    let mut out = String::new();
    out.push_str(GENERATED_BEGIN);
    out.push_str("\n\n");
    for binding in &collector.bindings {
        out.push_str(&render_binding_glue(binding));
        out.push('\n');
    }
    if !collector.skipped.is_empty() {
        out.push_str("// Skipped unsupported Rust APIs:\n");
        for skipped in &collector.skipped {
            out.push_str(&format!("// - {}\n", skipped));
        }
        out.push('\n');
    }
    out.push_str(GENERATED_END);
    out.push('\n');
    out
}

fn render_binding_glue(binding: &Binding) -> String {
    let async_kw = if binding.pure { "" } else { "async " };
    let mut body = String::new();
    body.push_str(&format!(
        "pub {}fn {}(args: Vec<RuntimeVal>) -> HostResult {{\n",
        async_kw, binding.exported_name
    ));
    body.push_str(&format!(
        "    RuntimeVal::expect_arity(&args, {}, \"{}\")?;\n",
        binding.params.len(),
        binding.exported_name
    ));
    for (idx, param) in binding.params.iter().enumerate() {
        body.push_str(&decode_param(binding, idx, param, &binding.exported_name));
    }
    let args = binding
        .params
        .iter()
        .map(|param| param.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let call = match &binding.source {
        BindingSource::CrateFunction { path } => format!("{}({})", path, args),
        BindingSource::ManualFunction { module, function } => {
            format!("{}::{}({})", module, function, args)
        }
        BindingSource::Constructor { path, .. } => format!("{}({})", path, args),
        BindingSource::Method {
            crate_ident: _,
            method_name,
        } => {
            let receiver = &binding.params[0].name;
            let rest = binding
                .params
                .iter()
                .skip(1)
                .map(|param| param.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}.{}({})", receiver, method_name, rest)
        }
    };

    if binding.can_error {
        let err_name = binding.error_name.as_deref().unwrap_or("HostError");
        body.push_str(&format!("    match {} {{\n", call));
        body.push_str(&format!(
            "        Ok(value) => Ok({}),\n",
            encode_value("value", &binding.return_type)
        ));
        body.push_str(&format!(
            "        Err(err) => Err(KiroError::message(\"{}\", err.to_string())),\n",
            err_name
        ));
        body.push_str("    }\n");
    } else {
        match binding.return_type {
            RustType::Void => {
                body.push_str(&format!("    {};\n", call));
                body.push_str("    Ok(RuntimeVal::Void)\n");
            }
            _ => {
                body.push_str(&format!("    let value = {};\n", call));
                body.push_str(&format!(
                    "    Ok({})\n",
                    encode_value("value", &binding.return_type)
                ));
            }
        }
    }
    body.push_str("}\n");
    body
}

fn decode_param(binding: &Binding, idx: usize, param: &Param, fn_name: &str) -> String {
    let arg = format!("RuntimeVal::expect_arg(&args, {}, \"{}\")?", idx, fn_name);
    match &param.rust_type {
        RustType::Str => format!("    let {} = {}.as_str()?.to_string();\n", param.name, arg),
        RustType::Num { rust } if rust == "f64" => {
            format!("    let {} = {}.as_num()?;\n", param.name, arg)
        }
        RustType::Num { rust } if rust == "f32" => {
            format!("    let {} = {}.as_num()? as f32;\n", param.name, arg)
        }
        RustType::Num { rust } => {
            format!("    let {} = {}.as_num()? as {};\n", param.name, arg, rust)
        }
        RustType::Bool => format!("    let {} = {}.as_bool()?;\n", param.name, arg),
        RustType::Handle(type_name) => format!(
            "    let {} = {}.as_handle(\"{}\")?.downcast_ref::<{}>().ok_or_else(|| KiroError::message(\"TypeError\", \"expected handle payload {}\"))?;\n",
            param.name,
            arg,
            type_name,
            handle_payload_type(binding, type_name),
            type_name
        ),
        RustType::List(inner) => decode_list_param(&param.name, &arg, inner),
        RustType::Map(inner) => decode_map_param(&param.name, &arg, inner),
        RustType::Void => format!("    let {} = ();\n", param.name),
    }
}

fn decode_list_param(name: &str, arg: &str, inner: &RustType) -> String {
    let mut out = format!("    let mut {} = Vec::new();\n", name);
    out.push_str(&format!("    for item in {}.as_list()? {{\n", arg));
    out.push_str(&format!(
        "        {}.push({});\n",
        name,
        decode_runtime_expr("item", inner)
    ));
    out.push_str("    }\n");
    out
}

fn decode_map_param(name: &str, arg: &str, inner: &RustType) -> String {
    let mut out = format!("    let mut {} = std::collections::HashMap::new();\n", name);
    out.push_str(&format!("    for (key, value) in {}.as_map()? {{\n", arg));
    out.push_str(&format!(
        "        {}.insert(key.clone(), {});\n",
        name,
        decode_runtime_expr("value", inner)
    ));
    out.push_str("    }\n");
    out
}

fn decode_runtime_expr(value: &str, ty: &RustType) -> String {
    match ty {
        RustType::Str => format!("{}.as_str()?.to_string()", value),
        RustType::Num { rust } if rust == "f64" => format!("{}.as_num()?", value),
        RustType::Num { rust } => format!("{}.as_num()? as {}", value, rust),
        RustType::Bool => format!("{}.as_bool()?", value),
        _ => {
            "return Err(KiroError::message(\"TypeError\", \"unsupported nested type\"))".to_string()
        }
    }
}

fn encode_value(name: &str, ty: &RustType) -> String {
    match ty {
        RustType::Void => "RuntimeVal::Void".to_string(),
        RustType::Map(inner) => format!(
            "RuntimeVal::Map({}.into_iter().map(|(k, v)| (k, {})).collect())",
            name,
            encode_inner_value("v", inner)
        ),
        RustType::Handle(type_name) => format!("RuntimeVal::handle(\"{}\", {})", type_name, name),
        _ => format!("RuntimeVal::from({})", name),
    }
}

fn encode_inner_value(name: &str, ty: &RustType) -> String {
    match ty {
        RustType::Str | RustType::Num { .. } | RustType::Bool | RustType::List(_) => {
            format!("RuntimeVal::from({})", name)
        }
        RustType::Map(inner) => format!(
            "RuntimeVal::Map({}.into_iter().map(|(k, v)| (k, {})).collect())",
            name,
            encode_inner_value("v", inner)
        ),
        RustType::Handle(type_name) => format!("RuntimeVal::handle(\"{}\", {})", type_name, name),
        RustType::Void => "RuntimeVal::Void".to_string(),
    }
}

fn kiro_type(ty: &RustType) -> String {
    match ty {
        RustType::Str => "str".to_string(),
        RustType::Num { .. } => "num".to_string(),
        RustType::Bool => "bool".to_string(),
        RustType::Void => "void".to_string(),
        RustType::List(inner) => format!("list {}", kiro_type(inner)),
        RustType::Map(inner) => format!("map str {}", kiro_type(inner)),
        RustType::Handle(name) => name.clone(),
    }
}

fn initial_rust_module(manual_module: &str) -> String {
    format!(
        r#"mod {manual_module} {{
    use super::*;
    use kiro_macros::{{kiro_export, kiro_handle, kiro_struct}};
    use std::collections::HashMap;
}}

// kiro:generated begin
// kiro:generated end
"#
    )
}

fn replace_generated_region(existing: &str, generated: &str) -> String {
    let Some(start) = existing.find(GENERATED_BEGIN) else {
        let mut out = existing.trim_end().to_string();
        out.push_str("\n\n");
        out.push_str(generated);
        return out;
    };
    let Some(end_rel) = existing[start..].find(GENERATED_END) else {
        let mut out = existing[..start].trim_end().to_string();
        out.push_str("\n\n");
        out.push_str(generated);
        return out;
    };
    let end = start + end_rel + GENERATED_END.len();
    let mut out = String::new();
    out.push_str(existing[..start].trim_end());
    out.push_str("\n\n");
    out.push_str(generated.trim_end());
    out.push('\n');
    out.push_str(existing[end..].trim_start_matches('\n'));
    out
}

fn is_public(vis: &Visibility) -> bool {
    matches!(vis, Visibility::Public(_))
}

fn impl_type_name(ty: &Type) -> Option<String> {
    type_last_ident(ty)
}

fn type_last_ident(ty: &Type) -> Option<String> {
    let Type::Path(type_path) = ty else {
        return None;
    };
    type_path
        .path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn matches_type(output: &ReturnType, name: &str) -> bool {
    let ReturnType::Type(_, ty) = output else {
        return false;
    };
    type_last_ident(ty).as_deref() == Some(name)
}

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn attr_is_pure(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("kiro_export")
            && attr
                .parse_args::<syn::Ident>()
                .is_ok_and(|ident| ident == "pure")
    })
}

fn to_snake_case(name: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if idx > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn handle_payload_type(binding: &Binding, type_name: &str) -> String {
    match &binding.source {
        BindingSource::Method { crate_ident, .. }
        | BindingSource::CrateFunction { path: crate_ident }
        | BindingSource::Constructor {
            path: crate_ident, ..
        } => {
            let crate_name = crate_ident.split("::").next().unwrap_or(crate_ident);
            format!("{}::{}", crate_name, type_name)
        }
        BindingSource::ManualFunction { module, .. } => format!("{}::{}", module, type_name),
    }
}

fn manual_module_name(module_name: &str) -> String {
    format!("__kiro_manual_{}", module_name)
}
