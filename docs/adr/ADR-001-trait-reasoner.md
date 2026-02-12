# ADR-001: Trait-based Reasoning Engine

| Field       | Value                          |
|-------------|--------------------------------|
| Status      | Proposed                       |
| Date        | 2026-02-11                     |
| Deciders    | spindle-rust maintainers       |
| Supersedes  | N/A                            |

## Context

Spindle currently exposes reasoning through a family of free functions in `crates/spindle-core/src/reason.rs`:

```rust
// crates/spindle-core/src/reason.rs
pub fn reason(theory: &Theory) -> Result<Vec<Conclusion>> { ... }
pub fn reason_with_options(theory: &Theory, opts: PrepareOptions) -> Result<Vec<Conclusion>> { ... }
pub fn reason_prepared(theory: &Theory) -> Result<Vec<Conclusion>> { ... }
```

These functions implement the standard DL(d) forward-chaining algorithm. There is no second reasoning backend in the tree today, but the project memory and design documents reference a planned scalable DL(d||) three-phase closure algorithm. Every call site -- the CLI (`crates/spindle-cli/src/cli/commands/reason.rs`), the WASM binding (`crates/spindle-wasm/src/lib.rs`), the query operators (`crates/spindle-core/src/query.rs`), the explanation engine (`crates/spindle-core/src/explanation.rs`), the trust-weighted pipeline (`crates/spindle-core/src/pipeline.rs`), and `Theory::reason()` itself -- directly imports and calls these free functions.

This tight coupling creates several problems:

1. **No backend selection.** When the scalable reasoner lands, callers would need `if/else` branches everywhere or a second parallel API surface.
2. **Untestable in isolation.** Modules like `query.rs` call `reason()` internally. There is no way to inject a mock or stub reasoner, so testing query logic requires running the full reasoning engine.
3. **No composition point.** Features like caching, instrumentation, logging, or alternative semantics (ambiguity propagation) cannot be layered without modifying the core function.
4. **Benchmarking friction.** Comparing two reasoning algorithms on the same theory requires separate benchmark harnesses rather than swapping an implementation.

## Decision

Introduce a `Reasoner` trait that abstracts over any algorithm producing `Vec<Conclusion>` from an `IndexedTheory`. Provide two concrete implementations: `StandardReasoner` (wrapping the current `reason_prepared` logic) and a future `ScalableReasoner`. Use static dispatch (`impl Reasoner` / generics) on the hot path within `spindle-core`, and dynamic dispatch (`Box<dyn Reasoner>` / `&dyn Reasoner`) at the CLI and WASM boundaries where runtime backend selection is needed.

### Trait definition

```rust
// crates/spindle-core/src/reasoner.rs

use crate::conclusion::Conclusion;
use crate::error::Result;
use crate::index::IndexedTheory;

/// A reasoning engine that computes conclusions from an indexed theory.
///
/// Implementors encapsulate a specific defeasible logic algorithm
/// (e.g., standard DL(d) forward chaining, scalable DL(d||) three-phase
/// closure, or a test stub).
pub trait Reasoner: Send + Sync {
    /// Compute all conclusions (positive and negative) for the given
    /// indexed theory.
    ///
    /// The theory must already be prepared (grounded, temporally filtered,
    /// validated) before being passed here.
    fn reason<'t>(&self, theory: &IndexedTheory<'t>) -> Result<Vec<Conclusion>>;

    /// Human-readable name for diagnostics and logging.
    fn name(&self) -> &str;
}
```

Key design choices in the trait:

- **`IndexedTheory` input, not `Theory`.** The trait operates post-indexing. The `prepare()` pipeline remains a separate concern and does not move behind the trait. This keeps the trait focused and avoids baking pipeline options into every implementor.
- **`Send + Sync` bounds.** Required for sharing a `dyn Reasoner` across threads in async CLI or WASM-worker contexts.
- **`&self` receiver.** Reasoners are stateless algorithm selectors; all mutable working state lives in stack-local variables within `reason()`.
- **`name()` method.** Minimal cost, large debuggability benefit for logging which backend produced a result.

