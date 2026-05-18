# Kiro Roadmap

Kiro is a small native-orchestration language. It is not trying to become an
all-in-one language. The core language should stay easy to learn, while real
capability comes from native host modules backed by Rust, and possibly C, C++,
or Zig later.

The goal is to make common native-powered work feel as direct as Python or
JavaScript, but lighter, faster to deploy, and closer to systems libraries.

## Product Thesis

Kiro should be used when the hard work already belongs in native code, but the
user should not have to fight memory management, borrow checking, async
plumbing, or verbose host setup just to orchestrate that work.

Good target domains:

- Workflow orchestration
- Automation and scripting
- Tiny backends
- ML and data workflows through native engines
- GUI behavior scripting through native GUI modules
- Rust application embedding
- Lightweight glue around files, network, environment, time, and processes

Kiro should grow through official modules, not through domain-specific syntax.

## Current Foundation

### Language Core

Kiro already has the basic language surface needed for small real programs:

- Variables and assignment
- Immutable-by-default bindings
- `num`, `str`, `bool`, and `void`
- Structs
- Lists and maps
- Field access and field mutation
- Arithmetic and comparisons
- Ranges
- `on` / `off` conditionals
- `loop on`, iterator loops, `per`, and loop filters
- `break`, `continue`, and `return`
- Normal `fn`
- `pure fn`
- `rust fn`
- Function types and pure function references
- Modules through `import`
- Qualified module calls such as `math.add(...)`

### Async And Workflow Model

Kiro has an async-first execution model:

- Normal functions compile to Rust `async fn`
- `run` starts fire-and-forget tasks
- Pipes provide explicit synchronization
- `give`, `take`, and `close` support typed message passing
- `pipe void` supports signal-style synchronization
- `rest` gives other running tasks a chance to continue

Recent stabilization decisions:

- `run worker()` remains fire-and-forget.
- Programs should use pipes when they need to wait for tasks.
- `rest` is the only cooperative scheduling keyword.
- Impure recursion is rejected for now.
- Pure recursion remains allowed.

### Errors, Checks, And Diagnostics

Kiro now has a clearer language-shaped error story:

- User-defined errors
- Failable functions with `!`
- Runtime `check condition`
- Runtime `check condition, "message"`
- Kiro diagnostics for common compile-time mistakes
- Runtime diagnostics for failed checks and known runtime failures

Recent diagnostics work covers:

- Wrong argument count
- Wrong argument type
- Wrong return type
- Missing return value
- Unknown variable
- Unknown function
- Unknown imported function
- Pure-function violation
- Immutable mutation
- Bad pipe/list/map use
- Unknown struct field
- Invalid `run` target
- Invalid `adr void` deref
- Failed `check`

### Native Boundary

Kiro already has the beginning of its most important feature: host modules.

- `rust fn` declares native-backed functions.
- `kiro_runtime` provides shared runtime values and errors.
- The compiler generates Rust glue around Kiro modules.
- The interpreter can simulate or call registered host functions.
- Embedded standard modules exist for filesystem, environment, time, I/O, and
  network access.

Current embedded standard modules:

- `std_fs`
- `std_env`
- `std_time`
- `std_io`
- `std_net`

### Tooling And Learning

Kiro already has:

- `kiro file.kiro`
- `kiro check file.kiro`
- `kiro build file.kiro`
- `--no-interpret`
- `--verbose`
- VS Code syntax support
- Hover documentation
- A compact `learn-kiro` course
- A final async task manager project

The current learning path is intentionally small enough for an experienced
programmer to understand the core concepts quickly.

## Near-Term Roadmap

### 1. Finish Bare-Language Stability

The bare language should feel predictable before adding more domains.

- Keep the current feature set small.
- Avoid enums, ADTs, traits, interfaces, closures, macros, pattern matching,
  overloading, defaults, sets, byte buffers, nullable types, union types, and
  custom generics.
- Make existing features consistent instead of adding new ones.
- Lock down async semantics around `run`, pipes, `rest`, and program exit.
- Keep pure functions deterministic and simple.

### 2. Improve Source Diagnostics

Diagnostics are one of the biggest quality multipliers for Kiro.

- Add source spans with file, line, and column.
- Attach labels to the exact expression or statement when possible.
- Add short help messages for common mistakes.
- Avoid leaking generated Rust paths and Rust compiler errors for ordinary Kiro
  mistakes.
- Add typo suggestions for unknown names when practical.

Target style:

```text
[KIRO2003:compile] Wrong argument count for 'add': expected 2, got 1.
help: add expects (num, num)
```

### 3. Strengthen The Host Module Contract

This is the core product surface.

