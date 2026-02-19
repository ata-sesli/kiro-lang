# Chapter 2: Control Flow

A useful program does not just execute lines blindly. It branches when conditions change and repeats work when input grows. In Kiro, this behavior is expressed with readable control keywords that make intent obvious.

Conditional branching uses `on` and `off`:

```kiro
score = 85

on (score >= 90) {
    print "Grade A"
} off {
    print "Not A"
}
```

Read this as: run the first block when the condition is true, otherwise run the second block. This direct style keeps condition logic easy to scan.

For repeated execution based on a condition, use `loop on (...)`:

```kiro
var i = 0
loop on (i < 3) {
    print i
    i = i + 1
}
```

When you already know a numeric interval, range loops are cleaner:

```kiro
loop x in 0..5 {
    print x
}
```

You can step through ranges with `per`:

```kiro
loop n in 0..10 per 2 {
    print n
}
```

And you can filter iterations with `on` inside range loops:

```kiro
loop n in 0..10 on (n > 5) {
    print n
}
```

Control keywords complete the model: `break` exits a loop, `continue` skips to the next iteration, and `return` exits a function.

## Common Pitfalls

Many loop bugs come from forgetting to update the loop variable in `loop on (...)`. The correct method is to place state updates near the end of each iteration so termination is guaranteed.

Another frequent issue is writing conditions without parentheses in complex expressions. The correct method is to keep conditions explicit and parenthesized for both readability and correctness.

Developers also overuse nested branches. The correct method is to keep branch blocks short and extract repeated logic into functions.

## Next Step

Continue with [Chapter 3: Functions & Modules](../chapter-03/03_functions.md).
