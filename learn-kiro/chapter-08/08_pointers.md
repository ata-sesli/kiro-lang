# Chapter 8: Pointers

Managing memory references safely.

## 1. References (`ref`)

Create a pointer of type `adr <T>`.

```kiro
var x = 10
var ptr = ref x
```

## 2. Dereferencing (`deref`)

Access the value.

```kiro
print (deref ptr)

// Mutation via pointer
deref ptr = 20
print x // 20
```

## 3. Auto-Deref

Struct fields can be accessed directly on pointers.

```kiro
var u_ptr = ref user
print u_ptr.name
```

## Next Step

[Chapter 9: Host Modules (Rust)](../chapter-09/09_host_rust.md).