### Standard reasoner implementation

```rust
// crates/spindle-core/src/reasoner.rs (continued)

/// Standard DL(d) forward-chaining reasoner.
///
/// Wraps the existing algorithm from `reason.rs`. This is the default
/// backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct StandardReasoner;

impl Reasoner for StandardReasoner {
    fn reason<'t>(&self, indexed: &IndexedTheory<'t>) -> Result<Vec<Conclusion>> {
        // Delegates to the existing forward-chaining implementation,
        // refactored to accept &IndexedTheory directly.
        crate::reason::reason_indexed(indexed)
    }

    fn name(&self) -> &str {
        "standard-dl-d"
    }
}
```

The current `reason_prepared` function builds an `IndexedTheory` internally. To support the trait, its body would be extracted into a new `reason_indexed(indexed: &IndexedTheory) -> Result<Vec<Conclusion>>` function that accepts the already-built index. The existing `reason_prepared` becomes a thin wrapper:

```rust
// crates/spindle-core/src/reason.rs (refactored)

pub fn reason_prepared(theory: &Theory) -> Result<Vec<Conclusion>> {
    let indexed = IndexedTheory::build(theory);
    reason_indexed(&indexed)
}

/// Core reasoning loop operating on an already-indexed theory.
///
/// This is the function that StandardReasoner delegates to.
pub(crate) fn reason_indexed(indexed: &IndexedTheory<'_>) -> Result<Vec<Conclusion>> {
    // ... existing Phase 1, Phase 2, Phase 3 logic moves here unchanged ...
}
```

### Scalable reasoner skeleton

```rust
/// Scalable DL(d||) three-phase closure reasoner.
///
/// Implements the parallel/partitioned algorithm described in the
/// project design documents. Placeholder until the scalable engine
/// is implemented.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScalableReasoner;

impl Reasoner for ScalableReasoner {
    fn reason<'t>(&self, indexed: &IndexedTheory<'t>) -> Result<Vec<Conclusion>> {
        // Phase 1: Delta closure (strict + definite)
        // Phase 2: Lambda closure (defeasible, with ambiguity blocking)
        // Phase 3: Negative conclusion generation
        todo!("scalable DL(d||) not yet implemented")
    }

    fn name(&self) -> &str {
        "scalable-dl-d-parallel"
    }
}
```

### Static dispatch on the hot path

Internal modules that always use one algorithm (benchmarks, the default `reason()` entry point) use generics to avoid vtable overhead:

```rust
// crates/spindle-core/src/reason.rs

pub fn reason(theory: &Theory) -> Result<Vec<Conclusion>> {
    reason_with(theory, PrepareOptions::default(), &StandardReasoner)
}

pub fn reason_with<R: Reasoner>(
    theory: &Theory,
    opts: PrepareOptions,
    reasoner: &R,
) -> Result<Vec<Conclusion>> {
    let prepared = prepare(theory, opts)?;
    let indexed = IndexedTheory::build(&prepared.theory);
    reasoner.reason(&indexed)
}
```

The compiler monomorphises `reason_with::<StandardReasoner>` into a direct call with zero vtable indirection.

### Dynamic dispatch for runtime backend selection

CLI and WASM select the backend at startup and pass it as a trait object:

```rust
// crates/spindle-cli/src/cli/commands/reason.rs (sketch)

fn select_reasoner(backend: &str) -> Box<dyn Reasoner> {
    match backend {
        "standard" => Box::new(StandardReasoner),
        "scalable" => Box::new(ScalableReasoner),
        _ => Box::new(StandardReasoner),
    }
}

pub(crate) fn run_reason(
    file: Option<&PathBuf>,
    backend: &str,
    // ... other args ...
) -> Result<CommandOutput, CliError> {
    let reasoner = select_reasoner(backend);
    let theory = load_theory_source(&source)?;
    let prepared = prepare(&theory, opts)?;
    let indexed = IndexedTheory::build(&prepared.theory);
    let conclusions = reasoner.reason(&indexed)?;
    // ... format output ...
}
```

