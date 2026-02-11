# ADR-006: Reasoning Phase Extraction

| Field       | Value                                     |
|-------------|-------------------------------------------|
| Status      | Proposed                                  |
| Date        | 2026-02-11                                |
| Deciders    | spindle-core maintainers                  |
| Supersedes  | --                                        |
| Workstream  | `crates/spindle-core/src/reason.rs`       |

## Context

### Current state

`reason_prepared()` in `crates/spindle-core/src/reason.rs` is a single ~200-line
function that performs the entire DL(d) forward-chaining algorithm inline.  It
allocates all mutable state as local variables at the top of the function --
`definite_proven`, `defeasible_proven`, `body_remaining`, `worklist`, `enqueued`,
and `conclusions` -- then threads those variables through three logically
distinct phases that are separated only by comments:

```
// Phase 1: Initialize with facts (deduplicated)
//   + Phase 1b: Initialize empty-body non-fact rules
// Phase 2: Forward chaining (worklist loop)
// Phase 3: Compute negative conclusions
```

The three phases have clear sequential dependencies but they are fused into one
scope, meaning:

1. **No unit-testable boundaries.** Testing Phase 2 in isolation requires
   constructing the exact state that Phase 1 would produce, but because that
   state is private to the function body, every test must run the entire
   pipeline.

2. **Scattered mutation.** Six local `mut` bindings are threaded through
   all three phases. A reader must mentally track which variables each phase
   reads versus writes. For example, `body_remaining` is populated before
   Phase 1 but only consumed during Phase 2; `definite_proven` is written in
   Phases 1 and 2 but read in Phases 2 and 3. These data-flow dependencies are
   implicit.

3. **Interleaved concern.** Phase 1b (empty-body non-fact rules) already
   performs defeasible blocking checks (`is_blocked_by_superior`) that
   conceptually belong to defeasible resolution, yet it runs before the
   forward-chaining worklist loop even starts.  This makes the phase
   boundaries fuzzy and the code harder to audit for correctness.

4. **Bug surface area.** Known bugs documented in the project memory
   (duplicate-fact double-decrement, credulous semantics for `+d`) are
   difficult to localize because the bug-prone logic is inlined between
   unrelated concerns.

### What the three phases actually do

**Phase 1 -- Fact Initialization** (lines 137-207 of `reason.rs`):
Iterates `theory.facts()` and empty-body rules.  For each, it marks the
head literal as proven in the appropriate `LiteralBitSet`, pushes a
`Conclusion`, and seeds the `worklist`.  The `enqueued` bitset deduplicates.
Empty-body strict rules also get definite+defeasible conclusions; empty-body
defeasible rules check `is_blocked_by_superior` before committing.

**Phase 2 -- Forward Chaining** (lines 210-288):
Drains the `worklist`.  For each literal, looks up rules whose body contains
that literal via `indexed.rules_with_body()`, decrements `body_remaining`,
and fires rules whose counter hits zero.  Strict rules produce `+D`/`+d`;
defeasible rules check complement-blocking and superiority before producing
`+d`; defeaters produce nothing but participate in blocking.

**Phase 3 -- Negative Conclusions** (lines 291-304):
Iterates all interned `LitId` values.  Any literal not in `definite_proven`
gets a `-D` conclusion; any not in `defeasible_proven` gets a `-d`.

## Decision

Extract the monolithic `reason_prepared()` into a `ReasoningState` struct
and three phase functions, organized as a module tree:

```
crates/spindle-core/src/reason/
    mod.rs          -- public API (reason, reason_with_options, reason_prepared)
    state.rs        -- ReasoningState struct + LiteralBitSet
    facts.rs        -- Phase 1: initialize_facts()
    definite.rs     -- Phase 2: forward_chain_strict()
    defeasible.rs   -- Phase 3: resolve_defeasible()
```

