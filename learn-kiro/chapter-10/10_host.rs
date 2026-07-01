// Glue code for 10_host.kiro
// Note: This is appended to header.rs, so we have access to kiro_runtime types via imports in header.

pub async fn read_file(args: Vec<kiro_runtime::RuntimeVal>) -> kiro_runtime::HostResult {
    // 1. Convert Args
    kiro_runtime::RuntimeVal::expect_arity(&args, 1, "read_file")?;
    let path = kiro_runtime::RuntimeVal::expect_arg(&args, 0, "read_file")?.as_str()?;

    // 2. Do Work (Mock Implementation for safety/demo)
    // Real glue should not assume optional async crates are available by default.
    // Here we just return a greeting to verify it works.
    let content = format!("Content of {}: Hello from Rust Glue!", path);

    // 3. Return Value
    Ok(kiro_runtime::RuntimeVal::from(content))
}
