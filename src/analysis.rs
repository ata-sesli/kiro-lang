use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::compiler::{Compiler, FunctionInfo};
use crate::errors::{ErrorCode, ErrorPhase, KiroError, SourceSpan};
use crate::grammar::{self, grammar as ast};
use crate::hir::{HirModule, HirProgram, SourceId};
use crate::{
    StdAssets, is_reserved_std_module_name, removed_print_statement, std_asset_path,
    unsupported_let_statement,
};

#[path = "analysis/hir_lower.rs"]
mod hir_lower;
#[path = "compiler/diagnostics.rs"]
mod semantic;

use hir_lower::{HirModuleInput, lower_modules};

pub type SourceOverlays = HashMap<PathBuf, String>;

pub(crate) fn validate_program_semantics(
    compiler: &mut Compiler,
    program: &ast::Program,
    module: &str,
    source: &str,
) -> Result<(), KiroError> {
    semantic::validate_semantics(compiler, program, module, source)
}

pub struct AnalyzedModule {
    pub name: String,
    pub path: PathBuf,
    pub source: String,
    pub program: ast::Program,
    pub hir: HirModule,
}

pub struct AnalysisResult {
    pub root: PathBuf,
    pub modules: HashMap<String, AnalyzedModule>,
    pub module_functions: HashMap<(String, String), FunctionInfo>,
    pub hir: HirProgram,
}

pub fn analyze_path(path: impl AsRef<Path>, overlays: &SourceOverlays) -> Result<(), KiroError> {
    analyze_path_with_info(path, overlays).map(|_| ())
}

pub fn analyze_path_with_info(
    path: impl AsRef<Path>,
    overlays: &SourceOverlays,
) -> Result<AnalysisResult, KiroError> {
    let root = normalize_path(path.as_ref());
    if !root.exists() && !overlays.contains_key(&root) {
        return Err(KiroError::file_not_found(&root.display().to_string()));
    }

    let base_dir = root
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let name = root
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| KiroError::file_not_found(&root.display().to_string()))?
        .to_string();

    let mut ctx = AnalysisCtx {
        root_dir: base_dir.clone(),
        overlays: normalize_overlays(overlays),
        seen: HashSet::new(),
        modules: HashMap::new(),
        module_functions: HashMap::new(),
        missing_glue: Vec::new(),
    };

    ctx.collect_recursive(&name, &base_dir, Some(root.clone()))?;

    let known_modules: HashSet<String> = ctx.modules.keys().cloned().collect();
    for module in ctx.modules.values() {
        let mut compiler = Compiler::with_module_functions(ctx.module_functions.clone());
        compiler.current_module = module.name.clone();
        compiler.known_modules = known_modules.clone();
        compiler.validate_semantics(&module.program, &module.file_name(), &module.source)?;
    }

    let mut module_names = ctx.modules.keys().cloned().collect::<Vec<_>>();
    module_names.sort();
    let hir_inputs = module_names
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let module = &ctx.modules[name];
            Ok(HirModuleInput {
                name: &module.name,
                program: &module.program,
                source: SourceId::try_from(index).map_err(|error| {
                    KiroError::new(
                        ErrorCode::CompilerPanic,
                        ErrorPhase::Compile,
                        error.to_string(),
                    )
                })?,
            })
        })
        .collect::<Result<Vec<_>, KiroError>>()?;
    let hir = lower_modules(&hir_inputs).map_err(|message| {
        KiroError::new(ErrorCode::CompilerPanic, ErrorPhase::Compile, message)
    })?;
    let mut hir_modules = hir
        .modules
        .iter()
        .cloned()
        .map(|module| (module.name.clone(), module))
        .collect::<HashMap<_, _>>();
    for (name, module) in &mut ctx.modules {
        module.hir = hir_modules
            .remove(name)
            .expect("HIR lowering must produce every analyzed module");
    }
    if let Some(missing) = ctx.missing_glue.first() {
        return Err(missing_host_glue_error(
            &missing.module,
            &missing.source,
            &missing.function,
            missing.span,
            &missing.module_path,
            &missing.glue_path,
        ));
    }

    Ok(AnalysisResult {
        root,
        modules: ctx.modules,
        module_functions: ctx.module_functions,
        hir,
    })
}