> Note: the current `reason.rs` becomes `reason/mod.rs` with no public API
> change.  All existing call sites (`reason()`, `reason_with_options()`,
> `reason_prepared()`) continue to work.

### ReasoningState

A single struct that owns all mutable state shared across phases:

```rust
use std::collections::VecDeque;
use fixedbitset::FixedBitSet;
use rustc_hash::FxHashMap;

use crate::conclusion::Conclusion;
use crate::index::LitId;
use crate::literal::Literal;

/// Bit set optimized for tracking proven literals.
/// Maps LitId to bit indices for O(1) contains/insert.
/// Uses 2 bits per atom (positive + negated).
pub(crate) struct LiteralBitSet {
    bits: FixedBitSet,
}

impl LiteralBitSet {
    pub(crate) fn new(atom_count: usize) -> Self {
        Self {
            bits: FixedBitSet::with_capacity(atom_count * 2),
        }
    }

    #[inline]
    fn to_index(id: LitId) -> usize {
        let atom_idx = id.atom().as_raw() as usize;
        let negated = if id.is_negated() { 1 } else { 0 };
        atom_idx * 2 + negated
    }

    #[inline]
    pub(crate) fn contains(&self, id: LitId) -> bool {
        let idx = Self::to_index(id);
        idx < self.bits.len() && self.bits.contains(idx)
    }

    #[inline]
    pub(crate) fn insert(&mut self, id: LitId) {
        let idx = Self::to_index(id);
        if idx >= self.bits.len() {
            self.bits.grow(idx + 1);
        }
        self.bits.insert(idx);
    }
}

/// All mutable state for a single reasoning pass.
///
/// Invariant: `definite_proven` is a subset of `defeasible_proven`.
/// Invariant: once a LitId is inserted into a proven set, it is never removed
///            (monotonic growth).
pub(crate) struct ReasoningState<'a> {
    /// Worklist for BFS forward chaining.
    pub worklist: VecDeque<Literal>,

    /// Literals proven via facts or strict rules (+D).
    pub definite_proven: LiteralBitSet,

    /// Literals proven via any rule type (+d).  Superset of definite_proven.
    pub defeasible_proven: LiteralBitSet,

    /// Per-rule counter: how many body literals remain unsatisfied.
    /// Keyed by rule label (borrowed from the theory).
    pub body_remaining: FxHashMap<&'a str, usize>,

    /// Accumulated conclusions.
    pub conclusions: Vec<Conclusion>,

    /// Tracks which LitIds have been enqueued to prevent duplicate processing.
    pub enqueued: LiteralBitSet,
}

impl<'a> ReasoningState<'a> {
    /// Construct initial state sized for the given theory.
    pub fn new(atom_count: usize, rule_count: usize, estimated_conclusions: usize) -> Self {
        Self {
            worklist: VecDeque::with_capacity(rule_count),
            definite_proven: LiteralBitSet::new(atom_count),
            defeasible_proven: LiteralBitSet::new(atom_count),
            body_remaining: FxHashMap::with_capacity_and_hasher(
                rule_count,
                Default::default(),
            ),
            conclusions: Vec::with_capacity(estimated_conclusions),
            enqueued: LiteralBitSet::new(atom_count),
        }
    }
}
```

Key design choices:

- **Monotonic proven sets.** Once `definite_proven.insert(id)` is called, the
  bit is never cleared.  This matches the semantics of DL(d) where a
  derivation, once established, is permanent within a single pass.  No `remove`
  or `unmark` method exists on `LiteralBitSet`.

- **Single mutable state object.** All six previously-scattered `let mut`
  bindings are consolidated into one struct passed as `&mut ReasoningState`.
  This makes the data-flow dependencies between phases explicit in the type
  signature and enables the borrow checker to enforce them.

- **`pub(crate)` visibility.** The struct and its fields are crate-internal.
  The public API remains `reason()`, `reason_with_options()`, and
  `reason_prepared()`.

### Phase functions

