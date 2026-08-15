use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kiro_runtime::{KiroError as HostError, RuntimeVal as HostRuntimeVal};

use crate::analysis::{self, AnalysisOptions, SourceOverlays};
use crate::eir::{EirProgram, lower_program};
use crate::grammar::{self, Statement};
use crate::hir::{FunctionId, HirCallKind, HirExprKind, HirStmtKind};
use crate::interpreter::eir_runtime::{EirRuntime, EirRuntimeError, EirRuntimeErrorKind};
use crate::interpreter::values::RuntimeVal as InterpreterRuntimeVal;
use crate::interpreter::{HostCallCtx as InterpreterHostCallCtx, HostFnHandler, InterpreterLimits};
use crate::{
    StdAssets, canonical_std_module_name, removed_print_statement, std_asset_path,
    unsupported_let_line,
};

pub use crate::interpreter::HostMode;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
    Handle(kiro_runtime::KiroHandle),
    Void,
    Error { name: String, description: String },
}

impl TryFrom<HostRuntimeVal> for Value {
    type Error = EngineError;

    fn try_from(value: HostRuntimeVal) -> Result<Self, EngineError> {
        match value {
            HostRuntimeVal::Num(n) => Ok(Value::Num(n)),
            HostRuntimeVal::Str(s) => Ok(Value::Str(s)),
            HostRuntimeVal::Bool(b) => Ok(Value::Bool(b)),
            HostRuntimeVal::List(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(Value::try_from(item)?);
                }
                Ok(Value::List(out))
            }
            HostRuntimeVal::Map(map) => {
                let mut out = HashMap::with_capacity(map.len());
                for (k, v) in map {
                    out.insert(k, Value::try_from(v)?);
                }
                Ok(Value::Map(out))
            }
            HostRuntimeVal::Handle(handle) => Ok(Value::Handle(handle)),
            HostRuntimeVal::Void => Ok(Value::Void),
        }
    }
}

impl TryFrom<Value> for HostRuntimeVal {
    type Error = EngineError;

