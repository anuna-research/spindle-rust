| Field | Value |
|---|---|
| Document ID | SPEC-019 |
| Title | Incremental Superiority Resolution for Defeasible Reasoning |
| Version | 1.0.0 |
| Status | Draft |
| Created | 2026-03-06 |
| Last Updated | 2026-03-06 |
| Authors | Claude (AI agent) |
| Reviewers | Core Maintainers |
| Protocol | USDD Agent Protocol v1.3.0 |
| Traces | SPEC-018 (Temporal Atom Identity), BUG-002 |

---

# SPEC-019: Incremental Superiority Resolution for Defeasible Reasoning

## 1. Executive Summary

### The Regression

The defeasible reasoning phase (`try_prove_defeasible` in `defeasible.rs`) performs a full rescan of all attackers and all supporters every time it evaluates whether a literal can be defeasibly proved. For a literal `q`:

1. It collects all applicable supporters `Rsd[q]` — O(|Rsd[q]|)
2. For **each** attacker in `R[~q]`, it scans **all** applicable supporters checking `is_superior(t, s)` — O(|R[~q]| x |Rsd[q]|)

This makes a single call to `try_prove_defeasible` cost O(|R[~q]| x |Rsd[q]|). Since the function is called repeatedly from the worklist for potentially every literal in the theory, Phase 2 overall runs in **O(L x R^2)** where L is the number of literals and R is the maximum number of rules per literal.

### SPINdle Java's O(N) Approach

SPINdle Java (v2.2.4) uses **incremental undefeated-attacker counters**. For each literal `q`, it maintains:

- `undefeated_attacker_count[q]`: the number of applicable attackers in `R[~q]` that have not yet been defeated by any superior supporter
- When a supporter `t` becomes applicable, it iterates only over the attackers that `t` specifically defeats (via the precomputed superiority index) and decrements the counter
- When an attacker `s` becomes applicable, it checks if any already-applicable supporter defeats it; if not, it increments the counter
- `+d q` is proved when the counter reaches zero (and at least one supporter is applicable)

This makes each rule application O(|sup(t)|) amortized — proportional to the number of superiority relations involving that rule — and the total Phase 2 cost is **O(N)** where N = |rules| + |superiority relations| + |literals|.

### Impact

For theories with many conflicting rules per literal (common in policy reasoning, legal reasoning, and multi-agent trust scenarios), the current implementation is quadratically slower than necessary. A theory with 100 rules for/against a single literal performs ~10,000 comparisons per evaluation instead of ~100.

---

## 2. Root Cause Analysis

### Current Code Path

In `crates/spindle-core/src/reason/defeasible.rs`, lines 351-504, `try_prove_defeasible`:

```rust
// Line 395-410: Collect ALL applicable supporters (full scan of Rsd[q])
let applicable_supporters: Vec<&Rule> = supporting_rules
    .iter()
    .filter(|r| /* applicable check */)
    .collect();

// Line 412-453: For EACH attacker, scan ALL applicable supporters
for attacker in &attacking_rules {
    // ...
    let defeated_by_superior = applicable_supporters
        .iter()
        .any(|t| theory.is_superior(t.template_label(), attacker.template_label()));
    // ...
}
```

The same pattern exists in `try_disprove_defeasible` (lines 508-617) with the mirror condition.

### Why This Happens

The functions are stateless with respect to superiority resolution. They recompute the full attacker/supporter relationship from scratch on every invocation. There is no persistent data structure tracking which attackers have been defeated as supporters become applicable over time.

### Contrast with Phase 1

Phase 1 (`definite.rs`) correctly uses incremental `body_remaining` counters: when a literal is proved, only the rules containing it in their body are updated (O(1) per rule). Phase 2 already uses these same body counters for rule applicability, but does not extend the incremental approach to superiority checking.

---

## 3. Requirements

### REQ-001: Incremental Attacker Tracking