Each phase is a free function in its own submodule, taking an `&IndexedTheory`,
an `&Theory`, and an `&mut ReasoningState`:

#### Phase 1: `facts.rs`

```rust
use crate::conclusion::Conclusion;
use crate::index::IndexedTheory;
use crate::rule::RuleType;
use crate::theory::Theory;

use super::state::ReasoningState;
use super::is_blocked_by_superior;

/// Phase 1: Seed the worklist with facts and empty-body rules.
///
/// Postconditions:
/// - Every fact literal is in `state.definite_proven` and `state.defeasible_proven`.
/// - Every empty-body strict rule head is in `state.definite_proven`.
/// - Every unblocked empty-body defeasible rule head is in `state.defeasible_proven`.
/// - All seeded literals are in `state.worklist` (deduplicated via `state.enqueued`).
/// - `state.body_remaining` is populated for all rules in the theory.
/// - `+D` and `+d` conclusions are pushed for all proven literals.
pub(crate) fn initialize_facts(
    theory: &Theory,
    indexed: &mut IndexedTheory<'_>,
    state: &mut ReasoningState<'_>,
) {
    // Populate body_remaining for all rules
    for rule in theory.rules() {
        state.body_remaining.insert(&rule.label, rule.body.len());
    }

    // Seed facts (deduplicated)
    for fact in theory.facts() {
        let lit = fact.head_literal().clone();
        let lit_id = indexed.intern_literal(&lit);

        if state.enqueued.contains(lit_id) {
            continue;
        }
        state.enqueued.insert(lit_id);

        state.definite_proven.insert(lit_id);
        state.defeasible_proven.insert(lit_id);

        state.conclusions.push(
            Conclusion::definitely_provable(lit.clone()).with_rule(&fact.label),
        );
        state.conclusions.push(
            Conclusion::defeasibly_provable(lit.clone()).with_rule(&fact.label),
        );

        state.worklist.push_back(lit);
    }

    // Seed empty-body non-fact rules
    for rule in theory.rules() {
        if rule.body.is_empty() && rule.rule_type != RuleType::Fact {
            let head_lit = rule.head_literal().clone();
            let head_id = indexed.intern_literal(&head_lit);

            match rule.rule_type {
                RuleType::Strict => {
                    if !state.definite_proven.contains(head_id) {
                        state.definite_proven.insert(head_id);
                        state.defeasible_proven.insert(head_id);
                        state.conclusions.push(
                            Conclusion::definitely_provable(head_lit.clone())
                                .with_rule(&rule.label),
                        );
                        state.conclusions.push(
                            Conclusion::defeasibly_provable(head_lit.clone())
                                .with_rule(&rule.label),
                        );
                    }
                    if !state.enqueued.contains(head_id) {
                        state.enqueued.insert(head_id);
                        state.worklist.push_back(head_lit);
                    }
                }
                RuleType::Defeasible => {
                    if !state.defeasible_proven.contains(head_id) {
                        let blocked = is_blocked_by_superior(
                            indexed,
                            theory,
                            rule,
                            &state.defeasible_proven,
                        );
                        if !blocked {
                            state.defeasible_proven.insert(head_id);
                            state.conclusions.push(
                                Conclusion::defeasibly_provable(head_lit.clone())
                                    .with_rule(&rule.label),
                            );
                        }
                    }
                    if state.defeasible_proven.contains(head_id)
                        && !state.enqueued.contains(head_id)
                    {
                        state.enqueued.insert(head_id);
                        state.worklist.push_back(head_lit);
                    }
                }
                _ => {}
            }
        }
    }
}
```

#### Phase 2: `definite.rs`

