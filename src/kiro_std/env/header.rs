// Kiro Standard Library: Environment (std::env)
// Glue layer between Kiro and Rust environment functions

use kiro_runtime::{HostResult, KiroError, RuntimeVal};

pub async fn get(args: Vec<RuntimeVal>) -> HostResult {
    let key = args[0].as_str()?;
    match std::env::var(key) {
        Ok(value) => Ok(RuntimeVal::from(value)),
        Err(_) => Err(KiroError::new("EnvNotFound")),
    }
}

pub async fn set(args: Vec<RuntimeVal>) -> HostResult {
    let key = args[0].as_str()?;
    let value = args[1].as_str()?;
    std::env::set_var(key, value);
    Ok(RuntimeVal::Void)
}

pub async fn args(_args: Vec<RuntimeVal>) -> HostResult {
    let args: Vec<RuntimeVal> = std::env::args().map(|s| RuntimeVal::from(s)).collect();
    Ok(RuntimeVal::List(args))
}
