# ADR-003: Scoped Interner Replacing Global State

| Field        | Value                              |
|--------------|------------------------------------|
| Status       | Proposed                           |
| Date         | 2026-02-11                         |
| Decision     | Replace global `INTERNER` with scoped `Context`-owned `StringInterner` |
| Drivers      | Data-race risk, memory leak, testability, API ergonomics |

## Context

### Current design

The string interner lives as a process-wide singleton in
`crates/spindle-core/src/intern.rs`:

```rust
static INTERNER: RwLock<Option<Interner>> = RwLock::new(None);
```

Every call to `intern()` or `resolve()` goes through this lock. The `Interner`
struct itself uses `Box::leak` to produce `&'static str` references, meaning
every string interned during the process lifetime is leaked into the heap and
never deallocated:

```rust
fn intern(&mut self, s: &str) -> SymbolId {
    if let Some(&id) = self.map.get(s) {
        return id;
    }
    let id = SymbolId(self.strings.len() as u32);
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    self.strings.push(leaked);
    self.map.insert(leaked, id);
    id
}
```

### Problems

1. **Data races under concurrent use.** The `RwLock<Option<Interner>>`
   pattern is technically thread-safe (no UB), but the *semantic* safety is
   fragile. Two `reason()` invocations running in parallel share the same
   interner state. A `SymbolId` minted by one call is silently valid in the
   other, even when the two theories have no relationship. In long-running
   servers that handle many theories, this conflation leads to unbounded
   growth and stale cross-theory references.

2. **Memory leak by design.** `Box::leak` converts owned strings into
   `&'static str`. The interner never frees these allocations. For CLI
   one-shot usage this is acceptable, but in the WASM target
   (`spindle-wasm`) and any server/daemon scenario, every theory reasoned
   about monotonically grows the resident set. There is no `reset()` or
   `drop()` path.

3. **Lazy-init footgun.** The `Option<Interner>` wrapping requires every
   accessor to check-and-initialize:

   ```rust
   fn with_interner<F, R>(f: F) -> R
   where F: FnOnce(&Interner) -> R {
       let guard = INTERNER.read().unwrap();
       if let Some(ref interner) = *guard {
           return f(interner);
       }
       drop(guard);
       let mut guard = INTERNER.write().unwrap();
       if guard.is_none() {
           *guard = Some(Interner::new());
       }
       f(guard.as_ref().unwrap())
   }
   ```

   This double-check locking adds latency on the first call and complicates
   reasoning about initialization order. Tests that run in parallel may
   observe different interner contents depending on execution order.

4. **Implicit coupling.** Every module that calls `intern()` or `resolve()`
   silently depends on global state. The dependency is invisible in function
   signatures. Functions like `reason()`, `prepare()`, `query()`,
   `what_if()`, `why_not()`, and `abduce()` all transitively depend on the
   interner, but their signatures show only `&Theory` and option structs.

5. **Testing interference.** Because all tests within the same process share
   one `INTERNER`, interned strings from one test case bleed into another.
   The `interned_count()` test already documents this:

   ```rust
   assert!(after >= before); // May be equal if already interned in another test
   ```

## Decision

Introduce a `Context` struct that owns a `StringInterner` and is passed
explicitly to all public API entry points. The global `INTERNER` is retained
during migration but ultimately removed.

### `StringInterner`: owned, droppable, non-static

