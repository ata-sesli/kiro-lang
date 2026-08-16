use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::MetadataCommand;
use syn::{
    Attribute, FnArg, GenericArgument, ImplItem, Item, ItemFn, ItemImpl, ItemMod, Pat,
    PathArguments, ReturnType, Type, TypeParamBound, UseTree, Visibility,
};

use crate::errors::{ErrorCode, ErrorPhase, KiroError};
use crate::project::{self, KiroProject};

mod render;

use render::{initial_rust_module, render_kiro_module, render_rust_glue, replace_generated_region};

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
        receiver_mutable: bool,
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

#[derive(Debug, Clone)]
struct TypeContext {
    public_structs: BTreeSet<String>,
    records: BTreeSet<String>,
    result_aliases: BTreeMap<String, String>,
    std_path_names: BTreeSet<String>,
    self_type: Option<String>,
}

impl TypeContext {
    fn new(
        public_structs: &BTreeSet<String>,
        records: &BTreeSet<String>,
        result_aliases: &BTreeMap<String, String>,
        items: &[Item],
    ) -> Self {
        Self {
            public_structs: public_structs.clone(),
            records: records.clone(),
            result_aliases: result_aliases.clone(),
            std_path_names: std_path_names(items),
            self_type: None,
        }
    }

    fn manual(handles: &BTreeSet<String>) -> Self {
        Self {
            public_structs: handles.clone(),
            records: BTreeSet::new(),
            result_aliases: BTreeMap::new(),
            std_path_names: BTreeSet::new(),
            self_type: None,
        }
    }

