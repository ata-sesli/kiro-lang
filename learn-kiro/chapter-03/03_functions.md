# Chapter 3: Functions & Modules

As programs grow, structure matters more than syntax. Functions let you name behavior once and reuse it everywhere. Modules let you group related behavior into files that can be imported where needed.

A Kiro function declares inputs and output type explicitly:

```kiro
fn add(a: num, b: num) -> num {
    return a + b
}

print add(2, 3)
```

That signature is a contract. Readers immediately understand what the function accepts and what it returns.

When branching inside a function, keep return types consistent:

```kiro
fn clamp_to_zero(x: num) -> num {
    on (x < 0) {
        return 0
    }
    return x
}
```

Kiro also supports `pure fn` for deterministic, side-effect-free logic:

```kiro
pure fn square(x: num) -> num {
    return x * x
}
```

Use pure functions for computation and transformation. Keep I/O and side effects in non-pure functions.

Function references are now supported for named pure functions. This lets you pass behavior around without closures or lambdas.

A function reference is created with `ref` and a function name:

```kiro
pure fn inc(x: num) -> num { return x + 1 }

f = ref inc
print f(10)
```

You can type function parameters with `fn(...) -> ...`:

```kiro
pure fn apply(x: num, f: fn(num) -> num) -> num {
    return f(x)
}

print apply(5, ref inc)
```

You can also return function references:

```kiro
pure fn dec(x: num) -> num { return x - 1 }

pure fn pick(up: bool) -> fn(num) -> num {
    on (up) {
        return ref inc
    } off {
        return ref dec
    }
}
```

Current rule: function references are for named pure functions only. This keeps the model simple and predictable.

Modules are just `.kiro` files. Import by module name without the extension.

`mylib.kiro`:

```kiro
pure fn pi() -> num {
    return 3.14
}
```

`main.kiro`:

```kiro
import mylib
print mylib.pi()
```

This module boundary is the foundation for larger projects.

## Common Pitfalls

A common error is drifting return types across branches. The correct method is to decide the return type first and enforce it in every path.

Another issue is writing overly large functions with mixed responsibilities. The correct method is to split logic into small, single-purpose functions and compose them.

Function references can fail if you try to reference impure functions. The correct method is to use `ref` with named pure functions and match parameter/return signatures exactly.

Module organization also breaks when imports become circular. The correct method is to define clear ownership boundaries so each module has one direction of dependency.

## Next Step

Continue with [Chapter 4: Data Structures](../chapter-04/04_data.md).
