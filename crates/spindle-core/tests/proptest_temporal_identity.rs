//! TEST-PBT-001: Property-based test for temporal identity bijection.
//!
//! Verifies: intern(p) == intern(q) iff all fields including temporal match.
//!
//! The bijection has two directions:
//! 1. (Forward) Equal fields => equal LitId
//! 2. (Backward) Different in any field => different LitId

mod proptest_helpers;

use proptest::prelude::*;

use spindle_core::index::IndexedTheory;
use spindle_core::literal::Literal;
use spindle_core::theory::Theory;

use proptest_helpers::{arb_mode, arb_temporal, ATOMS};

/// Generate a full `Literal` with temporal bounds, mode, and predicate args.
fn arb_literal_with_temporal() -> impl Strategy<Value = Literal> {
    (
        proptest::sample::select(ATOMS),
        proptest::bool::ANY,
        arb_mode(),
        arb_temporal(),
        proptest::collection::vec(
            proptest::sample::select(&["alice", "bob", "carol"][..]).prop_map(String::from),
            0..=2,
        ),
    )
        .prop_map(|(name, negated, mode, temporal, preds)| {
            Literal::new(name, negated, mode, temporal, preds)
        })
}

// =============================================================================
// Forward: identical literals produce identical LitIds
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Interning the same literal twice must produce the same LitId.
    #[test]
    fn identical_literals_same_litid(lit in arb_literal_with_temporal()) {
        let theory = Theory::new();
        let mut indexed = IndexedTheory::build(&theory);

        let id1 = indexed.intern_literal(&lit);
        let id2 = indexed.intern_literal(&lit);

        prop_assert_eq!(id1, id2, "Identical literals must produce identical LitIds");
    }
}

// =============================================================================
// Backward: differing in temporal => different AtomId
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Two literals that differ only in temporal bounds must have different AtomIds.
    #[test]
    fn different_temporal_different_atomid(
        name in proptest::sample::select(ATOMS),
        mode in arb_mode(),
        t1 in arb_temporal(),
        t2 in arb_temporal(),
        preds in proptest::collection::vec(
            proptest::sample::select(&["alice", "bob", "carol"][..]).prop_map(String::from),
            0..=2,
        ),
    ) {
        prop_assume!(t1 != t2);

        let lit1 = Literal::new(name, false, mode.clone(), t1, preds.clone());
        let lit2 = Literal::new(name, false, mode, t2, preds);

        let theory = Theory::new();
        let mut indexed = IndexedTheory::build(&theory);

        let id1 = indexed.intern_literal(&lit1);
        let id2 = indexed.intern_literal(&lit2);

        prop_assert_ne!(
            id1.atom(), id2.atom(),
            "Literals differing in temporal must have different AtomIds"
        );
    }
}

// =============================================================================
// Backward: differing in name => different AtomId
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Two literals that differ only in functor name must have different AtomIds.
    #[test]
    fn different_name_different_atomid(
        n1 in proptest::sample::select(ATOMS),
        n2 in proptest::sample::select(ATOMS),
        temporal in arb_temporal(),
    ) {
        prop_assume!(n1 != n2);

        let lit1 = Literal::new(n1, false, Default::default(), temporal.clone(), vec![]);
        let lit2 = Literal::new(n2, false, Default::default(), temporal, vec![]);

        let theory = Theory::new();
        let mut indexed = IndexedTheory::build(&theory);

        let id1 = indexed.intern_literal(&lit1);
        let id2 = indexed.intern_literal(&lit2);

        prop_assert_ne!(
            id1.atom(), id2.atom(),
            "Literals differing in name must have different AtomIds"
        );
    }
}

// =============================================================================
// Backward: differing in predicate args => different AtomId
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Two literals that differ only in predicate args must have different AtomIds.
    #[test]
    fn different_args_different_atomid(
        name in proptest::sample::select(ATOMS),
        temporal in arb_temporal(),
    ) {
        let lit1 = Literal::new(name, false, Default::default(), temporal.clone(), vec!["alice".into()]);
        let lit2 = Literal::new(name, false, Default::default(), temporal, vec!["bob".into()]);

        let theory = Theory::new();
        let mut indexed = IndexedTheory::build(&theory);

        let id1 = indexed.intern_literal(&lit1);
        let id2 = indexed.intern_literal(&lit2);

        prop_assert_ne!(
            id1.atom(), id2.atom(),
            "Literals differing in args must have different AtomIds"
        );
    }
}

// =============================================================================
// Full bijection: equal LitId iff all fields match
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// intern(p) == intern(q) iff p and q have identical fields.
    #[test]
    fn temporal_identity_bijection(
        p in arb_literal_with_temporal(),
        q in arb_literal_with_temporal(),
    ) {
        let theory = Theory::new();
        let mut indexed = IndexedTheory::build(&theory);

        let id_p = indexed.intern_literal(&p);
        let id_q = indexed.intern_literal(&q);

        let fields_match =
            p.name_id() == q.name_id()
            && p.negation == q.negation
            && p.mode == q.mode
            && p.temporal == q.temporal
            && p.predicate_args() == q.predicate_args();

        if fields_match {
            prop_assert_eq!(id_p, id_q, "All fields match but LitIds differ: p={:?}, q={:?}", p, q);
        } else {
            prop_assert_ne!(id_p, id_q, "Fields differ but LitIds are equal: p={:?}, q={:?}", p, q);
        }
    }
}

// =============================================================================
// Roundtrip: intern then resolve preserves all fields including temporal
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// resolve(intern(lit)) must preserve name, negation, mode, temporal, and args.
    #[test]
    fn intern_resolve_roundtrip(lit in arb_literal_with_temporal()) {
        let theory = Theory::new();
        let mut indexed = IndexedTheory::build(&theory);

        let lit_id = indexed.intern_literal(&lit);
        let resolved = indexed.resolve_literal(lit_id);

        prop_assert_eq!(resolved.predicate_args(), lit.predicate_args(), "args mismatch");
        prop_assert_eq!(resolved.name_id(), lit.name_id(), "name mismatch");
        prop_assert_eq!(resolved.negation, lit.negation, "negation mismatch");
        prop_assert_eq!(&resolved.mode, &lit.mode, "mode mismatch");
        prop_assert_eq!(&resolved.temporal, &lit.temporal, "temporal mismatch");
    }
}
