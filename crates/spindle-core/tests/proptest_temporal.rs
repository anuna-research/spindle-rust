//! Property-based tests for temporal reasoning and Allen interval algebra.
//!
//! Tests:
//! - TimePoint total order (trichotomy)
//! - Allen relation mutual exclusivity
//! - Allen relation inverse symmetry
//! - Temporal intersection properties
//! - from_bounds auto-swap invariant

mod proptest_helpers;

use proptest::prelude::*;

use spindle_core::temporal::{AllenRelation, Temporal};

use proptest_helpers::{arb_allen_relation, arb_finite_temporal, arb_finite_timepoint, arb_timepoint};

// =============================================================================
// TimePoint total order: exactly one of a < b, a == b, a > b
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// For any two TimePoints, exactly one ordering relation holds.
    #[test]
    fn timepoint_trichotomy(a in arb_timepoint(), b in arb_timepoint()) {
        let lt = a < b;
        let eq = a == b;
        let gt = a > b;

        let count = [lt, eq, gt].iter().filter(|&&x| x).count();
        prop_assert_eq!(count, 1, "Exactly one of <, ==, > should hold for {:?} vs {:?}", a, b);
    }
}

// =============================================================================
// TimePoint transitivity
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// If a <= b and b <= c, then a <= c.
    #[test]
    fn timepoint_transitivity(
        a in arb_timepoint(),
        b in arb_timepoint(),
        c in arb_timepoint(),
    ) {
        if a <= b && b <= c {
            prop_assert!(a <= c, "TimePoint ordering should be transitive: {:?} <= {:?} <= {:?}", a, b, c);
        }
    }
}

// =============================================================================
// Allen relation mutual exclusivity
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// For any two non-trivial intervals, exactly one Allen relation holds.
    #[test]
    fn allen_mutual_exclusivity(t1 in arb_finite_temporal(), t2 in arb_finite_temporal()) {
        let relations = [
            t1.before(&t2),
            t1.after(&t2),
            t1.meets(&t2),
            t1.met_by(&t2),
            t1.overlaps(&t2),
            t1.overlapped_by(&t2),
            t1.starts(&t2),
            t1.started_by(&t2),
            t1.during(&t2),
            t1.contains(&t2),
            t1.finishes(&t2),
            t1.finished_by(&t2),
            t1.equals(&t2),
        ];

        let true_count = relations.iter().filter(|&&r| r).count();
        prop_assert_eq!(
            true_count, 1,
            "Exactly one Allen relation should hold between {:?} and {:?}, got {}",
            t1, t2, true_count
        );
    }
}

// =============================================================================
// Allen relation inverse symmetry
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// For any relation R, if R(t1, t2) holds then R.inverse()(t2, t1) holds.
    #[test]
    fn allen_inverse_symmetry(t1 in arb_finite_temporal(), t2 in arb_finite_temporal()) {
        let rel = t1.relation(&t2);
        let inv = rel.inverse();

        prop_assert!(
            inv.holds(&t2, &t1),
            "If {:?}({:?}, {:?}) then {:?}({:?}, {:?}) should hold",
            rel, t1, t2, inv, t2, t1
        );
    }
}

// =============================================================================
// Allen inverse is an involution
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// inverse(inverse(R)) == R for all Allen relations.
    #[test]
    fn allen_inverse_involution(rel in arb_allen_relation()) {
        prop_assert_eq!(
            rel, rel.inverse().inverse(),
            "inverse(inverse({:?})) should equal {:?}", rel, rel
        );
    }
}

// =============================================================================
// Allen relation <-> relation() method consistency
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// The `.relation()` method returns the same relation as the individual
    /// boolean check methods.
    #[test]
    fn allen_relation_method_consistent(t1 in arb_finite_temporal(), t2 in arb_finite_temporal()) {
        let rel = t1.relation(&t2);
        prop_assert!(
            rel.holds(&t1, &t2),
            "relation() returned {:?} but holds() says false for {:?} vs {:?}",
            rel, t1, t2
        );
    }
}

// =============================================================================
// Temporal::new auto-swaps inverted bounds
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Temporal::new(a, b) always produces start <= end.
    #[test]
    fn temporal_new_ensures_valid_interval(
        a in arb_finite_timepoint(),
        b in arb_finite_timepoint(),
    ) {
        let t = Temporal::new(a, b);
        prop_assert!(
            t.start <= t.end,
            "Temporal::new should ensure start <= end, got {:?}", t
        );
    }
}

// =============================================================================
// Temporal::from_bounds auto-swaps inverted bounds
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Temporal::from_bounds(a, b) always produces start <= end.
    #[test]
    fn from_bounds_ensures_valid_interval(a in -1000i64..=1000, b in -1000i64..=1000) {
        let t = Temporal::from_bounds(a, b);
        prop_assert!(
            t.start <= t.end,
            "from_bounds({a}, {b}) should ensure start <= end"
        );
    }
}

// =============================================================================
// Temporal intersection is commutative
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Intersection of two temporals is commutative.
    #[test]
    fn temporal_intersection_commutative(
        t1 in arb_finite_temporal(),
        t2 in arb_finite_temporal(),
    ) {
        let i1 = t1.intersection(&t2);
        let i2 = t2.intersection(&t1);
        prop_assert_eq!(i1, i2, "Intersection should be commutative");
    }
}

// =============================================================================
// Temporal intersection is a subset of both operands
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// If intersection exists, its bounds are within both operands.
    #[test]
    fn temporal_intersection_subset(
        t1 in arb_finite_temporal(),
        t2 in arb_finite_temporal(),
    ) {
        if let Some(inter) = t1.intersection(&t2) {
            prop_assert!(inter.start >= t1.start && inter.start >= t2.start);
            prop_assert!(inter.end <= t1.end && inter.end <= t2.end);
        }
    }
}

// =============================================================================
// Allen keyword round-trip
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// spl_keyword -> from_keyword round-trips for all Allen relations.
    #[test]
    fn allen_keyword_roundtrip(rel in arb_allen_relation()) {
        let keyword = rel.spl_keyword();
        let parsed = AllenRelation::from_keyword(keyword);
        prop_assert_eq!(
            Some(rel), parsed,
            "from_keyword(spl_keyword({:?})) should round-trip", rel
        );
    }
}
