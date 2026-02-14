# Chapter 6: Async & Go (`run` keyword)

Concurrency in Kiro is built-in. This chapter explores how to run tasks in the background.

## 1. The `run` Keyword

Use `run` to execute a function asynchronously. This is similar to the `go` keyword in Go.

```kiro
fn worker() {
    print "Working in background..."
}

run worker()
print "Main thread continues..."
```

## 2. Passing Arguments

You can pass arguments to async functions just like normal ones.

```kiro
fn log(msg: str) {
    print "Log: " + msg
}

run log("Async Message")
```

## Next Step

[Chapter 7: Pipes (Channels)](../chapter-07/07_pipes.md).
