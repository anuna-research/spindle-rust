# Comprehensive Audit: Spindle-Rust v1.7.0

| Field | Value |
|---|---|
| Document ID | SPEC-016 |
| Title | Comprehensive Audit: Features, Issues, and Improvements |
| Version | 2.0.0 |
| Status | Active |
| Created | 2026-02-16 |
| Last Updated | 2026-02-16 |
| Authors | Claude (AI agent) |
| Reviewers | Core Maintainers |
| Plan | [AUDIT-001](../plans/AUDIT-001-comprehensive-audit.spl) |
| Protocol | [USDD Agent Protocol v1.0.0](../../handbook/engineering/usdd-agent-protocol.md) |

---

## 1. Executive Summary

This specification documents the findings of a comprehensive audit of spindle-rust v1.7.0 (commit `d53ad94`, branch `test/prop-test`). v2.0.0 is a full rewrite of v1.0.0, replacing findings derived from documentation (BUGS.md, regression tests) with findings derived from **direct implementation code review** of all core modules.

### Key Metrics

| Metric | Value |
|---|---|
| Workspace crates | 5 (core, parser, contract, cli, wasm) |
| Total source lines | ~45,000 |
| Total tests | 1,462 |
| Tests passing | 1,461 (1 ignored) |
| Clippy warnings | 0 |
| Actual bugs found (code review) | 5 |
| Performance issues found | 4 |
| Stale documentation | 3 items |
| Unimplemented specs | 3 (SPEC-010, IMPL-010, DESIGN-001) |
| `.expect()` / `.unwrap()` in non-test hot path | 8 |

### Severity Summary

| Severity | Count | Category |
|---|---|---|
| Critical | 1 | Temporal identity collapse in reasoning index |
| High | 3 | Abduce unverified, rule-level temporal bounds, stale regression tests |
| Medium | 6 | Panics in hot path, non-deterministic iteration, performance |
| Low | 8 | Dead code, doc coverage, feature gaps |

### v1.0.0 Corrections

Several findings in SPEC-016 v1.0.0 were derived from `BUGS.md` and `regression_known_bugs.rs` without verifying the current code. Direct code review revealed:

| v1.0.0 Claim | Actual Status | Evidence |
|---|---|---|
| OBS-001: Credulous semantics (no ambiguity blocking) | **FIXED** — ambiguity blocking works correctly | `defeasible.rs:766-787` unit test passes, asserting neither `+d q` nor `+d ~q` |
| OBS-003: Grounding ignores mode/temporal | **FIXED** — `match_literal` checks mode at line 90-91, temporal at 190-196 | `grounding.rs:86-93,190-196` |
| OBS-004: Empty-body non-fact rules never fire | **FIXED** — handled in Phase 1 (strict) and Phase 2 (defeasible) | `facts.rs:62-77`, `defeasible.rs:102-126` |
| OBS-005: Duplicate fact double-decrement | **FIXED** — deduplication via `enqueued` bitset | `facts.rs:47-49` |
| OBS-006.1: `why_not` misses defeaters | **FIXED** — regression test asserts correct behavior | `regression_known_bugs.rs:297-355` |
| OBS-006.3: `explain()` stack overflow | **FIXED** — regression test passes | `regression_known_bugs.rs:462-543` |

---

## 2. Scope

**In scope:**

- All 5 workspace crates: `spindle-core`, `spindle-parser`, `spindle-contract`, `spindle-cli`, `spindle-wasm`
- Direct reading and analysis of implementation source code
- Existing specs in `specs/`
- CI/CD configuration in `.woodpecker/`
- Test infrastructure and coverage

**Out of scope:**

- Performance benchmarking (no benchmark suite exists)
- WASM browser-side integration testing
- Formal verification (SPEC-015 is a separate concern)

---

## 3. Observations

### 3.1 Correctness

#### OBS-001: Temporal Identity Collapse in IndexedTheory

**Severity:** Critical
**Location:** `crates/spindle-core/src/index.rs:70-75,134-157`

