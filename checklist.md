Kiro language feature checklist (current status)

1. Source files can be executed directly (script mode) — Supported
How: `kiro file.kiro` runs via interpreter/check pipeline.

2. Compiles to a native binary — Supported
How: transpiles to Rust and builds an executable.

3. REPL / interactive prompt exists — Not Supported

4. Package manager exists — Supported
How: `kiro create/add/remove` with `kiro.toml` and Cargo integration.

5. Standard library ships with the compiler/interpreter — Supported
How: embedded `src/kiro_std/*` assets.

6. Modules can be split across files and imported — Supported
How: `import module_name` resolves `.kiro` modules.

7. Namespaces / qualified access (module.member) — Supported
How: module exports are accessed via `mod.fn` / `mod.value`.

8. Cyclic imports are detected and reported nicely — Planning

9. Versioned dependency resolution — Planning

10. Lockfile support for reproducible builds — Planning

11. Static typing (types checked before running) — Supported
How: typed signatures + compile-time Rust type checks.

12. Dynamic typing (values carry types at runtime) — Supported
How: interpreter uses `RuntimeVal` runtime typing.

13. Type inference (omit some type annotations) — Supported
How: variable declarations infer from assigned expression.

14. User-defined types (struct/class) — Supported
How: `struct Name { ... }`.

15. Algebraic data types (sum types / enums with payloads) — Not Supported

16. Generics (parametric polymorphism) — Not Supported

17. Interfaces / traits / protocols — Not Supported

18. Nullable/optional type (T?) — Not Supported

19. Union types (A or B) — Not Supported

20. Type casting / conversion syntax — Planning

21. Immutable-by-default variables — Supported
How: `x = ...` creates immutable bindings.

22. Mutable variables via explicit keyword — Supported
How: `var x = ...`.

23. Constants (compile-time constants) — Not Supported

24. Block scope (variables scoped to { } blocks) — Supported
How: block execution uses scoped environments.

25. First-class functions (functions are values) — Supported
How: named function refs via `ref foo`.

26. Closures (functions can capture outer variables) — Not Supported

27. Higher-order functions (accept/return functions) — Supported
How: function type syntax `fn(...) -> ...` and function refs.

28. Tail-call optimization — Not Supported

29. Pattern matching — Not Supported

30. Macros / metaprogramming — Not Supported

31. If/else style branching — Not Supported

32. Alternative branching syntax (e.g., on/off) — Supported
How: `on (...) { ... } off { ... }`.

33. While loops — Supported
How: `loop on (cond) { ... }`.

34. For-each loops over ranges/collections — Supported
How: `loop x in iterable { ... }`.

35. Break/continue in loops — Supported
How: `break` and `continue`.

36. Functions with explicit return types — Supported
How: `fn name(...) -> type`.

37. Void/no-return functions — Supported
How: `-> void`.

38. Recursion is supported — Supported
How: functions can call themselves.

39. Overloading (same name, different params) — Not Supported

40. Default parameter values — Not Supported

41. Built-in list/array type — Supported
How: `list type { ... }`.

42. Built-in map/dictionary type — Supported
How: `map key_type value_type { ... }`.

43. Built-in set type — Not Supported

44. Strings are a first-class type — Supported
How: `str` type and string literals.

45. Byte array / buffer type — Not Supported

46. References/pointers exist (safe) — Supported
How: `adr`, `ref`, `deref` model.

47. Unsafe memory access exists — Not Supported

48. Concurrency primitives built in (threads/tasks) — Supported
How: `run` schedules async task execution.

49. Message passing / channels exist — Supported
How: `pipe`, `give`, `take`, `close`; bounded pipe via `pipe T N`.

50. Error handling is structured (typed errors / result type) — Supported
How: `error` declarations, failable returns `!`, explicit branching.
