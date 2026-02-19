# Chapter 7: Pipes (Channels)

Pipes are Kiro's core primitive for communication between concurrent tasks. They let one part of the program send typed data and another part receive it safely, without relying on implicit shared state.

Create a typed pipe like this:

```kiro
var p = pipe num
```

Now pair a producer with a consumer:

```kiro
fn producer(c: pipe num) {
    give c 10
    give c 20
    close c
}

var p = pipe num
run producer(p)

print (take p)
print (take p)
```

`give` sends values into the channel, `take` receives them, and `close` signals completion when no more values will be sent.

Pipes work best when each channel has a clear purpose. In larger designs, use structs as message payloads so each message carries named fields rather than loosely connected primitives.

## Common Pitfalls

A frequent error is forgetting to close producer-owned pipes. The correct method is to close the channel at the point where production is known to be complete.

Another issue is sending values that do not match the pipe type. The correct method is to treat pipe type as a hard contract and perform conversion before sending.

Blocking bugs often come from taking values with no producer path. The correct method is to design sender and receiver flows together and verify each `take` has a corresponding send or terminal condition.

## Next Step

Continue with [Chapter 8: Pointers](../chapter-08/08_pointers.md).