The system SHALL maintain, for each literal `q`, a count of undefeated applicable attackers. This count SHALL be updated incrementally when:
- An attacker rule becomes applicable (body fully satisfied): increment if no existing applicable supporter defeats it
- An attacker rule becomes discarded (body literal disproved): decrement
- A supporter rule becomes applicable: decrement for each attacker it defeats
- A supporter rule becomes discarded: re-evaluate and increment for any attacker that was previously defeated only by that supporter

Acceptance criteria:
- The count is never negative
- The count reaches zero if and only if all applicable attackers are defeated by at least one superior applicable supporter
- `+d q` is emitted when: (1) at least one applicable Rsd[q] supporter exists, (2) `-D ~q`, and (3) the undefeated attacker count is zero

Trace:
- TEST-001, TEST-002, TEST-003

### REQ-002: Precomputed Superiority Index

The system SHALL precompute, during index construction, a mapping from each rule label to the set of rules it defeats (its inferiors) and the set of rules that defeat it (its superiors). This mapping SHALL be derived from the `SuperiorityIndex` and the head-index so that, given a supporter `t` for `q`, the set of attackers in `R[~q]` that `t` defeats can be retrieved in O(1).

Acceptance criteria:
- Lookup of "which attackers does supporter t defeat?" is O(1) (hash lookup + iterate over precomputed set)
- The precomputed index is consistent with `theory.is_superior()`
- The index is built once during `IndexedTheory::build()` and not mutated during reasoning

Trace:
- TEST-004, TEST-005

### REQ-003: Semantic Equivalence

The system SHALL produce identical conclusions (same set of `+d`, `-d`, `+D`, `-D` conclusions for all literals) as the current implementation for all valid theories.

Acceptance criteria:
- All existing tests in `defeasible.rs`, `reason.rs`, and integration tests pass without modification
- Differential testing (proptest) confirms equivalence on 1000+ random theories
- The penguin example, Nixon diamond, and all SPL examples produce identical output

Trace:
- TEST-006, TEST-007, TEST-008

### REQ-004: try_prove_defeasible Amortized Cost

A single invocation of `try_prove_defeasible(q)` SHALL NOT iterate over the full set of supporters when checking superiority against attackers. Instead, the function SHALL consult the precomputed attacker count.

Acceptance criteria:
- The function body contains no nested loop over `applicable_supporters x attacking_rules` for superiority checking
- The function performs O(1) work to determine if the attacker count is zero (beyond the existing applicability checks)

Trace:
- TEST-009, NFR-001

### REQ-005: Strict Attacker Bypass

Strict attackers (rules with `RuleType::Strict`) SHALL NOT be subject to superiority defeat. An applicable strict attacker SHALL permanently block `+d q` regardless of any superiority relation.

This preserves the existing semantics where strict rules cannot be overridden by defeasible superiority.

Acceptance criteria:
- A strict attacker with a satisfied body always blocks `+d q`
- The undefeated attacker count treats strict attackers as unconditionally undefeated

Trace:
- TEST-010

### REQ-006: Defeater Handling

Defeater rules (`RuleType::Defeater`) that become applicable SHALL contribute to the attacker count for the complement literal, consistent with current semantics. Defeaters block but do not prove.

Acceptance criteria:
- An applicable defeater for `~q` increments the undefeated attacker count for `q` if no superior supporter exists
- Defeaters participate in superiority as inferiors (can be defeated) but not as supporters

Trace:
- TEST-011

---

## 4. Non-Functional Requirements

### NFR-001: Phase 2 Time Complexity

Phase 2 (defeasible resolution) SHALL run in O(N) time where N = |rules| + |superiority_relations| + |literals|, amortized across the full fixed-point computation.

Acceptance criteria:
- Benchmark a theory with K conflicting rules per literal for K in {10, 50, 100, 500}
- Execution time scales linearly with K, not quadratically
- Regression benchmark added to CI

Trace:
- TEST-009

### NFR-002: Memory Overhead