    fn with_self_type(&self, self_type: String) -> Self {
        let mut next = self.clone();
        next.self_type = Some(self_type);
        next
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RustType {
    Str,
    Bytes { borrowed: bool },
    Num { rust: String },
    Bool,
    Void,
    List(Box<RustType>),
    Map(Box<RustType>),
    Record(String),
    Handle(String),
}

#[derive(Debug, Clone)]
struct RecordField {
    name: String,
    rust_type: RustType,
}

#[derive(Debug, Clone)]
struct RecordDef {
    fields: Vec<RecordField>,
}

#[derive(Debug, Default)]
struct Collector {
    bindings: Vec<Binding>,
    handles: BTreeSet<String>,
    mutable_handles: BTreeSet<String>,
    records: BTreeMap<String, RecordDef>,
    skipped: Vec<String>,
    crate_ident: String,
    manual_module: String,
}

impl Collector {
    fn push_binding(&mut self, binding: Binding) {
        if self
            .bindings
            .iter()
            .any(|existing| existing.exported_name == binding.exported_name)
        {
            return;
        }
        self.bindings.push(binding);
    }
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
        declarations: collector.bindings.len() + collector.handles.len() + collector.records.len(),
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
    let file = parse_rust_file(root)?;
    let base_dir = root.parent().unwrap_or_else(|| Path::new("."));
    let reexports = collect_root_reexports(&file, base_dir, collector)?;

    let mut public_structs = public_struct_names(&file.items, None);
    for reexport in &reexports {
        public_structs.extend(public_struct_names(
            &reexport.file.items,
            Some(&reexport.names),
        ));
    }
    let mut result_alias_map = result_aliases(&file.items, None);
    for reexport in &reexports {
        result_alias_map.extend(result_aliases(&reexport.file.items, Some(&reexport.names)));
    }

    let no_records = BTreeSet::new();
    let root_probe = TypeContext::new(&public_structs, &no_records, &result_alias_map, &file.items);
    let mut records = public_record_defs(&file.items, None, &root_probe);
    for reexport in &reexports {
        let probe = TypeContext::new(
            &public_structs,
            &no_records,
            &result_alias_map,
            &reexport.file.items,
        );
        records.extend(public_record_defs(
            &reexport.file.items,
            Some(&reexport.names),
            &probe,
        ));
    }
    let record_names = records.keys().cloned().collect();
    collector.records = records;

    let root_context = TypeContext::new(
        &public_structs,
        &record_names,
        &result_alias_map,
        &file.items,
    );
    collect_items(&file.items, None, &root_context, collector);
    for reexport in &reexports {
        let context = TypeContext::new(
            &public_structs,
            &record_names,
            &result_alias_map,
            &reexport.file.items,
        );
        collect_items(
            &reexport.file.items,
            Some(&reexport.names),
            &context,
            collector,
        );
    }
    Ok(())
}

struct ReexportModule {
    names: BTreeSet<String>,
    file: syn::File,
}

struct ReexportEntry {
    module_path: Vec<String>,
    name: String,
}

fn parse_rust_file(path: &Path) -> Result<syn::File, KiroError> {
    let source = fs::read_to_string(path).map_err(|e| {
        KiroError::new(
            ErrorCode::FileNotFound,
            ErrorPhase::Cli,
            format!("Failed to read '{}': {}", path.display(), e),
        )
    })?;
    syn::parse_file(&source).map_err(|e| {
        KiroError::new(
            ErrorCode::ParseFailed,
            ErrorPhase::Cli,
            format!("Failed to parse Rust source '{}': {}", path.display(), e),
        )
    })
}

fn public_struct_names(
    items: &[Item],
    allowed_names: Option<&BTreeSet<String>>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for item in items {
        if let Item::Struct(item_struct) = item
            && is_public(&item_struct.vis)
            && is_allowed_name(allowed_names, &item_struct.ident.to_string())
        {
            names.insert(item_struct.ident.to_string());
        }
    }
    names
}

fn public_record_defs(
    items: &[Item],
    allowed_names: Option<&BTreeSet<String>>,
    context: &TypeContext,
) -> BTreeMap<String, RecordDef> {
    let mut records = BTreeMap::new();
    for item in items {
        let Item::Struct(item_struct) = item else {
            continue;
        };
        let name = item_struct.ident.to_string();
        if !is_public(&item_struct.vis) || !is_allowed_name(allowed_names, &name) {
            continue;
        }
        if has_public_inherent_methods(items, &name) {
            continue;
        }
        let syn::Fields::Named(fields) = &item_struct.fields else {
            continue;
        };
        let mut record_fields = Vec::new();
        let mut supported = !fields.named.is_empty();
        for field in &fields.named {
            let Some(ident) = &field.ident else {
                supported = false;
                break;
            };
            let field_name = ident.to_string();
            if !is_public(&field.vis) || !is_kiro_identifier(&field_name) {
                supported = false;
                break;
            }
            let Ok(rust_type) = rust_type_from_syn(&field.ty, context) else {
                supported = false;
                break;
            };
            if contains_opaque_type(&rust_type) {
                supported = false;
                break;
            }
            record_fields.push(RecordField {
                name: field_name,
                rust_type,
            });
        }
        if supported {
            records.insert(
                name,
                RecordDef {
                    fields: record_fields,
                },
            );
        }
    }
    records
}

fn has_public_inherent_methods(items: &[Item], type_name: &str) -> bool {
    items.iter().any(|item| {
        let Item::Impl(item_impl) = item else {
            return false;
        };
        item_impl.trait_.is_none()
            && impl_type_name(&item_impl.self_ty).as_deref() == Some(type_name)
            && item_impl
                .items
                .iter()
                .any(|item| matches!(item, ImplItem::Fn(method) if is_public(&method.vis)))
    })
}

fn contains_opaque_type(ty: &RustType) -> bool {
    match ty {
        RustType::Handle(_)
        | RustType::Record(_)
        | RustType::List(_)
        | RustType::Map(_)
        | RustType::Void => true,
        RustType::Str | RustType::Bytes { .. } | RustType::Num { .. } | RustType::Bool => false,
    }
}

fn is_kiro_identifier(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte == b'_' || byte.is_ascii_lowercase())
}

fn result_aliases(
    items: &[Item],
    allowed_names: Option<&BTreeSet<String>>,
) -> BTreeMap<String, String> {
    let mut aliases = BTreeMap::new();
    for item in items {
        let Item::Type(item_type) = item else {
            continue;
        };
        let alias_name = item_type.ident.to_string();
        if !is_public(&item_type.vis) || !is_allowed_name(allowed_names, &alias_name) {
            continue;
        }
        if item_type.generics.params.len() != 1 {
            continue;
        }
        let Some(error_name) = result_alias_error_name(&item_type.ty, &item_type.generics) else {
            continue;
        };
        aliases.insert(alias_name, error_name);
    }
    aliases
}

fn result_alias_error_name(ty: &Type, generics: &syn::Generics) -> Option<String> {
    let generic_name = generics.params.iter().find_map(|param| match param {
        syn::GenericParam::Type(param) => Some(param.ident.to_string()),
        _ => None,
    })?;
    let Type::Path(type_path) = ty else {
        return None;
    };
    let segment = type_path.path.segments.last()?;
    if segment.ident != "Result" {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let mut args = args.args.iter();
    let Some(GenericArgument::Type(ok_ty)) = args.next() else {
        return None;
    };
    if type_last_ident(ok_ty).as_deref() != Some(generic_name.as_str()) {
        return None;
    }
    let Some(GenericArgument::Type(err_ty)) = args.next() else {
        return None;
    };
    if args.next().is_some() {
        return None;
    }
    type_last_ident(err_ty)
}

fn std_path_names(items: &[Item]) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for item in items {
        let Item::Use(item_use) = item else {
            continue;
        };
        collect_std_path_names(&item_use.tree, Vec::new(), &mut names);
    }
    names
}

fn collect_std_path_names(tree: &UseTree, prefix: Vec<String>, names: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix;
            next.push(path.ident.to_string());
            collect_std_path_names(&path.tree, next, names);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_std_path_names(item, prefix.clone(), names);
            }
        }
        UseTree::Name(name) => {
            if prefix == ["std", "path"] && name.ident == "Path" {
                names.insert("Path".to_string());
            }
        }
        UseTree::Rename(rename) => {
            if prefix == ["std", "path"] && rename.ident == "Path" {
                names.insert(rename.rename.to_string());
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn collect_items(
    items: &[Item],
    allowed_names: Option<&BTreeSet<String>>,
    context: &TypeContext,
    collector: &mut Collector,
) {
    for item in items {
        match item {
            Item::Fn(item_fn)
                if is_public(&item_fn.vis)
                    && is_allowed_name(allowed_names, &item_fn.sig.ident.to_string()) =>
            {
                match binding_from_fn(
                    item_fn,
                    BindingSource::CrateFunction {
                        path: format!("{}::{}", collector.crate_ident, item_fn.sig.ident),
                    },
                    context,
                    false,
                ) {
                    Ok(binding) => collector.push_binding(binding),
                    Err(reason) => collector
                        .skipped
                        .push(format!("{}: {}", item_fn.sig.ident, reason)),
                }
            }
            Item::Impl(item_impl) => collect_impl(item_impl, allowed_names, context, collector),
            _ => {}
        }
    }
}

fn collect_root_reexports(
    file: &syn::File,
    base_dir: &Path,
    collector: &mut Collector,
) -> Result<Vec<ReexportModule>, KiroError> {
    let mut module_exports: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for item in &file.items {
        let Item::Use(item_use) = item else {
            continue;
        };
        if !is_public(&item_use.vis) {
            continue;
        }
        let mut entries = Vec::new();
        collect_reexport_entries(
            &item_use.tree,
            Vec::new(),
            &mut entries,
            &mut collector.skipped,
        );
        for entry in entries {
            let Some(path) = resolve_local_module_file(base_dir, &entry.module_path) else {
                collector.skipped.push(format!(
                    "pub use {}::{}: local module file was not found",
                    entry.module_path.join("::"),
                    entry.name
                ));
                continue;
            };
            module_exports.entry(path).or_default().insert(entry.name);
        }
    }

    let mut modules = Vec::new();
    for (path, names) in module_exports {
        let file = parse_rust_file(&path)?;
        modules.push(ReexportModule { names, file });
    }
    Ok(modules)
}

fn collect_reexport_entries(
    tree: &UseTree,
    prefix: Vec<String>,
    entries: &mut Vec<ReexportEntry>,
    skipped: &mut Vec<String>,
) {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix;
            next.push(path.ident.to_string());
            collect_reexport_entries(&path.tree, next, entries, skipped);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_reexport_entries(item, prefix.clone(), entries, skipped);
            }
        }
        UseTree::Name(name) => {
            if prefix.is_empty() {
                skipped.push(format!(
                    "pub use {}: root-name re-exports are unsupported",
                    name.ident
                ));
            } else {
                entries.push(ReexportEntry {
                    module_path: prefix,
                    name: name.ident.to_string(),
                });
            }
        }
        UseTree::Rename(rename) => {
            skipped.push(format!(
                "pub use {}::{} as {}: alias re-exports are unsupported",
                prefix.join("::"),
                rename.ident,
                rename.rename
            ));
        }
        UseTree::Glob(_) => {
            skipped.push(format!(
                "pub use {}::*: glob re-exports are unsupported",
                prefix.join("::")
            ));
        }
    }
}

