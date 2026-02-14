# Chapter 5: Error Handling

Kiro uses explicit error types and handling blocks.

## 1. Defining Errors

```kiro
error TooSmall = "Value is too small"
error TooBig = "Value is too big"
```

## 2. Failable Functions (`!`)

Append `!` to return type.

```kiro
fn check(val: num) -> str! {
    // If error, return the error type (as a value)
    on (val < 10) {
        return TooSmall
    }

    // If successful, return the value
    return "Valid: " + val
}
```

## 3. Handling Errors

Use `on` for success, `error` for failure.

```kiro
var res = check(-1)

on (res == TooSmall) {
    print "Item too small"
} off {
    on (res == TooBig) {
        print "Item too big"
    } off {
        print "Success: " + res
    }
}
```

Since errors are values, you can compare them directly.

## Next Step

[Chapter 6: Async & Run](../chapter-06/06_async.md).
