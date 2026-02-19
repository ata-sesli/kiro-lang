# Chapter 10: Host Modules (Kiro Side)

After implementing Rust glue, Kiro-side usage is intentionally simple. You declare a host function signature and call it as part of normal program flow.

A declaration looks like this:

```kiro
rust fn read_file(path: str) -> str!
```

This declaration is a contract between Kiro code and Rust code. Name, parameter types, and return type must stay aligned with the Rust implementation.

Calling the function is no different from calling a regular Kiro function:

```kiro
error FileNotFound = "File not found"

var content = read_file("data.txt")

on (content == FileNotFound) {
    print "Missing file"
} off {
    print content
}
```

In real projects, host calls are often at system boundaries: filesystem access, networking, cryptography, or integration with existing Rust crates.

## Common Pitfalls

A common issue is signature drift between `.kiro` declaration and Rust glue function. The correct method is to update both sides together and treat mismatches as build blockers.

Another issue is assuming host calls never fail. The correct method is to keep return types failable when appropriate and implement explicit success/failure handling at call sites.

Teams also frequently test only successful paths. The correct method is to create at least one controlled failure case for every host function and validate the produced error behavior.

## Final Step

Move to the [Final Project](../final-project/final_project.md).
