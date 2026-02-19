# Chapter 6 (Optional): Advanced Concepts

This optional chapter connects features you already learned into one coherent design style. Instead of treating pointers, concurrency, and host functions as isolated tools, you can view them as complementary mechanisms for ownership, coordination, and capability extension.

Pointers (`adr`, `ref`, `deref`) give reference semantics when sharing or mutating existing values is necessary:

```kiro
var x = 10
var p = ref x
print (deref p)
deref p = 20
print x
```

Concurrency (`run`) helps independent work proceed without blocking, while pipes keep communication explicit:

```kiro
fn worker(out: pipe str) {
    give out "done"
}

var ch = pipe str
run worker(ch)
print (take ch)
```

Host declarations (`rust fn`) extend Kiro with Rust implementations where system-level integration or specialized libraries are needed:

```kiro
rust fn read_file(path: str) -> str!
```

In practice, robust systems usually combine these patterns: pure functions for logic, structs for data shape, pipes for concurrency boundaries, and host functions for external capabilities.

## Common Pitfalls

A common advanced-level mistake is combining every feature at once during initial implementation. The correct method is to introduce one abstraction at a time and validate behavior incrementally.

Another issue is choosing shared pointer mutation where message passing would be simpler. The correct method is to default to pipes for cross-task coordination and reserve pointer mutation for tightly scoped cases.

Host integration also fails when declarations and glue drift apart. The correct method is to keep Kiro signatures and Rust implementations synchronized as part of normal review.

## Next Step

Continue with [Chapter 7: Pipes](../chapter-07/07_pipes.md) if you have not completed it yet.
