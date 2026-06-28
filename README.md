<p align="center">
  <img src="kiro-logo.png" width="400" alt="Kiro Logo">
</p>

# 🌀 Kiro Language

**Kiro** is a modern, experimental programming language designed to explore the intersection of high-level expressivity and low-level performance concepts. It features a dual-execution model (Interpreter + Transpiler to Rust) and introduces unique syntax for flow control and concurrency.

## 🚀 Key Features

- **Safe by Default**: Variables are immutable unless explicitly declared with `var`.
- **Pure Functions**: Strict `pure fn` keyword ensures functions are side-effect free and deterministic.
- **Module System**: Organize code across multiple files with `import` and qualified access (`math.add`).
- **Expressive Loops**: Powerful `loop` constructs with built-in filtering (`on`) and stepping (`per`).
- **Pointers Made Easy**: Simple `ref` and `deref` syntax that compiles to safe Rust concurrency primitives (`Arc<Mutex>`).
- **Async First**: Built-in `run` keyword to easily spawn asynchronous tasks.
- **Host Modules (Rust FFI)**: Powerful, type-safe access to the Rust ecosystem via `rust fn` and a shared runtime contract.
- **Transpiled to Rust**: Code is compiled to robust Rust code, benefiting from Rust's ecosystem and performance.

---

## 📦 Installation & Usage

Value your time? Just clone and run.

### Prerequisites

