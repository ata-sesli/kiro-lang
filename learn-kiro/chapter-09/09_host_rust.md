# Chapter 9: Host Modules (Rust Side)

Kiro is designed to be extensible through host functions implemented in Rust. This allows you to keep high-level logic in Kiro while delegating system access and ecosystem integration to Rust code.

When Kiro declares a `rust fn`, the runtime expects a matching Rust implementation. The Rust function typically receives runtime values, validates/converts them, performs work, and returns either a runtime value or a structured error.

```rust
use kiro_runtime::{RuntimeVal, KiroError};

pub async fn read_file(args: Vec<RuntimeVal>) -> Result<RuntimeVal, KiroError> {
    let path = args[0].as_str()?;
    let content = std::fs::read_to_string(path)
        .map_err(|e| KiroError::new(&format!("read_file failed: {}", e)))?;

    Ok(RuntimeVal::Str(content))
}
```

The practical workflow is consistent: decode arguments, execute Rust logic, map success to `RuntimeVal`, map failures to `KiroError`.

Keep host functions narrow in scope. Small host surfaces are easier to test, easier to review, and safer to evolve as language features grow.

## Common Pitfalls

A frequent integration failure is declaring host functions in Kiro but omitting Rust glue. The correct method is to treat declaration and implementation as one change set and verify both in the same run.

Another issue is trusting argument shape without validation. The correct method is to convert and check each runtime argument before use and return explicit errors for invalid input.

Panic-driven host code is also brittle. The correct method is to convert all expected failure paths into structured `KiroError` values.

## Next Step

Continue with [Chapter 10: Host Modules (Kiro Side)](../chapter-10/10_host_kiro.md).
