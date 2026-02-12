# ADR-004: Query Operator Decomposition

| Field       | Value                                        |
|-------------|----------------------------------------------|
| Status      | Proposed                                     |
| Date        | 2026-02-11                                   |
| Authors     | spindle-rust maintainers                     |
| Supersedes  | N/A                                          |

## Context

`crates/spindle-core/src/query.rs` is a 2362-line monolith containing four
major query operators (`what_if`, `why_not`, `abduce`, `requires`), roughly
fifty helper types and functions, trust filtering infrastructure, and a 1400+
line test suite. All of this lives in a single flat file.

The current structure creates several concrete problems:

1. **Navigability.** A developer looking for the abduction algorithm must
   scroll past the what-if implementation, its helper structs, the why-not
   implementation, and the why-not helper structs to reach it. There are no
   module boundaries to guide the reader.

2. **Testing friction.** Every query operator calls `reason()` directly,
   making it impossible to inject a mock or alternate reasoner. Unit tests
   must exercise the full reasoning pipeline to test query logic, which
   increases test latency, makes failure diagnosis harder, and couples query
   semantics to the specific behavior of the `reason.rs` engine.

3. **Known bugs entangled with correct code.** Two documented bugs live in
   this file:
   - `why_not` does not fully account for defeater blocking when there is no
     superiority relation (the "ambiguity" fallback at line 691-701 is a
     catch-all that obscures the real blocking cause).
   - `abduce` returns candidate fact sets without verifying that adding them
     to the theory actually makes the goal provable. A candidate blocked by a
     defeater or superiority relation will still appear as a valid solution.

   Fixing these bugs in-place risks disturbing the 50+ tests that exercise
   unrelated operators in the same file.

4. **No shared operator contract.** The four operators share a common shape
   (accept a theory, run reasoning, inspect conclusions, return a typed
   result) but express it through ad hoc function signatures. There is no
   trait that captures "query operator" as a concept, which prevents generic
   dispatch, plugin operators, or operator-level middleware (logging,
   caching, tracing).

5. **Redundant reasoning calls.** `what_if` previously called `reason()`
   three times per invocation (baseline, modified theory, and then again
   through `query()`). This was partially fixed but the tight coupling to
   `reason()` makes it easy for regressions to reappear.

## Decision

Decompose `query.rs` into a `query/` module directory with the following
structure:

```
crates/spindle-core/src/query/
    mod.rs          -- shared types, QueryOperator trait, re-exports
    what_if.rs      -- what_if operator and WhatIfResult
    why_not.rs      -- why_not operator and WhyNotResult
    abduce.rs       -- abduce operator and AbductionResult
    requires.rs     -- requires operator (thin wrapper around abduce)
```

### QueryOperator Trait

Introduce a `QueryOperator` trait that all operators implement. The trait
accepts `&dyn Reasoner` instead of calling `reason()` directly, enabling mock
testing and alternate reasoning backends.

```rust
// query/mod.rs

use crate::conclusion::Conclusion;
use crate::error::Result;
use crate::theory::Theory;
use crate::pipeline::PrepareOptions;

/// Trait for reasoning backends. Decouples query operators from a specific
/// reasoning algorithm.
pub trait Reasoner: Send + Sync {
    /// Run reasoning on the given theory and return conclusions.
    fn reason(&self, theory: &Theory) -> Result<Vec<Conclusion>>;

    /// Run reasoning with custom pipeline options.
    fn reason_with_options(
        &self,
        theory: &Theory,
        opts: PrepareOptions,
    ) -> Result<Vec<Conclusion>>;
}

/// The standard reasoner that delegates to `crate::reason::reason()`.
pub struct StandardReasoner;

impl Reasoner for StandardReasoner {
    fn reason(&self, theory: &Theory) -> Result<Vec<Conclusion>> {
        crate::reason::reason(theory)
    }

    fn reason_with_options(
        &self,
        theory: &Theory,
        opts: PrepareOptions,
    ) -> Result<Vec<Conclusion>> {
        crate::reason::reason_with_options(theory, opts)
    }
}

/// Arguments common to all query operators.
#[derive(Debug, Clone, Default)]
pub struct QueryArgs {
    /// Pipeline options for reasoning (temporal filtering, etc.)
    pub prepare_options: PrepareOptions,
    /// Maximum number of solutions (for abduce/requires).
    pub max_solutions: usize,
}

/// Unified result envelope returned by all query operators.
#[derive(Debug, Clone)]
pub enum QueryResult {
    /// Result from the basic query or what-if operator.
    Status(StatusResult),
    /// Result from the what-if operator.
    WhatIf(WhatIfResult),
    /// Result from the why-not operator.
    WhyNot(WhyNotResult),
    /// Result from the abduce operator.
    Abduce(AbductionResult),
    /// Result from the requires operator.
    Requires(Vec<Literal>),
}

/// A query operator that can be executed against a theory.
pub trait QueryOperator {
    /// The specific result type this operator produces.
    type Output;

    /// Execute the operator against a theory using the given reasoner.
    fn execute(
        &self,
        theory: &Theory,
        reasoner: &dyn Reasoner,
        args: QueryArgs,
    ) -> Result<Self::Output>;
}
```

