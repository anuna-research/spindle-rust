//! Reasoning state for the DL(d) forward-chaining algorithm.
//!
//! Contains the [`ReasoningState`] struct that consolidates all mutable state
//! previously scattered as local variables in `reason_prepared()`, and the
//! [`LiteralBitSet`] type for O(1) proven-literal tracking.

use std::collections::VecDeque;

use fixedbitset::FixedBitSet;
use rustc_hash::FxHashMap;

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
/// Consolidates the six `let mut` bindings that were previously scattered
/// across `reason_prepared()` into a single struct. This makes the data-flow
/// dependencies between phases explicit and enables phase-isolated testing.
///
/// # Invariants
///
/// - **Monotonic proven sets**: once a `LitId` is inserted into
///   `definite_proven` or `defeasible_proven`, it is never removed.
/// - **Subset invariant**: `definite_proven` is always a subset of
///   `defeasible_proven`. Every code path that inserts into `definite_proven`
///   must also insert into `defeasible_proven`. Use [`mark_definitely_proven`]
///   to enforce this atomically.
///
/// [`mark_definitely_proven`]: ReasoningState::mark_definitely_proven
pub(crate) struct ReasoningState<'a> {
    /// Worklist for BFS forward chaining.
    pub(crate) worklist: VecDeque<Literal>,

    /// Literals proven via facts or strict rules (+D).
    pub(crate) definite_proven: LiteralBitSet,

    /// Literals proven via any rule type (+d). Superset of `definite_proven`.
    pub(crate) defeasible_proven: LiteralBitSet,

    /// Per-rule counter: how many body literals remain unsatisfied.
    /// Keyed by rule label (borrowed from the theory).
    pub(crate) body_remaining: FxHashMap<&'a str, usize>,

    /// Accumulated conclusions.
    pub(crate) conclusions: Vec<Conclusion>,

    /// Tracks which `LitId` values have been enqueued to prevent duplicate
    /// processing in the worklist.
    pub(crate) enqueued: LiteralBitSet,
}

impl<'a> ReasoningState<'a> {
    /// Construct initial state sized for the given theory.
    ///
    /// All bitsets start empty, the worklist is empty, and `body_remaining`
    /// is pre-allocated but not yet populated (that happens in Phase 1).
    pub(crate) fn new(atom_count: usize, rule_count: usize, estimated_conclusions: usize) -> Self {
        Self {
            worklist: VecDeque::with_capacity(rule_count),
            definite_proven: LiteralBitSet::new(atom_count),
            defeasible_proven: LiteralBitSet::new(atom_count),
            body_remaining: FxHashMap::with_capacity_and_hasher(rule_count, Default::default()),
            conclusions: Vec::with_capacity(estimated_conclusions),
            enqueued: LiteralBitSet::new(atom_count),
        }
    }

    /// Mark a literal as definitely (and therefore defeasibly) proven.
    ///
    /// Maintains the invariant: `definite_proven` is a subset of
    /// `defeasible_proven`.
    #[inline]
    pub(crate) fn mark_definitely_proven(&mut self, id: LitId) {
        self.definite_proven.insert(id);
        self.defeasible_proven.insert(id);
    }

    /// Check if a literal is definitely proven (+D).
    #[inline]
    pub(crate) fn is_definitely_proven(&self, id: LitId) -> bool {
        self.definite_proven.contains(id)
    }

    /// Check if a literal is defeasibly proven (+d).
    #[inline]
    pub(crate) fn is_defeasibly_proven(&self, id: LitId) -> bool {
        self.defeasible_proven.contains(id)
    }

    /// Add a conclusion to the accumulated results.
    #[inline]
    pub(crate) fn add_conclusion(&mut self, conclusion: Conclusion) {
        self.conclusions.push(conclusion);
    }

    /// Try to enqueue a literal into the worklist if not already enqueued.
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

    /// Pop the next literal from the worklist, or `None` if empty.
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
        assert!(state.body_remaining.is_empty());
    }

    #[test]
    fn test_mark_definitely_proven_maintains_subset_invariant() {
        let mut state = ReasoningState::new(10, 5, 20);
        let lit_id = LitId::new(AtomId::from_raw(3), false);

        state.mark_definitely_proven(lit_id);

        assert!(state.is_definitely_proven(lit_id));
        assert!(
            state.is_defeasibly_proven(lit_id),
            "mark_definitely_proven must also set defeasible_proven"
        );
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