The `AtomKey` struct used for atom interning includes `functor`, `mode`, and `args` — but **excludes temporal information**:

```rust
struct AtomKey {
    functor: SymbolId,
    mode: (SymbolId, bool),
    args: Vec<SymbolId>,
}
```

This means `p[1,10]` and `p[20,30]` intern to the **same** `LitId`. The `LiteralBitSet` used for proven-tracking and body counter decrements operates at the `LitId` level. Consequence: if `p[1,10]` is proven as a fact, a rule body requiring `p[20,30]` has its body counter decremented, because both map to the same `LitId`.

**Code path:**
1. `IndexedTheory::intern_literal()` (index.rs:134) builds `AtomKey` without temporal
2. Body counter in `ReasoningState::definite_body_remaining` is keyed by rule label, decremented when any temporal variant of a body literal fires
3. `forward_chain_strict()` (definite.rs:30) iterates `indexed.rules_with_body(&lit)` — the body index maps `LitId` (temporal-collapsed) to rule labels

**Impact:** Rules with concrete temporal body literals fire incorrectly when a different temporal window of the same predicate is proven. This is the underlying mechanism behind BUGS.md bug #1.

**Note:** The comment at index.rs:200 says "Temporal lost in identity, which is correct for reasoning" — this is incorrect for rules with concrete temporal body bounds.

#### OBS-002: Rule-Level Temporal Bounds Not Checked in Pipeline

**Severity:** High
**Location:** `crates/spindle-core/src/pipeline/mod.rs`

The `filter_temporal` pipeline step checks literal-level temporal bounds (`rule.head[*].temporal`, `rule.body[*].temporal`) but not `rule.temporal`. A rule with an inactive rule-level temporal window fires if its literal-level temporals pass the filter.

**Evidence:** Documented in `BUGS.md` bug #1 and confirmed by code review of the pipeline module.

#### OBS-003: `abduce` Does Not Verify Solutions Against Defeaters

**Severity:** High
**Location:** `crates/spindle-core/src/query/abduce.rs`

The `abduce` function proposes sets of facts that satisfy rule bodies, but does not verify that adding those facts actually produces the target conclusion. When a proposed fact also activates a defeater that blocks the goal, the solution is invalid.

**Evidence:** `regression_known_bugs.rs:367-422` — the test adds proposed facts to the theory, re-reasons, and confirms the goal is NOT provable. The test documents this as a known bug without failing.

#### OBS-004: Unreachable Panic in Grounding Loop

**Severity:** Low (dead code)
**Location:** `crates/spindle-core/src/grounding.rs:596-598`

```rust
for iteration in 0..max_iterations {
    if iteration >= max_iterations {
        panic!("Max iterations ({max_iterations}) reached, possible infinite loop");
    }
```

The `if` condition is always false because the `for` loop bounds `iteration < max_iterations`. The panic is unreachable dead code. The `limit_hit` flag at line 678 already handles the limit correctly.

#### OBS-005: Ambiguity Blocking Is Working Correctly

**Severity:** Informational (positive finding — corrects v1.0.0 OBS-001)
**Location:** `crates/spindle-core/src/reason/defeasible.rs:347-503`

`try_prove_defeasible` correctly implements SDL condition (3): for EVERY applicable attacker `s ∈ R[~q]`, either `s` is discarded, or there exists `t ∈ Rsd[q]` with `t` applicable AND `t > s`. When no superiority resolves the conflict, the function returns without proving `+d q`.

**Evidence:**
- Unit test `test_ambiguity_blocking_no_superiority` (defeasible.rs:766-787) asserts neither `+d q` nor `+d ~q` — **passes**.
- Unit test `test_superiority_resolves_ambiguity` (defeasible.rs:790-806) asserts `+d q` when `r1 > r2` — **passes**.
- The `regression_known_bugs.rs` test uses a permissive assertion that accepts both the buggy and correct behavior. The correct behavior branch is what actually executes.

#### OBS-006: Mode-Aware Grounding Is Working Correctly