### Shared Infrastructure in `mod.rs`

The following types remain in `query/mod.rs` because they are used across
multiple operators:

- `QueryStatus` (Provable / Refuted / Unknown)
- `StatusResult` (literal + status + conclusion type) -- renamed from the
  current `QueryResult` to avoid collision with the new envelope type
- `TrustFilter` and its `passes()` method
- `QueryArgs` and the `QueryResult` enum
- `Reasoner` trait and `StandardReasoner`
- The `query()` and `query_with_options()` free functions (simple wrappers
  that construct a `StandardReasoner` internally to preserve backward
  compatibility)

All public types and functions are re-exported from `mod.rs` so that
`crate::query::what_if`, `crate::query::WhyNotResult`, etc. continue to
work without changing downstream import paths.

### Operator Files

Each operator file contains:

1. The operator-specific result type(s) and helper structs.
2. A struct implementing `QueryOperator`.
3. A free function preserving the current public API.

#### `what_if.rs`

```rust
use crate::conclusion::Conclusion;
use crate::error::Result;
use crate::literal::Literal;
use crate::theory::Theory;
use super::{QueryArgs, QueryOperator, Reasoner, StatusResult, QueryStatus};

pub struct WhatIfOperator {
    pub hypotheticals: Vec<HypotheticalClaim>,
    pub goal: Literal,
}

impl QueryOperator for WhatIfOperator {
    type Output = WhatIfResult;

    fn execute(
        &self,
        theory: &Theory,
        reasoner: &dyn Reasoner,
        args: QueryArgs,
    ) -> Result<WhatIfResult> {
        // 1. Baseline: reasoner.reason(theory)
        let baseline = reasoner.reason(theory)?;

        // 2. Clone theory, inject hypotheticals as facts
        let mut modified = theory.clone();
        for hyp in &self.hypotheticals {
            let label = next_hyp_label(&modified, /* ... */);
            modified.add_rule(Rule::fact(&label, hyp.literal.clone()));
        }

        // 3. Modified: reasoner.reason(&modified) -- single call, not two
        let modified_conclusions = reasoner.reason(&modified)?;

        // 4. Diff baseline vs. modified
        // ... (same logic as today)
        todo!()
    }
}

/// Backward-compatible free function.
pub fn what_if(
    theory: &Theory,
    hypotheticals: Vec<HypotheticalClaim>,
    goal: &Literal,
) -> Result<WhatIfResult> {
    let op = WhatIfOperator {
        hypotheticals,
        goal: goal.clone(),
    };
    op.execute(theory, &super::StandardReasoner, QueryArgs::default())
}
```

#### `why_not.rs` -- with defeater-blocking bug fix

```rust
impl QueryOperator for WhyNotOperator {
    type Output = WhyNotResult;

    fn execute(
        &self,
        theory: &Theory,
        reasoner: &dyn Reasoner,
        args: QueryArgs,
    ) -> Result<WhyNotResult> {
        let conclusions = reasoner.reason(theory)?;
        let proven: HashSet<_> = conclusions
            .iter()
            .filter(|c| c.conclusion_type.is_positive())
            .map(|c| c.literal.clone())
            .collect();

        // ... find rules that could derive self.literal ...

        // BUG FIX: When body is satisfied but conclusion is not proven,
        // explicitly check defeaters. The old code fell through to a
        // generic "ambiguity" catch-all that hid the real blocking cause.
        for attacker in theory.rules() {
            if attacker.head_literal() != &complement {
                continue;
            }
            let attacker_satisfied = attacker.body.iter().all(|b| proven.contains(b));
            if !attacker_satisfied {
                continue;
            }

            match attacker.rule_type {
                RuleType::Defeater => {
                    // Defeaters block unless the target rule is strictly
                    // superior. This is the fix: we now always report
                    // defeater blocking when no superiority overrides it.
                    if !theory.is_superior(&rule.label, &attacker.label) {
                        result.blocked_by.push(
                            BlockingCondition::defeated(&rule.label, &attacker.label)
                        );
                    }
                }
                RuleType::Defeasible | RuleType::Strict => {
                    // Symmetric superiority check for conflicting rules
                    let attacker_wins = theory.is_superior(&attacker.label, &rule.label);
                    let rule_wins = theory.is_superior(&rule.label, &attacker.label);
                    if rule_wins && !attacker_wins {
                        continue; // rule beats attacker
                    }
                    result.blocked_by.push(
                        BlockingCondition::contradicted(&rule.label, &attacker.label)
                    );
                }
                _ => {}
            }
        }

        // No generic "ambiguity" fallback. If no attackers were found and
        // the body was satisfied, something else is wrong; surface it as
        // an internal diagnostic rather than silently blaming ambiguity.

        todo!()
    }
}
```