```rust
use crate::conclusion::Conclusion;
use crate::index::IndexedTheory;
use crate::rule::RuleType;
use crate::theory::Theory;

use super::state::ReasoningState;
use super::is_blocked_by_superior;

/// Phase 2: Forward-chain from the worklist until fixpoint.
///
/// Preconditions:
/// - Phase 1 has completed: all facts and empty-body rule heads are seeded.
/// - `state.body_remaining` is populated for every rule.
///
/// Postconditions:
/// - The worklist is empty (fixpoint reached).
/// - All strict-derivable literals are in `state.definite_proven`.
/// - All defeasible-derivable (unblocked) literals are in `state.defeasible_proven`.
/// - Corresponding `+D` / `+d` conclusions are in `state.conclusions`.
pub(crate) fn forward_chain_strict(
    theory: &Theory,
    indexed: &IndexedTheory<'_>,
    state: &mut ReasoningState<'_>,
) {
    while let Some(lit) = state.worklist.pop_front() {
        for rule in indexed.rules_with_body(&lit) {
            let remaining = state
                .body_remaining
                .get_mut(rule.label.as_str())
                .unwrap();
            if *remaining > 0 {
                *remaining -= 1;

                if *remaining == 0 {
                    let head_lit = rule.head_literal().clone();
                    let head_id = indexed
                        .get_lit_id(&head_lit)
                        .expect("Head literal missing from index");

                    match rule.rule_type {
                        RuleType::Fact => unreachable!("Facts have no body"),
                        RuleType::Strict => {
                            if !state.definite_proven.contains(head_id) {
                                state.definite_proven.insert(head_id);
                                state.defeasible_proven.insert(head_id);
                                state.conclusions.push(
                                    Conclusion::definitely_provable(head_lit.clone())
                                        .with_rule(&rule.label),
                                );
                                state.conclusions.push(
                                    Conclusion::defeasibly_provable(head_lit.clone())
                                        .with_rule(&rule.label),
                                );
                                if !state.enqueued.contains(head_id) {
                                    state.enqueued.insert(head_id);
                                    state.worklist.push_back(head_lit);
                                }
                            }
                        }
                        RuleType::Defeasible => {
                            let comp_id = head_id.complement();
                            if !state.definite_proven.contains(comp_id)
                                && !state.defeasible_proven.contains(head_id)
                            {
                                let blocked = is_blocked_by_superior(
                                    indexed,
                                    theory,
                                    rule,
                                    &state.defeasible_proven,
                                );
                                if !blocked {
                                    state.defeasible_proven.insert(head_id);
                                    state.conclusions.push(
                                        Conclusion::defeasibly_provable(head_lit.clone())
                                            .with_rule(&rule.label),
                                    );
                                    if !state.enqueued.contains(head_id) {
                                        state.enqueued.insert(head_id);
                                        state.worklist.push_back(head_lit);
                                    }
                                }
                            }
                        }
                        RuleType::Defeater => {
                            // Defeaters don't prove anything; blocking
                            // is handled in is_blocked_by_superior.
                        }
                    }
                }
            }
        }
    }
}
```

#### Phase 3: `defeasible.rs`

```rust
use crate::conclusion::{Conclusion, ConclusionType};
use crate::index::{IndexedTheory, LitId};

use super::state::ReasoningState;

/// Phase 3: Emit negative conclusions for all unproven literals.
///
/// Preconditions:
/// - Phases 1 and 2 have completed: the proven sets are at fixpoint.
///
/// Postconditions:
/// - For every LitId in the indexed theory:
///   - if not in `definite_proven`, a `-D` conclusion is emitted.
///   - if not in `defeasible_proven`, a `-d` conclusion is emitted.
pub(crate) fn resolve_defeasible(
    indexed: &IndexedTheory<'_>,
    state: &mut ReasoningState<'_>,
) {
    let all_ids: Vec<LitId> = indexed.all_literal_ids().cloned().collect();

    for lit_id in all_ids {
        if !state.definite_proven.contains(lit_id) {
            let lit = indexed.resolve_literal(lit_id);
            state.conclusions.push(
                Conclusion::new(ConclusionType::DefinitelyNotProvable, lit),
            );
        }

        if !state.defeasible_proven.contains(lit_id) {
            let lit = indexed.resolve_literal(lit_id);
            state.conclusions.push(
                Conclusion::new(ConclusionType::DefeasiblyNotProvable, lit),
            );
        }
    }
}
```

