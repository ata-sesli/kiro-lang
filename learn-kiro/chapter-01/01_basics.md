# Chapter 1: The Basics

Kiro programs are easiest to understand when you see them as a sequence of explicit statements. You write values, combine them with operations, and print or return the result. This chapter introduces that rhythm so every later feature feels natural rather than abstract.

Begin with the smallest possible script:

```kiro
print "Hello, Kiro!"
```

Run it with:

```bash
kiro 01_basics.kiro
```

The language executes from top to bottom, so each line does one clear thing. Once this feels intuitive, variables become straightforward.

Assignment in Kiro uses `=`:

```kiro
name = "Ada"
age = 21
is_active = true
```

By default, these bindings are immutable. If you need to change a value later, declare it with `var`:

```kiro
var count = 0
count = count + 1
count = count + 1
print count
```

This design encourages stable values by default and explicit mutability only when necessary.

Kiro's core value types are simple: `num` for numbers, `str` for text, `bool` for true/false logic, and `void` for "no value" return contexts. You will use these types in every chapter.

Comments come in two forms. Use `//` for normal inline notes and `///` for documentation comments attached to the next item:

```kiro
/// Computes area from radius.
fn area(r: num) -> num {
    return 3.14 * r * r
}
```

## Common Pitfalls

New users often try to reassign immutable values and assume assignment is broken. The correct method is to decide upfront whether a variable will change; if it will, declare it with `var`.

Another common issue is mixing values in string output without clear concatenation. The correct method is to build output explicitly with `+` so each conversion step is intentional.

Some users skip environment checks and debug the wrong problem. The correct method is to confirm `kiro --version` first whenever execution behaves unexpectedly.

## Next Step

Continue with [Chapter 2: Control Flow](../chapter-02/02_control_flow.md).