- [Rust & Cargo](https://rustup.rs/) installed.

### Installing

Compile the Kiro binary and add it to your path:

```bash
cargo build --release
cp target/release/kiro-lang /usr/local/bin/kiro # example
```

### Usage

Kiro is designed for a fast feedback loop. By default, it runs your code through the interpreter for immediate validation, then compiles and executes it via Rust for production performance.

```bash
# Full Pipeline: Interpret -> Compile -> Execute
kiro main.kiro

# Inside a project with kiro.toml, use the manifest entry
kiro
kiro run
kiro build
kiro check

# Fast Validation: Interpret ONLY
kiro check main.kiro

# Production Build: Compile ONLY
kiro build main.kiro

# Format source files in place
kiro fmt main.kiro

# Check formatting without writing changes
kiro fmt --check

# Run *_test.kiro files
kiro test
kiro test tests/

# Skip interpreter validation (e.g. for CI or heavy host modules usage)
kiro main.kiro --no-interpret

# Show compiler logs
kiro main.kiro --verbose
```

Kiro reports common language errors before Rust codegen when possible: wrong argument counts, unknown names, pure-function violations, immutable mutation, bad imported calls, bad pipe/list/map use, and failed `check` guards.

Single-file scripts do not need a manifest. For project commands without an explicit file, Kiro walks upward to the nearest `kiro.toml` and uses `[package].entry`.

---

## 📚 Syntax Guide

### 1. Variables & Types

Kiro uses a "sane default" approach to mutability.

- **Immutable Declaration**: Omit `var` to create a constant.
  ```kiro
  pi = 3.14      // Immutable
  name = "Kiro"  // Immutable
  ```
- **Mutable Declaration**: Use `var` to allow changes.
  ```kiro
  var count = 0
  count = count + 1 // OK
  ```

**Supported Types:**

- `num`: 64-bit Floating point numbers (e.g., `3.14`, `42`).
- `str`: Strings (e.g., `"Hello"`).
- `bool`: Booleans (`true`, `false`).
- `void`: Represents the absence of a value.
- `adr <type>`: Type-safe addresses/pointers.
- `pipe <type>`: Typed channels for asynchronous communication.
- **Strict Typed Collections**: `list <type>` and `map <key> <val>`.
- **Structs**: Custom named types (e.g., `User`).

#### Operators & Expressions

- **Concatenation**: Use `+` to concatenate strings. It supports concatenating strings with any other type (e.g., `"Result: " + true` or `10 + " items"`).
- **Deep Equality**: `==` and `!=` work deeply for Structs, Lists, and Maps.
- **Size**: Use `len` to get the length of strings and collections (`len my_list`).

### 2. Module System (Separate Files)

Kiro supports code modularization with flat sibling modules. In V1, `import math` resolves to `math.kiro` beside the importing file. Dotted imports, path imports, aliases, and nested module trees are intentionally not part of the project layout yet.

```text
my_app/
  kiro.toml
  main.kiro
  math.kiro
  tools.kiro
  tools.rs
```

`kiro.toml` is optional. Use it when a directory should behave like a project:

```toml
[package]
name = "my_app"
entry = "main.kiro"

[dependencies]
```

**`math.kiro`**:

```kiro
fn add(a: num, b: num) {
    return a + b
}
```

**`main.kiro`**:

```kiro
import math

fn main() {
    var result = math.add(10, 20)
    print result
}
main()
```

- **Qualified Access**: Use `module.member` to access exported functions or variables.
- **Embedded Standard Library**: Kiro comes with a built-in standard library (e.g., `std_fs`, `std_net`, `std_env`) embedded directly in the binary for zero-configuration portability.

### 3. Structs & Mutation

#### Definition & Initialization

Struct names **must** start with a Capital letter. Fields use lowercase.

```kiro
struct User {
    name: str
    age: num
}

var u = User { name: "Kiro", age: 1 }
```

#### Field Mutation

Fields can be mutated if the struct instance is declared with `var`.

```kiro
var player = User { name: "Hero", age: 20 }
player.age = 21 // OK!
```

### 4. Collections

Kiro features strictly typed lists and maps with command-style operations.

#### Lists

```kiro
var numbers = list num { 10, 20, 30 }
print numbers at 0
numbers push 40
```

#### Maps

```kiro
var scores = map str num { "Alice" 100, "Bob" 90 }
print scores at "Alice"
```

### 5. Control Flow

#### Conditionals (`on` / `off`)

```kiro
on (x > 10) {
    print "High"
} off {
    print "Low"
}
```

#### Loops

- **While**: `loop on (cond) { ... }`
- **Iterator**: `loop i in 0..10 { ... }`
- **Advanced**: `loop x in list per 2 on (x > 5) { ... }`

#### Control Signals

Kiro supports standard control flow signals within functions and loops:

- `return value`: Exit function with a value.
- `break`: Exit the innermost loop.
- `continue`: Skip to the next iteration of the loop.

#### Runtime Checks

Use `check` when a condition must be true for the program to continue. A failed check stops the program with a Kiro diagnostic.

```kiro
check user.age > 0, "age must be positive"
```

`check` is also the seed of Kiro's tiny test story. Files named `*_test.kiro` are ordinary Kiro programs, and `kiro test` runs them as isolated compiled programs. A failed `check` marks that file as failed.

**`math_test.kiro`**:

```kiro
import math

check math.add(2, 3) == 5, "add should sum numbers"
```

### 6. Pointers & Memory (`ref` / `deref`)

Kiro abstractly manages memory while giving you pointer-like behavior. References are thread-safe (`Arc<Mutex<T>>`).

#### Typed Pointers & Lazy Initialization

Pointers are declared with `adr <type>`. If initialized without a value, they are "lazy" (empty).

```kiro
var x = 10
var ptr = ref x
deref ptr = 20 // Mutate 'x' via pointer
print x // Returns 20

var lazy_ptr = adr str // Uninitialized pointer
lazy_ptr = ref "Now I exist"
```

#### Opaque Pointers (`adr void`)

Use `adr void` to store a raw memory address without type information. This is useful for passing handles or legacy pointers.

```kiro
var x = 10
var raw = adr void
raw = ref x // Address is extracted as a usize
print raw   // Prints the raw address (e.g. 5829058176)
```

**Auto-Deref**: Struct fields can be accessed directly on typed references: `ptr.name` instead of `(deref ptr).name`.

### 7. Concurrency & Pipes

#### Async Execution

Use `run` to spawn a fire-and-forget background task. The task starts independently and may be cancelled when the program exits unless you synchronize explicitly.

```kiro
run worker(id)
```

Use `rest` inside long-running tasks to give other tasks a chance to continue.

```kiro
rest
```

#### Pipes (Channels)

Channels for safe communication between tasks. Pipes are typed: `pipe <type>`.

```kiro
var p = pipe num // Create a channel for numbers
give p 42
var x = take p
```

- `give p val`: Send value.
- `take p`: Receive value (awaits).
- `close p`: Close the channel.
- `pipe void`: A signal-only channel.

Use pipes to wait for fire-and-forget tasks when completion matters:

```kiro
var done = pipe void
run worker(done)
take done
```

### 8. Functions

Functions are declared with `fn`. Arguments and return types are explicit.

```kiro
fn add(a: num, b: num) -> num {
    return a + b
}

fn do_nothing() -> void {
    print "Working..."
    return // Optional for void
}

add(10, 20)
do_nothing()
```

- **Void Functions**: If the return type is omitted, it defaults to `void`.
- **Explicit Return**: Use `-> type` to specify the return value.

#### Pure Functions

Use the `pure` keyword to declare side-effect free functions. Pure functions are enforced at both the Interpreter and Transpiler levels.

```kiro
pure fn add(a: num, b: num) -> num {
    return a + b
}

fn main() {
    print add(10, 20)
}
```

- **Strict Constraints:**

1. **No IO**: `print`, `give`, and `take` are forbidden inside `pure` functions.
2. **Immutable Arguments**: You cannot pass a mutable variable (`var x`) to a pure function. Only literals or immutable variables are allowed.
3. **No Side Effects**: Pure functions cannot mutate data outside their own local scope.

### 9. Error Handling

Kiro provides a structured error handling system integrated into its core control flow.

#### Error Definitions

Define custom, type-only errors with optional descriptions. Error names must start with a Capital letter.

```kiro
error NotFound = "File not found"
error PermissionDenied = "Access denied"
```

#### Failable Functions (`!`)

Functions that can return an error must be marked with the `!` suffix on their return type.

```kiro
fn maybe_fail(code: num) -> str! {
    on (code == 1) {
        return NotFound
    }
    return "Success!"
}
```

- **Success**: Returns the value (automatically wrapped in `Ok`).
- **Failure**: Returns the error type (automatically wrapped in `Err`).

#### Handling Errors (`on` / `error`)

Use the `on` statement to branch based on success or failure.

```kiro
var result = maybe_fail(1)

on (result) {
    // Smart Casting: 'result' is shadowed here as a 'str'
    print "Success: " + result
} error PermissionDenied {
    print "Error: Access denied."
} error NotFound {
    print "Error: File was not found."
} error {
    // Catch-all block
    print "An unknown error occurred."
}
```

- **Smart Casting**: Inside the success block of an `on` statement, failable variables are automatically unwrapped and shadowed by their successful value.
- **Implicit Propagation**: If an `error` block doesn't explicitly return or handle the error, the error is implicitly re-thrown to the caller.
- **Catch-all**: A bare `error { ... }` catches any unhandled error types.

### 10. Host Modules (Rust FFI)

Kiro provides zero-friction access to the Rust ecosystem. You can call arbitrary Rust code without introducing unsafe or complex FFI signatures in your `.kiro` files.

#### 1. Rust Function Declaration

Declare external functions using the `rust` keyword. These functions are implemented in Rust but called like any other Kiro function.

```kiro
error NotFound = "File not found"

// Explicit return types are required for rust fn
rust fn read_file(path: str) -> str!
```

#### 2. Rust Glue Layer (`mylib.rs`)

The logic lives in an adjacent Rust glue file. A user module `mylib.kiro` uses `mylib.rs`; `main.kiro` uses `main.rs` if it declares `rust fn`. Standard modules keep embedded headers. There is no `native/` fallback in V1. Kiro scripts and Rust glue communicate through the `kiro_runtime` ABI.

- **Glue implementation**: Use the `kiro_runtime` crate to convert types between Kiro and Rust.
- **ABI v1**: Host functions are async and return `kiro_runtime::HostResult`.
- **Missing Glue**: If a module declares `rust fn` but the matching `.rs` file is absent, Kiro reports a compile diagnostic before Rust build.

```rust
// Example Glue Implementation
use kiro_runtime::{HostResult, KiroError, RuntimeVal};

pub async fn read_file(args: Vec<RuntimeVal>) -> HostResult {
    RuntimeVal::expect_arity(&args, 1, "read_file")?;
    let path = RuntimeVal::expect_arg(&args, 0, "read_file")?.as_str()?;

    match tokio::fs::read_to_string(path).await {
        Ok(c) => Ok(RuntimeVal::from(c)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err(KiroError::message("NotFound", path.to_string()))
        }
        Err(e) => Err(KiroError::message("IoError", e.to_string())),
    }
}
```

- **Interpreter Behavior**: The interpreter includes a **Simulator**. It does not execute Rust glue, but it validates the call (argument count/types) and returns a mock value (e.g., an empty string or `0.0`) so the script can proceed with logical validation without crashing.
- **Compiler parity**: Results from Rust are strictly type-checked and integrated into Kiro's error handling (`on/error`).
- **Error Matching**: Kiro matches host errors by name (`NotFound`). Optional host error messages are kept for diagnostics.

---

## 🏗️ Architecture

Kiro uses a **Double Pass** system:

1.  **Interpreter (`src/interpreter/`)**:
    - Walks the AST and maintains a runtime environment.
    - **Simulator**: Provides mock responses for `rust fn` to enable validation without Rust compilation.
    - Recursively loads and executes imported modules in isolation.
2.  **Transpiler (`src/compiler/`)**:
    - Converts Kiro to idiomatic **Rust**.
    - **Recursive Build**: The transpiler identifies dependencies and compiles them as Rust modules (`pub mod {name}`).
    - Hoists struct definitions and imports to ensure valid Rust output.

## 🛠️ Repository Structure

- `src/grammar/`: Language rules and parser (Rust Sitter).
- `src/interpreter/`: Recursive execution engine and value representations.
- `src/compiler/`: Rust code generation logic.
- `src/project.rs`: Kiro project discovery and manifest entry resolution.
- `src/kiro_std/`: Standard library source code (Embedded in binary).
- `src/build_manager.rs`: Cargo project lifecycle management.
- `main.kiro`: Entry point script.

---

_Built with ❤️ in Rust._