### Mock reasoner for testing

The trait immediately enables mock-based testing without running the full engine:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A mock reasoner that returns a fixed set of conclusions.
    struct MockReasoner {
        conclusions: Vec<Conclusion>,
    }

    impl Reasoner for MockReasoner {
        fn reason<'t>(&self, _theory: &IndexedTheory<'t>) -> Result<Vec<Conclusion>> {
            Ok(self.conclusions.clone())
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_query_with_mock_reasoner() {
        let mock = MockReasoner {
            conclusions: vec![
                Conclusion::definitely_provable(Literal::simple("bird")),
            ],
        };
        // Pass mock to query logic -- tests query without exercising reason.rs
        let result = query_with_reasoner(&theory, &Literal::simple("bird"), &mock).unwrap();
        assert!(result.is_provable());
    }
}
```

### Module layout after migration

```
crates/spindle-core/src/
  reasoner.rs       -- Reasoner trait, StandardReasoner, ScalableReasoner
  reason.rs         -- reason(), reason_with_options(), reason_indexed() [internal]
  query.rs          -- query operators parameterised by R: Reasoner
  explanation.rs    -- explain() parameterised by R: Reasoner
  pipeline.rs       -- prepare() unchanged, compute_weighted_conclusions()
  index.rs          -- IndexedTheory unchanged
  lib.rs            -- pub mod reasoner; re-exports in prelude
```

## Trade-offs

### Costs

| Cost | Magnitude | Notes |
|------|-----------|-------|
| Vtable indirection on `dyn Reasoner` | Negligible | One virtual call per `reason()` invocation (not per rule or literal). The call itself takes ~1ns; reasoning takes microseconds to milliseconds. |
| API surface expansion | Small | One new trait, two structs, one new module file. Existing free-function API remains and delegates internally. |
| `IndexedTheory` lifetime threading | Moderate | The `'t` lifetime on `IndexedTheory<'t>` must be threaded through the trait signature. This is a one-time design cost; callers already deal with `IndexedTheory` today. |
| Migration churn | Moderate | Internal modules (`query.rs`, `explanation.rs`) need a generic parameter or a `&dyn Reasoner` argument threaded through. See Migration Path below. |

### Benefits

| Benefit | Magnitude | Notes |
|---------|-----------|-------|
| Testability | Large | Mock reasoners eliminate full-engine runs in query/explanation/pipeline tests. Test suites run faster and test exactly one concern at a time. |
| Backend swapping | Large | CLI `--backend scalable` flag, WASM constructor option, benchmark harnesses comparing algorithms on identical theories. |
| Composability | Medium | Decorators for caching, logging, tracing, or result validation can wrap any `Reasoner` without modifying the inner algorithm. |
| Future-proofing | Medium | Additional algorithms (ambiguity propagation mode, well-founded semantics, probabilistic) slot in by implementing one trait. |
| Code clarity | Small | Separating "which algorithm" from "how to prepare a theory" makes the architecture more explicit. |

### Why not an enum instead of a trait?

An enum `ReasoningBackend { Standard, Scalable }` with a `match` inside a single function is simpler but:

- Cannot be extended by downstream crates or tests without modifying the enum.
- Does not enable mock injection.
- Does not support decorators (caching wrapper, logging wrapper).
- Requires recompiling `spindle-core` to add a variant.

The trait costs almost nothing over the enum approach (same vtable cost pattern) while being strictly more extensible.

### Why `IndexedTheory` and not `Theory`?

The `prepare()` pipeline (validation, temporal filtering, grounding) is orthogonal to the reasoning algorithm. If the trait accepted `&Theory`, every implementor would either duplicate preparation logic or skip it inconsistently. Keeping preparation outside the trait ensures:

- Each reasoner gets a clean, pre-processed input.
- The pipeline can evolve independently (new validation rules, new grounding strategies).
- The trait signature is minimal and focused.

## Migration Path

The migration can proceed incrementally across several PRs without breaking the public API at any step.

### Phase 1: Extract `reason_indexed` (non-breaking)

Refactor `reason_prepared` in `reason.rs` to separate indexing from the core loop:

```rust
// Before
pub fn reason_prepared(theory: &Theory) -> Result<Vec<Conclusion>> {
    let mut indexed = IndexedTheory::build(theory);
    // ... 200 lines of reasoning ...
}