**Severity:** Informational (positive finding — corrects v1.0.0 OBS-003)
**Location:** `crates/spindle-core/src/grounding.rs:84-199`

`match_literal` checks mode compatibility at line 90-91:
```rust
if pattern.mode != ground.mode {
    return None;
}
```

And `fact_index_key` (line 353) includes `Mode` in the index key. Tests `test_match_literal_mode_mismatch` (line 1639) and `test_ground_theory_mode_discrimination` (line 1686) verify this behavior and pass.

Concrete temporal bounds are checked at lines 190-196, rejecting matches when non-variable temporal bounds differ.

### 3.2 Code Quality

#### OBS-007: `.unwrap()` / `.expect()` Panics in Reasoning Hot Path

**Severity:** Medium
**Location:** Multiple files in `crates/spindle-core/src/reason/`

8 panic points in non-test reasoning code:

| Location | Expression | Risk |
|---|---|---|
| `definite.rs:37` | `.unwrap()` on `get_mut(rule.label)` | Panics if label missing from body_remaining |
| `definite.rs:44` | `.expect("Head literal missing from index")` | Panics if head not interned |
| `defeasible.rs:112` | `.expect("Head literal missing from index")` | Same |
| `defeasible.rs:142` | `.unwrap()` on `defeasible_body_remaining` | Panics if label missing |
| `defeasible.rs:147` | `.unwrap()` on `rule_discarded` | Panics if label missing |
| `defeasible.rs:155` | `.unwrap()` on `theory.get_rule()` | Panics if rule deleted |
| `defeasible.rs:167` | `.expect("Head literal missing from index")` | Same |
| `defeasible.rs:219,268` | `.expect("Head literal missing from index")` | Same (2 more) |

These are invariant-preserving (the data should exist if init ran correctly), but panics in a library crate are problematic. Should return `Result` errors.

#### OBS-008: String Allocation in Fixed-Point Hot Loop

**Severity:** Medium (performance)
**Location:** `crates/spindle-core/src/reason/defeasible.rs:131-135`

```rust
let rules_with_q: Vec<String> = indexed
    .rules_with_body_id(q_id)
    .iter()
    .map(|r| r.label.clone())
    .collect();
```

On every iteration of the fixed-point worklist loop, all rule labels containing the current literal in their body are cloned into a `Vec<String>`. For a theory with 1,000 rules and a worklist of 500 items, this creates ~500 allocations of varying-size `Vec<String>`.

**Fix:** Borrow references instead of cloning labels, or restructure to avoid the intermediate collection.

#### OBS-009: O(n*m) Conclusion Scan in Defeasible Seeding

**Severity:** Medium (performance)
**Location:** `crates/spindle-core/src/reason/defeasible.rs:51-63`

For each `+D` literal, the code does a linear scan of **all** conclusions to find the matching `+D` conclusion:

```rust
let (lit, definite_label) = state
    .conclusions
    .iter()
    .find_map(|c| {
        if c.conclusion_type == ConclusionType::DefinitelyProvable
            && indexed.get_lit_id(&c.literal) == Some(lit_id)
        { ... }
    })
    .unwrap_or_else(|| (indexed.resolve_literal(lit_id), None));
```

With `n` proven literals and `m` conclusions, this is O(n*m). A `HashMap<LitId, usize>` index built once would reduce this to O(n).

#### OBS-010: Vec Allocation on Every Index Lookup

**Severity:** Low (performance)
**Location:** `crates/spindle-core/src/index.rs:224-256`

`rules_with_head_id()` and `rules_with_body_id()` return `Vec<&Rule>` on every call, allocating a new Vec. These are called in tight loops in both `forward_chain_strict()` and `resolve_defeasible()`. Returning an iterator or a borrowed slice would avoid these allocations.

#### OBS-011: Non-Deterministic Rule Iteration

**Severity:** Medium
**Location:** `crates/spindle-core/src/theory.rs:45`

```rust
rules: HashMap<RuleLabel, Rule>,
```

