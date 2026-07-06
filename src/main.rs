use kiro_lang::analysis::{self, SourceOverlays};
use kiro_lang::build_manager::{BuildManager, BuildRequirements, GENERATED_BUILD_DIR};
use kiro_lang::compiler;
use kiro_lang::errors::{self, KiroError, emit_error, panic_payload_to_string};
use kiro_lang::formatter;
use kiro_lang::grammar;
use kiro_lang::host_generator::{self, HostGenOptions};
use kiro_lang::interpreter::SessionRuntime;
use kiro_lang::ir::IrModule;
use kiro_lang::project;
use kiro_lang::test_runner;
use kiro_lang::{
    StdAssets, canonical_std_module_name, is_reserved_std_module_name, removed_print_statement,
    std_asset_path, unsupported_let_statement,
};

use std::collections::HashMap;
use std::fs;

use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::{DocumentMut, Item, Table, value};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Default file argument (if no subcommand is used)
    file: Option<String>,

    /// Skip execution after build
    #[arg(long)]
    no_run: bool,

    /// Output generated Rust code to stdout
    #[arg(long)]
    emit_rust: bool,

    /// Show compiler output
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Analyze, compile, and execute through Rust
    Run {
        file: Option<String>,
        #[arg(long)]
        no_run: bool,
        #[arg(long)]
        emit_rust: bool,
        #[arg(short, long)]
        verbose: bool,
    },
    /// Static validation only (no Rust build or execution)
    Check { file: Option<String> },
    /// Analyze and execute directly with the Kiro interpreter
    Interpret { file: Option<String> },
    /// Run the Kiro language server over stdio
    Lsp,
    /// Analyze, transpile, and build ONLY (No Execution)
    Build {
        file: Option<String>,
        #[arg(long)]
        emit_rust: bool,
        #[arg(short, long)]
        verbose: bool,
    },
    /// Format Kiro source files
    Fmt {
        /// Files or directories to format. Defaults to the current project.
        paths: Vec<String>,
        /// Check formatting without writing changes.
        #[arg(long)]
        check: bool,
    },
    /// Run Kiro test files
    Test {
        /// Files or directories to test. Defaults to *_test.kiro under the current project.
        paths: Vec<String>,
    },
    /// Create a new Kiro project
    Create { project_name: String },
    /// Add a dependency
    Add { dependency: String },
    /// Remove a dependency
    Remove { dependency: String },
    /// Generate Rust-backed Kiro host modules
    Host {
        #[command(subcommand)]
        command: HostCommands,
    },
}

#[derive(Subcommand, Debug)]
enum HostCommands {
    /// Generate a host module from a Cargo dependency
    Gen {
        crate_name: String,
        #[arg(long)]
        module: Option<String>,
    },
}

