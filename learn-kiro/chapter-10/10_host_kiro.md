# Chapter 10: Host Modules (The Kiro Side)

Calling Rust from Kiro.

## 1. Declarations (`rust fn`)

Declare the external function signature.

```kiro
rust fn read_file(path: str) -> str!
```

## 2. Using Host Functions

Call them like normal functions.

```kiro
var content = read_file("data.txt")
```

## 3. Error Handling

Host errors are regular Kiro errors.

```kiro
on (content) {
    print content
} error {
    print "Rust said no."
}
```

## Final Step

[Final Project](../final-project/final_project.md).