`HashMap` does not preserve insertion order. `theory.rules()` returns rules in arbitrary (hash-dependent) order. This means reasoning output order may vary between runs, though the set of conclusions should be identical.

**Impact:** Non-deterministic output ordering makes test assertions fragile and diff-based debugging harder. The CLI compensates by sorting conclusions before output (reason.rs:93-96), but internal processing order varies.

#### OBS-012: Zero Clippy Warnings

**Severity:** Informational (positive finding)

`cargo clippy --workspace` reports zero warnings. The codebase is lint-clean.

#### OBS-013: `try_prove_defeasible` Has 10 Parameters

**Severity:** Low
**Location:** `crates/spindle-core/src/reason/defeasible.rs:350-362`

```rust
#[allow(clippy::too_many_arguments)]
fn try_prove_defeasible(
    q: LitId,
    indexed: &IndexedTheory<'_>,
    theory: &Theory,
    definite_proven: &LiteralBitSet,
    defeasible_proven: &mut LiteralBitSet,
    defeasible_disproven: &mut LiteralBitSet,
    body_remaining: &FxHashMap<&str, usize>,
    rule_discarded: &FxHashMap<&str, bool>,
    worklist: &mut VecDeque<(LitId, bool)>,
    conclusions: &mut Vec<Conclusion>,
)
```

This function deconstructs `ReasoningState` into individual borrows to satisfy Rust's borrow checker (can't pass `&mut state` while also reading `state.definite_proven`). The `#[allow]` annotation suppresses the lint. A "proof context" struct with split borrows would be cleaner.

### 3.3 Testing

#### OBS-014: Strong Test Suite Baseline

**Severity:** Informational (positive finding)

1,462 tests, all passing. Test types include:
- Unit tests (in-module `#[cfg(test)]`) in every core module
- Integration tests (`crates/*/tests/`): 22 test files
- Property tests (proptest): 7 files covering reasoning, grounding, temporal, structural, query, parser, and adversarial scenarios
- Regression tests targeting 6 historical bugs
- Fixture-based SDL conformance tests

#### OBS-015: Stale Regression Test Documentation

**Severity:** High
**Location:** `crates/spindle-core/tests/regression_known_bugs.rs`

The regression test file documents 6 bugs. Code review reveals:

| Bug | regression_known_bugs.rs Status | Actual Code Status |
|---|---|---|
| Bug 1: Credulous semantics | "KNOWN BUG" (permissive assertion) | **FIXED** — `defeasible.rs` unit test confirms correct ambiguity blocking |
| Bug 2: Empty-body rules | "STATUS: FIXED" | **FIXED** — confirmed |
| Bug 3: Duplicate fact | "STATUS: FIXED" | **FIXED** — confirmed |
| Bug 4: `why_not` defeaters | "STATUS: FIXED" | **FIXED** — confirmed |
| Bug 5: `abduce` unverified | "KNOWN BUG" (permissive assertion) | **STILL PRESENT** |
| Bug 6: `explain()` cycles | "STATUS: FIXED" | **FIXED** — confirmed |

**Problem:** Bug 1's test still says "KNOWN BUG" in comments and uses a permissive `if/else` that never fails. This is misleading — the bug IS fixed. The test should assert correct behavior directly.

#### OBS-016: Sparse Doc-Tests

**Severity:** Low

Only 2 `/// # Examples` sections exist across the entire codebase. Public API items lack usage examples.

#### OBS-017: No CLI Integration Tests for Contract Conformance

**Severity:** Medium

`specs/SPINDLE-RUST-IMPLEMENTATION.md` Section 6 specifies `contract_matrix_tests.rs` and `contract_guard_tests.rs` — these files do not exist.

#### OBS-018: No Benchmarks

**Severity:** Low

No benchmark suite exists. For a reasoning engine, performance regression detection is valuable.

### 3.4 Architecture

#### OBS-019: CLI Module Decomposition Done

**Severity:** Informational (positive finding — corrects v1.0.0 OBS-007)

