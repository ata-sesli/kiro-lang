# kiro_runtime

`kiro_runtime` is a small Rust crate that defines shared runtime data structures used at the Kiro <-> Rust boundary.

It exists to keep host glue behavior consistent and avoid re-defining value conversion logic in every Rust integration point.

## What This Crate Provides

### ABI Version

The current host ABI version is:

```rust
pub const KIRO_RUNTIME_ABI_VERSION: u32 = 1;
```

Host glue for ABI v1 uses this shape:

```rust
pub async fn name(args: Vec<RuntimeVal>) -> HostResult
```

where:

```rust
pub type HostResult = Result<RuntimeVal, KiroError>;
```

### Runtime Value Model

`RuntimeVal` represents Kiro values in Rust:

- `Num(f64)`
- `Str(String)`
- `Bool(bool)`
- `List(Vec<RuntimeVal>)`
- `Map(HashMap<String, RuntimeVal>)`
- `Void`

### Error Type

`KiroError` is the error type for glue-level failures.

- `KiroError::new("NotFound")`
- `KiroError::message("IoError", "failed to read config.txt")`
- `Display` + `std::error::Error` implemented

Kiro error matching uses the error `name` only. The optional message is for diagnostics and logs.

### Conversions

The crate provides conversion helpers both directions:

From Rust to `RuntimeVal`:

- `f64`, `String`, `&str`, `bool`, `()`
- `Vec<T>` where `T: Into<RuntimeVal>`

From `RuntimeVal` to Rust:

- `TryFrom<RuntimeVal>` for `String`, `f64`, `bool`, `()`, `Vec<String>`
- Accessor helpers:
  - `RuntimeVal::as_str()`
  - `RuntimeVal::as_num()`
  - `RuntimeVal::as_bool()`
  - `RuntimeVal::as_list()`
  - `RuntimeVal::as_map()`
  - `RuntimeVal::as_void()`

Argument helpers:

- `RuntimeVal::expect_arity(args, expected, fn_name)`
- `RuntimeVal::expect_arg(args, index, fn_name)`

## Why It Exists

Without a shared runtime crate, host modules often duplicate:

- Value enum definitions
- Type conversion logic
- Basic error conventions

`kiro_runtime` gives one place to evolve those contracts.

## Basic Example

```rust
use kiro_runtime::{HostResult, KiroError, RuntimeVal};
use std::convert::TryFrom;

fn roundtrip() -> Result<(), KiroError> {
    let raw = RuntimeVal::from(42.0);
    let n = f64::try_from(raw)?;

    let text = RuntimeVal::from("hello");
    let s = String::try_from(text)?;

    let _ = (n, s);
    Ok(())
}
```

## Integration Pattern (Host Glue)

Typical glue flow:

1. Receive Kiro values as `Vec<RuntimeVal>`.
2. Validate arity and convert values with `expect_*`, `TryFrom`, or `as_*` helpers.
3. Run host logic.
4. Convert result back into `RuntimeVal`.
5. Convert expected failures into named `KiroError` values.

This keeps glue explicit and type-checked.

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

## Current Limitations

This crate intentionally stays small. Depending on language evolution, you may later extend it with:

- Struct-like runtime records with typed metadata
- Result/error envelope helpers
- Runtime representations for function refs, pipes, and managed address handles

## Versioning Notes

Because this crate encodes host boundary contracts, changes should be treated carefully:

- Prefer additive changes.
- Keep conversion behavior stable.
- Keep error matching name-based.
- Changing `RuntimeVal`, `KiroError`, or the host function signature requires a new ABI version.
- Coordinate updates with compiler/interpreter changes.

## License

MIT
