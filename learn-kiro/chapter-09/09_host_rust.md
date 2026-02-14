# Chapter 9: Host Modules (The Rust Side)

Kiro runs on Rust. You can extend it securely.

## 1. The Glue (`header.rs`)

All host functions live in `src/header.rs`.

```rust
use kiro_runtime::{RuntimeVal, KiroError};

pub fn read_file(args: Vec<RuntimeVal>) -> Result<RuntimeVal, KiroError> {
    // 1. Convert Args
    let path = args[0].as_str()?;

    // 2. Do Work
    let content = std::fs::read_to_string(path)
        .map_err(|_| KiroError::new("FileError"))?;

    // 3. Return Value
    Ok(RuntimeVal::Str(content))
}
```

## 2. Type Conversion

- `RuntimeVal::Num(f64)` <-> `num`
- `RuntimeVal::Str(String)` <-> `str`

## Next Step

[Chapter 10: Host Modules (Kiro)](../chapter-10/10_host_kiro.md).