The CLI is decomposed into modules under `crates/spindle-cli/src/cli/`:
- `commands/` — individual command handlers (reason, query, explain, etc.)
- `output.rs` — centralized output boundary with `emit_and_exit`
- `error.rs` — `CliError` type with RFC 9457 `ProblemDetails`
- `redact.rs` — error message sanitization

This is no longer a monolithic `main.rs`.

#### OBS-020: Duplicate Transport Types Across CLI and WASM

**Severity:** Medium
**Location:** `crates/spindle-cli/src/cli/commands/reason.rs` and `crates/spindle-wasm/src/lib.rs`

CLI and WASM both define their own output types (`ConclusionEntry` / `JsConclusionStruct`, `ReasonOutput` / `JsReasonOutput`). `spindle-contract` crate exists to share these but migration is incomplete.

#### OBS-021: Crate Dependency Architecture Is Sound

**Severity:** Informational (positive finding)

The dependency graph is acyclic and well-layered. No circular dependencies.

### 3.5 Feature Completeness

#### OBS-022: Error Module (SPEC-010) Partially Implemented

**Severity:** Medium

SPEC-010 defines RFC 9457 error model. Current status:
- `CliError` type exists with `code`, `message`, `ProblemDetails` fields
- `emit_and_exit` renders JSON error envelopes
- `--explain CODE` mechanism: not implemented
- `source_context` line highlighting: not implemented

#### OBS-023: CLI Contract Implementation Gaps

**Severity:** Medium

Multiple flags specified in `SPINDLE-CONTRACT.md` are not implemented: `--at`, `--given`, `--givens-file`, `--source-weights-file`, `--trust-policy-file`, `--trust-mode`, `--timeout-ms`, `--max-solutions`, `--max-ground-instances`, `--max-input-bytes`, `--max-trust-contributors`.

#### OBS-024: WASM Crate Lacks Feature Parity

**Severity:** Low

`spindle-wasm` supports `reason` and SPL parsing but does not expose `query`, `why_not`, `explain`, `requires`, `what_if`, `validate`, or `stats`.

#### OBS-025: Trust Module Implemented but Not Pipeline-Integrated

**Severity:** Low
**Location:** `crates/spindle-core/src/trust.rs`, `crates/spindle-core/src/pipeline/mod.rs`

The trust module has rich types (`TrustPolicy`, `WeightedConclusion`, `DecayModel`). The pipeline computes weighted conclusions post-reasoning (in `pipeline/mod.rs`), but `reason()` itself does not use trust weights to influence derivation. Trust is a post-processing overlay, not an inference-time feature.

### 3.6 Specification Health

#### OBS-026: Three Drafted Specs Not Yet Fully Implemented

**Severity:** Medium

| Spec | Status | Description |
|---|---|---|
| SPEC-010 (Error Module) | Draft v0.6.0, partially implemented | RFC 9457 error model |
| IMPL-010 (Simplification) | Draft, partially implemented | CLI decomposition done; contract crate incomplete |
| DESIGN-001 (Contract Crate) | Draft | Crate exists but migration incomplete |

#### OBS-027: BUGS.md Partially Stale

**Severity:** Low

`BUGS.md` documents 2 bugs. Bug #2 (grounding matches across modes) has been fixed — `match_literal` now checks mode at line 90-91. Bug #1 (temporal bounds ignored) is real but the root cause is in the index (OBS-001), not the pipeline location cited.

#### OBS-028: Formal Semantics Spec Is Accurate and Complete

**Severity:** Informational (positive finding)

`specs/DEFEASIBLE-LOGIC-SEMANTICS.md` provides rigorous pseudocode for ambiguity blocking, worked examples, termination proofs, and data structure definitions. The implementation in `defeasible.rs` closely follows this spec.

---

## 4. Requirements

### Functional Requirements (Correctness — Priority Critical)

REQ-001: Temporal-Aware Atom Identity