struct AnalysisCtx {
    root_dir: PathBuf,
    overlays: SourceOverlays,
    seen: HashSet<String>,
    modules: HashMap<String, AnalyzedModule>,
    module_functions: HashMap<(String, String), FunctionInfo>,
    missing_glue: Vec<MissingGlueInfo>,
}

struct MissingGlueInfo {
    module: String,
    source: String,
    function: String,
    span: grammar::AstSpan,
    module_path: PathBuf,
    glue_path: PathBuf,
}

impl AnalysisCtx {
    fn collect_recursive(
        &mut self,
        name: &str,
        base_dir: &Path,
        explicit_path: Option<PathBuf>,
    ) -> Result<(), KiroError> {
        let (path, source) = self.load_module(name, base_dir, explicit_path)?;
        let module_name = self.canonical_module_name(name, &path);
        if self.seen.contains(&module_name) {
            return Ok(());
        }
        self.seen.insert(module_name.clone());

        if let Some(found) = unsupported_let_statement(&source) {
            return Err(KiroError::unsupported_keyword_with_source(
                &file_name_for(&path),
                &source,
                found.line,
                found.column,
                "let",
            ));
        }
        if let Some(removed) = removed_print_statement(&source) {
            return Err(KiroError::removed_print_statement(
                &file_name_for(&path),
                &source,
                removed.line,
                removed.column,
            ));
        }
        let program = grammar::parse(&source)
            .map_err(|e| KiroError::parse_failed_with_source(&file_name_for(&path), &source, &e))?;

        for (fn_name, info) in Compiler::collect_program_functions(&program) {
            self.module_functions
                .insert((module_name.clone(), fn_name), info);
        }

        let rust_decls = rust_decl_infos(&program);
        if !is_reserved_std_module_name(&module_name) && !rust_decls.is_empty() {
            let glue_path = path.with_extension("rs");
            if !glue_path.exists() {
                let mut missing = rust_decls;
                missing.sort_by(|a, b| a.name.cmp(&b.name));
                self.missing_glue.push(MissingGlueInfo {
                    module: module_name.clone(),
                    source: source.clone(),
                    function: missing[0].name.clone(),
                    span: missing[0].span,
                    module_path: path.clone(),
                    glue_path,
                });
            }
        }

        for import in imports(&program) {
            let import_dir = if is_reserved_std_module_name(&import.name) {
                PathBuf::from(".")
            } else {
                path.parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_else(|| base_dir.to_path_buf())
            };
            if let Err(err) = self.collect_recursive(&import.name, &import_dir, None) {
                if matches!(err.code, ErrorCode::FileNotFound) {
                    return Err(KiroError::new(
                        ErrorCode::ImportError,
                        ErrorPhase::Compile,
                        format!("Module '{}' not found.", import.name),
                    )
                    .with_byte_span(
                        file_name_for(&path),
                        &source,
                        SourceSpan::new(import.span.0, import.span.1),
                        "missing module",
                    )
                    .with_help(format!(
                        "add '{}' beside '{}'",
                        grammar::module_path_file_path(&import.name).display(),
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("this file")
                    )));
                }
                return Err(err);
            }
        }

        self.modules.insert(
            module_name.clone(),
            AnalyzedModule {
                name: module_name.clone(),
                path,
                source,
                program,
                hir: HirModule::empty(module_name),
            },
        );

        Ok(())
    }

