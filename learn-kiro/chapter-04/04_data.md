# Chapter 4: Data Structures

Organizing complex data.

## 1. Structs

Named collections of fields. Capitalized names required.

```kiro
struct User {
    name: str
    age: num
}

var u = User { name: "Kiro", age: 10 }
print u.name
```

## 2. Lists

Ordered collections of one type.

```kiro
var nums = list num { 1, 2, 3 }
nums push 4
print (nums at 0)
```

## 3. Maps

Key-value pairs.

```kiro
var scores = map str num {
    "Alice" 10,
    "Bob" 5
}
print (scores at "Alice")
```

## Next Step

[Chapter 5: Error Handling](../chapter-05/05_errors.md).