The `IndexedTheory` atom interning SHALL include temporal bounds in the `AtomKey` so that `p[1,10]` and `p[20,30]` are distinct `LitId` values. Body counter decrements SHALL only apply to rules whose body literals have matching temporal windows.

Trace:
- OBS-001
- TEST-001

REQ-002: Rule-Level Temporal Filtering

The `filter_temporal` pipeline step SHALL check `rule.temporal` in addition to literal-level temporal bounds.

Trace:
- OBS-002
- TEST-002

### Functional Requirements (High Priority)

REQ-003: Abduce Solution Verification

`abduce` SHALL verify that proposed solutions actually produce the target conclusion by re-reasoning with the proposed facts added. Solutions that are blocked by defeaters or conflicts SHALL be marked as invalid or excluded.

Trace:
- OBS-003
- TEST-003

REQ-004: Clean Up Stale Regression Tests

`regression_known_bugs.rs` SHALL be updated: Bug 1 test SHALL assert correct ambiguity blocking behavior directly (remove permissive `if/else`). Bugs 2, 3, 4, 6 tests SHALL assert correct behavior directly (they already do via `assert!` but comments are misleading). Bug 5 test SHALL use `#[ignore = "known bug: abduce unverified"]` with correct-behavior assertions.

Trace:
- OBS-015
- TEST-004

### Functional Requirements (Code Quality — Priority Medium)

REQ-005: Library Crate Panic Safety

All `.unwrap()` and `.expect()` calls in the reasoning hot path (8 locations in OBS-007) SHALL be replaced with `Result` returns or documented `// SAFETY:` invariant proofs.

Trace:
- OBS-007
- TEST-005

REQ-006: Remove Dead Code in Grounding

The unreachable panic at `grounding.rs:596-598` SHALL be removed.

Trace:
- OBS-004

### Functional Requirements (Performance — Priority Medium)

REQ-007: Reduce Allocations in Defeasible Fixed-Point

The fixed-point loop in `defeasible.rs` SHALL avoid cloning rule labels into `Vec<String>` on every iteration. The O(n*m) conclusion scan SHALL be replaced with an indexed lookup.

Trace:
- OBS-008, OBS-009
- TEST-006

### Functional Requirements (Architecture — Priority Medium)

REQ-008: Complete Transport Type Migration

`spindle-cli` and `spindle-wasm` SHALL use shared types from `spindle-contract` for all structured output. Duplicate type definitions SHALL be removed.

Trace:
- OBS-020
- DESIGN-001

### Functional Requirements (Testing — Priority Medium)

REQ-009: CLI Contract Conformance Tests

The CLI SHALL have integration tests validating JSON output shape, exit codes, and determinism as specified in `SPINDLE-CONTRACT.md`.

Trace:
- OBS-017

### Functional Requirements (Features — Priority Low)

REQ-010: CLI `--at` Flag

The CLI SHALL implement `--at <rfc3339>` for temporal "as-of" queries.

Trace:
- OBS-023
- `specs/SPINDLE-CONTRACT.md`

REQ-011: CLI `--given` Flags

The CLI SHALL support `--given "<spl-literal>"` and `--givens-file <path>`.

Trace:
- OBS-023

### Non-Functional Requirements

NFR-001: Reasoning Performance Baseline

A benchmark suite SHALL be established using `criterion`, covering small, medium, and large theories.

Trace:
- OBS-018
- TEST-007

NFR-002: Doc-Test Coverage

All public API items with non-trivial behavior SHALL have at least one `/// # Examples` doc-test.

Trace:
- OBS-016

---

## 5. Architecture Decisions

ADR-001: Fix Temporal Identity Before Other Reasoning Changes

**Decision:** Fix `AtomKey` to include temporal bounds (REQ-001) as the first correctness fix.

**Rationale:** The temporal identity collapse affects all reasoning involving temporal literals. It is the root cause of BUGS.md bug #1 and manifests as incorrect rule firing when multiple temporal windows exist for the same predicate. Unlike the original assessment that ambiguity blocking was broken (it isn't), this is the actual critical bug.

