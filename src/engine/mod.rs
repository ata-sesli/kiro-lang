use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use kiro_runtime::{KiroError as HostError, RuntimeVal as HostRuntimeVal};

use crate::grammar::{self, Expression, Statement};
use crate::interpreter::values::RuntimeVal as InterpreterRuntimeVal;
use crate::interpreter::{
    HostCallCtx as InterpreterHostCallCtx, HostFnHandler, Interpreter, InterpreterLimits,
};
use crate::{StdAssets, unsupported_let_line};

pub use crate::interpreter::HostMode;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
    Bool(bool),
    List(Vec<Value>),
    Map(HashMap<String, Value>),
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
    has_main: bool,
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
        if module_name.starts_with("std_") {
            let key = &module_name[4..];
            let asset_path = format!("{}/{}.kiro", key, module_name);
            let source = StdAssets::get(&asset_path)
                .map(|f| std::str::from_utf8(f.data.as_ref()).unwrap().to_string())
                .ok_or_else(|| {
                    EngineError::Load(format!(
                        "Standard library module '{}' not found in embedded assets",
                        module_name
                    ))
                })?;

            return Ok(LoadedModule {
                cache_key: format!("std://{}", module_name),
                source,
                base_dir: from_dir.to_path_buf(),
            });
        }

        let filename = format!("{}.kiro", module_name);
        let full_path = from_dir.join(&filename);
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

#[derive(Clone)]
struct LoaderAdapter {
    inner: Arc<dyn ModuleLoader>,
}