The additional memory for the attacker-count data structure SHALL be O(L) where L is the number of literals, plus O(S) where S is the number of superiority relations for precomputed lookups.

Acceptance criteria:
- No per-rule-pair allocation (no `O(R^2)` structure)
- Memory increase over current implementation is bounded by `2 * sizeof(usize) * |literals| + |superiority_relations| * sizeof(pointer)`

---

## 5. Architecture Decision

### ADR-001: Counter-Based Incremental Resolution vs. Lazy Caching

**Context:** Two approaches can eliminate the nested loop in `try_prove_defeasible`:

1. **Incremental counters** (SPINdle Java approach): Maintain `undefeated_attacker_count[q]` as a running integer, updated when rules change state. The `try_prove_defeasible` function checks `count == 0` instead of re-scanning.

2. **Lazy cache with invalidation**: Cache the result of `try_prove_defeasible(q)` and invalidate when any attacker or supporter of `q` changes state. Re-evaluate only on cache miss.

**Decision:** Incremental counters (option 1).

**Rationale:**

- **Proven design**: SPINdle Java has used this approach for 15+ years in production. The counter semantics are well-understood and formally analysed.
- **Precise updates**: When a supporter `t` becomes applicable, we know exactly which attackers it defeats (via `superiority_index.inferiors_of(t)`). Each decrement is O(1). No need to re-evaluate the entire literal.
- **Simpler invalidation**: The lazy cache approach requires tracking *which* supporters defeated *which* attackers to know when a cache entry is stale. This is essentially reconstructing the counter approach but with extra indirection.
- **Fits existing architecture**: Phase 1 already uses `body_remaining` counters in exactly this pattern. Phase 2's attacker counters are the natural extension.

**Trade-off:** The incremental approach requires careful bookkeeping when a supporter is discarded (must re-check if any attacker it was defeating is now undefeated). This adds complexity to the `-d` path. However, this is the same complexity SPINdle Java manages, and the `try_disprove_defeasible` function already handles this logic — it just does so by rescanning.

### ADR-002: Attacker Count Storage Location

**Context:** Where should the `undefeated_attacker_count` map live?

**Decision:** In `ReasoningState`, alongside the existing `defeasible_body_remaining` and `rule_discarded` maps.

**Rationale:**

- `ReasoningState` already owns all mutable reasoning state
- The count is per-literal (keyed by `LitId`), not per-rule, so it belongs with the literal-level bitsets (`defeasible_proven`, `defeasible_disproven`)
- No changes to `IndexedTheory` mutability are needed

### ADR-003: Precomputed Superiority Cross-Reference

**Context:** To efficiently find "which attackers does supporter `t` defeat?", we need a cross-reference between the superiority index and the head index.

**Decision:** During `ReasoningState` initialization (or as a separate setup step before Phase 2), precompute a map:

```
defeated_attackers: HashMap<RuleLabel, Vec<RuleLabel>>
```

For each rule `t` that heads literal `q`, `defeated_attackers[t]` contains the labels of all rules in `R[~q]` where `t > s`. This is built by iterating over all superiority pairs and filtering by head-index membership.

**Rationale:**

- Built once in O(|superiority_relations| x |head_index_lookup|) which is O(S) since head lookups are O(1)
- Queried in O(1) per supporter during reasoning
- Memory is O(S) — proportional to the number of superiority relations, which is typically small

---

## 6. Purity Boundary Map

### Pure Core (no I/O, no shared state, deterministic)

- `try_prove_defeasible(q)`: Reads attacker count, returns prove/no-prove decision
- `try_disprove_defeasible(q)`: Reads attacker count + supporter state, returns disprove/no-disprove decision
- Attacker count initialization: Computes initial counts from rule/superiority structure
- Defeated-attacker precomputation: Builds cross-reference from superiority + head indices

### Effectful Shell (orchestrates state mutation)

- `resolve_defeasible()`: Orchestrates the fixed-point loop, calls pure core functions, mutates `ReasoningState`
- Worklist management: Pushes/pops `(LitId, bool)` tuples

