//! Reasoning state for the DL(d) forward-chaining algorithm.
//!
//! Contains the [`ReasoningState`] struct that consolidates all mutable state
//! previously scattered as local variables in `reason_prepared()`, and the
//! [`LiteralBitSet`] type for O(1) proven-literal tracking.

use std::collections::VecDeque;

use fixedbitset::FixedBitSet;
use rustc_hash::{FxHashMap, FxHashSet};

/// Per-rule tracking of which body slots have been satisfied, keyed by rule
/// label. Each bitset has one bit per body position; a set bit means some
/// proven literal has satisfied that slot. Paired with `*_body_remaining` to
/// make counter decrements idempotent per slot (SPEC-020 family matching:
/// several temporal members of one family satisfy a single atemporal body
/// slot exactly once).
pub(crate) type SlotsSatisfied<'a> = FxHashMap<&'a str, FixedBitSet>;

use crate::conclusion::Conclusion;
use crate::index::LitId;
use crate::literal::Literal;

/// A bit set optimized for tracking proven literals.
///
/// Maps `LitId` to bit indices for O(1) contains/insert operations.
/// Uses 2 bits per atom (positive + negated).
///
/// # Monotonicity
///
/// This type intentionally does not expose a `remove` method. Once a literal
/// is inserted, it stays inserted for the lifetime of the reasoning pass.
/// This matches DL(d) semantics where derivations are permanent within a
/// single pass.
#[derive(Clone)]
pub(crate) struct LiteralBitSet {
    bits: FixedBitSet,
}

impl LiteralBitSet {
    /// Create a new LiteralBitSet sized for the indexed theory.
    pub(crate) fn new(atom_count: usize) -> Self {
        // Each atom needs 2 bits: one for positive, one for negated
        let size = atom_count * 2;
        Self {
            bits: FixedBitSet::with_capacity(size),
        }
    }

    /// Convert a LitId to a bit index.
    #[inline]
    fn to_index(id: LitId) -> usize {
        let atom_idx = id.atom().as_raw() as usize;
        let negated = if id.is_negated() { 1 } else { 0 };
        atom_idx * 2 + negated
    }

    /// Check if a literal has been proven.
    #[inline]
    pub(crate) fn contains(&self, id: LitId) -> bool {
        let idx = Self::to_index(id);
        idx < self.bits.len() && self.bits.contains(idx)
    }

    /// Mark a literal as proven.
    ///
    /// Automatically grows the bitset if needed, preventing silent data loss
    /// when new atoms are interned after the bitset is initially sized.
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
/// Consolidates the mutable bindings that were previously scattered
/// across `reason_prepared()` into a single struct. This makes the data-flow
/// dependencies between phases explicit and enables phase-isolated testing.
///
/// # Phases
///
/// - **Phase 1 (Definite)**: Uses `worklist`, `enqueued`, `definite_proven`,
///   and `definite_body_remaining` to compute +D via strict-only forward chaining.
/// - **Phase 2 (Defeasible)**: Uses `defeasible_proven`, `defeasible_disproven`,
///   `defeasible_body_remaining`, and `rule_discarded` with a separate worklist
///   to compute +d/-d via a fixed-point loop with ambiguity blocking.
/// - **Phase 3 (Negatives)**: Emits -D and -d for all unproven literals.
///
/// # Invariants
///
/// - **Monotonic proven sets**: once a `LitId` is inserted into any proven/disproven
///   set, it is never removed.
pub(crate) struct ReasoningState<'a> {
    /// Phase 1 worklist for BFS forward chaining (strict rules only).
    pub(crate) worklist: VecDeque<Literal>,

    /// Tracks which `LitId` values have been enqueued in the Phase 1 worklist
    /// to prevent duplicate processing.
    pub(crate) enqueued: LiteralBitSet,

    /// Literals proven via facts or strict rules (+D).
    pub(crate) definite_proven: LiteralBitSet,

    /// Literals proven defeasibly (+d). Populated in Phase 2.
    pub(crate) defeasible_proven: LiteralBitSet,

    /// Literals disproven defeasibly (-d). Populated in Phase 2.
    pub(crate) defeasible_disproven: LiteralBitSet,

    /// Phase 1 per-rule body counter (strict rules only).
    pub(crate) definite_body_remaining: FxHashMap<&'a str, usize>,

    /// Phase 1 per-rule satisfied body-slot bitsets (strict rules only).
    /// Keeps `definite_body_remaining` decrements idempotent per slot.
    pub(crate) definite_slots_satisfied: SlotsSatisfied<'a>,

    /// Phase 2 per-rule body counter (all rule types).
    pub(crate) defeasible_body_remaining: FxHashMap<&'a str, usize>,

    /// Phase 2 per-rule satisfied body-slot bitsets (all rule types).
    /// Keeps `defeasible_body_remaining` decrements idempotent per slot.
    pub(crate) defeasible_slots_satisfied: SlotsSatisfied<'a>,

    /// Phase 2 per-rule tracking: has any body literal been proved -d?
    pub(crate) rule_discarded: FxHashMap<&'a str, bool>,

    /// Accumulated conclusions.
    pub(crate) conclusions: Vec<Conclusion>,

    /// Additional rule labels that should be projected even when they do not
    /// appear on a positive conclusion, such as applicable blockers.
    pub(crate) projection_labels: FxHashSet<String>,
}