fn scaffold_project(project_name: &str) {
    let path = PathBuf::from(project_name);
    if path.exists() {
        eprintln!("Error: Directory '{}' already exists.", project_name);
        std::process::exit(1);
    }

    if let Err(e) = fs::create_dir(&path) {
        eprintln!("Error creating directory: {}", e);
        std::process::exit(1);
    }

    let toml_content = format!(
        r#"[package]
name = "{}"
entry = "main.kiro"

[dependencies]
"#,
        project_name
    );

    let main_kiro_content = format!(
        r#"import io

io.print("Hello from {}!")"#,
        project_name
    );

    if let Err(e) = fs::write(path.join("kiro.toml"), toml_content) {
        eprintln!("Error creating kiro.toml: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = fs::write(path.join("main.kiro"), main_kiro_content) {
        eprintln!("Error creating main.kiro: {}", e);
        std::process::exit(1);
    }

    // Reserve generated state space. Kiro creates .kiro/build during build/run.
    let dot_kiro_path = path.join(".kiro");
    if let Err(e) = fs::create_dir(&dot_kiro_path) {
        eprintln!("Error creating .kiro directory: {}", e);
        std::process::exit(1);
    }

    println!("✨ Created new Kiro project: {}", project_name);
}

fn handle_add(dep: &str) {
    let Some((project, dep_name, requested_version)) = add_context(dep) else {
        std::process::exit(1);
    };
    let dep_version = match requested_version {
        Some(version) => version,
        None => match host_generator::resolve_crate(&project, &dep_name, "*") {
            Ok(resolved) => resolved.version,
            Err(e) => {
                emit_error(&e);
                std::process::exit(1);
            }
        },
    };

    let content = fs::read_to_string(&project.manifest_path).unwrap();
    let mut doc = content
        .parse::<DocumentMut>()
        .expect("Invalid kiro.toml format");

    if !doc.as_table().contains_key("dependencies") {
        doc["dependencies"] = Item::Table(Table::new());
    }

    doc["dependencies"][&dep_name] = value(dep_version.as_str());
    fs::write(&project.manifest_path, doc.to_string()).unwrap();
    println!(
        "➕ Added Cargo dependency '{}' = \"{}\" to kiro.toml",
        dep_name, dep_version
    );

    try_generate_added_host_module(&project, &dep_name);
}

fn handle_remove(dep: &str) {
    let Some((project, dep_name)) = remove_context(dep) else {
        std::process::exit(1);
    };

    let content = fs::read_to_string(&project.manifest_path).unwrap();
    let mut doc = content
        .parse::<DocumentMut>()
        .expect("Invalid kiro.toml format");

    if let Some(deps) = doc
        .get_mut("dependencies")
        .and_then(|d| d.as_table_like_mut())
    {
        if deps.remove(&dep_name).is_some() {
            println!("➖ Removed '{}' from kiro.toml", dep_name);
        } else {
            eprintln!("Warning: Dependency '{}' not found in kiro.toml", dep_name);
        }
    }

    fs::write(&project.manifest_path, doc.to_string()).unwrap();
}

fn add_context(dep: &str) -> Option<(project::KiroProject, String, Option<String>)> {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("Error reading current directory: {}", e);
            return None;
        }
    };
    let project = match project::find_project(cwd) {
        Ok(Some(project)) => project,
        Ok(None) => {
            eprintln!("Error: kiro.toml not found. Are you in a Kiro project?");
            return None;
        }
        Err(e) => {
            emit_error(&e);
            return None;
        }
    };

    let (name, version) = parse_dependency_spec(dep)?;
    validate_dependency_name_and_version(&name, version.as_deref())?;

    Some((project, name, version))
}

fn remove_context(dep: &str) -> Option<(project::KiroProject, String)> {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            eprintln!("Error reading current directory: {}", e);
            return None;
        }
    };
    let project = match project::find_project(cwd) {
        Ok(Some(project)) => project,
        Ok(None) => {
            eprintln!("Error: kiro.toml not found. Are you in a Kiro project?");
            return None;
        }
        Err(e) => {
            emit_error(&e);
            return None;
        }
    };
    let name = dep.trim().to_string();
    validate_dependency_name_and_version(&name, None)?;
    Some((project, name))
}

fn validate_dependency_name_and_version(name: &str, version: Option<&str>) -> Option<()> {
    if !project::is_valid_cargo_dependency_name(&name) {
        eprintln!("Error: Invalid dependency name '{}'.", name);
        return None;
    }
    if version.is_some_and(|version| version.trim().is_empty()) {
        eprintln!("Error: Dependency version must not be empty.");
        return None;
    }
    if is_reserved_std_module_name(&name) {
        eprintln!(
            "Error: Dependency '{}' conflicts with a reserved Kiro std module name.",
            name
        );
        return None;
    }

    Some(())
}

fn parse_dependency_spec(spec: &str) -> Option<(String, Option<String>)> {
    if let Some((name, version)) = spec.split_once('@') {
        let name = name.trim();
        let version = version.trim();
        if name.is_empty() || version.is_empty() {
            eprintln!("Error: use `kiro add crate@version` or `kiro add crate`.");
            return None;
        }
        Some((name.to_string(), Some(version.to_string())))
    } else {
        Some((spec.trim().to_string(), None))
    }
}