### Boundary Contracts

- `UndefeatedAttackerCount`: `FxHashMap<LitId, usize>` — count of undefeated applicable attackers per literal
- `DefeatedAttackerMap`: `FxHashMap<&str, Vec<&str>>` — for each supporter label, which attacker labels it defeats
- `ApplicableSupporterCount`: `FxHashMap<LitId, usize>` — count of applicable Rsd supporters per literal (for condition 1)

### Dependency Rule

Dependencies point inward: `resolve_defeasible` (shell) calls `try_prove_defeasible` (core). Core functions read but do not mutate worklist or conclusions directly — they return decisions that the shell applies.

### Enforcement

- `try_prove_defeasible` and `try_disprove_defeasible` should ideally return a decision enum rather than mutating state directly (stretch goal — current API can be preserved if the counter logic is correct)
- Code review verifies no nested attacker x supporter loops remain

---

## 7. Detailed Design

### 7.1 New Data Structures

Add to `ReasoningState`:

```rust
/// Number of applicable attackers for ~q that are NOT defeated by any
/// applicable supporter of q. When this reaches 0 and at least one
/// Rsd[q] supporter is applicable, +d q can be emitted.
pub(crate) undefeated_attackers: FxHashMap<LitId, usize>,

/// Number of applicable Rsd[q] supporters (strict + defeasible + fact).
/// Condition (1) of +d requires this to be > 0.
pub(crate) applicable_supporter_count: FxHashMap<LitId, usize>,

/// For each supporter rule label, the attacker rule labels it defeats
/// (precomputed from superiority index + head index).
pub(crate) defeated_by_supporter: FxHashMap<String, Vec<String>>,

/// For each attacker rule label, how many applicable supporters currently
/// defeat it. When this drops to 0, the attacker becomes "undefeated".
pub(crate) attacker_defeat_count: FxHashMap<String, usize>,
```

### 7.2 Initialization (Before Phase 2)

```
for each literal q in the theory:
    undefeated_attackers[q] = 0
    applicable_supporter_count[q] = 0

for each superiority pair (t > s):
    if head(t) = q and head(s) = ~q (or vice versa):
        defeated_by_supporter[t.label].push(s.label)

for each attacker rule s heading ~q:
    attacker_defeat_count[s.label] = 0
```

### 7.3 Updated Worklist Processing

When a rule `r` heading literal `h` becomes **applicable** (body_remaining reaches 0, not discarded):

```
if r is Rsd (strict/defeasible/fact):
    applicable_supporter_count[h] += 1
    for each attacker_label in defeated_by_supporter[r.label]:
        if attacker is applicable:
            if attacker_defeat_count[attacker_label] == 0:
                // This attacker was undefeated, now defeated
                undefeated_attackers[h] -= 1
            attacker_defeat_count[attacker_label] += 1
    check_prove_defeasible(h)

if r is attacker for ~q (any type including defeater):
    if r is strict:
        // Strict attackers are unconditionally undefeated
        undefeated_attackers[q] += 1  // permanent block
    else:
        if attacker_defeat_count[r.label] == 0:
            undefeated_attackers[q] += 1
    check_prove_defeasible(q)  // re-check, may now be blocked
```

When a rule `r` heading literal `h` becomes **discarded** (body literal disproved):

```
if r is Rsd:
    applicable_supporter_count[h] -= 1
    for each attacker_label in defeated_by_supporter[r.label]:
        attacker_defeat_count[attacker_label] -= 1
        if attacker_defeat_count[attacker_label] == 0 and attacker is applicable:
            // Attacker is now undefeated again
            undefeated_attackers[h] += 1
    check_disprove_defeasible(h)

if r is attacker for ~q:
    if attacker_defeat_count[r.label] == 0 (was undefeated):
        undefeated_attackers[q] -= 1
    check_prove_defeasible(q)  // may now be unblocked
```

### 7.4 Simplified try_prove_defeasible

