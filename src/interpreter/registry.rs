use std::collections::HashMap;

use crate::interpreter::HostFnHandler;
use crate::ir::{IrFunction, IrRustFunction, IrSignature};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunctionEntryKind {
    InterpretedKiro,
    HostNative,
    CompiledKiro,
}

#[derive(Clone)]
pub enum FunctionEntry {
    InterpretedKiro {
        function: IrFunction,
    },
    HostNative {
        declaration: IrRustFunction,
        handler: Option<HostFnHandler>,
    },
    CompiledKiro {
        signature: IrSignature,
    },
}

impl FunctionEntry {
    pub fn kind(&self) -> FunctionEntryKind {
        match self {
            FunctionEntry::InterpretedKiro { .. } => FunctionEntryKind::InterpretedKiro,
            FunctionEntry::HostNative { .. } => FunctionEntryKind::HostNative,
            FunctionEntry::CompiledKiro { .. } => FunctionEntryKind::CompiledKiro,
        }
    }

    pub fn signature(&self) -> &IrSignature {
        match self {
            FunctionEntry::InterpretedKiro { function } => &function.signature,
            FunctionEntry::HostNative { declaration, .. } => &declaration.signature,
            FunctionEntry::CompiledKiro { signature } => signature,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct FunctionKey {
    pub module: String,
    pub name: String,
}

impl FunctionKey {
    pub fn new(module: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            module: module.into(),
            name: name.into(),
        }
    }
}

#[derive(Clone, Default)]
pub struct FunctionRegistry {
    entries: HashMap<FunctionKey, FunctionEntry>,
}

impl FunctionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_interpreted(&mut self, module: &str, function: IrFunction) {
        let key = FunctionKey::new(module, &function.name);
        self.entries
            .insert(key, FunctionEntry::InterpretedKiro { function });
    }

    pub fn register_host_decl(&mut self, module: &str, declaration: IrRustFunction) {
        let key = FunctionKey::new(module, &declaration.name);
        self.entries.insert(
            key,
            FunctionEntry::HostNative {
                declaration,
                handler: None,
            },
        );
    }

    pub fn attach_host_handler(
        &mut self,
        module: &str,
        name: &str,
        handler: HostFnHandler,
    ) -> Result<(), String> {
        let key = FunctionKey::new(module, name);
        match self.entries.get_mut(&key) {
            Some(FunctionEntry::HostNative { handler: slot, .. }) => {
                *slot = Some(handler);
                Ok(())
            }
            Some(_) => Err(format!("'{}.{}' is not a host function.", module, name)),
            None => Err(format!(
                "Host function '{}.{}' is not declared.",
                module, name
            )),
        }
    }

    pub fn get(&self, module: &str, name: &str) -> Option<&FunctionEntry> {
        self.entries.get(&FunctionKey::new(module, name))
    }

    pub fn entry_kind(&self, module: &str, name: &str) -> Option<FunctionEntryKind> {
        self.get(module, name).map(FunctionEntry::kind)
    }

    pub fn extend_from(&mut self, other: &FunctionRegistry) {
        for (key, entry) in &other.entries {
            self.entries.insert(key.clone(), entry.clone());
        }
    }
}