#### `abduce.rs` -- with solution verification bug fix

```rust
impl QueryOperator for AbduceOperator {
    type Output = AbductionResult;

    fn execute(
        &self,
        theory: &Theory,
        reasoner: &dyn Reasoner,
        args: QueryArgs,
    ) -> Result<AbductionResult> {
        let conclusions = reasoner.reason(theory)?;
        // ... collect proven set, find candidate fact sets ...

        // BUG FIX: Verify each candidate solution actually works.
        // The old code returned raw missing-premise sets without checking
        // whether adding them would be blocked by a defeater or conflict.
        let mut verified_solutions = Vec::new();
        for candidate in &raw_solutions {
            let mut test_theory = theory.clone();
            for fact in candidate {
                test_theory.add_fact(&fact.to_spl());
            }
            let test_conclusions = reasoner.reason(&test_theory)?;
            let goal_proven = test_conclusions
                .iter()
                .any(|c| c.literal == self.goal && c.conclusion_type.is_positive());
            if goal_proven {
                verified_solutions.push(candidate.clone());
            }
        }

        todo!()
    }
}
```

The verification call uses `reasoner.reason()`, so in tests a mock reasoner
can control exactly what the "verification" step returns without running the
full pipeline.

### Mock Testing Example

The `&dyn Reasoner` parameter is the key enabler for isolated unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// A mock reasoner that returns a fixed set of conclusions.
    struct MockReasoner {
        conclusions: Vec<Conclusion>,
    }

    impl Reasoner for MockReasoner {
        fn reason(&self, _theory: &Theory) -> Result<Vec<Conclusion>> {
            Ok(self.conclusions.clone())
        }

        fn reason_with_options(
            &self,
            _theory: &Theory,
            _opts: PrepareOptions,
        ) -> Result<Vec<Conclusion>> {
            Ok(self.conclusions.clone())
        }
    }

    #[test]
    fn test_why_not_with_mock_reasoner() {
        // Arrange: mock returns no positive conclusions for "flies"
        let reasoner = MockReasoner {
            conclusions: vec![
                Conclusion::definitely_provable(Literal::simple("bird")),
            ],
        };

        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let op = WhyNotOperator { literal: Literal::simple("flies") };
        let result = op.execute(&theory, &reasoner, QueryArgs::default()).unwrap();

        // The mock says "flies" was never proven, so why_not should report
        // missing/blocking conditions based on the theory structure.
        assert!(result.has_blockers());
    }

    #[test]
    fn test_abduce_verification_uses_reasoner() {
        // Arrange: first call returns no goal; second call (verification)
        // also returns no goal, proving the candidate is invalid.
        let reasoner = MockReasoner {
            conclusions: vec![], // goal never provable
        };

        let theory = Theory::new();
        let op = AbduceOperator {
            goal: Literal::simple("q"),
            max_solutions: 10,
        };
        let result = op.execute(&theory, &reasoner, QueryArgs::default()).unwrap();

        // With empty mock, only the trivial "add q itself" solution
        // would be generated, but verification would also fail because
        // the mock returns nothing. Behavior depends on implementation
        // details, but the point is: no real reasoning runs.
        assert!(!result.is_already_provable());
    }
}
```

### Backward Compatibility

Every existing public symbol (`query`, `query_with_options`, `what_if`,
`what_if_provable`, `why_not`, `abduce`, `requires`, `QueryStatus`,
`QueryResult` (renamed to `StatusResult` internally but re-exported as
`QueryResult` via a type alias), `WhatIfResult`, `WhyNotResult`,
`AbductionResult`, `AbductionSolution`, `HypotheticalClaim`,
`BlockingCondition`, `BlockingType`, `TrustFilter`) is re-exported from
`query/mod.rs`. Downstream code that uses `use spindle_core::query::*` or
`use spindle_core::prelude::*` sees no change.

The free functions (`what_if()`, `why_not()`, `abduce()`, `requires()`)
remain as thin wrappers that instantiate `StandardReasoner` internally:

```rust
// query/mod.rs re-exports

