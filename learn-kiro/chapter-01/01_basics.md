# Chapter 1: The Basics

Welcome! In this first chapter, we'll write your first script and learn about basic types, mutability, and documentation.

## 1. Setup

Kiro is distributed as a single binary. Ensure `kiro` is in your PATH.
Check it:

```bash
kiro --version
```

## 2. Hello, World!

The classic entry point. Create `01_basics.kiro`:

```kiro
print "Hello, Kiro!"
```

Run it:

```bash
kiro 01_basics.kiro
```

## 3. Variables & Types

Kiro variables are **immutable by default**. Use `var` for mutability.

- **`num`**: Numbers (`3.14`, `42`).
- **`str`**: Strings (`"Hello"`).
- **`bool`**: `true` or `false`.
- **`void`**: No value.

```kiro
// Constant (Immutable)
pi = 3.14

// Mutable
var count = 0
count = count + 1
```

## 4. Documentation Comments (`///`)

Kiro supports documentation comments with `///`. These are attached to the following item (function or struct) in the AST.

```kiro
/// Calculates the area of a circle.
fn calculate_area(radius: num) -> num {
    return 3.14 * radius * radius
}
```

Standard comments use `//` and are ignored.

## Next Step

Let's move to [Chapter 2: Control Flow](../chapter-02/02_control_flow.md).
