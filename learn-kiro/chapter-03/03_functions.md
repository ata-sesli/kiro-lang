# Chapter 3: Functions & Modules

Structuring code for reuse.

## 1. Functions

Defined with `fn`. Arguments and return types are explicit.

```kiro
fn add(a: num, b: num) -> num {
    return a + b
}
```

## 2. Pure Functions (`pure fn`)

Guaranteed to have no side effects (no global mutation, no IO).

```kiro
pure fn square(x: num) -> num {
    return x * x
}
```

## 3. Modules

Every `.kiro` file is a module. Import them by filename (without extension).

### `mylib.kiro`

```kiro
pure fn pi() -> num { return 3.14 }
```

### `main.kiro`

```kiro
import mylib
print mylib.pi()
```

## Next Step

[Chapter 4: Data Structures](../chapter-04/04_data.md).