    fn try_from(value: Value) -> Result<Self, EngineError> {
        match value {
            Value::Num(n) => Ok(HostRuntimeVal::Num(n)),
            Value::Str(s) => Ok(HostRuntimeVal::Str(s)),
            Value::Bool(b) => Ok(HostRuntimeVal::Bool(b)),
            Value::List(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(HostRuntimeVal::try_from(item)?);
                }
                Ok(HostRuntimeVal::List(out))
            }
            Value::Map(map) => {
                let mut out = HashMap::with_capacity(map.len());
                for (k, v) in map {
                    out.insert(k, HostRuntimeVal::try_from(v)?);
                }
                Ok(HostRuntimeVal::Map(out))
            }
            Value::Handle(handle) => Ok(HostRuntimeVal::Handle(handle)),
            Value::Void => Ok(HostRuntimeVal::Void),
            Value::Error { .. } => Err(EngineError::Type(
                "Cannot convert Value::Error into a host runtime value".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub enum EngineError {
    Parse(String),
    Runtime(String),
    Type(String),
    Load(String),
    HostRegistration(String),
}

impl Display for EngineError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::Parse(msg) => write!(f, "parse error: {}", msg),
            EngineError::Runtime(msg) => write!(f, "runtime error: {}", msg),
            EngineError::Type(msg) => write!(f, "type error: {}", msg),
            EngineError::Load(msg) => write!(f, "module load error: {}", msg),
            EngineError::HostRegistration(msg) => {
                write!(f, "host registration error: {}", msg)
            }
        }
    }
}

impl Error for EngineError {}

#[derive(Debug, Clone, PartialEq)]
pub struct Limits {
    pub max_steps: Option<u64>,
    pub max_call_depth: Option<usize>,
    pub timeout_ms: Option<u64>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_steps: None,
            max_call_depth: None,
            timeout_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecOptions {
    pub host_mode: HostMode,
    pub limits: Limits,
}

impl Default for ExecOptions {
    fn default() -> Self {
        Self {
            host_mode: HostMode::Simulate,
            limits: Limits::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HostFnSpec {
    pub module: String,
    pub name: String,
    pub params: Vec<grammar::KiroType>,
    pub ret: grammar::KiroType,
    pub can_error: bool,
}

#[derive(Debug, Clone)]
pub struct HostCallCtx {
    pub module_name: String,
    pub function_name: String,
    pub step_count: u64,
}

pub type HostResult = Result<Value, HostError>;

#[derive(Debug, Clone)]
struct HostDecl {
    params: Vec<grammar::KiroType>,
    ret: grammar::KiroType,
    can_error: bool,
}

#[derive(Debug, Clone)]
pub struct CompiledScript {
    pub module_name: String,
    pub source: String,
    pub base_dir: PathBuf,
    eir_program: EirProgram,
    functions: HashMap<String, FunctionId>,
    main: Option<FunctionId>,
    host_decls: HashMap<(String, String), HostDecl>,
}

pub trait ModuleLoader: Send + Sync {
    fn load(&self, module_name: &str, from_dir: &Path) -> Result<LoadedModule, EngineError>;
}

#[derive(Debug, Clone)]
pub struct LoadedModule {
    pub cache_key: String,
    pub source: String,
    pub base_dir: PathBuf,
}

#[derive(Default)]
pub struct DefaultModuleLoader;

impl ModuleLoader for DefaultModuleLoader {
    fn load(&self, module_name: &str, from_dir: &Path) -> Result<LoadedModule, EngineError> {
        if let Some(canonical) = canonical_std_module_name(module_name) {
            let asset_path = std_asset_path(module_name, &format!("{}.kiro", canonical))
                .expect("known std module should have an asset path");
            let source = StdAssets::get(&asset_path)
                .map(|f| std::str::from_utf8(f.data.as_ref()).unwrap().to_string())
                .ok_or_else(|| {
                    EngineError::Load(format!(
                        "Standard library module '{}' not found in embedded assets",
                        module_name
                    ))
                })?;

            return Ok(LoadedModule {
                cache_key: format!("std://{}", canonical),
                source,
                base_dir: from_dir.to_path_buf(),
            });
        }
        if module_name.starts_with("std_") {
            return Err(EngineError::Load(format!(
                "Standard library module '{}' not found in embedded assets",
                module_name
            )));
        }

        let full_path = from_dir.join(crate::grammar::module_path_file_path(module_name));
        let resolved = std::fs::canonicalize(&full_path).unwrap_or(full_path.clone());
        let source = std::fs::read_to_string(&resolved)
            .map_err(|_| EngineError::Load(format!("Module '{}' not found", resolved.display())))?;
        let parent = resolved
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        Ok(LoadedModule {
            cache_key: resolved.to_string_lossy().to_string(),
            source,
            base_dir: parent,
        })
    }
}

pub struct EngineBuilder {
    base_dir: PathBuf,
    default_options: ExecOptions,
    module_loader: Option<Arc<dyn ModuleLoader>>,
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self {
            base_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            default_options: ExecOptions::default(),
            module_loader: None,
        }
    }
}

pub struct Engine {
    base_dir: PathBuf,
    default_options: ExecOptions,
    module_loader: Arc<dyn ModuleLoader>,
    host_specs: HashMap<(String, String), HostFnSpec>,
    host_handlers: HashMap<(String, String), HostFnHandler>,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn base_dir(mut self, base_dir: impl Into<PathBuf>) -> Self {
        self.base_dir = base_dir.into();
        self
    }

    pub fn default_options(mut self, options: ExecOptions) -> Self {
        self.default_options = options;
        self
    }

    pub fn module_loader(mut self, loader: Arc<dyn ModuleLoader>) -> Self {
        self.module_loader = Some(loader);
        self
    }

    pub fn build(self) -> Engine {
        Engine {
            base_dir: self.base_dir,
            default_options: self.default_options,
            module_loader: self
                .module_loader
                .unwrap_or_else(|| Arc::new(DefaultModuleLoader)),
            host_specs: HashMap::new(),
            host_handlers: HashMap::new(),
        }
    }
}

impl Engine {
    pub fn builder() -> EngineBuilder {
        EngineBuilder::new()
    }

    pub fn register_host_fn<F>(&mut self, spec: HostFnSpec, f: F) -> Result<(), EngineError>
    where
        F: Fn(HostCallCtx, &[Value]) -> HostResult + Send + Sync + 'static,
    {
        let key = (spec.module.clone(), spec.name.clone());
        if self.host_specs.contains_key(&key) {
            return Err(EngineError::HostRegistration(format!(
                "Host function '{}.{}' is already registered",
                spec.module, spec.name
            )));
        }

        let handler: HostFnHandler = Arc::new(move |ctx: InterpreterHostCallCtx, args| {
            let mut converted_args = Vec::with_capacity(args.len());
            for arg in args {
                let converted = Value::try_from(arg).map_err(|_| HostError::new("TypeError"))?;
                converted_args.push(converted);
            }

            let api_ctx = HostCallCtx {
                module_name: ctx.module_name,
                function_name: ctx.function_name,
                step_count: ctx.step_count,
            };

            let result = f(api_ctx, &converted_args)?;
            HostRuntimeVal::try_from(result).map_err(|_| HostError::new("TypeError"))
        });

        self.host_specs.insert(key.clone(), spec);
        self.host_handlers.insert(key, handler);
        Ok(())
    }

    pub fn compile_module(
        &self,
        module_name: &str,
        source: &str,
    ) -> Result<CompiledScript, EngineError> {
        if let Some(line) = unsupported_let_line(source) {
            return Err(EngineError::Parse(format!(
                "Unsupported keyword 'let' in module '{}' at line {}.",
                module_name, line
            )));
        }
        if let Some(removed) = removed_print_statement(source) {
            return Err(EngineError::Parse(format!(
                "'print' statement was removed in module '{}' at line {}. use `import io` and `io.print(value)`",
                module_name, removed.line
            )));
        }

        let root_path = self
            .base_dir
            .join(crate::grammar::module_path_file_path(module_name));
        let mut overlays = SourceOverlays::new();
        overlays.insert(root_path.clone(), source.to_string());
        self.collect_module_overlays(
            source,
            root_path.parent().unwrap_or(&self.base_dir),
            &self.base_dir,
            &mut overlays,
            &mut std::collections::HashSet::new(),
        )?;
        let analysis = analysis::analyze_path_with_info_options(
            &root_path,
            &overlays,
            AnalysisOptions {
                allow_registered_host_functions: true,
            },
        )
        .map_err(|error| EngineError::Parse(error.message))?;
        let root_module_name = analysis
            .modules
            .values()
            .find(|module| module.path == analysis.root)
            .map(|module| module.name.clone())
            .ok_or_else(|| EngineError::Parse("analyzed root module was not found".to_string()))?;
        let root_module = analysis
            .hir
            .module(&root_module_name)
            .ok_or_else(|| EngineError::Parse("root HIR module was not found".to_string()))?;
        let functions = root_module
            .functions
            .iter()
            .map(|function| (function.name.clone(), function.id))
            .collect::<HashMap<_, _>>();
        let main = functions.get("main").copied();
        let mut hir = analysis.hir.clone();
        if let Some(main) = main {
            let root_module = hir
                .modules
                .iter_mut()
                .find(|module| module.name == root_module_name)
                .expect("root module was resolved above");
            root_module.statements.retain(|statement| {
                !matches!(
                    &statement.kind,
                    HirStmtKind::Expr(expression)
                        if matches!(
                            &expression.kind,
                            HirExprKind::Call {
                                kind: HirCallKind::Direct(function),
                                ..
                            } if *function == main
                        )
                )
            });
        }
        let eir_program = lower_program(&hir)
            .map_err(|error| EngineError::Parse(format!("EIR lowering failed: {error}")))?;
        let mut host_decls = HashMap::new();
        for module in analysis.modules.values() {
            collect_host_decls(&module.name, &module.program, &mut host_decls);
        }

        Ok(CompiledScript {
            module_name: module_name.to_string(),
            source: source.to_string(),
            base_dir: self.base_dir.clone(),
            eir_program,
            functions,
            main,
            host_decls,
        })
    }

    pub fn run_main(
        &self,
        script: &CompiledScript,
        options: ExecOptions,
    ) -> Result<Value, EngineError> {
        let mut runtime = self.prepare_runtime(script, options)?;
        runtime
            .run_initializers()
            .map_err(|error| EngineError::Runtime(engine_runtime_message(error)))?;
        let Some(main) = script.main else {
            return Ok(Value::Void);
        };
        let value = runtime
            .call_function(main, Vec::new())
            .map_err(|error| EngineError::Runtime(engine_runtime_message(error)))?;
        interpreter_to_value(value)
    }

    pub fn call_fn(
        &self,
        script: &CompiledScript,
        fn_name: &str,
        args: Vec<Value>,
        options: ExecOptions,
    ) -> Result<Value, EngineError> {
        let mut runtime = self.prepare_runtime(script, options)?;
        let mut arg_values = Vec::with_capacity(args.len());
        for arg in args {
            arg_values.push(value_to_interpreter_runtime(arg)?);
        }

        runtime
            .run_initializers()
            .map_err(|error| EngineError::Runtime(engine_runtime_message(error)))?;
        let function = script.functions.get(fn_name).copied().ok_or_else(|| {
            EngineError::Runtime(format!(
                "Function '{}.{}' not found",
                script.module_name, fn_name
            ))
        })?;
        let result = runtime
            .call_function(function, arg_values)
            .map_err(|error| EngineError::Runtime(engine_runtime_message(error)))?;

        interpreter_to_value(result)
    }

    fn prepare_runtime<'program>(
        &self,
        script: &'program CompiledScript,
        options: ExecOptions,
    ) -> Result<EirRuntime<'program>, EngineError> {
        let options = if options == ExecOptions::default() {
            self.default_options.clone()
        } else {
            options
        };

        self.validate_host_contracts(script, &options)?;

        let mut runtime = EirRuntime::new(&script.eir_program)
            .map_err(|error| EngineError::Runtime(error.to_string()))?;
        runtime.set_host_mode(options.host_mode);
        runtime.set_limits(InterpreterLimits {
            max_steps: options.limits.max_steps,
            max_call_depth: options.limits.max_call_depth,
            timeout: options.limits.timeout_ms.map(Duration::from_millis),
        });
        for ((module, name), handler) in &self.host_handlers {
            runtime.register_host_fn(module, name, handler.clone());
        }

        Ok(runtime)
    }