**Alternatives Considered:**
1. Add temporal to `LitId` directly — rejected because it would increase `LitId` size beyond 4 bytes, hurting bitset performance.
2. Keep temporal-collapsed identity but filter at rule-fire time — viable but adds complexity to the hot path.
3. Include temporal in `AtomKey` — chosen; cleanest fix, ensures body counters track the correct temporal window.

ADR-002: Update Regression Tests to Match Reality

**Decision:** Update `regression_known_bugs.rs` to reflect actual fix status. Convert permissive assertions to direct assertions for fixed bugs.

**Rationale:** The current file is misleading — it documents Bug 1 as "KNOWN BUG" with permissive assertions, but the bug is actually fixed. This wastes developer time investigating non-issues.

ADR-003: Prioritize Error Module Implementation in Phases

**Decision:** Implement SPEC-010 in phases: (1) stabilize existing `CliError`/`ProblemDetails`, (2) `--explain CODE`, (3) source context. Phase 1 is partially done.

**Rationale:** Phased delivery avoids a large destabilizing change.

---

## 6. Recommended Prioritization

### Phase 1: Critical Correctness

| ID | Description | Effort | Risk |
|---|---|---|---|
| REQ-001 | Temporal-aware atom identity | Medium | Medium — affects bitset sizing and index structure |
| REQ-002 | Rule-level temporal filtering | Small | Low |

### Phase 2: Cleanup & Stability

| ID | Description | Effort | Risk |
|---|---|---|---|
| REQ-004 | Clean up stale regression tests | Small | Low |
| REQ-005 | Remove panics from hot path | Small | Low |
| REQ-006 | Remove dead code in grounding | Trivial | Low |
| REQ-007 | Reduce allocations in fixed-point | Small | Low |

### Phase 3: Features & Architecture

| ID | Description | Effort | Risk |
|---|---|---|---|
| REQ-003 | Abduce solution verification | Medium | Low |
| REQ-008 | Transport type migration | Medium | Low |
| REQ-009 | CLI contract conformance tests | Medium | Low |
| REQ-010 | `--at` flag | Medium | Low |
| REQ-011 | `--given` flags | Small | Low |

### Phase 4: Polish

| ID | Description | Effort | Risk |
|---|---|---|---|
| NFR-001 | Benchmark suite | Medium | Low |
| NFR-002 | Doc-test coverage | Small | Low |
| OBS-022 | SPEC-010 full implementation | Large | Medium |

---

## 7. Testing Strategy

### Test Specifications

TEST-001: Temporal-Aware Atom Identity (REQ-001)
- Theory: fact `p[1,10]`, rule body requires `p[20,30]`. Assert: rule does NOT fire.
- Theory: fact `p[1,10]`, rule body requires `p[1,10]`. Assert: rule fires.
- Verify `IndexedTheory::intern_literal` returns different `LitId` for `p[1,10]` vs `p[20,30]`.

TEST-002: Rule-Level Temporal Filtering (REQ-002)
- Rule with `temporal = [1000, 2000]`, reference_time = 3000. Assert: rule excluded.
- Rule with `temporal = [1000, 2000]`, reference_time = 1500. Assert: rule included.

TEST-003: Abduce Solution Verification (REQ-003)
- Goal `q` with rule `p => q` and defeater `p ~> ~q`. Assert: `abduce(q)` returns no valid solutions (or marks the solution as unverified).

TEST-004: Regression Test Cleanup (REQ-004)
- All tests in `regression_known_bugs.rs` either assert correct behavior directly or use `#[ignore]`.
- `cargo test` does not print "KNOWN BUG" messages for fixed bugs.

TEST-005: No Panics on Reasoning (REQ-005)
- Run `cargo test` with no panics. Fuzz test with proptest covering edge cases.
- Specifically: theory with rules referencing unlabeled/missing rules should return `Err`, not panic.

TEST-006: Allocation Reduction (REQ-007)
- Before/after benchmarks for medium theory (1,000 rules) showing reduced allocation count.

