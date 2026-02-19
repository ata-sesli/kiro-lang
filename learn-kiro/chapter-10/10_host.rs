// Glue code for 10_host.kiro
// Note: This is appended to header.rs, so we have access to kiro_runtime types via imports in header.

pub async fn read_file(
    args: Vec<kiro_runtime::RuntimeVal>,
) -> Result<kiro_runtime::RuntimeVal, kiro_runtime::KiroError> {
    // 1. Convert Args
    let path = args
        .get(0)
        .ok_or_else(|| kiro_runtime::KiroError::new("Missing argument"))?
        .as_str()?;

    // 2. Do Work (Mock Implementation for safety/demo)
    // In a real app, we would use tokio::fs::read_to_string(path).await
    // Here we just return a greeting to verify it works.
    let content = format!("Content of {}: Hello from Rust Glue!", path);

    // 3. Return Value
    Ok(kiro_runtime::RuntimeVal::Str(content))
}