    fn collect_module_overlays(
        &self,
        source: &str,
        analysis_dir: &Path,
        loader_dir: &Path,
        overlays: &mut SourceOverlays,
        seen: &mut std::collections::HashSet<String>,
    ) -> Result<(), EngineError> {
        let program =
            grammar::parse(source).map_err(|error| EngineError::Parse(format!("{error:?}")))?;
        for statement in &program.statements {
            let Statement::Import { module_name, .. } = statement else {
                continue;
            };
            let name = crate::grammar::module_path_name(module_name);
            if crate::is_reserved_std_module_name(name) {
                continue;
            }
            let loaded = self.module_loader.load(name, loader_dir)?;
            if !seen.insert(loaded.cache_key.clone()) {
                continue;
            }
            let overlay_path = analysis_dir.join(crate::grammar::module_path_file_path(name));
            overlays.insert(overlay_path.clone(), loaded.source.clone());
            self.collect_module_overlays(
                &loaded.source,
                overlay_path.parent().unwrap_or(analysis_dir),
                &loaded.base_dir,
                overlays,
                seen,
            )?;
        }
        Ok(())
    }

    fn validate_host_contracts(
        &self,
        script: &CompiledScript,
        options: &ExecOptions,
    ) -> Result<(), EngineError> {
        if options.host_mode != HostMode::Execute {
            return Ok(());
        }

        for (key, decl) in &script.host_decls {
            let Some(spec) = self.host_specs.get(key) else {
                return Err(EngineError::HostRegistration(format!(
                    "Missing host registration for '{}.{}'",
                    key.0, key.1
                )));
            };

            if spec.params.len() != decl.params.len() {
                return Err(EngineError::HostRegistration(format!(
                    "Host signature mismatch for '{}.{}': parameter count differs",
                    key.0, key.1
                )));
            }

            let params_match = spec
                .params
                .iter()
                .zip(decl.params.iter())
                .all(|(a, b)| format!("{:?}", a) == format!("{:?}", b));

            if !params_match {
                return Err(EngineError::HostRegistration(format!(
                    "Host signature mismatch for '{}.{}': parameter types differ",
                    key.0, key.1
                )));
            }

            if format!("{:?}", spec.ret) != format!("{:?}", decl.ret) {
                return Err(EngineError::HostRegistration(format!(
                    "Host signature mismatch for '{}.{}': return type differs",
                    key.0, key.1
                )));
            }

            if spec.can_error != decl.can_error {
                return Err(EngineError::HostRegistration(format!(
                    "Host signature mismatch for '{}.{}': failable marker differs",
                    key.0, key.1
                )));
            }
        }

        Ok(())
    }
}

fn collect_host_decls(
    module_name: &str,
    program: &grammar::Program,
    declarations: &mut HashMap<(String, String), HostDecl>,
) {
    for statement in &program.statements {
        let declaration = match statement {
            Statement::RustFnDecl(declaration) => Some(declaration),
            Statement::Documented {
                item: grammar::AnnotatableItem::RustFnDecl(declaration),
                ..
            } => Some(declaration),
            _ => None,
        };
        let Some(declaration) = declaration else {
            continue;
        };
        let name = crate::grammar::function_name(&declaration.name).to_string();
        declarations.insert(
            (module_name.to_string(), name),
            HostDecl {
                params: declaration
                    .params
                    .iter()
                    .map(|parameter| parameter.command_type.clone())
                    .collect(),
                ret: declaration.return_type.clone(),
                can_error: declaration.can_error.is_some(),
            },
        );
    }
}

fn engine_runtime_message(error: EirRuntimeError) -> String {
    match &error.kind {
        EirRuntimeErrorKind::HostCallDenied { module, function } => {
            format!("Host call denied for '{module}.{function}'")
        }
        _ => error.to_string(),
    }
}

fn value_to_interpreter_runtime(value: Value) -> Result<InterpreterRuntimeVal, EngineError> {
    match value {
        Value::Num(n) => Ok(InterpreterRuntimeVal::Float(n)),
        Value::Str(s) => Ok(InterpreterRuntimeVal::String(s)),
        Value::Bool(b) => Ok(InterpreterRuntimeVal::Bool(b)),
        Value::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(value_to_interpreter_runtime(item)?);
            }
            Ok(InterpreterRuntimeVal::List(out))
        }
        Value::Map(map) => {
            let mut out = HashMap::with_capacity(map.len());
            for (key, value) in map {
                out.insert(key, value_to_interpreter_runtime(value)?);
            }
            Ok(InterpreterRuntimeVal::Map(out))
        }
        Value::Handle(handle) => Ok(InterpreterRuntimeVal::Handle(handle)),
        Value::Void => Err(EngineError::Type(
            "Cannot pass Value::Void as a function argument".to_string(),
        )),
        Value::Error { .. } => Err(EngineError::Type(
            "Cannot pass Value::Error as a function argument".to_string(),
        )),
    }
}

fn interpreter_to_value(value: InterpreterRuntimeVal) -> Result<Value, EngineError> {
    match value {
        InterpreterRuntimeVal::Float(n) => Ok(Value::Num(n)),
        InterpreterRuntimeVal::String(s) => Ok(Value::Str(s)),
        InterpreterRuntimeVal::Bool(b) => Ok(Value::Bool(b)),
        InterpreterRuntimeVal::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(interpreter_to_value(item)?);
            }
            Ok(Value::List(out))
        }
        InterpreterRuntimeVal::Map(map) => {
            let mut out = HashMap::with_capacity(map.len());
            for (k, v) in map {
                out.insert(k, interpreter_to_value(v)?);
            }
            Ok(Value::Map(out))
        }
        InterpreterRuntimeVal::Handle(handle) => Ok(Value::Handle(handle)),
        InterpreterRuntimeVal::Void => Ok(Value::Void),
        InterpreterRuntimeVal::Error(name, description) => Ok(Value::Error { name, description }),
        other => Err(EngineError::Type(format!(
            "Unsupported interpreter return value for embedding: {}",
            other
        ))),
    }
}