fn try_generate_added_host_module(project: &project::KiroProject, dep_name: &str) {
    let Ok(Some(updated_project)) = project::find_project(&project.manifest_path) else {
        eprintln!("Warning: Added dependency, but could not reload kiro.toml for host generation.");
        return;
    };
    match host_generator::generate(
        &updated_project,
        HostGenOptions {
            crate_name: dep_name.to_string(),
            module_name: None,
        },
    ) {
        Ok(result) => {
            println!(
                "🔗 Generated host module '{}' ({} declarations)",
                result.module_name, result.declarations
            );
            for skipped in result.skipped {
                println!("  - skipped {}", skipped);
            }
        }
        Err(e) => {
            eprintln!(
                "Warning: Added dependency, but no Kiro host module was generated automatically."
            );
            emit_error(&e);
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Run {
            file,
            no_run,
            emit_rust,
            verbose,
        }) => {
            let Some(file) = resolve_input_file(file.as_deref()) else {
                std::process::exit(1);
            };
            if !execute_compiled_pipeline(&file, !*no_run, *emit_rust, *verbose) {
                std::process::exit(1);
            }
        }
        Some(Commands::Check { file }) => {
            let Some(file) = resolve_input_file(file.as_deref()) else {
                std::process::exit(1);
            };
            if !run_static_check(&file) {
                std::process::exit(1);
            }
        }
        Some(Commands::Interpret { file }) => {
            let Some(file) = resolve_input_file(file.as_deref()) else {
                std::process::exit(1);
            };
            if !run_interpret_pipeline(&file) {
                std::process::exit(1);
            }
        }
        Some(Commands::Lsp) => {
            #[cfg(feature = "lsp")]
            if let Err(e) = kiro_lang::lsp::run() {
                emit_error(&e);
                std::process::exit(1);
            }
            #[cfg(not(feature = "lsp"))]
            {
                emit_error(
                    &KiroError::new(
                        errors::ErrorCode::BuildGraphFailed,
                        errors::ErrorPhase::Cli,
                        "The 'lsp' subcommand is unavailable because this binary was built without the 'lsp' feature.",
                    )
                    .with_help("rebuild with `--features lsp` or use the default feature set"),
                );
                std::process::exit(1);
            }
        }
        Some(Commands::Build {
            file,
            emit_rust,
            verbose,
        }) => {
            let Some(file) = resolve_input_file(file.as_deref()) else {
                std::process::exit(1);
            };
            let Some(analysis) = analyze_with_info_or_emit(&file) else {
                std::process::exit(1);
            };
            match run_compiler(analysis, *emit_rust, *verbose) {
                Ok(_) => {}
                Err(e) => {
                    emit_error(&e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Fmt { paths, check }) => {
            if !handle_fmt(paths, *check) {
                std::process::exit(1);
            }
        }
        Some(Commands::Test { paths }) => match test_runner::run(paths) {
            Ok(summary) => {
                if !summary.is_success() {
                    std::process::exit(1);
                }
            }
            Err(e) => {
                emit_error(&e);
                std::process::exit(1);
            }
        },
        Some(Commands::Create { project_name }) => {
            scaffold_project(project_name);
        }
        Some(Commands::Add { dependency }) => {
            handle_add(dependency);
        }
        Some(Commands::Remove { dependency }) => {
            handle_remove(dependency);
        }
        Some(Commands::Host { command }) => match command {
            HostCommands::Gen { crate_name, module } => {
                if !handle_host_gen(crate_name, module.clone()) {
                    std::process::exit(1);
                }
            }
        },
        None => {
            if let Some(file) = resolve_input_file(cli.file.as_deref()) {
                // Default behavior: Analyze -> Compile -> Run
                if !execute_compiled_pipeline(&file, !cli.no_run, cli.emit_rust, cli.verbose) {
                    std::process::exit(1);
                }
            } else {
                std::process::exit(1);
            }
        }
    }
}

fn handle_host_gen(crate_name: &str, module: Option<String>) -> bool {
    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            emit_error(&KiroError::new(
                errors::ErrorCode::FileNotFound,
                errors::ErrorPhase::Cli,
                format!("Failed to read current directory: {}", e),
            ));
            return false;
        }
    };
    let project = match project::find_project(cwd) {
        Ok(Some(project)) => project,
        Ok(None) => {
            emit_error(&project::no_input_error());
            return false;
        }
        Err(e) => {
            emit_error(&e);
            return false;
        }
    };
    match host_generator::generate(
        &project,
        HostGenOptions {
            crate_name: crate_name.to_string(),
            module_name: module,
        },
    ) {
        Ok(result) => {
            println!(
                "Generated host module '{}' with {} declaration(s).",
                result.module_name, result.declarations
            );
            println!("  - {}", result.kiro_path.display());
            println!("  - {}", result.rust_path.display());
            for skipped in result.skipped {
                println!("  - skipped {}", skipped);
            }
            true
        }
        Err(e) => {
            emit_error(&e);
            false
        }
    }
}

fn resolve_input_file(explicit: Option<&str>) -> Option<String> {
    if let Some(file) = explicit {
        return Some(file.to_string());
    }

    let cwd = match std::env::current_dir() {
        Ok(cwd) => cwd,
        Err(e) => {
            emit_error(&KiroError::new(
                errors::ErrorCode::FileNotFound,
                errors::ErrorPhase::Cli,
                format!("Failed to read current directory: {}", e),
            ));
            return None;
        }
    };

    match project::find_project(cwd) {
        Ok(Some(project)) => {
            let entry = project.entry_path();
            if !entry.exists() {
                emit_error(
                    &KiroError::file_not_found(&entry.display().to_string())
                        .with_help("check [package].entry in kiro.toml"),
                );
                return None;
            }
            if let Err(e) = std::env::set_current_dir(&project.root) {
                emit_error(&KiroError::new(
                    errors::ErrorCode::FileNotFound,
                    errors::ErrorPhase::Cli,
                    format!(
                        "Failed to enter project root '{}': {}",
                        project.root.display(),
                        e
                    ),
                ));
                return None;
            }
            Some(entry.display().to_string())
        }
        Ok(None) => {
            let _ = <Cli as clap::CommandFactory>::command().print_help();
            println!();
            emit_error(&project::no_input_error());
            None
        }
        Err(e) => {
            emit_error(&e);
            None
        }
    }
}

fn handle_fmt(paths: &[String], check: bool) -> bool {
    let path_bufs: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let files = match formatter::collect_kiro_files(&path_bufs) {
        Ok(files) => files,
        Err(e) => {
            emit_error(&e);
            return false;
        }
    };

    let mut changed = Vec::new();
    for file in files {
        let source = match fs::read_to_string(&file) {
            Ok(source) => source,
            Err(e) => {
                emit_error(
                    &KiroError::new(
                        errors::ErrorCode::FileNotFound,
                        errors::ErrorPhase::Cli,
                        format!("Read error: {}", e),
                    )
                    .with_file(file.display().to_string()),
                );
                return false;
            }
        };
        let display = file.display().to_string();
        let formatted = match formatter::format_source_for_file(&source, &display) {
            Ok(formatted) => formatted,
            Err(e) => {
                emit_error(&e);
                return false;
            }
        };

        if formatted != source {
            if check {
                println!("Would format {}", display);
                changed.push(display);
            } else if let Err(e) = fs::write(&file, formatted) {
                emit_error(
                    &KiroError::new(
                        errors::ErrorCode::BuildGraphFailed,
                        errors::ErrorPhase::Cli,
                        format!("Failed to write '{}': {}", display, e),
                    )
                    .with_file(display),
                );
                return false;
            } else {
                println!("Formatted {}", display);
            }
        }
    }

    if check && !changed.is_empty() {
        eprintln!("{} file(s) need formatting.", changed.len());
        return false;
    }

    true
}

fn run_static_check(filename: &str) -> bool {
    if analyze_with_info_or_emit(filename).is_some() {
        println!("OK {}", filename);
        true
    } else {
        false
    }
}

fn analyze_with_info_or_emit(filename: &str) -> Option<analysis::AnalysisResult> {
    let overlays = SourceOverlays::new();
    match analysis::analyze_path_with_info(filename, &overlays) {
        Ok(result) => Some(result),
        Err(e) => {
            emit_error(&e);
            None
        }
    }
}

fn run_interpret_pipeline(file: &str) -> bool {
    if analyze_with_info_or_emit(file).is_none() {
        return false;
    }
    run_interpreter(file)
}

// Returns true if success
fn execute_compiled_pipeline(file: &str, do_run: bool, emit_rust: bool, verbose: bool) -> bool {
    println!("🚀 Kiro Build System v0.2");

    let Some(analysis) = analyze_with_info_or_emit(file) else {
        return false;
    };

    if verbose {
        println!("🔨 --- COMPILING ---");
    } else {
        println!("🔨 --- COMPILING --- (Output hidden, use --verbose to show)");
    }

    match run_compiler(analysis, emit_rust, verbose) {
        Ok(exe_path) => {
            if do_run {
                println!("🚀 --- RUNNING ---");
                if let Err(e) = execute_binary(exe_path) {
                    eprintln!("Execution Error: {}", e);
                    return false;
                }
            }
        }
        Err(e) => {
            emit_error(&e);
            return false;
        }
    }

    true
}

fn run_interpreter(filename: &str) -> bool {
    if !std::path::Path::new(filename).exists() {
        emit_error(&KiroError::file_not_found(filename));
        return false;
    }

    let source = match fs::read_to_string(filename) {
        Ok(s) => s,
        Err(e) => {
            emit_error(&KiroError::new(
                errors::ErrorCode::FileNotFound,
                errors::ErrorPhase::Cli,
                format!("Read error: {}", e),
            ));
            return false;
        }
    };
    if let Some(found) = unsupported_let_statement(&source) {
        emit_error(&KiroError::unsupported_keyword_with_source(
            filename,
            &source,
            found.line,
            found.column,
            "let",
        ));
        return false;
    }
    if let Some(removed) = removed_print_statement(&source) {
        emit_error(&KiroError::removed_print_statement(
            filename,
            &source,
            removed.line,
            removed.column,
        ));
        return false;
    }

    let prog = match grammar::parse(&source) {
        Ok(p) => p,
        Err(e) => {
            emit_error(&KiroError::parse_failed_with_source(filename, &source, &e));
            return false;
        }
    };

    let base_dir = std::path::Path::new(filename)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let module_name = std::path::Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("main")
        .to_string();
    let module = IrModule::lower(module_name.clone(), prog);
    let mut runtime = SessionRuntime::new(module, base_dir);
    runtime.set_current_module(module_name);
    if let Err(e) = runtime.run() {
        let err = if let Some(site) = runtime.take_last_error_site() {
            let message = if let Some(message) = e.strip_prefix("Check failed: ") {
                format!("Check failed: {}", message)
            } else {
                e.clone()
            };
            let mut err = KiroError::new(site.code, errors::ErrorPhase::Runtime, message)
                .with_byte_span(
                    filename,
                    &source,
                    errors::SourceSpan::new(site.span.0, site.span.1),
                    site.label,
                );
            if let Some(help) = site.help {
                err = err.with_help(help);
            }
            err
        } else if let Some(message) = e.strip_prefix("Check failed: ") {
            KiroError::runtime_check_failed(message)
        } else {
            KiroError::new(
                errors::ErrorCode::ParseFailed,
                errors::ErrorPhase::Runtime,
                format!("Interpreter error: {}", e),
            )
        };
        emit_error(&err);
        return false;
    }
    true
}

fn run_compiler(
    analysis: analysis::AnalysisResult,
    _emit_rust: bool,
    verbose: bool,
) -> Result<PathBuf, KiroError> {
    let pm = BuildManager::new(GENERATED_BUILD_DIR);
    if let Err(e) = pm.init() {
        return Err(KiroError::new(
            errors::ErrorCode::BuildGraphFailed,
            errors::ErrorPhase::Compile,
            format!("Init error: {}", e),
        ));
    }

    let mut requirements = build_requirements(&analysis.modules);
    let project_dependencies = project::find_project(&analysis.root)?
        .map(|project| project.dependencies)
        .unwrap_or_default();
    let header = build_header_content(&analysis.modules, &requirements);
    requirements.record_host_macros(
        header.contains("kiro_export")
            || header.contains("kiro_handle")
            || header.contains("kiro_struct"),
    );
    let root_name = module_name_from_path(&analysis.root)?;
    let module_functions = analysis.module_functions.clone();
    let mut modules = analysis.modules;

    let mut module_names: Vec<String> = modules
        .keys()
        .filter(|name| *name != &root_name)
        .cloned()
        .collect();
    module_names.sort();

    for name in module_names {
        if requirements.skips_module_import(&name) {
            continue;
        }
        if let Some(module) = modules.remove(&name) {
            compile_analyzed_module(module, false, &module_functions, &requirements, &pm)?;
        }
    }

    let root_module = modules.remove(&root_name).ok_or_else(|| {
        KiroError::new(
            errors::ErrorCode::BuildGraphFailed,
            errors::ErrorPhase::Compile,
            format!("Analyzed root module '{}' was not found.", root_name),
        )
    })?;
    compile_analyzed_module(root_module, true, &module_functions, &requirements, &pm)?;

    if let Err(e) = pm.save_header(&header) {
        return Err(KiroError::new(
            errors::ErrorCode::BuildGraphFailed,
            errors::ErrorPhase::Compile,
            format!("Failed to save header.rs: {}", e),
        ));
    }

    pm.write_cargo_toml(&requirements, &project_dependencies)?;

    match pm.build(verbose) {
        Ok(output_path) => Ok(output_path),
        Err(e) => Err(KiroError::new(
            errors::ErrorCode::BuildGraphFailed,
            errors::ErrorPhase::Compile,
            format!("Build error: {}", e),
        )),
    }
}

fn execute_binary(path: PathBuf) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("Binary not found at {:?}", path));
    }

    let status = Command::new(&path)
        .status()
        .map_err(|e| format!("Failed to execute binary: {}", e))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("Process exited with status: {}", status))
    }
}

