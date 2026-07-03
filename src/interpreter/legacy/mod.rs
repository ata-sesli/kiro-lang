use crate::grammar::{self, Statement};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use super::values::{RuntimeVal, Value};
use super::{
    HostFnHandler, HostMode, HostRegistry, InterpreterLimits, ModuleLoader, StatementResult,
};

pub mod expression;
pub mod statement;

pub struct Interpreter {
    pub env: HashMap<String, Value>,
    pub functions: HashMap<String, Statement>,
    pub in_pure_mode: bool,
    pub in_failable_fn: bool,
    pub error_types: HashMap<String, String>,
    pub pure_scope_params: HashSet<String>,
    pub module_cache: HashMap<String, RuntimeVal>,
    pub current_dir: PathBuf,
    pub current_module: String,
    pub host_mode: HostMode,
    pub host_registry: HostRegistry,
    pub limits: InterpreterLimits,
    pub step_count: u64,
    pub call_depth: usize,
    pub started_at: Option<Instant>,
    pub module_loader: Option<Arc<dyn ModuleLoader>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self::with_base_dir(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self {
            env: HashMap::new(),
            functions: HashMap::new(),
            in_pure_mode: false,
            in_failable_fn: false,
            error_types: HashMap::new(),
            pure_scope_params: HashSet::new(),
            module_cache: HashMap::new(),
            current_dir: base_dir,
            current_module: "main".to_string(),
            host_mode: HostMode::Simulate,
            host_registry: HostRegistry::default(),
            limits: InterpreterLimits::default(),
            step_count: 0,
            call_depth: 0,
            started_at: None,
            module_loader: None,
        }
    }

    pub fn set_current_module(&mut self, module: impl Into<String>) {
        self.current_module = module.into();
    }

    pub fn set_host_mode(&mut self, mode: HostMode) {
        self.host_mode = mode;
    }

    pub fn set_limits(&mut self, limits: InterpreterLimits) {
        self.limits = limits;
    }

    pub fn set_module_loader(&mut self, loader: Arc<dyn ModuleLoader>) {
        self.module_loader = Some(loader);
    }

    pub fn register_host_fn(
        &mut self,
        module: impl Into<String>,
        name: impl Into<String>,
        handler: HostFnHandler,
    ) {
        self.host_registry.register(module, name, handler);
    }

    pub(crate) fn tick(&mut self) -> Result<(), String> {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        self.step_count = self.step_count.saturating_add(1);

        if let Some(limit) = self.limits.max_steps
            && self.step_count > limit
        {
            return Err(format!(
                "Interpreter Error: Step limit exceeded ({} > {}).",
                self.step_count, limit
            ));
        }

        if let (Some(timeout), Some(started_at)) = (self.limits.timeout, self.started_at)
            && started_at.elapsed() > timeout
        {
            return Err(format!(
                "Interpreter Error: Timeout exceeded (>{} ms).",
                timeout.as_millis()
            ));
        }

        Ok(())
    }

    pub(crate) fn enter_call(&mut self) -> Result<(), String> {
        self.call_depth = self.call_depth.saturating_add(1);
        if let Some(limit) = self.limits.max_call_depth
            && self.call_depth > limit
        {
            let current_depth = self.call_depth;
            self.call_depth = self.call_depth.saturating_sub(1);
            return Err(format!(
                "Interpreter Error: Call depth limit exceeded ({} > {}).",
                current_depth, limit
            ));
        }
        Ok(())
    }

    pub(crate) fn exit_call(&mut self) {
        self.call_depth = self.call_depth.saturating_sub(1);
    }

    pub fn run(&mut self, program: grammar::Program) -> Result<(), String> {
        if self.started_at.is_none() {
            self.started_at = Some(Instant::now());
        }
        for statement in program.statements {
            let res = self.execute_statement(statement)?;
            match res {
                StatementResult::Normal(_) => {}
                StatementResult::Return(_) => return Ok(()),
                StatementResult::Break | StatementResult::Continue => {
                    return Err("Cannot break/continue outside of loop".to_string());
                }
            }
        }
        Ok(())
    }
}
