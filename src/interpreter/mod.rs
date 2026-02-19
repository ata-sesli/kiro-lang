use crate::grammar::{self, Statement};
use std::collections::HashMap;
use std::path::PathBuf;

pub mod expression;
pub mod statement;
pub mod values;

use values::{RuntimeVal, Value};

use std::collections::HashSet;

#[derive(Debug, Clone)]
pub enum StatementResult {
    Normal(RuntimeVal),
    Return(RuntimeVal),
    Break,
    Continue,
}

pub struct Interpreter {
    pub env: HashMap<String, Value>,
    pub functions: HashMap<String, Statement>,
    pub in_pure_mode: bool,
    pub in_failable_fn: bool,
    pub error_types: HashMap<String, String>, // name -> description
    pub pure_scope_params: HashSet<String>,   // Allowed params in pure scope
    pub module_cache: HashMap<String, RuntimeVal>, // Loaded modules
    pub current_dir: PathBuf,                 // Base dir for relative imports
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
        }
    }
    pub fn run(&mut self, program: grammar::Program) -> Result<(), String> {
        for statement in program.statements {
            let res = self.execute_statement(statement)?;
            // If top-level statement returns Return/Break/Continue, that's an error
            // (or we could just exit logic, but standard is scripts only return via exit)
            match res {
                StatementResult::Normal(_) => {}
                StatementResult::Return(_) => return Ok(()), // Allow script to return
                StatementResult::Break | StatementResult::Continue => {
                    return Err("Cannot break/continue outside of loop".to_string());
                }
            }
        }
        Ok(())
    }
}