fn resolve_local_module_file(base_dir: &Path, module_path: &[String]) -> Option<PathBuf> {
    if module_path.is_empty() {
        return None;
    }
    let mut path = base_dir.to_path_buf();
    for segment in module_path {
        path.push(segment);
    }
    let rs_path = path.with_extension("rs");
    if rs_path.exists() {
        return Some(rs_path);
    }
    let mod_path = path.join("mod.rs");
    if mod_path.exists() {
        return Some(mod_path);
    }
    None
}

fn is_allowed_name(allowed_names: Option<&BTreeSet<String>>, name: &str) -> bool {
    allowed_names.is_none_or(|names| names.contains(name))
}

fn collect_impl(
    item_impl: &ItemImpl,
    allowed_names: Option<&BTreeSet<String>>,
    context: &TypeContext,
    collector: &mut Collector,
) {
    if item_impl.trait_.is_some() {
        return;
    }
    let Some(type_name) = impl_type_name(&item_impl.self_ty) else {
        return;
    };
    if !context.public_structs.contains(&type_name) {
        return;
    }
    if context.records.contains(&type_name) {
        return;
    }
    if !is_allowed_name(allowed_names, &type_name) {
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
            if receiver.reference.is_none() {
                collector.skipped.push(format!(
                    "{}::{}: by-value receivers are unsupported",
                    type_name, method.sig.ident
                ));
                continue;
            }
            let receiver_mutable = receiver.mutability.is_some();
            let method_context = context.with_self_type(type_name.clone());
            let mut params = vec![Param {
                name: to_snake_case(&type_name),
                rust_type: RustType::Handle(type_name.clone()),
            }];
            match params_from_signature(method.sig.inputs.iter().skip(1), &method_context) {
                Ok(mut rest) => params.append(&mut rest),
                Err(reason) => {
                    collector
                        .skipped
                        .push(format!("{}::{}: {}", type_name, method.sig.ident, reason));
                    continue;
                }
            }
            let Ok((return_type, can_error, error_name)) =
                return_type_from_signature(&method.sig.output, &method_context)
            else {
                collector.skipped.push(format!(
                    "{}::{}: unsupported return type",
                    type_name, method.sig.ident
                ));
                continue;
            };
            collector.handles.insert(type_name.clone());
            if receiver_mutable {
                collector.mutable_handles.insert(type_name.clone());
            }
            collector.push_binding(Binding {
                exported_name: format!("{}_{}", to_snake_case(&type_name), method.sig.ident),
                source: BindingSource::Method {
                    crate_ident: collector.crate_ident.clone(),
                    method_name: method.sig.ident.to_string(),
                    receiver_mutable,
                },
                params,
                return_type,
                can_error,
                error_name,
                pure: false,
            });
        } else {
            let method_context = context.with_self_type(type_name.clone());
            let Ok((resolved_return_type, _, _)) =
                return_type_from_signature(&method.sig.output, &method_context)
            else {
                continue;
            };
            if resolved_return_type != RustType::Handle(type_name.clone()) {
                continue;
            }
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
                &method_context,
                false,
            ) {
                Ok(mut binding) => {
                    collector.handles.insert(type_name.clone());
                    binding.exported_name =
                        format!("{}_{}", to_snake_case(&type_name), method.sig.ident);
                    collector.push_binding(binding);
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
                    &TypeContext::manual(handles),
                    pure,
                ) {
                    Ok(binding) => collector.push_binding(binding),
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
    context: &TypeContext,
    pure: bool,
) -> Result<Binding, String> {
    if !item_fn.sig.generics.params.is_empty() || item_fn.sig.generics.lt_token.is_some() {
        return Err("generics are unsupported".to_string());
    }
    if item_fn.sig.variadic.is_some() {
        return Err("variadic functions are unsupported".to_string());
    }
    let params = params_from_signature(item_fn.sig.inputs.iter(), context)?;
    let (return_type, can_error, error_name) =
        return_type_from_signature(&item_fn.sig.output, context)?;
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
    context: &TypeContext,
) -> Result<Vec<Param>, String> {
    let mut params = Vec::new();
    for arg in inputs {
        let FnArg::Typed(arg) = arg else {
            return Err("method receivers are only supported on inherent methods".to_string());
        };
        let Pat::Ident(name) = arg.pat.as_ref() else {
            return Err("only named parameters are supported".to_string());
        };
        let rust_type = rust_type_from_syn(&arg.ty, context)?;
        params.push(Param {
            name: name.ident.to_string(),
            rust_type,
        });
    }
    Ok(params)
}

fn return_type_from_signature(
    output: &ReturnType,
    context: &TypeContext,
) -> Result<(RustType, bool, Option<String>), String> {
    match output {
        ReturnType::Default => Ok((RustType::Void, false, None)),
        ReturnType::Type(_, ty) => {
            if let Some((ok, err)) = result_type(ty, context)? {
                Ok((ok, true, Some(err)))
            } else {
                Ok((rust_type_from_syn(ty, context)?, false, None))
            }
        }
    }
}

fn result_type(ty: &Type, context: &TypeContext) -> Result<Option<(RustType, String)>, String> {
    let Type::Path(type_path) = ty else {
        return Ok(None);
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Ok(None);
    };
    let alias_name = segment.ident.to_string();
    let alias_error = context
        .result_aliases
        .get(&alias_name)
        .filter(|_| path_can_refer_to_result_alias(&type_path.path, &alias_name));
    if alias_name != "Result" && alias_error.is_none() {
        return Ok(None);
    }
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err("Result must use explicit type arguments".to_string());
    };
    let mut args = args.args.iter();
    let Some(GenericArgument::Type(ok_ty)) = args.next() else {
        return Err("Result ok type is unsupported".to_string());
    };
    let ok = rust_type_from_syn(ok_ty, context)?;
    let Some(next_arg) = args.next() else {
        let Some(err) = alias_error else {
            return Err(format!(
                "{} with one type argument requires a public crate-local alias",
                alias_name
            ));
        };
        return Ok(Some((ok, err.clone())));
    };
    let GenericArgument::Type(err_ty) = next_arg else {
        return Err("Result error type is unsupported".to_string());
    };
    let err = type_last_ident(err_ty).unwrap_or_else(|| "HostError".to_string());
    Ok(Some((ok, err)))
}

fn path_can_refer_to_result_alias(path: &syn::Path, alias_name: &str) -> bool {
    let segments = path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    matches!(
        segments.as_slice(),
        [single] if single == alias_name
    ) || matches!(
        segments.as_slice(),
        [prefix, last] if matches!(prefix.as_str(), "crate" | "self" | "super") && last == alias_name
    )
}

fn rust_type_from_syn(ty: &Type, context: &TypeContext) -> Result<RustType, String> {
    match ty {
        Type::Reference(reference) => {
            if reference.mutability.is_some() {
                return Err("mutable references are unsupported".to_string());
            }
            match reference.elem.as_ref() {
                Type::Path(path) if path.path.is_ident("str") => Ok(RustType::Str),
                Type::Slice(slice)
                    if matches!(
                        slice.elem.as_ref(),
                        Type::Path(path) if path.path.is_ident("u8")
                    ) =>
                {
                    Ok(RustType::Bytes { borrowed: true })
                }
                _ => Err("borrowed types are unsupported".to_string()),
            }
        }
        Type::ImplTrait(impl_trait) => impl_trait_type(impl_trait, context),
        Type::Tuple(tuple) if tuple.elems.is_empty() => Ok(RustType::Void),
        Type::Path(type_path) => {
            let Some(segment) = type_path.path.segments.last() else {
                return Err("unsupported path type".to_string());
            };
            let name = segment.ident.to_string();
            match name.as_str() {
                "Self" => context
                    .self_type
                    .as_ref()
                    .map(|name| RustType::Handle(name.clone()))
                    .ok_or_else(|| "Self is only supported inside inherent impls".to_string()),
                "String" => Ok(RustType::Str),
                "f64" | "f32" | "i64" | "i32" | "i16" | "i8" | "isize" | "u64" | "u32" | "u16"
                | "u8" | "usize" => Ok(RustType::Num { rust: name }),
                "bool" => Ok(RustType::Bool),
                "Vec" => {
                    let inner = one_generic_type(segment, "Vec")?;
                    if matches!(inner, Type::Path(path) if path.path.is_ident("u8")) {
                        return Ok(RustType::Bytes { borrowed: false });
                    }
                    Ok(RustType::List(Box::new(rust_type_from_syn(
                        inner, context,
                    )?)))
                }
                "HashMap" | "BTreeMap" => {
                    let (key, value) = two_generic_types(segment, &name)?;
                    let key_ty = rust_type_from_syn(key, context)?;
                    if key_ty != RustType::Str {
                        return Err("map keys must be String/str".to_string());
                    }
                    Ok(RustType::Map(Box::new(rust_type_from_syn(value, context)?)))
                }
                _ if context.records.contains(&name) => Ok(RustType::Record(name)),
                _ if context.public_structs.contains(&name) => Ok(RustType::Handle(name)),
                _ => Err(format!("unsupported type '{}'", name)),
            }
        }
        _ => Err("unsupported type".to_string()),
    }
}

fn impl_trait_type(ty: &syn::TypeImplTrait, context: &TypeContext) -> Result<RustType, String> {
    let mut bounds = ty.bounds.iter();
    let Some(TypeParamBound::Trait(bound)) = bounds.next() else {
        return Err("impl Trait is unsupported".to_string());
    };
    if bounds.next().is_some() {
        return Err("impl Trait with multiple bounds is unsupported".to_string());
    }
    if trait_bound_is_as_ref_path(bound, context) {
        Ok(RustType::Str)
    } else {
        Err("impl Trait is unsupported except impl AsRef<std::path::Path>".to_string())
    }
}

fn trait_bound_is_as_ref_path(bound: &syn::TraitBound, context: &TypeContext) -> bool {
    let Some(segment) = bound.path.segments.last() else {
        return false;
    };
    if segment.ident != "AsRef" {
        return false;
    }
    let Ok(inner) = one_generic_type(segment, "AsRef") else {
        return false;
    };
    is_std_path_type(inner, context)
}

fn is_std_path_type(ty: &Type, context: &TypeContext) -> bool {
    let Type::Path(type_path) = ty else {
        return false;
    };
    let segments = type_path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    segments == ["std", "path", "Path"]
        || segments == ["core", "path", "Path"]
        || (segments.len() == 1 && context.std_path_names.contains(&segments[0]))
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