```rust
/// A scoped string interner that owns its allocations.
///
/// Unlike the global `INTERNER`, this struct can be dropped,
/// freeing all interned strings.
pub struct StringInterner {
    map: FxHashMap<u32, SymbolId>,   // hash(string) -> id
    strings: Vec<String>,            // id -> owned string
}

impl StringInterner {
    pub fn new() -> Self {
        let mut si = Self {
            map: FxHashMap::default(),
            strings: Vec::with_capacity(1024),
        };
        si.strings.push(String::new()); // index 0 = empty
        si.map.insert(Self::hash_str(""), SymbolId::EMPTY);
        si
    }

    pub fn intern(&mut self, s: &str) -> SymbolId {
        let h = Self::hash_str(s);
        if let Some(&id) = self.map.get(&h) {
            // Verify actual equality (collision guard)
            if self.strings[id.as_raw() as usize] == s {
                return id;
            }
        }
        let id = SymbolId::from_raw(self.strings.len() as u32);
        self.strings.push(s.to_owned());
        self.map.insert(h, id);
        id
    }

    /// Resolve without returning `&'static str` -- returns `&str`
    /// bounded by the interner's lifetime.
    pub fn resolve(&self, id: SymbolId) -> &str {
        self.strings.get(id.as_raw() as usize)
            .map(|s| s.as_str())
            .unwrap_or("")
    }

    pub fn len(&self) -> usize {
        self.strings.len()
    }

    fn hash_str(s: &str) -> u32 {
        let mut hasher = rustc_hash::FxHasher::default();
        std::hash::Hash::hash(s, &mut hasher);
        std::hash::Hasher::finish(&hasher) as u32
    }
}

impl Drop for StringInterner {
    fn drop(&mut self) {
        // All strings are owned `String` values; they free
        // automatically. No `Box::leak` means no permanent leak.
    }
}
```

Key change: strings are stored as `String` (owned), not `&'static str`
(leaked). The `resolve()` return type becomes `&str` tied to the interner's
lifetime, not `&'static str`.

### `Context`: the single-parameter bundle

```rust
/// Reasoning context that bundles all session-scoped state.
///
/// `Context` is `!Sync` -- it cannot be shared across threads.
/// Each `reason()` call creates (or receives) its own `Context`.
pub struct Context {
    /// Scoped string interner -- dropped when Context is dropped.
    pub interner: StringInterner,
    /// Diagnostic messages collected during reasoning.
    pub diagnostics: Vec<Diagnostic>,
    /// Pipeline options.
    pub options: PrepareOptions,
}

// Prevent sharing across threads: reasoning is single-threaded by design.
impl !Sync for Context {}

impl Context {
    pub fn new() -> Self {
        Self {
            interner: StringInterner::new(),
            diagnostics: Vec::new(),
            options: PrepareOptions::default(),
        }
    }

    pub fn with_options(options: PrepareOptions) -> Self {
        Self {
            interner: StringInterner::new(),
            diagnostics: Vec::new(),
            options,
        }
    }

    /// Intern a string in this context's interner.
    #[inline]
    pub fn intern(&mut self, s: &str) -> SymbolId {
        self.interner.intern(s)
    }

    /// Resolve a SymbolId to its string.
    #[inline]
    pub fn resolve(&self, id: SymbolId) -> &str {
        self.interner.resolve(id)
    }
}
```

`Context` is intentionally `!Sync`. Defeasible logic reasoning in Spindle is
single-threaded by design (both `reason.rs` and the scalable algorithm are
sequential forward-chaining loops). Making `Context` non-`Sync` prevents
accidental sharing and eliminates the need for any locking.

### Updated public API

Before:

```rust
pub fn reason(theory: &Theory) -> Result<Vec<Conclusion>>;
pub fn reason_with_options(theory: &Theory, opts: PrepareOptions) -> Result<Vec<Conclusion>>;
pub fn prepare(theory: &Theory, opts: PrepareOptions) -> Result<PipelineResult>;
pub fn query(theory: &Theory, literal: &Literal) -> Result<QueryResult>;
pub fn what_if(theory: &Theory, additions: &[Rule], query: &Literal) -> Result<WhatIfResult>;
pub fn why_not(theory: &Theory, literal: &Literal) -> Result<WhyNotResult>;
pub fn abduce(theory: &Theory, goal: &Literal, max: usize) -> Result<AbductionResult>;
```

After:

```rust
pub fn reason(ctx: &mut Context, theory: &Theory) -> Result<Vec<Conclusion>>;
pub fn reason_with_options(ctx: &mut Context, theory: &Theory) -> Result<Vec<Conclusion>>;
pub fn prepare(ctx: &mut Context, theory: &Theory) -> Result<PipelineResult>;
pub fn query(ctx: &mut Context, theory: &Theory, literal: &Literal) -> Result<QueryResult>;
pub fn what_if(ctx: &mut Context, theory: &Theory, additions: &[Rule], query: &Literal) -> Result<WhatIfResult>;
pub fn why_not(ctx: &mut Context, theory: &Theory, literal: &Literal) -> Result<WhyNotResult>;
pub fn abduce(ctx: &mut Context, theory: &Theory, goal: &Literal, max: usize) -> Result<AbductionResult>;
```