    fn load_module(
        &self,
        name: &str,
        base_dir: &Path,
        explicit_path: Option<PathBuf>,
    ) -> Result<(PathBuf, String), KiroError> {
        if explicit_path.is_none()
            && let Some(asset_path) = std_asset_path(
                name,
                &format!(
                    "{}.kiro",
                    crate::canonical_std_module_name(name).unwrap_or(name)
                ),
            )
        {
            let source = StdAssets::get(&asset_path)
                .map(|f| std::str::from_utf8(f.data.as_ref()).unwrap().to_string())
                .ok_or_else(|| KiroError::file_not_found(&format!("{}.kiro", name)))?;
            let canonical = crate::canonical_std_module_name(name).unwrap_or(name);
            return Ok((PathBuf::from(format!("{}.kiro", canonical)), source));
        }
        if explicit_path.is_none() && name.starts_with("std_") {
            return Err(KiroError::file_not_found(&format!("{}.kiro", name)));
        }

        let path =
            explicit_path.unwrap_or_else(|| base_dir.join(grammar::module_path_file_path(name)));
        let normalized = normalize_path(&path);
        if let Some(source) = self.overlays.get(&normalized) {
            return Ok((normalized, source.clone()));
        }
        let source = std::fs::read_to_string(&normalized)
            .map_err(|_| KiroError::file_not_found(&normalized.display().to_string()))?;
        Ok((normalized, source))
    }

    fn canonical_module_name(&self, requested_name: &str, path: &Path) -> String {
        if is_reserved_std_module_name(requested_name) {
            return requested_name.to_string();
        }

        let relative = path.strip_prefix(&self.root_dir).unwrap_or(path);
        let without_extension = relative.with_extension("");
        let segments = without_extension
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(part) => part.to_str().map(str::to_string),
                _ => None,
            })
            .collect::<Vec<_>>();

        if segments.is_empty() {
            requested_name.to_string()
        } else {
            segments.join(".")
        }
    }
}

impl AnalyzedModule {
    pub fn file_name(&self) -> String {
        file_name_for(&self.path)
    }
}

#[derive(Debug, Clone)]
struct ImportInfo {
    name: String,
    span: grammar::AstSpan,
}

fn imports(program: &ast::Program) -> Vec<ImportInfo> {
    program
        .statements
        .iter()
        .filter_map(|stmt| match stmt {
            ast::Statement::Import { module_name, .. } => Some(ImportInfo {
                name: grammar::module_path_name(module_name).to_string(),
                span: grammar::module_path_span(module_name),
            }),
            _ => None,
        })
        .collect()
}

#[derive(Debug, Clone)]
struct RustDeclInfo {
    name: String,
    span: grammar::AstSpan,
}

fn rust_decl_infos(program: &ast::Program) -> Vec<RustDeclInfo> {
    let mut decls = Vec::new();
    for stmt in &program.statements {
        match stmt {
            ast::Statement::RustFnDecl(def) => {
                decls.push(RustDeclInfo {
                    name: grammar::function_name(&def.name).to_string(),
                    span: grammar::rust_fn_decl_span(def),
                });
            }
            ast::Statement::Documented { item, .. } => {
                if let ast::AnnotatableItem::RustFnDecl(def) = item {
                    decls.push(RustDeclInfo {
                        name: grammar::function_name(&def.name).to_string(),
                        span: grammar::rust_fn_decl_span(def),
                    });
                }
            }
            _ => {}
        }
    }
    decls
}

fn normalize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

fn normalize_overlays(overlays: &SourceOverlays) -> SourceOverlays {
    overlays
        .iter()
        .map(|(path, source)| (normalize_path(path), source.clone()))
        .collect()
}

fn file_name_for(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| path.display().to_string())
}

fn missing_host_glue_error(
    module: &str,
    source: &str,
    fn_name: &str,
    span: grammar::AstSpan,
    module_path: &Path,
    glue_path: &Path,
) -> KiroError {
    KiroError::new(
        ErrorCode::MissingHostGlue,
        ErrorPhase::Compile,
        format!(
            "Missing Rust glue for host function '{}.{}'.",
            module, fn_name
        ),
    )
    .with_file(file_name_for(module_path))
    .with_help(format!(
        "add '{}' with `pub async fn {}(args: Vec<RuntimeVal>) -> HostResult`",
        glue_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("module.rs"),
        fn_name
    ))
    .with_byte_span(
        file_name_for(module_path),
        source,
        SourceSpan::new(span.0, span.1),
        "missing glue",
    )
}
