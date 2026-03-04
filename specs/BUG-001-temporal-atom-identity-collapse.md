---
id: BUG-001
title: Temporal Atom Identity Collapse
severity: S2
priority: P1
status: in-progress
reported-by: Claude Opus 4.6 (adversarial review of fix/temporal-atom-identity branch)
assigned-to: unassigned
branch: fix/temporal-atom-identity
date-reported: 2026-02-16
date-documented: 2026-03-04
---

# BUG-001: Temporal Atom Identity Collapse

**Severity:** S2 — Core reasoning produces incorrect results for temporal theories; non-temporal theories unaffected
**Priority:** P1 — Affects all temporal reasoning; blocks correct use of `(during ...)` temporal annotations
**Status:** in-progress (33 commits on `fix/temporal-atom-identity`, not yet merged)

## Specification Reference

- Violates: SPEC-016 (Temporal Reasoning) — temporal literals with distinct windows must be treated as distinct atoms
- Related: No existing `TEST-###` caught this; temporal propagation tests (`temporal_propagation_tests.rs`) test grounding and pipeline but not reasoning-level atom identity
- Gap: The specification for temporal reasoning did not explicitly state that `AtomKey` must include temporal bounds; the identity invariant was implicit

## Environment

- Rust workspace: `spindle-core` 0.3.0 (post-arithmetic module merge)
- Reasoner: `crates/spindle-core/src/reason/defeasible.rs` (three-phase SDL closure)
- Indexer: `crates/spindle-core/src/index.rs` (`AtomKey`, `intern_literal`, `resolve_literal`)
- Detection method: Manual analysis of temporal reasoning failures
- Detecting model: Claude Opus 4.6

## Steps to Reproduce

1. Define a theory with two temporal facts sharing the same predicate but different windows:

   ```spl
   >> p [1, 10].    % Fact: p holds from time 1 to 10
   >> p [20, 30].   % Fact: p holds from time 20 to 30
   p => q.          % Defeasible rule: if p then q
   ```

2. Run the reasoning pipeline via `prepare()` then `scalable_reason()`.

3. Observe: the body counter for rule `p => q` is decremented **twice** (once per temporal fact), but only one decrement is needed. Both temporal facts intern to the **same `LitId`** because `AtomKey` excludes temporal bounds.

4. More critically, consider:

   ```spl
   >> p [1, 10].         % Only p holds during [1,10]
   p [20, 30] => q.      % Rule: if p during [20,30] then q
   ```

5. Expected: `q` is NOT derived (no fact for `p[20,30]`).
   Actual: `q` IS derived because `p[1,10]` and `p[20,30]` collapse to the same `LitId`.

## Expected Behaviour

Per temporal reasoning semantics: `p[1,10]` and `p[20,30]` are **distinct atoms**. A rule requiring `p[20,30]` in its body must not be satisfied by the existence of `p[1,10]`. Each temporal window defines a separate truth interval. The `AtomKey` used for atom interning must include temporal bounds so that distinct windows produce distinct `LitId` values.

## Actual Behaviour

`AtomKey` on `main` is defined as:

```rust
struct AtomKey {
    functor: SymbolId,
    mode: (SymbolId, bool),
    args: Vec<SymbolId>,
    // temporal: MISSING
}
```

Because temporal bounds are excluded, `intern_literal` maps `p[1,10]` and `p[20,30]` to the same `AtomId` and therefore the same `LitId`. This causes:

1. **Body counter corruption**: Rules with `p` in the body get their counter decremented once per temporal variant, but the counter only expects one decrement per distinct `LitId`. Two temporal facts → double decrement → premature rule firing.
2. **Cross-window satisfaction**: A rule requiring `p[20,30]` fires when only `p[1,10]` exists, because both resolve to the same `LitId`.
3. **Conclusion conflation**: Query operators (`query`, `what_if`, `abduce`, `why_not`, `explain`) cannot distinguish between temporal windows in their results.

## Evidence

### Minimal Failing Test (from `temporal_proof_lift_tests.rs` on branch)

