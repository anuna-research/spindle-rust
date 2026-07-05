//! Property-based tests for family/exact mapping invariants.
//!
//! Verifies that literals differing only in temporal bounds share a `FamilyId`
//! but produce distinct `ExactLitId` values, and that non-temporal differences
//! (negation, args, mode) correctly separate families.

mod proptest_helpers;

use std::collections::HashSet;

use proptest::prelude::*;

use spindle_core::index::IndexedTheory;
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::projection::FamilyId;
use spindle_core::rule::Rule;
use spindle_core::temporal::Temporal;
use spindle_core::theory::Theory;

use proptest_helpers::{arb_finite_temporal, arb_literal, arb_mode};

// ===========================================================================
// Generators
// ===========================================================================

/// Atom names used in generated literals.
const ATOMS: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "h"];

/// Predicate argument pools.
const PRED_ARGS: &[&str] = &["alice", "bob", "carol", "dave"];

/// Generate a pair of distinct finite temporals.
fn arb_two_distinct_temporals() -> impl Strategy<Value = (Temporal, Temporal)> {
    (arb_finite_temporal(), arb_finite_temporal())
        .prop_filter("temporals must differ", |(a, b)| a != b)
}

/// Generate a literal and a second temporal for creating a temporal variant.
fn arb_literal_and_variant_temporal() -> impl Strategy<Value = (Literal, Temporal)> {
    (
        proptest::sample::select(ATOMS),
        proptest::bool::ANY,
        arb_mode(),
        proptest::collection::vec(
            proptest::sample::select(PRED_ARGS).prop_map(String::from),
            0..=2,
        ),
        arb_two_distinct_temporals(),
    )
        .prop_map(|(name, negated, mode, preds, (t1, t2))| {
            let lit = Literal::new(name, negated, mode, t1, preds);
            (lit, t2)
        })
}

/// Build a temporal variant of a literal (same name, negation, mode, args; different temporal).
fn with_temporal(lit: &Literal, temporal: Temporal) -> Literal {
    Literal::new(
        lit.name(),
        lit.negation,
        lit.mode.clone(),
        temporal,
        lit.predicates(),
    )
}

// ===========================================================================
// 1. Temporal variants share the same FamilyId
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn temporal_variants_share_family_id(
        (lit, t2) in arb_literal_and_variant_temporal()
    ) {
        let variant = with_temporal(&lit, t2);

        let family_a = FamilyId::from(&lit);
        let family_b = FamilyId::from(&variant);

        prop_assert_eq!(
            family_a, family_b,
            "literals differing only in temporal should share family: {} vs {}",
            lit, variant
        );
    }
}

// ===========================================================================
// 2. Temporal variants produce distinct ExactLitIds
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn temporal_variants_have_distinct_exact_ids(
        (lit, t2) in arb_literal_and_variant_temporal()
    ) {
        let variant = with_temporal(&lit, t2);

        // Build a theory containing both variants so the index sees them.
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", lit.clone()));
        theory.add_rule(Rule::fact("f2", variant.clone()));

        let mut idx = IndexedTheory::build(&theory);
        let exact_a = idx.exact_lit_id(&lit);
        let exact_b = idx.exact_lit_id(&variant);

        prop_assert_ne!(
            exact_a, exact_b,
            "temporal variants should have different ExactLitIds: {} vs {}",
            lit, variant
        );
    }
}

// ===========================================================================
// 3. FamilyId is stable regardless of temporal bounds
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn family_id_ignores_temporal(
        name in proptest::sample::select(ATOMS),
        negated in proptest::bool::ANY,
        mode in arb_mode(),
        preds in proptest::collection::vec(
            proptest::sample::select(PRED_ARGS).prop_map(String::from),
            0..=2,
        ),
        t1 in arb_finite_temporal(),
        t2 in arb_finite_temporal(),
    ) {
        let lit_a = Literal::new(name, negated, mode.clone(), t1, preds.clone());
        let lit_b = Literal::new(name, negated, mode, t2, preds);

        prop_assert_eq!(FamilyId::from(&lit_a), FamilyId::from(&lit_b));
    }
}

// ===========================================================================
// 4. Negation separates families
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn negation_separates_families(
        name in proptest::sample::select(ATOMS),
        mode in arb_mode(),
        preds in proptest::collection::vec(
            proptest::sample::select(PRED_ARGS).prop_map(String::from),
            0..=2,
        ),
        temporal in arb_finite_temporal(),
    ) {
        let pos = Literal::new(name, false, mode.clone(), temporal.clone(), preds.clone());
        let neg = Literal::new(name, true, mode, temporal, preds);

        prop_assert_ne!(
            FamilyId::from(&pos),
            FamilyId::from(&neg),
            "positive and negative literals must have different families"
        );
    }
}

// ===========================================================================
// 5. Different predicate args separate families
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn different_args_separate_families(
        name in proptest::sample::select(ATOMS),
        negated in proptest::bool::ANY,
        mode in arb_mode(),
        temporal in arb_finite_temporal(),
        (arg_a, arg_b) in (
            proptest::sample::select(PRED_ARGS).prop_map(String::from),
            proptest::sample::select(PRED_ARGS).prop_map(String::from),
        ).prop_filter("args must differ", |(a, b)| a != b),
    ) {
        let lit_a = Literal::new(name, negated, mode.clone(), temporal.clone(), vec![arg_a]);
        let lit_b = Literal::new(name, negated, mode, temporal, vec![arg_b]);

        prop_assert_ne!(
            FamilyId::from(&lit_a),
            FamilyId::from(&lit_b),
            "literals with different args must have different families"
        );
    }
}

