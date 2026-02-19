# Chapter 8: Pointers

Pointers in Kiro let you reference existing values directly when ownership or mutation patterns require it. They are explicit, typed, and intentionally visible in code, which keeps reference-heavy logic understandable. Kiro pointers are managed references, not raw memory pointers.

Create a pointer with `ref`:

```kiro
var x = 10
var ptr = ref x
```

The pointer type is `adr num`, meaning "managed address handle to a number".

Read and write through the pointer using `deref`:

```kiro
print (deref ptr)
deref ptr = 20
```

`ref` gives you a managed pointer value. In the current model, treat pointer mutation as mutation through that handle, not as an aliasing guarantee for the original variable binding.

When you need an untyped handle, use `adr void`. This is an opaque managed handle, not a raw numeric memory address:

```kiro
var x = 42
var p = ref x

var h = adr void
h = p

print h        // handle display
```

`adr void` is useful as an opaque transport/identity handle. Keep typed access through `adr T` pointers when you need `deref` reads/writes.

For structs, field access is ergonomic because pointer field use is auto-dereferenced:

```kiro
struct User {
    name: str
}

var user = User { name: "Kiro" }
var up = ref user
print up.name
```

Use pointers when reference semantics simplify your design. If plain values already express the behavior clearly, keep the simpler option.

## Common Pitfalls

A common issue is introducing pointers prematurely for simple data flow. The correct method is to start with plain values and only adopt pointers when shared mutation is genuinely needed.

Another issue is mutating shared data through pointers across concurrent tasks without coordination. The correct method is to define clear ownership boundaries or communicate changes through pipes.

Pointer logic also becomes fragile when dereference operations are scattered across large functions. The correct method is to keep pointer manipulation localized and wrapped in small helper functions.

## Next Step

Continue with [Chapter 9: Host Modules (Rust Side)](../chapter-09/09_host_rust.md).