impl<'a> ReasoningState<'a> {
    /// Construct initial state sized for the given theory.
    ///
    /// All bitsets start empty, the worklist is empty, and body_remaining maps
    /// are pre-allocated but not yet populated (that happens in Phase 1 init).
    pub(crate) fn new(atom_count: usize, rule_count: usize, estimated_conclusions: usize) -> Self {
        Self {
            worklist: VecDeque::with_capacity(rule_count),
            enqueued: LiteralBitSet::new(atom_count),
            definite_proven: LiteralBitSet::new(atom_count),
            defeasible_proven: LiteralBitSet::new(atom_count),
            defeasible_disproven: LiteralBitSet::new(atom_count),
            definite_body_remaining: FxHashMap::with_capacity_and_hasher(
                rule_count,
                Default::default(),
            ),
            definite_slots_satisfied: FxHashMap::with_capacity_and_hasher(
                rule_count,
                Default::default(),
            ),
            defeasible_body_remaining: FxHashMap::with_capacity_and_hasher(
                rule_count,
                Default::default(),
            ),
            defeasible_slots_satisfied: FxHashMap::with_capacity_and_hasher(
                rule_count,
                Default::default(),
            ),
            rule_discarded: FxHashMap::with_capacity_and_hasher(rule_count, Default::default()),
            conclusions: Vec::with_capacity(estimated_conclusions),
            projection_labels: FxHashSet::with_capacity_and_hasher(rule_count, Default::default()),
        }
    }

    /// Check if a literal is definitely proven (+D).
    #[inline]
    pub(crate) fn is_definitely_proven(&self, id: LitId) -> bool {
        self.definite_proven.contains(id)
    }

    /// Add a conclusion to the accumulated results.
    #[inline]
    pub(crate) fn add_conclusion(&mut self, conclusion: Conclusion) {
        self.conclusions.push(conclusion);
    }

    /// Try to enqueue a literal into the Phase 1 worklist if not already enqueued.
    ///
    /// Returns `true` if the literal was enqueued (first time), `false` if
    /// it was already in the enqueued set.
    #[inline]
    pub(crate) fn try_enqueue(&mut self, id: LitId, lit: Literal) -> bool {
        if self.enqueued.contains(id) {
            return false;
        }
        self.enqueued.insert(id);
        self.worklist.push_back(lit);
        true
    }

    /// Pop the next literal from the Phase 1 worklist, or `None` if empty.
    #[inline]
    pub(crate) fn drain_worklist(&mut self) -> Option<Literal> {
        self.worklist.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::AtomId;

    #[test]
    fn test_literal_bitset_new_is_empty() {
        let bitset = LiteralBitSet::new(10);
        let lit_id = LitId::new(AtomId::from_raw(0), false);
        assert!(!bitset.contains(lit_id));
    }

    #[test]
    fn test_literal_bitset_insert_and_contains() {
        let mut bitset = LiteralBitSet::new(10);
        let lit_id = LitId::new(AtomId::from_raw(3), false);
        let neg_id = LitId::new(AtomId::from_raw(3), true);

        assert!(!bitset.contains(lit_id));
        assert!(!bitset.contains(neg_id));

        bitset.insert(lit_id);
        assert!(bitset.contains(lit_id));
        assert!(!bitset.contains(neg_id));

        bitset.insert(neg_id);
        assert!(bitset.contains(lit_id));
        assert!(bitset.contains(neg_id));
    }

    #[test]
    fn test_literal_bitset_grows_on_insert_beyond_capacity() {
        let mut bitset = LiteralBitSet::new(2);
        let lit_id = LitId::new(AtomId::from_raw(10), false);

        bitset.insert(lit_id);
        assert!(
            bitset.contains(lit_id),
            "Bitset should contain the inserted literal after growing"
        );
    }

    #[test]
    fn test_literal_bitset_preserves_existing_on_grow() {
        let mut bitset = LiteralBitSet::new(2);

        let lit_0 = LitId::new(AtomId::from_raw(0), false);
        bitset.insert(lit_0);
        assert!(bitset.contains(lit_0));

        let lit_10 = LitId::new(AtomId::from_raw(10), true);
        bitset.insert(lit_10);

        assert!(
            bitset.contains(lit_0),
            "Existing bit at atom 0 should be preserved after grow"
        );
        assert!(
            bitset.contains(lit_10),
            "New bit at atom 10 should be present after grow"
        );
    }

    #[test]
    fn test_reasoning_state_new_is_empty() {
        let state = ReasoningState::new(10, 5, 20);
        assert!(state.worklist.is_empty());
        assert!(state.conclusions.is_empty());
        assert!(state.definite_body_remaining.is_empty());
        assert!(state.defeasible_body_remaining.is_empty());
        assert!(state.rule_discarded.is_empty());
        assert!(state.projection_labels.is_empty());
    }

    #[test]
    fn test_try_enqueue_deduplicates() {
        let mut state = ReasoningState::new(10, 5, 20);
        let lit_id = LitId::new(AtomId::from_raw(0), false);
        let lit = Literal::simple("test");

        assert!(state.try_enqueue(lit_id, lit.clone()));
        assert!(!state.try_enqueue(lit_id, lit.clone()));
        assert_eq!(state.worklist.len(), 1);
    }

    #[test]
    fn test_drain_worklist() {
        let mut state = ReasoningState::new(10, 5, 20);
        let lit_id = LitId::new(AtomId::from_raw(0), false);
        let lit = Literal::simple("test");

        state.try_enqueue(lit_id, lit.clone());

        let popped = state.drain_worklist();
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().name(), "test");

        assert!(state.drain_worklist().is_none());
    }
}