The `PrepareOptions` move into `Context`, so `reason_with_options` can
collapse into `reason` once migration completes:

```rust
let mut ctx = Context::with_options(PrepareOptions {
    reference_time: Some(TimePoint::from_millis(1707220800000)),
    ..Default::default()
});
let conclusions = reason(&mut ctx, &theory)?;
// ctx.diagnostics contains any warnings
// ctx is dropped here, freeing all interned strings
```

### Backward compatibility: `with_default_context()`

To avoid a flag-day migration, a helper function creates a temporary context
for callers that do not need to manage one:

```rust
/// Run a closure with a default Context.
///
/// This is a convenience wrapper for callers that do not need to
/// inspect diagnostics or control options beyond defaults.
///
/// ```rust
/// use spindle_core::context::with_default_context;
/// use spindle_core::reason::reason;
///
/// let conclusions = with_default_context(|ctx| reason(ctx, &theory))?;
/// ```
pub fn with_default_context<F, R>(f: F) -> R
where
    F: FnOnce(&mut Context) -> R,
{
    let mut ctx = Context::new();
    f(&mut ctx)
}
```

The existing zero-argument `Theory::reason()` convenience method continues to
work by calling `with_default_context` internally:

```rust
impl Theory {
    pub fn reason(&self) -> Result<Vec<Conclusion>> {
        with_default_context(|ctx| crate::reason::reason(ctx, self))
    }
}
```

### `SymbolId` lifetime implications

Today `resolve()` returns `&'static str` because `Box::leak` makes the
backing storage immortal. With an owned `StringInterner`, `resolve()` returns
`&str` bounded by the interner's lifetime. This affects:

- `Literal::name()` -- currently returns `&'static str`; will return `&str`
  tied to the `Context`.
- `Display` / `Debug` implementations on `SymbolId` and `LiteralId` -- these
  currently call the global `resolve()`. They will need a `&Context`
  parameter or will format as raw IDs when no context is available.

To handle `Display`/`Debug` gracefully without requiring a context parameter
on every format call, a `DisplayWith` pattern is used:

```rust
impl SymbolId {
    pub fn display<'a>(&self, ctx: &'a Context) -> impl fmt::Display + 'a {
        struct SymbolDisplay<'a> {
            id: SymbolId,
            ctx: &'a Context,
        }
        impl fmt::Display for SymbolDisplay<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.ctx.resolve(self.id))
            }
        }
        SymbolDisplay { id: *self, ctx }
    }
}
```

## Migration Plan

### Phase 1: Introduce `Context` alongside global (non-breaking)

- Add `context.rs` module with `Context` and `StringInterner`.
- Add `&mut Context` parameter to internal functions (`reason_prepared`,
  `IndexedTheory::build`, grounding helpers).
- Public API functions gain `_with_context` variants:
  `reason_with_context()`, `query_with_context()`, etc.
- Existing public functions (`reason()`, `query()`, etc.) continue to use
  the global interner via `with_default_context`.
- All new code uses `Context`-based paths.
- Estimated scope: ~40 functions gain a `&mut Context` or `&Context` param.

### Phase 2: Deprecate global API

- Mark `intern()`, `resolve()`, `interned_count()` as `#[deprecated]`.
- Mark non-context public functions as `#[deprecated]`.
- Update `spindle-cli`, `spindle-wasm`, and `spindle-parser` to use
  `Context`-based entry points.
- Add `clippy::allow(deprecated)` gates in the crate for the transition.

### Phase 3: Remove global interner

- Remove `static INTERNER`.
- Remove `with_interner` / `with_interner_mut`.
- Remove deprecated shims.
- `resolve()` is no longer a free function; it requires `&Context`.
- `Literal::name()` returns `&str` (not `&'static str`).

### Files affected (estimated)