TEST-007: Performance Baseline (NFR-001)
- `criterion` benchmarks: penguin (5 rules), medium (1,000 rules), grounded (100 var rules x 50 facts).

---

## 8. Traceability Matrix

```
REQ-001  -->  TEST-001     -->  index.rs (AtomKey)
REQ-002  -->  TEST-002     -->  pipeline/mod.rs
REQ-003  -->  TEST-003     -->  query/abduce.rs
REQ-004  -->  TEST-004     -->  tests/regression_known_bugs.rs
REQ-005  -->  TEST-005     -->  reason/definite.rs, reason/defeasible.rs
REQ-006  -->  (code review) -->  grounding.rs:596-598
REQ-007  -->  TEST-006     -->  reason/defeasible.rs
REQ-008  -->  DESIGN-001   -->  spindle-contract/
REQ-009  -->  (new tests)  -->  cli/tests/
REQ-010  -->  SPINDLE-CONTRACT  -->  cli commands
REQ-011  -->  SPINDLE-CONTRACT  -->  cli commands
NFR-001  -->  TEST-007     -->  benches/
NFR-002  -->  (doc-tests)  -->  src/**/*.rs
```

---

## 9. Relationship to Existing Specs

| Spec | Relationship |
|---|---|
| DEFEASIBLE-LOGIC-SEMANTICS.md | Implementation follows this spec; ambiguity blocking is correct |
| spindle-rust-fixes-spec.md | REQ-001, REQ-002 overlap with milestones documented there |
| SPINDLE-CONTRACT.md | REQ-009, REQ-010, REQ-011 implement contract requirements |
| ERROR-MODULE-SPEC.md (SPEC-010) | OBS-022 tracks implementation status; Phase 4 dependency |
| IMPL-010 | OBS-019 shows CLI decomposition is partially done |
| DESIGN-001 (Contract Crate) | REQ-008 completes the transport type consolidation |
| BUGS.md | OBS-027 — Bug #2 is fixed; Bug #1 root cause is OBS-001 |

---

## 10. AI Trust Boundary

| Field | Value |
|---|---|
| Model | Claude Opus 4.6 |
| Prompts | User requested comprehensive audit of spindle-rust with direct code review |
| Inputs | Full implementation source read of all core modules |
| Outputs | This specification (v2.0.0) |
| Reviewer | Pending human review |
| Decision | Pending |

**Findings verified by direct code review (files read):**

| File | Lines | Key Findings |
|---|---|---|
| `reason/defeasible.rs` | 913 | Ambiguity blocking correct; 6 `.expect()` panics; allocation in hot loop |
| `reason/definite.rs` | 273 | 2 panics (`.unwrap()`, `.expect()`) |
| `reason/facts.rs` | 342 | Deduplication working; empty-body handling correct |
| `reason/state.rs` | 276 | `LiteralBitSet` grows dynamically; monotonic invariant maintained |
| `reason/mod.rs` | 1,927 | Three-phase orchestration correct; `Reasoner` trait clean |
| `index.rs` | 331 | **Temporal excluded from AtomKey** — root cause of OBS-001 |
| `grounding.rs` | 2,119 | Mode checking correct; dead panic at line 597; recursive matching |
| `theory.rs` | 500 | `HashMap` for rules — non-deterministic iteration |
| `regression_known_bugs.rs` | 628 | 4/6 bugs fixed; stale documentation |

**Findings NOT verified (require additional investigation):**
- OBS-002 (rule-level temporal filtering) — pipeline module read by agent, not fully verified by primary reviewer
- OBS-024 (trust integration) — requires domain knowledge on intended design

---

## Document History

| Version | Date | Author | Changes |
|---|---|---|---|
| 1.0.0 | 2026-02-16 | Claude (AI agent) | Initial audit (documentation-based) |
| 2.0.0 | 2026-02-16 | Claude (AI agent) | Full rewrite based on direct implementation code review. Corrected 6 false findings from v1.0.0. Added 5 new findings from code. |

---

**END OF SPECIFICATION**