```rust
#[test]
fn multi_body_two_temporal_variants() {
    // Two temporal facts for the same predicate should count as ONE
    // body satisfaction, not double-decrement
    let theory = parse_spl_to_theory(
        ">> bird [1000, 2000].\n\
         >> bird [3000, 4000].\n\
         bird => can_fly.",
    );
    let (indexed, mut state) = prepare_and_run_definite(&theory);
    resolve_defeasible(&theory, &indexed, &mut state);
    // can_fly should be +d (proved once, not double-counted)
    let can_fly_id = indexed.get_lit_id_by_name("can_fly", false).unwrap();
    assert!(state.defeasible_proven.contains(can_fly_id));
}
```

### Branch Commit Trail

The 33 commits on `fix/temporal-atom-identity` document the progressive discovery of cascading failures after the initial fix:

| # | Commit | Summary |
|---|--------|---------|
| 1 | `fc1f632` | Add temporal to `AtomKey` — core identity fix |
| 2 | `1217259` | Regression tests for temporal proof lift |
| 3 | `a8ecba4` | Guard defeasible lift with SDL conditions |
| 4 | `460087d` | Allow lift for base literals pre-marked `-d` |
| 5 | `7b8cb45` | Restrict lift targets to supported temporal heads |
| 6–10 | `c218ac2`–`ac0c0dc` | Cascade retraction, stack overflow fix, revalidation |
| 11–18 | `559e8e4`–`756df4f` | Self-defeating cycle detection (5 iterations), attacker retry |
| 19–24 | `e14605e`–`12b350d` | Query operator alignment (`matches_query` migration) |
| 25–33 | `464206c`–`52d12b4` | Revalidation guards, stale `-d` cleanup, fmt |

## Root Cause

- **Category:** design-error
- **Analysis:**

The root cause is a **missing invariant in the `AtomKey` design**. When temporal annotations were added to the literal model (`Literal.temporal: Temporal`), the `AtomKey` struct — which serves as the identity key for atom interning — was not updated to include temporal bounds. This created a fundamental semantic gap: the literal model distinguished temporal windows but the indexing layer did not.

This is a design error rather than an implementation error because:

1. The `AtomKey` design predates temporal support — it was never updated when temporal was added.
2. No specification or ADR documented the invariant that `AtomKey` must include all semantically significant fields.
3. The grounding layer correctly propagates temporal bounds through variable binding, but the indexing layer silently discards them during interning.

### Secondary Design Issue: Temporal Proof Lift

The core fix (adding temporal to `AtomKey`) is correct and minimal. However, it creates a **semantic gap** for non-temporal rules referencing temporal facts. When `p[1,10]` now interns to a different `LitId` than `p` (base), rules with `p` in the body cannot see `p[1,10]` directly.

This necessitated a new mechanism — **temporal proof lift** — where proving `p[1,10]` also proves the base `p` under SDL conditions. This mechanism is novel to this branch and accounts for ~1,100 lines of new code in `defeasible.rs`, making the file grow from 913 to 2,003 lines.

The temporal proof lift interacts non-trivially with:

- **SDL ambiguity blocking** (new proofs create new attackers)
- **Retraction cascades** (retracting a base proof must cascade to dependents)
- **Self-defeating cycle detection** (a temporal lift can create `p ⇒ ~p` loops)
- **Post-hoc revalidation** (lifted proofs may invalidate earlier conclusions)
- **Query operators** (wildcard vs exact temporal matching)

## Branch Health Assessment: Circular Fixing Pattern

The branch exhibits a **convergence anti-pattern** identified in USDD as "oscillation disguised as activity." Specifically:

### Three Circular Cycles Observed

**Cycle 1: Cascade Guard Oscillation** (9 modifications to `retract_disproven_cascade`)

- `b747fff`: "Preserve `-d` for literals with zero Rsd rules"
- `756df4f`: Same guard needed again in a different code path
- `9b72d3d`: "Force-clear stale `-d` after cascade" — directly contradicts the preservation logic from `b747fff`
- `edcf259`: Exclude defeaters from cascade (same area, third time)

Each fix addressed a specific test failure but introduced state assumptions that broke in other scenarios.

**Cycle 2: Self-Defeating Check Evolution** (5 versions)

