//! Property-based tests for core data structure invariants.
//!
//! Tests structural properties of Literal, Theory, and SuperiorityIndex:
//! - Complement involution
//! - Literal equality ignores temporal bounds
//! - Rule label uniqueness
//! - Theory add/get round-trip
//! - Superiority asymmetry
//! - SuperiorityIndex consistency
//! - Metadata transparency

mod proptest_helpers;

use proptest::prelude::*;
use std::collections::HashSet;

use spindle_core::literal::Literal;
use spindle_core::reason::reason;
use spindle_core::superiority::{Superiority, SuperiorityIndex};

use proptest_helpers::{ATOMS, arb_literal, arb_mode, arb_temporal, arb_theory};

// =============================================================================
// Complement involution: complement(complement(L)) == L
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Complementing a literal twice returns an equal literal.
    #[test]
    fn complement_involution(lit in arb_literal()) {
        let double_comp = lit.complement().complement();
        prop_assert_eq!(lit, double_comp, "complement(complement(L)) should equal L");
    }
}

// =============================================================================
// Complement flips negation
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Complementing a literal flips its negation flag.
    #[test]
    fn complement_flips_negation(lit in arb_literal()) {
        let comp = lit.complement();
        prop_assert_ne!(lit.negation, comp.negation, "complement should flip negation");
        prop_assert_eq!(lit.name(), comp.name(), "complement should preserve name");
        prop_assert_eq!(lit.mode.clone(), comp.mode.clone(), "complement should preserve mode");
        prop_assert_eq!(lit.predicate_args(), comp.predicate_args(), "complement should preserve predicates");
    }
}

// =============================================================================
// Literal equality ignores temporal bounds
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// Two literals that differ only in temporal bounds should be equal.
    #[test]
    fn literal_equality_ignores_temporal(
        name in proptest::sample::select(ATOMS),
        negated in proptest::bool::ANY,
        mode in arb_mode(),
        t1 in arb_temporal(),
        t2 in arb_temporal(),
    ) {
        let lit1 = Literal::new(name, negated, mode.clone(), t1, vec![]);
        let lit2 = Literal::new(name, negated, mode, t2, vec![]);

        prop_assert_eq!(lit1, lit2, "Literals differing only in temporal should be equal");
    }
}

// =============================================================================
// Literal hash consistency with equality
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// If two literals are equal, they must have the same hash.
    #[test]
    fn equal_literals_same_hash(
        name in proptest::sample::select(ATOMS),
        negated in proptest::bool::ANY,
        mode in arb_mode(),
        t1 in arb_temporal(),
        t2 in arb_temporal(),
    ) {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let lit1 = Literal::new(name, negated, mode.clone(), t1, vec![]);
        let lit2 = Literal::new(name, negated, mode, t2, vec![]);

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        lit1.hash(&mut h1);
        lit2.hash(&mut h2);

        prop_assert_eq!(
            h1.finish(), h2.finish(),
            "Equal literals must have the same hash"
        );
    }
}

// =============================================================================
// Rule label uniqueness
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// All auto-generated rule labels in a theory should be unique.
    #[test]
    fn rule_labels_unique(theory in arb_theory()) {
        let labels: Vec<_> = theory.rules().map(|r| r.label.clone()).collect();
        let unique: HashSet<_> = labels.iter().collect();

        prop_assert_eq!(
            labels.len(), unique.len(),
            "Theory should have unique rule labels, got duplicates"
        );
    }
}

// =============================================================================
// Theory add/get round-trip
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// After adding facts via add_fact, they should be retrievable via get_rule.
    #[test]
    fn theory_add_get_roundtrip(
        atoms in proptest::collection::vec(
            proptest::sample::select(ATOMS),
            1..=8,
        )
    ) {
        let mut theory = spindle_core::theory::Theory::new();
        let mut labels = Vec::new();

        for &atom in &atoms {
            let label = theory.add_fact(atom);
            labels.push(label);
        }

        for label in &labels {
            let rule = theory.get_rule(label);
            prop_assert!(
                rule.is_some(),
                "Rule with label '{label}' should be retrievable after adding"
            );
            prop_assert!(
                rule.unwrap().is_fact(),
                "Retrieved rule should be a fact"
            );
        }
    }
}

// =============================================================================
// Superiority asymmetry
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// If is_superior(a, b), then NOT is_superior(b, a) (for non-circular).
    #[test]
    fn superiority_asymmetry(
        a in proptest::sample::select(ATOMS).prop_map(String::from),
        b in proptest::sample::select(ATOMS).prop_map(String::from),
    ) {
        prop_assume!(a != b);

        let sups = vec![Superiority::new(&a, &b)];
        let index = SuperiorityIndex::build(&sups);

        prop_assert!(index.is_superior(&a, &b), "a should be superior to b");
        prop_assert!(!index.is_superior(&b, &a), "b should NOT be superior to a");
    }
}

// =============================================================================
// SuperiorityIndex consistency: inferiors_of and superiors_of are inverse
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// If rule A beats rule B, then B should appear in inferiors_of(A)
    /// and A should appear in superiors_of(B).
    #[test]
    fn superiority_index_consistency(
        pairs in proptest::collection::vec(
            (
                proptest::sample::select(ATOMS).prop_map(String::from),
                proptest::sample::select(ATOMS).prop_map(String::from),
            ),
            1..=5,
        )
    ) {
        let sups: Vec<_> = pairs.iter()
            .filter(|(a, b)| a != b)
            .map(|(a, b)| Superiority::new(a.as_str(), b.as_str()))
            .collect();

        let index = SuperiorityIndex::build(&sups);

        for sup in &sups {
            prop_assert!(
                index.inferiors_of(&sup.superior).contains(&sup.inferior),
                "{} should be in inferiors_of({})", sup.inferior, sup.superior
            );
            prop_assert!(
                index.superiors_of(&sup.inferior).contains(&sup.superior),
                "{} should be in superiors_of({})", sup.superior, sup.inferior
            );
        }
    }
}

// =============================================================================
// Metadata transparency: adding metadata does not change conclusions
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Adding metadata to rules should not change reasoning conclusions.
    #[test]
    fn metadata_does_not_affect_conclusions(theory in arb_theory()) {
        let before = reason(&theory).unwrap();

        let mut with_meta = theory.clone();
        // Add metadata to all rules
        for rule in theory.rules() {
            with_meta.add_meta_string(&rule.label, "test-key", "test-value");
        }

        let after = reason(&with_meta).unwrap();

        let set_before: HashSet<_> = before.iter()
            .map(|c| (c.conclusion_type, c.literal.canonical_name()))
            .collect();
        let set_after: HashSet<_> = after.iter()
            .map(|c| (c.conclusion_type, c.literal.canonical_name()))
            .collect();

        prop_assert_eq!(set_before, set_after, "Metadata should not affect conclusions");
    }
}

// =============================================================================
// Mode complement involution
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Mode complement is an involution: complement(complement(m)) == m.
    #[test]
    fn mode_complement_involution(mode in arb_mode()) {
        let double = mode.complement().complement();
        prop_assert_eq!(mode, double, "Mode complement should be an involution");
    }
}
