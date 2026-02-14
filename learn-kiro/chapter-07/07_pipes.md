# Chapter 7: Pipes (Channels)

Tasks need to communicate. Kiro uses **Pipes** (typed channels).

## 1. Creating a Pipe

```kiro
fn producer(c: pipe num) {
    give c 10
    give c 20
}

var p = pipe num
run producer(p)
```

## 2. Sending (`give`)

Send a value into a pipe.

```kiro
give p "Hello"
```

## 3. Receiving (`take`)

Receive a value (blocks until available).

```kiro
var msg = take p
```

## 4. Closing (`close`)

Close a pipe when done.

```kiro
close p
```

## Next Step

[Chapter 8: Pointers](../chapter-08/08_pointers.md).
