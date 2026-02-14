# Chapter 2: Control Flow

Now let's control the execution flow.

## 1. Conditionals (`on` / `off`)

Instead of `if/else`, Kiro uses `on` (when true) and `off` (when false).

```kiro
score = 85
on (score > 90) {
    print "Grade A"
} off {
    print "Grade B or lower"
}
```

## 2. Loops (While)

Basic repetition as long as a condition holds.

```kiro
var i = 0
loop on (i < 3) {
    print i
    i = i + 1
}
```

## 3. Ranged/Iterator Loops

Loop over ranges or collections.

```kiro
// Range
loop x in 0..5 {
    print x
}
```

```kiro
// Even numbers only (using per)
loop n in 0..10 per 2 {
    print n
}
```

Kiro loops support filtering (`on`) and stepping (`per`).

## 4. Control Signals

- **`break`**: Exit loop.
- **`continue`**: Skip iteration.
- **`return`**: Exit function.

## Next Step

[Chapter 3: Functions](../chapter-03/03_functions.md).
