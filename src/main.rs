use kiro_lang::build_manager::BuildManager;
use kiro_lang::compiler;
use kiro_lang::errors::{self, KiroError, emit_error, panic_payload_to_string};
use kiro_lang::formatter;
use kiro_lang::grammar;
use kiro_lang::interpreter;
use kiro_lang::{StdAssets, unsupported_let_line};

use std::fs;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;
use toml_edit::{DocumentMut, Item, Table, value};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Default file argument (if no subcommand is used)
    file: Option<String>,

    /// Skip interpreter step
    #[arg(long)]
    no_interpret: bool,

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
    /// Parse, Compile, and Execute (Default)
    Run {
        file: String,
        #[arg(long)]
        no_interpret: bool,
        #[arg(long)]
        no_run: bool,
        #[arg(long)]
        emit_rust: bool,
        #[arg(short, long)]
        verbose: bool,
    },
    /// Interpret ONLY (No Compilation, No Host Modules)
    Check { file: String },
    /// Transpile and Build ONLY (No Execution)
    Build {
        file: String,
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
    /// Create a new Kiro project
    Create { project_name: String },
    /// Add a dependency
    Add { dependency: String },
    /// Remove a dependency
    Remove { dependency: String },
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

    let main_kiro_content = format!(r#"print "Hello from {}!""#, project_name);

    if let Err(e) = fs::write(path.join("kiro.toml"), toml_content) {
        eprintln!("Error creating kiro.toml: {}", e);
        std::process::exit(1);
    }

    if let Err(e) = fs::write(path.join("main.kiro"), main_kiro_content) {
        eprintln!("Error creating main.kiro: {}", e);
        std::process::exit(1);
    }

    // Initialize Cargo Project in .kiro/
    let dot_kiro_path = path.join(".kiro");
    if let Err(e) = fs::create_dir(&dot_kiro_path) {
        eprintln!("Error creating .kiro directory: {}", e);
        std::process::exit(1);
    }

    // Run cargo init --bin
    println!("Initializing Cargo project in .kiro/ ...");
    let status = Command::new("cargo")
        .args(["init", "--bin", "--name", project_name, "--edition", "2021"])
        .current_dir(&dot_kiro_path)
        .status();

    match status {
        Ok(s) => {
            if !s.success() {
                eprintln!("Warning: 'cargo init' failed. You may need to initialize it manually.");
            }
        }
        Err(e) => {
            eprintln!(
                "Warning: Failed to run 'cargo init': {}. Is cargo installed?",
                e
            );
        }
    }

    println!("✨ Created new Kiro project: {}", project_name);
}

fn handle_add(dep: &str) {
    let kiro_toml_path = "kiro.toml";
    if !std::path::Path::new(kiro_toml_path).exists() {
        eprintln!("Error: kiro.toml not found. Are you in a Kiro project?");
        std::process::exit(1);
    }

    // 1. Check for Reserved Prefix (std_)
    let embedded_path = if dep.starts_with("std_") {
        let key = dep.trim_start_matches("std_");
        format!("{}/header.rs", key)
    } else {
        format!("{}/header.rs", dep)
    };

    if dep.starts_with("std_") {
        if StdAssets::get(&embedded_path).is_none() {
            eprintln!(
                "Error: Module '{}' starts with reserved prefix 'std_' but is not part of the Kiro Standard Library.",
                dep
            );
            std::process::exit(1);
        }
    }

    // 2. Read and Parse kiro.toml
    let content = fs::read_to_string(kiro_toml_path).unwrap();
    let mut doc = content
        .parse::<DocumentMut>()
        .expect("Invalid kiro.toml format");

    // 3. Determine Dependency Type
    let is_embedded = StdAssets::get(&embedded_path).is_some();

    // 4. Update kiro.toml
    if !doc.as_table().contains_key("dependencies") {
        doc["dependencies"] = Item::Table(Table::new());
    }

    if is_embedded {
        doc["dependencies"][dep] = value("*");
        println!("➕ Added embedded dependency '{}' to kiro.toml", dep);
    } else {
        // External or Manual
        // If it starts with kiro_ but not embedded, maybe manual?
        // For now, we treat standard cargo crates as default unless manual specified (user can edit later)
        // But for this command, we assume external crate if not embedded.
        doc["dependencies"][dep] = value("*");
        println!("➕ Added external dependency '{}' to kiro.toml", dep);

        // 5. Run cargo add (only for external)
        let dot_kiro = std::path::Path::new(".kiro");
        if dot_kiro.exists() {
            println!("📦 Running 'cargo add {}' in .kiro/...", dep);
            let status = Command::new("cargo")
                .args(["add", dep])
                .current_dir(dot_kiro)
                .status();

            if let Ok(s) = status {
                if !s.success() {
                    eprintln!("Warning: 'cargo add' failed.");
                }
            }
        }
    }

    fs::write(kiro_toml_path, doc.to_string()).unwrap();
}

fn handle_remove(dep: &str) {
    let kiro_toml_path = "kiro.toml";
    if !std::path::Path::new(kiro_toml_path).exists() {
        eprintln!("Error: kiro.toml not found. Are you in a Kiro project?");
        std::process::exit(1);
    }

    // 1. Remove from kiro.toml
    let content = fs::read_to_string(kiro_toml_path).unwrap();
    let mut doc = content
        .parse::<DocumentMut>()
        .expect("Invalid kiro.toml format");

    if let Some(deps) = doc
        .get_mut("dependencies")
        .and_then(|d| d.as_table_like_mut())
    {
        if deps.remove(dep).is_some() {
            println!("➖ Removed '{}' from kiro.toml", dep);
        } else {
            eprintln!("Warning: Dependency '{}' not found in kiro.toml", dep);
        }
    }

    fs::write(kiro_toml_path, doc.to_string()).unwrap();

    // 2. Run cargo remove (if applicable, though we assume we just try it)
    let dot_kiro = std::path::Path::new(".kiro");
    if dot_kiro.exists() {
        // We only really need to remove if it was an external crate, but cargo remove is safe to run even if not present usually?
        // Or we can check if it exists in Cargo.toml.
        // Simple approach: try cargo remove, ignore failure if not found.
        println!("📦 Running 'cargo remove {}' in .kiro/...", dep);
        let _ = Command::new("cargo")
            .args(["remove", dep])
            .current_dir(dot_kiro)
            .status();
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Run {
            file,
            no_interpret,
            no_run,
            emit_rust,
            verbose,
        }) => {
            if !execute_pipeline(&file, !*no_interpret, !*no_run, *emit_rust, *verbose) {
                std::process::exit(1);
            }
        }
        Some(Commands::Check { file }) => {
            if !run_interpreter(&file) {
                std::process::exit(1);
            }
        }
        Some(Commands::Build {
            file,
            emit_rust,
            verbose,
        }) => match run_compiler(&file, *emit_rust, *verbose) {
            Ok(_) => {}
            Err(e) => {
                emit_error(&e);
                std::process::exit(1);
            }
        },
        Some(Commands::Fmt { paths, check }) => {
            if !handle_fmt(paths, *check) {
                std::process::exit(1);
            }
        }
        Some(Commands::Create { project_name }) => {
            scaffold_project(project_name);
        }
        Some(Commands::Add { dependency }) => {
            handle_add(dependency);
        }
        Some(Commands::Remove { dependency }) => {
            handle_remove(dependency);
        }
        None => {
            if let Some(file) = &cli.file {
                // Default behavior: Interpret -> Compile -> Run
                if !execute_pipeline(
                    file,
                    !cli.no_interpret,
                    !cli.no_run,
                    cli.emit_rust,
                    cli.verbose,
                ) {
                    std::process::exit(1);
                }
            } else {
                <Cli as clap::CommandFactory>::command()
                    .print_help()
                    .unwrap();
            }
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
                emit_error(&KiroError::new(
                    errors::ErrorCode::FileNotFound,
                    errors::ErrorPhase::Cli,
                    format!("Read error: {}", e),
                )
                .with_file(file.display().to_string()));
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
                emit_error(&KiroError::new(
                    errors::ErrorCode::BuildGraphFailed,
                    errors::ErrorPhase::Cli,
                    format!("Failed to write '{}': {}", display, e),
                )
                .with_file(display));
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

// Returns true if success
fn execute_pipeline(
    file: &str,
    do_interpret: bool,
    do_run: bool,
    emit_rust: bool,
    verbose: bool,
) -> bool {
    println!("🚀 Kiro Build System v0.2");

    if do_interpret {
        println!("🤖 --- INTERPRETER ---");
        if !run_interpreter(file) {
            return false;
        }
    }

    if verbose {
        println!("🔨 --- COMPILING ---");
    } else {
        println!("🔨 --- COMPILING --- (Output hidden, use --verbose to show)");
    }

    match run_compiler(file, emit_rust, verbose) {
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
    if let Some(line) = unsupported_let_line(&source) {
        emit_error(&KiroError::unsupported_keyword(filename, line, "let"));
        return false;
    }

    let prog = match grammar::parse(&source) {
        Ok(p) => p,
        Err(e) => {
            emit_error(&KiroError::parse_failed(filename, &format!("{:?}", e)));
            return false;
        }
    };

    let base_dir = std::path::Path::new(filename)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut i = interpreter::Interpreter::with_base_dir(base_dir);
    if let Err(e) = i.run(prog) {
        let err = if let Some(message) = e.strip_prefix("Check failed: ") {
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

fn run_compiler(filename: &str, _emit_rust: bool, verbose: bool) -> Result<PathBuf, KiroError> {
    if !std::path::Path::new(filename).exists() {
        return Err(KiroError::file_not_found(filename));
    }

    let pm = BuildManager::new("kiro_build_cache");
    if let Err(e) = pm.init() {
        return Err(KiroError::new(
            errors::ErrorCode::BuildGraphFailed,
            errors::ErrorPhase::Compile,
            format!("Init error: {}", e),
        ));
    }

    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut module_functions: std::collections::HashMap<(String, String), compiler::FunctionInfo> =
        std::collections::HashMap::new();
    let path = std::path::Path::new(filename);
    let name = path.file_stem().unwrap().to_str().unwrap();
    let dir = path.parent().map(|p| p.to_str().unwrap()).unwrap_or("");
    build_recursive(name, dir, &mut seen, &mut module_functions, &pm, true)?;

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

fn build_recursive(
    name: &str,
    base_dir: &str,
    seen: &mut std::collections::HashSet<String>,
    module_functions: &mut std::collections::HashMap<(String, String), compiler::FunctionInfo>,
    pm: &BuildManager,
    is_root: bool,
) -> Result<(), KiroError> {
    if seen.contains(name) {
        return Ok(());
    }
    seen.insert(name.to_string());

    // Try to resolve module path:
    // 1. If starts with "std_", look in embedded assets
    // 2. Otherwise, look in base_dir or current directory as {name}.kiro
    let src = if name.starts_with("std_") {
        let module_name = &name[4..]; // Remove "std_" prefix
        // Map std_fs -> fs/std_fs.kiro
        let asset_path = format!("{}/{}.kiro", module_name, name);
        StdAssets::get(&asset_path)
            .map(|f| std::str::from_utf8(f.data.as_ref()).unwrap().to_string())
            .expect(&format!(
                "Standard library module '{}' not found in embedded assets",
                name
            ))
    } else {
        let filename = if !base_dir.is_empty() {
            format!("{}/{}.kiro", base_dir, name)
        } else {
            format!("{}.kiro", name)
        };

        match fs::read_to_string(&filename) {
            Ok(s) => s,
            Err(_) => {
                return Err(KiroError::file_not_found(&filename));
            }
        }
    };

    if let Some(line) = unsupported_let_line(&src) {
        return Err(KiroError::unsupported_keyword(name, line, "let"));
    }

    let prog = match grammar::parse(&src) {
        Ok(p) => p,
        Err(e) => {
            return Err(KiroError::parse_failed(name, &format!("{:?}", e)));
        }
    };

    for (fn_name, info) in compiler::Compiler::collect_program_functions(&prog) {
        module_functions.insert((name.to_string(), fn_name), info);
    }

    // Collect rust fn declarations in this module so we can generate fallbacks
    // when no glue file is present.
    let mut rust_decl_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for s in &prog.statements {
        match s {
            grammar::grammar::Statement::RustFnDecl(def) => {
                rust_decl_names.insert(def.name.clone());
            }
            grammar::grammar::Statement::Documented { item, .. } => {
                if let grammar::grammar::AnnotatableItem::RustFnDecl(def) = item {
                    rust_decl_names.insert(def.name.clone());
                }
            }
            _ => {}
        }
    }

    // Find imports to recurse
    for s in &prog.statements {
        if let grammar::grammar::Statement::Import { module_name, .. } = s {
            // For imports, use base_dir for relative imports or "" for std imports
            let import_dir = if module_name.starts_with("std_") {
                ""
            } else {
                base_dir
            };
            build_recursive(module_name, import_dir, seen, module_functions, pm, false)?;
        }
    }

    let rs_path = if !base_dir.is_empty() {
        format!("{}/{}.rs", base_dir, name)
    } else {
        format!("{}.rs", name)
    };

    if !name.starts_with("std_")
        && !rust_decl_names.is_empty()
        && !std::path::Path::new(&rs_path).exists()
    {
        let mut missing: Vec<String> = rust_decl_names.iter().cloned().collect();
        missing.sort();
        return Err(missing_host_glue_error(name, &src, &missing[0]));
    }

    // Compile
    let mut c = compiler::Compiler::with_module_functions(module_functions.clone());
    let diagnostic_file = format!("{}.kiro", name);
    c.validate_semantics(&prog, &diagnostic_file, &src)?;
    let code =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.compile(prog, is_root))) {
            Ok(code) => code,
            Err(payload) => {
                return Err(KiroError::compiler_panic(
                    name,
                    &panic_payload_to_string(payload),
                ));
            }
        };

    let save_name = if is_root { "main" } else { name };
    if let Err(e) = pm.save_file(save_name, code) {
        return Err(KiroError::new(
            errors::ErrorCode::BuildGraphFailed,
            errors::ErrorPhase::Compile,
            format!("Failed to save {}: {}", save_name, e),
        ));
    } else {
        println!("  - Compiled {}", name);
    }

    // If this is a std module, also copy its header.rs content
    if name.starts_with("std_") {
        let module_suffix = &name[4..];
        let header_path = format!("{}/header.rs", module_suffix);
        if let Some(file) = StdAssets::get(&header_path) {
            let header_content = std::str::from_utf8(file.data.as_ref()).unwrap();
            // Strip the initial use statement since we already have it in the main header
            let content = header_content
                .lines()
                .filter(|l| {
                    !l.trim().starts_with("use crate::")
                        && !l.trim().starts_with("use kiro_runtime")
                })
                .collect::<Vec<_>>()
                .join("\n");

            if let Err(e) = pm.append_header(&content) {
                eprintln!("Failed to append header for {}: {}", name, e);
            }
        }
    } else {
        // For user modules, check if there is a corresponding .rs file (Glue Code)
        // e.g. for "mylib", check "mylib.rs" alongside "mylib.kiro"
        if std::path::Path::new(&rs_path).exists() {
            println!("  - Found Glue: {}", rs_path);
            match std::fs::read_to_string(&rs_path) {
                Ok(content) => {
                    if let Err(e) = pm.append_header(&content) {
                        eprintln!("Failed to append glue code for {}: {}", name, e);
                    }
                }
                Err(e) => eprintln!("Failed to read glue file {}: {}", rs_path, e),
            }
        }
    }
    Ok(())
}

fn missing_host_glue_error(module: &str, source: &str, fn_name: &str) -> KiroError {
    let glue_file = format!("{}.rs", module);
    let mut err = KiroError::new(
        errors::ErrorCode::MissingHostGlue,
        errors::ErrorPhase::Compile,
        format!(
            "Missing Rust glue for host function '{}.{}'.",
            module, fn_name
        ),
    )
    .with_file(format!("{}.kiro", module))
    .with_help(format!(
        "add '{}' with `pub async fn {}(args: Vec<RuntimeVal>) -> HostResult`",
        glue_file, fn_name
    ));

    let needle = format!("rust fn {}", fn_name);
    for (idx, line) in source.lines().enumerate() {
        if let Some(col) = line.find(&needle) {
            err = err.with_source_location(
                format!("{}.kiro", module),
                idx + 1,
                col + 1,
                line.to_string(),
                "missing glue",
            );
            break;
        }
    }

    err
}