```rust
fn try_prove_defeasible(q: LitId, state: &ReasoningState) -> bool {
    if state.defeasible_disproven.contains(q) { return false; }
    if state.defeasible_proven.contains(q) { return false; }

    // Condition 1: at least one applicable Rsd[q] supporter
    if state.applicable_supporter_count[q] == 0 { return false; }

    // Condition 2: -D ~q
    if state.definite_proven.contains(q.complement()) { return false; }

    // Condition 3: all attackers defeated
    state.undefeated_attackers[q] == 0
}
```

This is **O(1)** — three map lookups and two bitset checks.

---

## 8. Test Specifications

### TEST-001: Attacker Count Basic Lifecycle

Verify that the undefeated attacker count correctly tracks attacker state through the reasoning lifecycle.

**Scenario:** Theory with `p => q`, `p => ~q`, `p => q > p => ~q` (superiority).

**Steps:**
1. After fact seeding, both rules have body satisfied
2. Attacker `p => ~q` is defeated by supporter `p => q` via superiority
3. `undefeated_attackers[q]` should be 0
4. `+d q` is emitted

**Verifies:** REQ-001

### TEST-002: Attacker Count Without Superiority (Ambiguity Blocking)

**Scenario:** Theory with `p => q`, `p => ~q`, no superiority.

**Expected:** `undefeated_attackers[q] == 1`, `undefeated_attackers[~q] == 1`. Neither `+d q` nor `+d ~q` is emitted.

**Verifies:** REQ-001

### TEST-003: Attacker Count With Discarded Supporter

**Scenario:** Theory with `(p, r) => q`, `p => ~q`, `(p, r) => q > p => ~q`, but `r` is not provable.

**Expected:** Supporter `(p, r) => q` is discarded (r unprovable). Attacker `p => ~q` becomes undefeated. `+d ~q` may be emitted (or both blocked if undecided).

**Verifies:** REQ-001 (discard path)

### TEST-004: Precomputed Defeated-Attacker Map

**Scenario:** Theory with three rules and two superiority relations.

**Expected:** `defeated_by_supporter` map correctly maps each supporter to the specific attackers it defeats. Verify by inspecting the map after initialization.

**Verifies:** REQ-002

### TEST-005: Precomputed Map Consistency with is_superior

**Property-based test:** For all generated theories with superiority relations, the precomputed map agrees with `theory.is_superior()` for all supporter-attacker pairs sharing complementary heads.

**Verifies:** REQ-002

### TEST-006: Regression Test Suite Pass

All existing tests in `defeasible.rs` (8 tests), `reason.rs`, and all integration tests pass without modification.

**Verifies:** REQ-003

### TEST-007: Differential Testing — Standard Theories

Run the full SPL example suite (`examples/*.spl`) and compare output byte-for-byte with the pre-refactor implementation.

**Verifies:** REQ-003

### TEST-008: Differential Testing — Proptest Random Theories

Generate 1000+ random propositional theories with 1-50 rules, 1-20 literals, and 0-10 superiority relations. Compare conclusions between old and new implementations.

**Verifies:** REQ-003

### TEST-009: Performance Benchmark — Scaling

Benchmark theories with K conflicting rules per literal for K in {10, 50, 100, 500}:

```
For each K:
    Create theory: p is a fact
    Add K rules: r_i: p => q
    Add K rules: s_i: p => ~q
    Add K superiority relations: r_i > s_i
    Measure Phase 2 time
```

**Expected:** Time scales linearly with K (within 2x tolerance), not quadratically.

**Verifies:** NFR-001, REQ-004

### TEST-010: Strict Attacker Blocks Despite Superiority

**Scenario:** `p -> ~q` (strict), `p => q`, superiority `p => q > p -> ~q`.

**Expected:** `+d q` is NOT emitted because strict attackers cannot be defeated. `-d q` is emitted.

**Verifies:** REQ-005

### TEST-011: Defeater As Attacker