fn build_requirements(modules: &HashMap<String, analysis::AnalyzedModule>) -> BuildRequirements {
    let mut requirements = BuildRequirements::default();
    let std_io_module_needed = modules
        .values()
        .any(|module| compiler::program_uses_std_io_module(&module.program));
    for module in modules.values() {
        let module_name = canonical_std_module_name(&module.name).unwrap_or(&module.name);
        if module_name == "std_io" && !std_io_module_needed {
            requirements.skip_module_import(module.name.clone());
            continue;
        }
        requirements.record_module(module_name);
        requirements.record_pipes(compiler::program_uses_pipes(&module.program));
        requirements.record_anyhow(compiler::program_uses_anyhow(&module.program));
    }
    requirements
}

fn build_header_content(
    modules: &HashMap<String, analysis::AnalyzedModule>,
    requirements: &BuildRequirements,
) -> String {
    let mut header = BuildManager::header_preamble().to_string();
    let mut module_names: Vec<String> = modules.keys().cloned().collect();
    module_names.sort();
    let mut embedded_headers = std::collections::HashSet::new();

    for name in module_names {
        let Some(module) = modules.get(&name) else {
            continue;
        };
        if requirements.skips_module_import(&module.name) {
            continue;
        }
        if let Some(canonical) = canonical_std_module_name(&name) {
            if !embedded_headers.insert(canonical.to_string()) {
                continue;
            }
            let Some(header_path) = std_asset_path(canonical, "header.rs") else {
                continue;
            };
            if let Some(file) = StdAssets::get(&header_path) {
                let header_content = std::str::from_utf8(file.data.as_ref()).unwrap();
                let content = header_content
                    .lines()
                    .filter(|line| {
                        !line.trim().starts_with("use crate::")
                            && !line.trim().starts_with("use kiro_runtime")
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                header.push_str(&content);
            }
        } else {
            let rs_path = module.path.with_extension("rs");
            if rs_path.exists() {
                println!("  - Found Glue: {}", rs_path.display());
                match fs::read_to_string(&rs_path) {
                    Ok(content) => header.push_str(&sanitize_header_glue(&content)),
                    Err(e) => eprintln!("Failed to read glue file {}: {}", rs_path.display(), e),
                }
            }
        }
    }

    header
}

fn sanitize_header_glue(content: &str) -> String {
    content
        .lines()
        .filter_map(sanitize_header_glue_line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn sanitize_header_glue_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if matches!(
        trimmed,
        "use kiro_runtime::HostResult;"
            | "use kiro_runtime::KiroError;"
            | "use kiro_runtime::RuntimeVal;"
    ) {
        return None;
    }

    let Some(imports) = trimmed
        .strip_prefix("use kiro_runtime::{")
        .and_then(|rest| rest.strip_suffix("};"))
    else {
        return Some(line.to_string());
    };

    let keep = imports
        .split(',')
        .map(str::trim)
        .filter(|name| !matches!(*name, "HostResult" | "KiroError" | "RuntimeVal"))
        .collect::<Vec<_>>();
    if keep.is_empty() {
        None
    } else {
        Some(format!("use kiro_runtime::{{{}}};", keep.join(", ")))
    }
}

fn module_name_from_path(path: &Path) -> Result<String, KiroError> {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(str::to_string)
        .ok_or_else(|| KiroError::file_not_found(&path.display().to_string()))
}

fn compile_analyzed_module(
    module: analysis::AnalyzedModule,
    is_root: bool,
    module_functions: &HashMap<(String, String), compiler::FunctionInfo>,
    requirements: &BuildRequirements,
    pm: &BuildManager,
) -> Result<(), KiroError> {
    let module_name = module.name;
    let mut c = compiler::Compiler::with_options(
        module_functions.clone(),
        compiler::CompilerOptions {
            uses_pipes: requirements.uses_pipes,
            skipped_module_imports: requirements.skipped_module_imports.clone(),
        },
    );
    let code = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        c.compile(module.program, is_root)
    })) {
        Ok(code) => code,
        Err(payload) => {
            return Err(KiroError::compiler_panic(
                &module_name,
                &panic_payload_to_string(payload),
            ));
        }
    };

    let save_name = if is_root { "main" } else { &module_name };
    if let Err(e) = pm.save_file(save_name, code) {
        return Err(KiroError::new(
            errors::ErrorCode::BuildGraphFailed,
            errors::ErrorPhase::Compile,
            format!("Failed to save {}: {}", save_name, e),
        ));
    } else {
        println!("  - Compiled {}", module_name);
    }
    Ok(())
}