impl crate::interpreter::ModuleLoader for LoaderAdapter {
    fn load(
        &self,
        module_name: &str,
        current_dir: &Path,
    ) -> Result<crate::interpreter::LoadedModule, String> {
        let loaded = self
            .inner
            .load(module_name, current_dir)
            .map_err(|e| e.to_string())?;

        Ok(crate::interpreter::LoadedModule {
            cache_key: loaded.cache_key,
            source: loaded.source,
            base_dir: loaded.base_dir,
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

        let program = grammar::parse(source).map_err(|e| EngineError::Parse(format!("{:?}", e)))?;

        let mut has_main = false;
        let mut host_decls = HashMap::new();

        for stmt in &program.statements {
            match stmt {
                Statement::FunctionDef(def) if def.name == "main" => {
                    has_main = true;
                }
                Statement::RustFnDecl(def) => {
                    host_decls.insert(
                        (module_name.to_string(), def.name.clone()),
                        HostDecl {
                            params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                            ret: def.return_type.clone(),
                            can_error: def.can_error.is_some(),
                        },
                    );
                }
                Statement::Documented { item, .. } => match item {
                    grammar::AnnotatableItem::FunctionDef(def) if def.name == "main" => {
                        has_main = true;
                    }
                    grammar::AnnotatableItem::RustFnDecl(def) => {
                        host_decls.insert(
                            (module_name.to_string(), def.name.clone()),
                            HostDecl {
                                params: def.params.iter().map(|p| p.command_type.clone()).collect(),
                                ret: def.return_type.clone(),
                                can_error: def.can_error.is_some(),
                            },
                        );
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        Ok(CompiledScript {
            module_name: module_name.to_string(),
            source: source.to_string(),
            base_dir: self.base_dir.clone(),
            has_main,
            host_decls,
        })
    }

    pub fn run_main(
        &self,
        script: &CompiledScript,
        options: ExecOptions,
    ) -> Result<Value, EngineError> {
        if script.has_main {
            self.call_fn(script, "main", vec![], options)
        } else {
            let mut interpreter = self.prepare_interpreter(script, options)?;
            let program = grammar::parse(&script.source)
                .map_err(|e| EngineError::Parse(format!("{:?}", e)))?;
            interpreter.run(program).map_err(EngineError::Runtime)?;
            Ok(Value::Void)
        }
    }

    pub fn call_fn(
        &self,
        script: &CompiledScript,
        fn_name: &str,
        args: Vec<Value>,
        options: ExecOptions,
    ) -> Result<Value, EngineError> {
        let mut interpreter = self.prepare_interpreter(script, options)?;

        let program =
            grammar::parse(&script.source).map_err(|e| EngineError::Parse(format!("{:?}", e)))?;

        interpreter
            .run(declaration_program(program))
            .map_err(EngineError::Runtime)?;

        let mut arg_exprs = Vec::with_capacity(args.len());
        for arg in args {
            arg_exprs.push(value_to_expression(arg)?);
        }

        let call = Expression::Call(
            Box::new(Expression::Variable(grammar::VariableVal {
                value: fn_name.to_string(),
            })),
            (),
            arg_exprs,
            (),
        );

        let result = interpreter.eval_expr(call).map_err(EngineError::Runtime)?;

        interpreter_to_value(result)
    }

    fn prepare_interpreter(
        &self,
        script: &CompiledScript,
        options: ExecOptions,
    ) -> Result<Interpreter, EngineError> {
        let options = if options == ExecOptions::default() {
            self.default_options.clone()
        } else {
            options
        };

        self.validate_host_contracts(script, &options)?;

        let mut interpreter = Interpreter::with_base_dir(script.base_dir.clone());
        interpreter.set_current_module(script.module_name.clone());
        interpreter.set_host_mode(options.host_mode);
        interpreter.set_limits(InterpreterLimits {
            max_steps: options.limits.max_steps,
            max_call_depth: options.limits.max_call_depth,
            timeout: options.limits.timeout_ms.map(Duration::from_millis),
        });
        interpreter.set_module_loader(Arc::new(LoaderAdapter {
            inner: self.module_loader.clone(),
        }));

        for ((module, name), handler) in &self.host_handlers {
            interpreter.register_host_fn(module.clone(), name.clone(), handler.clone());
        }

        Ok(interpreter)
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

fn escape_kiro_string(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

fn declaration_program(program: grammar::Program) -> grammar::Program {
    grammar::Program {
        statements: program
            .statements
            .into_iter()
            .filter(is_declaration_statement)
            .collect(),
    }
}

fn is_declaration_statement(stmt: &Statement) -> bool {
    match stmt {
        Statement::ErrorDef { .. }
        | Statement::StructDef(_)
        | Statement::FunctionDef(_)
        | Statement::RustFnDecl(_)
        | Statement::Import { .. } => true,
        Statement::Documented { .. } => true,
        _ => false,
    }
}

fn value_to_expression(value: Value) -> Result<Expression, EngineError> {
    match value {
        Value::Num(n) => Ok(Expression::Number(grammar::NumberVal {
            value: n.to_string(),
        })),
        Value::Str(s) => Ok(Expression::StringLit(grammar::StringVal {
            value: format!("\"{}\"", escape_kiro_string(&s)),
        })),
        Value::Bool(true) => Ok(Expression::BoolLit(grammar::BoolVal::True(()))),
        Value::Bool(false) => Ok(Expression::BoolLit(grammar::BoolVal::False(()))),
        Value::List(items) => {
            let mut exprs = Vec::with_capacity(items.len());
            for item in items {
                exprs.push(value_to_expression(item)?);
            }
            Ok(Expression::ListInit(
                (),
                grammar::KiroType::Num,
                (),
                exprs,
                (),
            ))
        }
        Value::Map(map) => {
            let mut pairs = Vec::with_capacity(map.len());
            for (key, value) in map {
                pairs.push(grammar::MapPair {
                    key: Expression::StringLit(grammar::StringVal {
                        value: format!("\"{}\"", escape_kiro_string(&key)),
                    }),
                    value: value_to_expression(value)?,
                });
            }
            Ok(Expression::MapInit(
                (),
                grammar::KiroType::Str,
                grammar::KiroType::Num,
                (),
                pairs,
                (),
            ))
        }
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
        InterpreterRuntimeVal::Void => Ok(Value::Void),
        InterpreterRuntimeVal::Error(name, description) => Ok(Value::Error { name, description }),
        other => Err(EngineError::Type(format!(
            "Unsupported interpreter return value for embedding: {}",
            other
        ))),
    }
}