**Scenario:** `p => q`, `p ~> ~q` (defeater), no superiority.

**Expected:** Defeater blocks `+d q`. `+d q` is NOT emitted. Defeater does not prove `+d ~q`.

**Verifies:** REQ-006

---

## 9. Verification Strategy

| System characteristic | Technique | Scope |
|---|---|---|
| Core superiority resolution logic | Property-based testing | REQ-001, REQ-002 |
| Counter arithmetic invariants | Contract assertions (debug_assert) | REQ-001, REQ-004 |
| Semantic equivalence | Differential testing (proptest) | REQ-003 |
| Performance regression | Benchmark + CI | NFR-001 |
| Attacker/supporter lifecycle | Example-based testing | REQ-005, REQ-006 |

### Key Properties for Property-Based Testing

1. **Counter non-negativity**: `undefeated_attackers[q] >= 0` at all times
2. **Counter agreement**: At fixed point, `undefeated_attackers[q] == 0` iff all applicable attackers for `~q` are defeated by some applicable superior supporter of `q`
3. **Conclusion equivalence**: For all generated theories, new implementation produces same conclusions as old
4. **Monotonic progress**: Each worklist iteration either proves or disproves at least one literal, or the worklist empties (termination guarantee)

### Debug Assertions

Add `debug_assert!` checks in the counter update paths:

```rust
debug_assert!(self.undefeated_attackers[&q] > 0, "decrement below zero");
debug_assert!(self.attacker_defeat_count[label] > 0, "defeat count below zero");
```

These catch bookkeeping errors during development without runtime cost in release builds.

---

## 10. Implementation Plan

### Phase 1: Add Data Structures (Non-Breaking)

1. Add `undefeated_attackers`, `applicable_supporter_count`, `defeated_by_supporter`, `attacker_defeat_count` to `ReasoningState`
2. Initialize them during Phase 2 setup (after Phase 1 completes)
3. Build `defeated_by_supporter` from superiority index + head index
4. All existing logic continues to work — new fields are populated but not yet consulted

### Phase 2: Incremental Updates in Worklist Loop

1. When a rule becomes applicable: update `applicable_supporter_count` and `attacker_defeat_count`
2. When a rule becomes discarded: reverse the updates
3. Add debug assertions for counter invariants
4. Existing `try_prove_defeasible` / `try_disprove_defeasible` still do the full scan — counters are "shadow" state for validation

### Phase 3: Switch to Counter-Based Decisions

1. Replace the nested loop in `try_prove_defeasible` with counter check
2. Replace the nested loop in `try_disprove_defeasible` with counter check
3. Run full test suite + differential testing
4. Remove the old scanning code

### Phase 4: Benchmark and Validate

1. Add scaling benchmark (TEST-009)
2. Run proptest differential (TEST-008)
3. Verify all existing tests pass (TEST-006)
4. Add benchmark to CI

---

## 11. Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Counter bookkeeping error causes incorrect conclusions | Medium | High | Phase 2 shadow validation, proptest differential |
| Strict attacker special case missed | Low | High | Dedicated test (TEST-010), code review |
| Supporter discard re-evaluation logic is wrong | Medium | Medium | Property-based counter invariant testing |
| Performance improvement smaller than expected due to other bottlenecks | Low | Low | Benchmark identifies actual bottleneck before/after |
| Temporal atom identity (SPEC-018) interactions | Low | Medium | Run temporal test suite after refactor |

---

## 12. Out of Scope

- **Scalable reasoner** (`scalable.rs`): This spec targets only `reason/defeasible.rs`. The scalable reasoner uses a different three-phase algorithm that does not share this code path.
- **Phase 1 performance**: `definite.rs` already uses O(N) incremental counters correctly.
- **Grounding performance**: The O(n^k) grounding cost (MEMORY.md) is a separate concern from reasoning performance.
- **Query operators**: `what_if`, `why_not`, `abduce` in `query.rs` call `reason()` as a black box and benefit automatically from this fix.
