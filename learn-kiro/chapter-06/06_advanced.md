# Chapter 6: Advanced Concepts

In this final chapter, we'll explore some of Kiro's most powerful features: **Pointers**, **Concurrency**, and **Host Modules**.

## 1. Pointers (`adr`, `ref`, `deref`)

Kiro provides type-safe pointers (references) to manage memory and shared state.

### Creating Pointers

Use `ref` to create a reference to a variable. The type is `adr <T>`.

```kiro
var x = 10
var ptr = ref x // Type: adr num
```

### Accessing Values (`deref`)

Use `deref` to read or write the value at the pointer's address.

```kiro
deref ptr = 20
print x // 20
```

### Auto-Deref for Structs

For structs, you don't need explicit `deref` to access fields.

```kiro
struct User { name: str }
var u = User { name: "Kiro" }
var u_ptr = ref u
print u_ptr.name // Works directly!
```

## 2. Concurrency (`run`, `pipe`)

Kiro makes asynchronous programming easy with `run` and `pipe`.

### Spawning Tasks

Use `run` to execute a function call in the background.

```kiro
fn worker() {
    print "Working..."
}

run worker()
```

### Communication (Pipes)

Pipes are typed channels for sending data between tasks.

```kiro
var p = pipe str

// Sender
run fn() {
    give p "Message from worker"
}()

// Receiver
var msg = take p
print "Received: " + msg
```

## 3. Host Modules (Rust FFI)

Kiro can call Rust code directly through **Host Modules**. This allows you to leverage the entire Rust ecosystem.

### Declaring a Host Function

Use `rust fn` to declare a function implemented in Rust.

```kiro
// In your .kiro file
rust fn read_file(path: str) -> str!
```

This requires a corresponding Rust implementation in the `header.rs` file of the Kiro runtime.

## Conclusion

Congratulations! You've completed the Kiro Language Tour. You now have the knowledge to build powerful, efficient, and expressive applications with Kiro.

Go forth and code! 🌀