pub fn what_if(
    theory: &Theory,
    hypotheticals: Vec<HypotheticalClaim>,
    goal: &Literal,
) -> Result<WhatIfResult> {
    what_if::what_if(theory, hypotheticals, goal)
}
```

The `lib.rs` declaration changes from `pub mod query;` (file) to
`pub mod query;` (directory with `mod.rs`). Rust's module system handles
this transparently.

## Trade-offs

### Advantages

- **Isolated testing.** Each operator can be tested against a mock reasoner
  in under 1ms, without running the full pipeline. This cuts the query test
  suite from ~50 integration tests to ~50 fast unit tests plus a handful of
  integration tests that verify end-to-end behavior.

- **Bug isolation.** The `why_not` defeater-blocking fix and the `abduce`
  verification fix each live in their own file. Reviewing, reverting, or
  extending them does not risk disturbing other operators.

- **Extensibility.** New operators (e.g., `counterfactual`, `contrastive`,
  `sensitivity_analysis`) implement `QueryOperator` and get mock testing,
  tracing, and caching for free. The CLI and WASM layers can dispatch on
  `QueryOperator` generically.

- **Readability.** Each file is 200-400 lines instead of one file at 2362
  lines. A developer working on abduction never sees the what-if code.

- **Alternate reasoners.** The `Reasoner` trait lets the same query operators
  work against both `reason.rs` (standard DL(d)) and any future scalable
  backend. A `ScalableReasoner` struct would implement the same trait.

### Costs

- **Five files instead of one.** Navigation now requires knowing which
  operator lives where. Mitigated by re-exports in `mod.rs` and by the
  predictable naming convention (operator name = file name).

- **Trait object overhead.** `&dyn Reasoner` introduces a vtable indirection
  on each `reason()` call. Since reasoning is O(n^2) or worse in the theory
  size, a single vtable dispatch is negligible. For the paranoid,
  `#[inline(never)]` on the trait methods prevents the compiler from
  de-virtualizing across crate boundaries, but within `spindle-core` the
  compiler can often monomorphize the `StandardReasoner` path anyway.

- **`QueryResult` rename.** The current `QueryResult` struct must become
  `StatusResult` to free the name for the enum envelope. A type alias
  `pub type QueryResult = StatusResult;` in `mod.rs` preserves backward
  compatibility, but IDE go-to-definition will land on the alias. This is a
  minor ergonomic cost.

- **Migration effort.** The existing 1400+ lines of tests must be distributed
  across the new files. Operator-specific tests go into their operator's
  file; cross-operator integration tests go into a `tests/` submodule or
  remain in `mod.rs`. This is mechanical but time-consuming.

## Consequences

1. `crate::query` becomes a directory module. All downstream imports continue
   to resolve because `mod.rs` re-exports everything.

2. The `Reasoner` trait becomes a first-class concept in `spindle-core`. Other
   subsystems (e.g., `explanation.rs`, `mining.rs`) that currently call
   `reason()` directly can migrate to `&dyn Reasoner` incrementally.

3. The `why_not` defeater-blocking bug (documented in the bug-hunt notes) is
   fixed as part of the decomposition. Tests in `why_not.rs` explicitly
   assert that defeaters with satisfied bodies produce `BlockingType::Defeated`
   entries, not generic "ambiguity" messages.

4. The `abduce` verification bug is fixed by adding a verification pass that
   calls `reasoner.reason()` on a theory augmented with the candidate facts.
   Solutions that remain blocked by defeaters or conflicts are filtered out.
   This makes `abduce` slightly slower (one extra `reason()` call per
   candidate) but correct.

5. Existing tests that call the free functions (`what_if()`, `why_not()`,
   etc.) pass without modification. New tests can use `MockReasoner` for
   speed and isolation.

6. The `requires()` function remains a thin wrapper around `abduce()` and
   moves to its own file for symmetry, even though it is only ~10 lines. This
   keeps the "one operator per file" invariant clean and gives `requires` a
   natural home for future enhancements (e.g., returning a structured
   `RequiresResult` instead of `Vec<Literal>`).
