# Chapter 9: Host Modules (Rust Side)

Kiro is designed to be extensible through host functions implemented in Rust. This allows you to keep high-level logic in Kiro while delegating system access and ecosystem integration to Rust code.

When Kiro declares a `rust fn`, the runtime expects a matching Rust implementation. The Rust function typically receives runtime values, validates/converts them, performs work, and returns either a runtime value or a structured error.

```rust
use kiro_runtime::{HostResult, KiroError, RuntimeVal};

pub async fn read_file(args: Vec<RuntimeVal>) -> HostResult {
    RuntimeVal::expect_arity(&args, 1, "read_file")?;
    let path = RuntimeVal::expect_arg(&args, 0, "read_file")?.as_str()?;

    match tokio::fs::read_to_string(path).await {
        Ok(content) => Ok(RuntimeVal::from(content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Err(KiroError::message("NotFound", path.to_string()))
        }
        Err(err) => Err(KiroError::message("IoError", err.to_string())),
    }
}
```

The practical workflow is consistent: decode arguments, execute Rust logic, map success to `RuntimeVal`, map failures to `KiroError`.

Host error matching in Kiro uses the error name. The optional message is kept for diagnostics.

Keep host functions narrow in scope. Small host surfaces are easier to test, easier to review, and safer to evolve as language features grow.

## Common Pitfalls

A frequent integration failure is declaring host functions in Kiro but omitting Rust glue. The correct method is to treat declaration and implementation as one change set and verify both in the same run.

Another issue is trusting argument shape without validation. The correct method is to convert and check each runtime argument before use and return explicit errors for invalid input.

Panic-driven host code is also brittle. The correct method is to convert all expected failure paths into structured `KiroError` values.

## Next Step

Continue with [Chapter 10: Host Modules (Kiro Side)](../chapter-10/10_host_kiro.md).
