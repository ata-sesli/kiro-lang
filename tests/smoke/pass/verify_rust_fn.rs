use kiro_runtime::{HostResult, RuntimeVal};

pub async fn read_file(args: Vec<RuntimeVal>) -> HostResult {
    RuntimeVal::expect_arity(&args, 1, "read_file")?;
    Ok(RuntimeVal::from("MOCK_STRING"))
}