| File | Changes |
|------|---------|
| `intern.rs` | Add `StringInterner`, deprecate globals |
| `literal.rs` | `name()` signature, `Display` impl |
| `index.rs` | `IndexedTheory::build` takes `&mut Context` |
| `reason.rs` | `reason()`, `reason_prepared()` take `&mut Context` |
| `pipeline.rs` | `prepare()` takes `&mut Context`, options move to `Context` |
| `query.rs` | `query()`, `what_if()`, `why_not()`, `abduce()`, `requires()` |
| `grounding.rs` | `ground_theory()`, `ground_theory_with_limit()`, substitution helpers |
| `worklist.rs` | Formatting/display methods |
| `theory.rs` | `Theory::reason()` convenience uses `with_default_context` |
| `lib.rs` / `prelude` | Re-export `Context`, `with_default_context` |
| `spindle-parser/src/dfl.rs` | Parser creates `Context` for interning during parse |
| `spindle-parser/src/spl.rs` | Same as above |
| `spindle-cli` | CLI entry point creates `Context` |
| `spindle-wasm` | WASM bindings create/drop `Context` per invocation |

## Impact

### Memory

The primary win. Today, a server that reasons about 1,000 theories
accumulates every string ever interned across all of them. With scoped
interning, each `Context` is dropped after its `reason()` call completes, and
all `String` allocations are freed. Peak memory is proportional to the
largest single theory, not the sum of all theories.

### Performance

- **Removed:** `RwLock` acquisition on every `intern()` / `resolve()` call.
  In the scoped model, `&mut Context` gives exclusive access with zero
  synchronization overhead.
- **Added:** `String` allocation instead of `Box::leak`. This is a net wash
  for single-shot CLI usage and a win for repeated usage (no permanent
  leak). The `FxHashMap` lookup cost is unchanged.
- **Unchanged:** `SymbolId` remains a 4-byte `Copy` type. All hot-path
  comparisons still use integer equality, not string comparison.

### API surface

Approximately 40 internal functions and 7 public entry points gain a
`&mut Context` parameter. This is a deliberate trade-off: one explicit
parameter replaces five implicit dependencies (interner, diagnostics,
options, trust policy, grounding config). The `with_default_context` helper
ensures that simple use cases remain concise.

### Testing

Each test can create its own `Context`, eliminating cross-test pollution.
Property-based tests (proptest in `difftest.rs`) benefit especially: each
test case starts with a clean interner, making failures reproducible
regardless of execution order.

## Alternatives Considered

### 1. Thread-local interner

Replace `RwLock` with `thread_local!`. This avoids locking but does not
solve the memory leak (strings still leak within each thread) and makes
WASM single-thread usage no better. It also makes `SymbolId` values
non-portable across threads, which is confusing.

**Rejected:** Does not solve the leak. Adds thread-portability confusion.

### 2. `Arc<RwLock<Interner>>` shared reference

Make the interner reference-counted so multiple call sites can share it, but
it is dropped when the last reference goes away.

**Rejected:** Still requires locking. Still requires shared-reference
discipline. The `Context` approach is simpler because reasoning is
single-threaded.

### 3. Generational arena with `reset()`

Keep the global interner but add a `reset()` function that clears all
strings and resets the ID counter.

**Rejected:** Invalidates any `SymbolId` values held across the reset
boundary. Extremely error-prone: a stale `SymbolId` would silently resolve
to an unrelated string or panic.

### 4. Do nothing

Accept the leak and global state as-is. This is workable for CLI-only usage.

**Rejected:** The WASM target and planned server/daemon mode make unbounded
leaks unacceptable. Test interference is already causing flaky assertions.

## References

- `crates/spindle-core/src/intern.rs` -- current global interner
- `crates/spindle-core/src/literal.rs` -- `Literal` type using `SymbolId`
- `crates/spindle-core/src/index.rs` -- per-theory `AtomId`/`LitId` (local interning pattern)
- `crates/spindle-core/src/reason.rs` -- `reason()` entry point
- `crates/spindle-core/src/pipeline.rs` -- `prepare()` entry point
- `crates/spindle-core/src/query.rs` -- `query()`, `what_if()`, `why_not()`, `abduce()`
- Rust API Guidelines: [C-CALLER-CONTROL](https://rust-lang.github.io/api-guidelines/flexibility.html#c-caller-control)
