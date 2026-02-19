# Chapter 5: Error Handling

Reliable programs treat failure as a first-class design concern. Kiro follows this principle by making errors explicit values that must be handled deliberately. This keeps failure behavior visible at both function boundaries and call sites.

You can define domain-specific errors with clear names:

```kiro
error TooSmall = "Value is too small"
error TooBig = "Value is too big"
```

A function that can fail marks its return type with `!`:

```kiro
fn check(val: num) -> str! {
    on (val < 10) {
        return TooSmall
    }

    on (val > 100) {
        return TooBig
    }

    return "Valid: " + val
}
```

At the call site, handle outcomes explicitly:

```kiro
var res = check(55)

on (res == TooSmall) {
    print "Too small"
} off {
    on (res == TooBig) {
        print "Too big"
    } off {
        print "Success: " + res
    }
}
```

This explicit style is more verbose than implicit exceptions, but it scales better because success and failure paths are both visible in code review.

## Common Pitfalls

A frequent problem is defining vague error names that do not describe the situation. The correct method is to model errors around domain meaning, such as `InvalidEmail`, `NotFound`, or `PermissionDenied`.

Another issue is encoding failure as plain success-like strings. The correct method is to return declared error values from failable functions so callers can branch safely and predictably.

Developers also tend to handle only the happy path during prototyping. The correct method is to implement failure branches at the first call site and keep them in place as the program evolves.

## Next Step

Continue with [Chapter 6: Async & `run`](../chapter-06/06_async.md).
