use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use kiro_runtime::{KiroError as HostError, RuntimeVal as HostRuntimeVal};

pub mod legacy;
pub mod registry;
pub mod runtime;
pub mod values;

pub use runtime::SessionRuntime;

use values::RuntimeVal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HostMode {
    Execute,
    #[default]
    Simulate,
    Deny,
}

#[derive(Debug, Clone)]
pub struct InterpreterLimits {
    pub max_steps: Option<u64>,
    pub max_call_depth: Option<usize>,
    pub timeout: Option<Duration>,
}

impl Default for InterpreterLimits {
    fn default() -> Self {
        Self {
            max_steps: None,
            max_call_depth: None,
            timeout: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HostCallCtx {
    pub module_name: String,
    pub function_name: String,
    pub step_count: u64,
}

pub type HostFnHandler = Arc<
    dyn Fn(HostCallCtx, Vec<HostRuntimeVal>) -> Result<HostRuntimeVal, HostError> + Send + Sync,
>;

#[derive(Clone, Default)]
pub struct HostRegistry {
    handlers: HashMap<(String, String), HostFnHandler>,
}

impl HostRegistry {
    pub fn register(
        &mut self,
        module: impl Into<String>,
        name: impl Into<String>,
        handler: HostFnHandler,
    ) {
        self.handlers.insert((module.into(), name.into()), handler);
    }

    pub fn get(&self, module: &str, name: &str) -> Option<HostFnHandler> {
        self.handlers
            .get(&(module.to_string(), name.to_string()))
            .cloned()
    }
}

#[derive(Debug, Clone)]
pub struct LoadedModule {
    pub cache_key: String,
    pub source: String,
    pub base_dir: PathBuf,
}

pub trait ModuleLoader: Send + Sync {
    fn load(&self, module_name: &str, current_dir: &Path) -> Result<LoadedModule, String>;
}

#[derive(Debug, Clone)]
pub enum StatementResult {
    Normal(RuntimeVal),
    Return(RuntimeVal),
    Break,
    Continue,
}