#### Rewritten `reason_prepared()`

The orchestrator in `reason/mod.rs` becomes trivially readable:

```rust
pub fn reason_prepared(theory: &Theory) -> Result<Vec<Conclusion>> {
    let mut indexed = IndexedTheory::build(theory);

    let atom_count = indexed.atom_count();
    let rule_count = theory.rule_count();
    let estimated = rule_count * 2 + indexed.all_literal_ids().count() * 2;

    let mut state = ReasoningState::new(atom_count, rule_count, estimated);

    // Phase 1: Seed facts and empty-body rules
    facts::initialize_facts(theory, &mut indexed, &mut state);

    // Phase 2: Forward-chain to fixpoint
    definite::forward_chain_strict(theory, &indexed, &mut state);

    // Phase 3: Emit negative conclusions
    defeasible::resolve_defeasible(&indexed, &mut state);

    Ok(state.conclusions)
}
```

### Phase boundaries and invariants

The extraction enforces phase ordering through a simple sequential call
pattern.  Each phase function documents its preconditions and postconditions
as doc comments.

| Boundary | Invariant | Enforced by |
|----------|-----------|-------------|
| Before Phase 1 | `state` is freshly constructed; all bitsets empty, all counters at body length | `ReasoningState::new()` + `initialize_facts` populating `body_remaining` |
| After Phase 1 | All facts and empty-body rule heads are seeded into proven sets and worklist | `initialize_facts` postcondition |
| Phase 1 -> Phase 2 | `body_remaining` is populated for every rule; worklist contains all initial seeds | Phase 2 precondition check (panic on missing label) |
| After Phase 2 | Worklist is empty (fixpoint); all derivable positive conclusions are emitted | `while let Some(lit) = worklist.pop_front()` drains to completion |
| Phase 2 -> Phase 3 | `definite_proven` and `defeasible_proven` are at their final fixed point | Phase 3 only reads proven sets, never writes |
| After Phase 3 | All negative conclusions emitted; `state.conclusions` is complete | `resolve_defeasible` postcondition |

The monotonicity invariant -- proven sets only grow, never shrink -- is
structural: `LiteralBitSet` exposes `insert` and `contains` but no `remove`.

The subset invariant -- `definite_proven` is a subset of `defeasible_proven` --
is maintained by every code path that inserts into `definite_proven` also
inserting into `defeasible_proven` immediately after.  This can be upgraded to
an explicit `mark_definitely_proven()` method on `ReasoningState` that enforces
both insertions atomically:

```rust
impl ReasoningState<'_> {
    /// Mark a literal as definitely (and therefore defeasibly) proven.
    /// Maintains the invariant: definite_proven is a subset of defeasible_proven.
    pub(crate) fn mark_definitely_proven(&mut self, id: LitId) {
        self.definite_proven.insert(id);
        self.defeasible_proven.insert(id);
    }
}
```

## Trade-offs

### Benefits

- **Phase isolation enables unit testing.** Each phase function can be tested
  by constructing a `ReasoningState` with known initial values and asserting on
  the resulting state.  For example, testing that `forward_chain_strict`
  correctly fires a two-body strict rule requires only seeding two literals into
  `definite_proven` and `worklist`, without running the full pipeline:

  ```rust
  #[test]
  fn test_strict_two_body_fires() {
      let mut theory = Theory::new();
      theory.add_fact("a");
      theory.add_fact("b");
      theory.add_strict_rule(&["a", "b"], "c");

      let mut indexed = IndexedTheory::build(&theory);
      let mut state = ReasoningState::new(
          indexed.atom_count(),
          theory.rule_count(),
          16,
      );

      // Run only Phase 1
      facts::initialize_facts(&theory, &mut indexed, &mut state);

      // Assert Phase 1 postcondition: a and b are proven
      let a_id = indexed.get_lit_id(&Literal::simple("a")).unwrap();
      let b_id = indexed.get_lit_id(&Literal::simple("b")).unwrap();
      assert!(state.definite_proven.contains(a_id));
      assert!(state.definite_proven.contains(b_id));

      // Run only Phase 2
      definite::forward_chain_strict(&theory, &indexed, &mut state);

      // Assert Phase 2 postcondition: c is now proven
      let c_id = indexed.get_lit_id(&Literal::simple("c")).unwrap();
      assert!(state.definite_proven.contains(c_id));
  }
  ```

