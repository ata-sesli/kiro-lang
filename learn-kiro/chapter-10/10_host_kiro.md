# Chapter 10: Host Modules (Kiro Side)

After implementing Rust glue, Kiro-side usage is intentionally simple. You declare a host function signature and call it as part of normal program flow.

A declaration looks like this:

```kiro
rust fn read_file(path: str) -> str!
```

This declaration is a contract between Kiro code and Rust code. Name, parameter types, and return type must stay aligned with the Rust implementation.

Calling the function is no different from calling a regular Kiro function:

```kiro
error FileNotFound = "File not found"

var content = read_file("data.txt")

on (content == FileNotFound) {
    print "Missing file"
} off {
    print content
}
```

In real projects, host calls are often at system boundaries: filesystem access, networking, cryptography, or integration with existing Rust crates.

For a user module named `tools.kiro`, put the Rust implementation next to it as `tools.rs`. If `main.kiro` declares a `rust fn`, its glue lives in `main.rs`. There is no `native/` fallback in V1. If the `.kiro` file declares a `rust fn` and the adjacent `.rs` file is missing, Kiro reports a compile diagnostic before Rust build.

Rust glue uses the ABI v1 shape:

```rust
use kiro_runtime::{HostResult, KiroError, RuntimeVal};

pub async fn read_file(args: Vec<RuntimeVal>) -> HostResult {
    RuntimeVal::expect_arity(&args, 1, "read_file")?;
    let path = RuntimeVal::expect_arg(&args, 0, "read_file")?.as_str()?;
    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|_| KiroError::message("NotFound", path.to_string()))?;
    Ok(RuntimeVal::from(content))
}
```

## Common Pitfalls

A common issue is signature drift between `.kiro` declaration and Rust glue function. The correct method is to update both sides together and treat mismatches as build blockers.

Another issue is assuming host calls never fail. The correct method is to keep return types failable when appropriate and implement explicit success/failure handling at call sites.

Teams also frequently test only successful paths. The correct method is to create at least one controlled failure case for every host function and validate the produced error behavior.

## Final Step

Move to the [Final Project](../final-project/final_project.md).
