# Chapter 6: Async & `run`

Concurrency in Kiro begins with one idea: start independent work without blocking the current flow. The `run` keyword gives you that capability with minimal syntax.

A simple example shows the model:

```kiro
fn worker() {
    print "Working in background..."
}

run worker()
print "Main flow continues"
```

`run worker()` starts background work and immediately returns control to the next statement. This means output order between worker and main flow is not guaranteed.

Arguments are passed normally:

```kiro
fn log(msg: str) {
    print "Log: " + msg
}

run log("Async message")
```

As your programs grow, asynchronous execution should be paired with clear communication boundaries, which is exactly what pipes provide in the next chapter.

## Common Pitfalls

A frequent mistake is assuming statements after `run` wait for completion. The correct method is to treat `run` as non-blocking and design synchronization intentionally.

Another issue is mixing shared mutable state into multiple concurrent tasks too early. The correct method is to prefer message passing and narrow ownership so task behavior stays understandable.

Developers also spawn tasks without lifecycle planning. The correct method is to define when work starts, how results are collected, and when channels close.

## Next Step

Continue with [Chapter 7: Pipes](../chapter-07/07_pipes.md).