- **Bug localization.** The known duplicate-fact double-decrement bug is
  isolated to `initialize_facts` (the `enqueued` dedup guard).  The credulous
  semantics issue is isolated to `forward_chain_strict` (the defeasible
  branch).  Future fixes can target a single ~60-line function instead of a
  ~200-line monolith.

- **Readability.** The orchestrator (`reason_prepared`) becomes 10 lines of
  code.  A new contributor can understand the high-level algorithm by reading
  three function calls instead of parsing 200 lines of interleaved logic.

- **Extensibility.** Adding a new phase (e.g., ambiguity propagation) means
  adding a new file and a single function call in the orchestrator, rather
  than splicing code into the middle of a monolithic function.

### Costs

- **More files.** The `reason.rs` file becomes a `reason/` directory with five
  files.  Navigation requires slightly more effort, though modern editors handle
  this well.

- **State visibility.** The `ReasoningState` fields must be `pub(crate)` so
  that the phase functions in sibling modules can access them.  This is a wider
  visibility than the current local-variable approach where the state is
  entirely private to one function.  However, the `pub(crate)` boundary still
  prevents external crates from depending on the internal representation.

- **Migration effort.** Existing tests in `reason.rs` (approximately 40 tests)
  must move to `reason/mod.rs` or a `reason/tests.rs` submodule.  The tests
  themselves do not change -- they call the public API (`reason()`) which
  remains stable.

- **Indirection.** Debugging now requires following calls through three
  functions instead of reading one.  The trade-off is worthwhile because the
  three functions are individually small enough to reason about locally.

## Consequences

- The public API (`reason()`, `reason_with_options()`, `reason_prepared()`)
  does not change.  No downstream breakage.

- The `is_blocked_by_superior()` helper remains a shared utility in
  `reason/mod.rs`, used by both `facts.rs` (Phase 1b) and `definite.rs`
  (Phase 2).

- The `LiteralBitSet` type moves from a private struct inside `reason.rs` to
  `pub(crate)` in `reason/state.rs`.  This enables reuse if other crate-internal
  modules need proven-literal tracking.

- Future work on the ambiguity propagation mode (ADR TBD) will add a Phase 2.5
  as a new `reason/ambiguity.rs` file with a single function call inserted
  between `forward_chain_strict` and `resolve_defeasible` in the orchestrator.

- The differential testing harness (`difftest.rs`) and Lean oracle tests
  continue to work without modification, as they call `reason()` or
  `reason_prepared()`.

## Alternatives Considered

### Keep monolithic but add doc comments

More comments would help readability but do not enable phase-isolated testing
or reduce the bug surface area.  Comments also tend to drift from the code
over time.

### Extract phases as methods on IndexedTheory

This would couple the indexing concern with the reasoning concern.
`IndexedTheory` is a data structure for lookup; reasoning is a separate
algorithm that consumes it.  Mixing them violates single-responsibility.

### Use a trait-based phase pipeline

A trait like `ReasoningPhase` with a `fn execute(&self, state: &mut
ReasoningState)` method would allow runtime composition of phases.  This is
over-engineered for the current three-phase algorithm and adds vtable overhead.
If we later need pluggable phases, we can introduce traits at that time.