- Document the `rust fn` ABI clearly.
- Stabilize `kiro_runtime`.
- Make host error conversion simple and consistent.
- Improve native handle patterns such as `adr void` and typed handles.
- Define versioning expectations for host modules.
- Make missing glue errors clear.
- Make examples small enough to copy.

The long-term promise depends on native modules feeling boring and reliable.

### 4. Add A Formatter

Kiro needs one official style.

- Add `kiro fmt`.
- Format examples and learn material consistently.
- Keep formatting simple and unsurprising.
- Avoid style debates by making the formatter authoritative.

### 5. Clarify Project Layout

Real projects need predictable structure.

- Define where `.kiro` files live.
- Define how local modules resolve.
- Define where native glue lives.
- Decide whether Kiro needs a small `kiro.toml`.
- Keep single-file scripts frictionless.

This should not become a large package manager yet.

### 6. Add A Tiny Test Story

Kiro already has `check`, which can become the seed of testing.

Possible first version:

- `kiro test`
- Test files or test functions by convention
- Failed `check` marks the test as failed
- No large framework at first

The goal is simple confidence, not a full testing ecosystem.

## Mid-Term Roadmap

### 1. Official Domain Modules

Kiro should become more powerful through official native-backed modules.

Each domain should have one blessed path, not new syntax.

Priority candidates:

- `std_process` for running commands
- `std_json` for structured data
- `std_http` or `web` for tiny backends
- `db` or `postgres` for database workflows
- `ml` for model loading, inference, and training orchestration
- `gui` for lightweight native GUI behavior scripting

These modules should share the same design taste:

- Small function surface
- Clear handles
- Kiro-shaped errors
- No hidden magic
- Works well with `check`, `run`, and pipes

### 2. Embedded Engine Maturity

Kiro should be easy to embed inside Rust applications.

- Stabilize the engine API.
- Support loading modules from custom sources.
- Make registered host functions ergonomic.
- Keep `run_main` and `call_fn` predictable.
- Provide examples for embedding Kiro in real Rust apps.

### 3. Workflow Reliability

Kiro's strongest identity is orchestration, so workflow behavior should become
very clear.

- Document task lifecycle rules.
- Clarify cancellation behavior.
- Make pipe ownership patterns easy to teach.
- Add examples for fan-out/fan-in, retries, timeouts, and worker pools.
- Consider whether durable workflows belong in modules rather than the core
  language.

### 4. Better Standard Library Coverage

The standard library should cover common orchestration tasks without making the
language bigger.

Useful additions:

- Process execution
- Paths
- JSON
- HTTP client
- Basic date/time formatting
- Temporary files
- Environment parsing
- Logging

## Long-Term Roadmap

### 1. Native Ecosystem Beyond Rust

Rust should remain the first-class host path, but the larger idea can extend to
other native ecosystems.

Possible future targets:

- C ABI modules
- C++ modules
- Zig modules

This should happen only after the Rust host boundary feels stable.

### 2. Deployment Story

Kiro should be lightweight to ship.

Long-term goals:

- Build scripts into standalone native artifacts where practical.
- Keep startup fast.
- Avoid Python-style environment friction.
- Make native dependencies explicit and reproducible.

### 3. Compatibility And Versioning

Once real users depend on Kiro, the language needs stability promises.

- Version the language.
- Version the runtime ABI.
- Track breaking changes.
- Add migration notes.
- Keep experimental features clearly marked.

### 4. Documentation As Product

The docs should reinforce that Kiro is easy because the concept count is low.

- Keep `learn-kiro` short.
- Add domain guides once official modules exist.
- Show real workflows, not toy-only examples.
- Explain when not to use Kiro.
- Keep the public message focused on native orchestration.

## Non-Goals

Kiro should not try to become:

- A Python replacement for every domain
- A JavaScript replacement for browsers
- A Rust replacement
- A full ML tensor language
- A full GUI framework language
- A huge application language
- A language with many competing ways to do the same thing

Kiro should also continue avoiding:

- Enums and ADTs
- Traits and interfaces
- Closures
- Macros
- Pattern matching
- Operator overloading
- Default parameters
- Sets
- Byte buffers in the core language
- Nullable and union types
- Custom generics

These omissions are part of the taste. They keep Kiro learnable.

## Success Criteria

Kiro is on track if:

- An experienced programmer can learn the core in 1-2 hours.
- Small workflow programs are shorter and clearer than equivalent Rust.
- Native modules feel easier to use than writing direct Rust glue every time.
- Common mistakes produce Kiro errors, not Rust errors.
- The core language does not grow domain-specific syntax.
- Official modules make Kiro useful across multiple domains.
- Kiro remains simple enough to explain in one sentence:

> Kiro is a small native-orchestration language: simple like scripting, powered
> by native modules.