// ===========================================================================
// 6. Different modes separate families
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn different_modes_separate_families(
        name in proptest::sample::select(ATOMS),
        negated in proptest::bool::ANY,
        temporal in arb_finite_temporal(),
    ) {
        let plain = Literal::new(name, negated, Mode::empty(), temporal.clone(), vec![]);
        let obligatory = Literal::new(name, negated, Mode::obligation(), temporal, vec![]);

        prop_assert_ne!(
            FamilyId::from(&plain),
            FamilyId::from(&obligatory),
            "literals with different modes must have different families"
        );
    }
}

// ===========================================================================
// 7. family_members groups all temporal variants
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn family_members_groups_temporal_variants(
        name in proptest::sample::select(ATOMS),
        negated in proptest::bool::ANY,
        temporals in proptest::collection::vec(arb_finite_temporal(), 2..=5)
            .prop_filter("need at least 2 distinct temporals", |ts| {
                let unique: HashSet<_> = ts.iter().map(|t| format!("{t:?}")).collect();
                unique.len() >= 2
            }),
    ) {
        let literals: Vec<Literal> = temporals
            .iter()
            .map(|t| Literal::new(name, negated, Mode::empty(), t.clone(), vec![]))
            .collect();

        // Deduplicate temporals for expected count.
        let unique_temporals: HashSet<String> = temporals.iter().map(|t| format!("{t:?}")).collect();

        let mut theory = Theory::new();
        for (i, lit) in literals.iter().enumerate() {
            theory.add_rule(Rule::fact(format!("f{i}"), lit.clone()));
        }

        let idx = IndexedTheory::build(&theory);
        let family = idx.family_id(&literals[0]);
        let members = idx.family_members(&family);

        prop_assert_eq!(
            members.len(),
            unique_temporals.len(),
            "family_members should contain one ExactLitId per unique temporal variant"
        );
    }
}

// ===========================================================================
// 8. family_for_exact round-trips correctly
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn family_for_exact_round_trips(
        (lit, t2) in arb_literal_and_variant_temporal()
    ) {
        let variant = with_temporal(&lit, t2);

        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", lit.clone()));
        theory.add_rule(Rule::fact("f2", variant.clone()));

        let mut idx = IndexedTheory::build(&theory);

        let exact_a = idx.exact_lit_id(&lit);
        let exact_b = idx.exact_lit_id(&variant);

        let family_a = idx.family_for_exact(exact_a).unwrap();
        let family_b = idx.family_for_exact(exact_b).unwrap();

        // Both exact IDs map back to the same family.
        prop_assert_eq!(
            family_a, family_b,
            "family_for_exact should return the same family for temporal variants"
        );

        // And that family matches the direct computation.
        prop_assert_eq!(
            family_a.clone(),
            idx.family_id(&lit),
            "family_for_exact should match family_id"
        );
    }
}

// ===========================================================================
// 9. FamilyId::complement is involutive
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn family_complement_is_involution(lit in arb_literal()) {
        let family = FamilyId::from(&lit);
        let double_comp = family.complement().complement();

        prop_assert_eq!(
            family, double_comp,
            "complement applied twice should return the original family"
        );
    }
}

// ===========================================================================
// 10. FamilyId::complement flips negation
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn family_complement_flips_negation(lit in arb_literal()) {
        let family = FamilyId::from(&lit);
        let comp = family.complement();

        prop_assert_ne!(
            family.negated(), comp.negated(),
            "complement should flip negation"
        );
        prop_assert_eq!(family.functor(), comp.functor());
        prop_assert_eq!(family.args(), comp.args());
        prop_assert_eq!(family.mode(), comp.mode());
    }
}

// ===========================================================================
// 11. Literal::family_id matches FamilyId::from
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn literal_family_id_method_consistent(lit in arb_literal()) {
        prop_assert_eq!(
            lit.family_id(),
            FamilyId::from(&lit),
            "Literal::family_id() must equal FamilyId::from(&lit)"
        );
    }
}

// ===========================================================================
// 12. Atemporal literal and temporal variant share family
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn atemporal_and_temporal_share_family(
        name in proptest::sample::select(ATOMS),
        negated in proptest::bool::ANY,
        temporal in arb_finite_temporal(),
    ) {
        let atemporal = Literal::new(name, negated, Mode::empty(), Temporal::empty(), vec![]);
        let temporal_lit = Literal::new(name, negated, Mode::empty(), temporal, vec![]);

        prop_assert_eq!(
            FamilyId::from(&atemporal),
            FamilyId::from(&temporal_lit),
            "atemporal and temporal variants should share a family"
        );
    }
}

// ===========================================================================
// 13. Exact IDs are distinct across different families
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn exact_ids_distinct_across_families(
        (name_a, name_b) in (
            proptest::sample::select(ATOMS),
            proptest::sample::select(ATOMS),
        ).prop_filter("names must differ", |(a, b)| a != b),
        temporal in arb_finite_temporal(),
    ) {
        let lit_a = Literal::new(name_a, false, Mode::empty(), temporal.clone(), vec![]);
        let lit_b = Literal::new(name_b, false, Mode::empty(), temporal, vec![]);

        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", lit_a.clone()));
        theory.add_rule(Rule::fact("f2", lit_b.clone()));

        let mut idx = IndexedTheory::build(&theory);
        let exact_a = idx.exact_lit_id(&lit_a);
        let exact_b = idx.exact_lit_id(&lit_b);

        prop_assert_ne!(exact_a, exact_b);

        let family_a = idx.family_for_exact(exact_a).unwrap();
        let family_b = idx.family_for_exact(exact_b).unwrap();
        prop_assert_ne!(family_a, family_b);
    }
}