1. `12ea056`: Detect self-defeating cycles before committing lift
2. `475015c`: Include post-lift supporters
3. `3ff7dcb`: Count duplicate body occurrences (off-by-one)
4. `559e8e4`: Block on unresolved attackers
5. `d7d1c7e`: Extend to base attackers + add retry mechanism

Each version had an incomplete view of rule applicability state, requiring the next iteration.

**Cycle 3: Query Operator Over-/Under-Generalization**

- `fcd66a1`: Change `Literal::PartialEq` to include temporal bounds
- `c2f7025`: This breaks as-of queries → add `matches_query()` wildcard
- `e14605e`: Apply `matches_query` to rule-head scanning in `abduce`/`why_not`
- `14ba36e`: Apply `matches_query` to proven-set lookups
- `12b350d`: **Rollback** — body-premise checks must use exact equality, not wildcard

The `matches_query` migration was applied too broadly, then partially reversed.

### Diagnosis

The circular pattern stems from the **temporal proof lift** being developed incrementally inside the fix branch rather than designed upfront. The lift mechanism touches five subsystems (indexing, defeasible reasoning, retraction, revalidation, queries) with complex interactions that were discovered one failing test at a time.

This is consistent with the USDD "severity inversion" anti-pattern: later commits address more fundamental issues (self-defeating cycles, revalidation) than earlier ones (simple SDL guards), suggesting the design space was not fully mapped before implementation began.

## Resolution

### What the Core Fix Should Be

The core fix is **correct and minimal**: add `temporal: Temporal` to `AtomKey` (commit `fc1f632`). This is ~50 lines of change to `index.rs` and directly addresses the identity collapse.

### What Needs Architectural Review

The **temporal proof lift** mechanism (~1,100 lines of new code in `defeasible.rs`) requires architectural review before merge. Key questions:

1. **Is the lift mechanism specified?** There is no SPEC or ADR documenting the temporal subsumption semantics (when should `+d p[t]` imply `+d p`?). The current behaviour was defined by test cases, not by specification.

2. **Is the complexity justified?** The file more than doubled (913 → 2,003 lines). The lift interacts with every phase of defeasible reasoning. An alternative design — requiring explicit temporal bridging rules or treating temporal as metadata rather than identity — may be simpler.

3. **Is the cascade logic sound?** The retraction cascade was modified 13+ times with partially contradictory guards. A formal argument (or at minimum, property-based testing) is needed to establish that the cascade terminates and preserves SDL soundness.

4. **Are the query semantics specified?** The `matches_query` three-level matching system (exact / wildcard / body-premise) adds semantic complexity. The rules for when each level applies should be documented.

### Recommended Path

1. **Merge the core identity fix** (`AtomKey` + `temporal_base` map + `base_lit_id()`) as a minimal, correct change
2. **Write SPEC for temporal proof lift** before merging the lift mechanism — define subsumption semantics, SDL interaction, and query matching rules
3. **Property-test the cascade** — the 5 iterations of self-defeating check suggest edge cases that example-based tests alone cannot cover
4. **Consider factoring the branch** — separate the unrelated removals (arithmetic module, requires-verified, release tooling) from the temporal fix

### Regression Tests Required

The branch adds `temporal_proof_lift_tests.rs` (1,139 lines, 30+ tests) covering:

- Counter soundness (no double-decrement)
- SDL condition guards
- Self-defeating cycle detection
- Temporal complement lifting
- Cascading retractions
- Query operator temporal matching
- Attacker retry after discard

These tests should be retained regardless of how the lift mechanism is resolved.

## AI Detection Context

- **Detecting model:** Claude Opus 4.6
- **Detection method:** Adversarial review of branch commit history and code changes
- **Confidence:** High — identity collapse directly observed in `AtomKey` definition; circular fix pattern documented in commit messages
- **Session context:** Branch review session, 2026-03-04

## Trace

- Violates: SPEC-016 (Temporal Reasoning)
- Creates: Need for SPEC/ADR covering temporal proof lift semantics
- Regression tests: `crates/spindle-core/tests/temporal_proof_lift_tests.rs`
- Branch: `fix/temporal-atom-identity` (33 commits, not merged)
- Plan: `plans/FIX-001-temporal-atom-identity.spl`
