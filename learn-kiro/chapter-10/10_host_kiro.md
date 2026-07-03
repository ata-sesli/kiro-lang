# Chapter 10: Host Modules (Kiro Side)

After implementing Rust glue, Kiro-side usage is intentionally simple. You declare a host function signature and call it as part of normal program flow.

A declaration looks like this:

```kiro
rust fn read_file(path: str) -> str!
```

This declaration is a contract between Kiro code and Rust code. Name, parameter types, and return type must stay aligned with the Rust implementation.

Native resources should use named handles:

```kiro
handle Model

rust fn load(path: str) -> Model!
rust fn label(model: Model) -> str!
rust fn close(model: Model) -> void!
```

`handle` values are opaque. Kiro can store and pass them, but only Rust glue can create or inspect the underlying native value.

Calling the function is no different from calling a regular Kiro function:

```kiro
import io

error FileNotFound = "File not found"

var content = read_file("data.txt")

on (content == FileNotFound) {
    io.print("Missing file")
} off {
    io.print(content)
}
```

In real projects, host calls are often at system boundaries: filesystem access, networking, cryptography, or integration with existing Rust crates.

For a user module named `tools.kiro`, put the Rust implementation next to it as `tools.rs`. If `main.kiro` declares a `rust fn`, its glue lives in `main.rs`. There is no `native/` fallback in V1. If the `.kiro` file declares a `rust fn` and the adjacent `.rs` file is missing, Kiro reports a compile diagnostic before Rust build.

Rust glue uses the ABI v2 shape:

Generated Kiro builds only include dependencies required by Kiro source (`std_*` imports and pipes), so this example uses the Rust standard library instead of assuming optional async crates are present.

```rust
use kiro_runtime::{HostResult, KiroError, RuntimeVal};

pub async fn read_file(args: Vec<RuntimeVal>) -> HostResult {
    RuntimeVal::expect_arity(&args, 1, "read_file")?;
    let path = RuntimeVal::expect_arg(&args, 0, "read_file")?.as_str()?;
    let content = std::fs::read_to_string(path)
        .map_err(|_| KiroError::message("NotFound", path.to_string()))?;
    Ok(RuntimeVal::from(content))
}
```

Handle glue uses the same ABI:

```rust
use kiro_runtime::{HostResult, RuntimeVal};

pub async fn load(args: Vec<RuntimeVal>) -> HostResult {
    RuntimeVal::expect_arity(&args, 1, "load")?;
    let path = RuntimeVal::expect_arg(&args, 0, "load")?.as_str()?.to_string();
    Ok(RuntimeVal::handle("Model", path))
}

pub async fn label(args: Vec<RuntimeVal>) -> HostResult {
    RuntimeVal::expect_arity(&args, 1, "label")?;
    let model = RuntimeVal::expect_arg(&args, 0, "label")?.as_handle("Model")?;
    let path = model.downcast_ref::<String>().expect("Model payload should be String");
    Ok(RuntimeVal::from(path.clone()))
}
```

## Common Pitfalls

A common issue is signature drift between `.kiro` declaration and Rust glue function. The correct method is to update both sides together and treat mismatches as build blockers.

Another issue is assuming host calls never fail. The correct method is to keep return types failable when appropriate and implement explicit success/failure handling at call sites.

Teams also frequently test only successful paths. The correct method is to create at least one controlled failure case for every host function and validate the produced error behavior.

## Final Step

Move to the [Final Project](../final-project/final_project.md).