// After
pub fn reason_prepared(theory: &Theory) -> Result<Vec<Conclusion>> {
    let indexed = IndexedTheory::build(theory);
    reason_indexed(&indexed)
}

pub(crate) fn reason_indexed(indexed: &IndexedTheory<'_>) -> Result<Vec<Conclusion>> {
    // ... 200 lines of reasoning, moved here ...
}
```

All existing tests continue to pass unchanged. No public API change.

### Phase 2: Introduce trait and `StandardReasoner` (additive)

Add `reasoner.rs` with the `Reasoner` trait and `StandardReasoner`. Add `pub mod reasoner` to `lib.rs`. Export `Reasoner` and `StandardReasoner` from the prelude. No existing function signatures change.

### Phase 3: Add `reason_with` generic entry point (additive)

Add a new generic `reason_with<R: Reasoner>()` function. The existing `reason()` calls `reason_with(..., &StandardReasoner)`. Query and explanation modules gain `_with_reasoner` variants alongside existing functions.

### Phase 4: Thread `dyn Reasoner` through CLI and WASM (optional)

Add `--backend` flag to CLI. Accept optional reasoner parameter in WASM constructor. This phase can wait until the scalable reasoner is implemented.

### Phase 5: Add `ScalableReasoner` implementation

Implement the three-phase closure algorithm behind the `Reasoner` trait. Enable via `--backend scalable` in CLI. Differential tests compare `StandardReasoner` and `ScalableReasoner` on the same theories using the proptest harness in `crates/spindle-core/tests/difftest.rs`.

## Call sites requiring update

The following files currently call `reason()` or `reason_prepared()` directly and would eventually receive a `Reasoner` parameter or use the trait-based entry points:

| File | Current call | Migration |
|------|-------------|-----------|
| `crates/spindle-core/src/theory.rs:228` | `crate::reason::reason(self)` | Delegate to `reason_with(self, ..., &StandardReasoner)` |
| `crates/spindle-core/src/query.rs:233` | `reason_with_options(theory, opts)` | Add `query_with_reasoner` variant |
| `crates/spindle-core/src/query.rs:362,379` | `reason(theory)` in `what_if` | Accept `&dyn Reasoner` parameter |
| `crates/spindle-core/src/query.rs:599,822` | `reason(theory)` in `why_not`, `abduce` | Accept `&dyn Reasoner` parameter |
| `crates/spindle-core/src/explanation.rs:592` | `reason(theory)` | Accept `&dyn Reasoner` parameter |
| `crates/spindle-core/src/pipeline.rs:570,694` | `reason_prepared(&theory)` in tests | Use `StandardReasoner` directly |
| `crates/spindle-cli/src/cli/commands/reason.rs:39` | `reason_prepared(&pipeline_result.theory)` | Use `select_reasoner()` pattern |
| `crates/spindle-wasm/src/lib.rs:200,243,405` | `reason(&...)` | Accept backend option in constructor |
| `crates/spindle-core/benches/reasoning.rs` | `reason(theory)` | Parameterize benchmarks over `Reasoner` impl |

## Consequences

- The `Reasoner` trait becomes the primary abstraction for algorithm selection in `spindle-core`.
- The existing free-function API (`reason()`, `reason_with_options()`, `reason_prepared()`) remains as convenience wrappers that default to `StandardReasoner`. No downstream breakage.
- New reasoning algorithms (scalable DL(d||), ambiguity propagation, custom semantics) implement one trait method and become available everywhere.
- Test suites for `query.rs`, `explanation.rs`, and `pipeline.rs` can use `MockReasoner` to isolate their logic from the reasoning engine, improving both speed and precision of tests.
- The `IndexedTheory` type becomes part of the public trait contract. Changes to its API will require corresponding updates to all `Reasoner` implementations.
