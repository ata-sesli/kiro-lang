# Chapter 4: Data Structures

Once logic is clear, data modeling becomes the next major design step. Kiro gives you three core tools: structs for shaped records, lists for ordered sequences, and maps for key-value lookup. Choosing the right structure makes code simpler before optimization is ever needed.

A struct represents one entity with named fields:

```kiro
struct User {
    name: str
    age: num
}

var user = User { name: "Kiro", age: 10 }
print user.name
```

Use structs whenever fields belong together conceptually.

A list stores ordered values of one type:

```kiro
var nums = list num { 1, 2, 3 }
nums push 4
print (nums at 0)
print nums
```

A map stores values by key, useful for lookups:

```kiro
var scores = map str num {
    "Alice" 10,
    "Bob" 5
}

print (scores at "Alice")
```

The practical rule is simple: choose `struct` for one thing with many fields, `list` for many items in order, and `map` for fast access by key.

## Common Pitfalls

A common data-modeling mistake is forcing everything into lists. The correct method is to promote related fields into a struct so field names communicate intent.

Another issue is mixing value types inside typed collections. The correct method is to treat collection element types as strict contracts and convert data before insertion.

Map lookups often fail because keys were never inserted or are inconsistent in format. The correct method is to normalize key creation (for example, casing and spacing) at insertion time and reuse that normalization at lookup time.

## Next Step

Continue with [Chapter 5: Error Handling](../chapter-05/05_errors.md).
