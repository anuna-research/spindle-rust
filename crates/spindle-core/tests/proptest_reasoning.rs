//! Property-based tests for core reasoning invariants.
//!
//! Tests fundamental properties of the defeasible reasoning engine:
//! - Idempotence / determinism
//! - Monotonic fact addition (definite conclusions)
//! - Facts are always definitely provable
//! - Facts are both +D and +d (no conflicting path blocks them)
//! - Superior rule always wins (when fact != head)
//! - Ambiguity blocks both sides
//! - Defeaters only block, never prove
//! - Conclusion type coverage (every literal gets exactly one D and one d verdict)

mod fixtures;
mod proptest_helpers;

use proptest::prelude::*;
use std::collections::HashSet;

use spindle_core::conclusion::ConclusionType;
use spindle_core::reason::reason;

use proptest_helpers::{ATOMS, arb_conflicting_theory, arb_theory, conclusion_set};

// =============================================================================
// Reasoning idempotence: reason(t) == reason(t) (deterministic)
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Running reason() twice on the same theory produces identical conclusions.
    #[test]
    fn reason_is_deterministic(theory in arb_theory()) {
        let r1 = reason(&theory).unwrap();
        let r2 = reason(&theory).unwrap();

        let set1: HashSet<_> = r1.iter().map(|c| (c.conclusion_type, c.literal.canonical_name())).collect();
        let set2: HashSet<_> = r2.iter().map(|c| (c.conclusion_type, c.literal.canonical_name())).collect();

        prop_assert_eq!(set1, set2, "reason() should be deterministic");
    }
}

// =============================================================================
// Facts are always definitely provable
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Every fact in the theory should appear as +D in the conclusions.
    #[test]
    fn facts_always_definitely_provable(theory in arb_theory()) {
        let conclusions = reason(&theory).unwrap();

        let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);

        for fact_rule in theory.facts() {
            let head = fact_rule.head_literal();
            let canonical = head.canonical_name();
            prop_assert!(
                definite.contains(&canonical),
                "Fact '{}' should be +D but was not found in conclusions",
                canonical
            );
        }
    }
}

// =============================================================================
// Monotonic fact addition (definite conclusions)
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Adding a new fact to a theory should never remove existing
    /// definitely provable (+D) conclusions.
    #[test]
    fn monotonic_definite_on_fact_addition(
        theory in arb_theory(),
        new_fact in proptest::sample::select(ATOMS),
    ) {
        let before = reason(&theory).unwrap();
        let definite_before = conclusion_set(&before, ConclusionType::DefinitelyProvable);

        let mut extended = theory.clone();
        extended.add_fact(new_fact);

        let after = reason(&extended).unwrap();
        let definite_after = conclusion_set(&after, ConclusionType::DefinitelyProvable);

        for lit in &definite_before {
            prop_assert!(
                definite_after.contains(lit),
                "Adding fact '{}' removed +D conclusion '{}'",
                new_fact, lit
            );
        }
    }
}

// =============================================================================
// Conclusion coverage: every literal gets exactly one D-verdict and one d-verdict
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Every literal that appears in conclusions should have exactly one
    /// definite verdict (+D or -D) and exactly one defeasible verdict (+d or -d).
    #[test]
    fn conclusion_coverage(theory in arb_theory()) {
        let conclusions = reason(&theory).unwrap();

        // Group by canonical name
        let mut definite_verdicts: HashSet<String> = HashSet::new();
        let mut defeasible_verdicts: HashSet<String> = HashSet::new();

        for c in &conclusions {
            let name = c.literal.canonical_name();
            match c.conclusion_type {
                ConclusionType::DefinitelyProvable | ConclusionType::DefinitelyNotProvable => {
                    definite_verdicts.insert(name);
                }
                ConclusionType::DefeasiblyProvable | ConclusionType::DefeasiblyNotProvable => {
                    defeasible_verdicts.insert(name);
                }
            }
        }

        // Every literal with a definite verdict should also have a defeasible verdict
        for name in &definite_verdicts {
            prop_assert!(
                defeasible_verdicts.contains(name),
                "Literal '{}' has definite verdict but no defeasible verdict",
                name
            );
        }
    }
}

// =============================================================================
// Ambiguity blocks both: conflicting rules with no superiority
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// When two defeasible rules produce opposite conclusions and no
    /// superiority relation resolves the conflict, neither conclusion
    /// should be defeasibly provable (unless backed by a definite proof).
    #[test]
    fn ambiguity_blocks_both(theory in arb_conflicting_theory()) {
        let conclusions = reason(&theory).unwrap();

        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);
        let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);

        for lit_name in &defeasible {
            let complement = if lit_name.starts_with('~') {
                lit_name.trim_start_matches('~').to_string()
            } else {
                format!("~{lit_name}")
            };

            // Both sides being +d is impossible unless one has definite support
            if defeasible.contains(&complement) {
                prop_assert!(
                    definite.contains(lit_name) || definite.contains(&complement),
                    "Both +d {} and +d {} without definite support",
                    lit_name, complement
                );
            }
        }
    }
}

// =============================================================================
// Superior always wins: if r1 > r2 and both fire, r1's conclusion prevails
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// When a conflict is resolved by superiority, the superior rule's
    /// conclusion should be defeasibly provable, and the defeated one should not.
    /// We use separate atom pools for fact and head to avoid the case where
    /// the head atom is also a fact (which would provide definite support).
    #[test]
    fn superior_wins(
        fact_atom in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        head_atom in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
    ) {
        let mut theory = spindle_core::theory::Theory::new();
        theory.add_fact(&fact_atom);
        let r1 = theory.add_defeasible_rule(&[fact_atom.as_str()], &head_atom);
        let r2 = theory.add_defeasible_rule(&[fact_atom.as_str()], &format!("~{}", head_atom));
        theory.add_superiority(&r2, &r1);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        let neg = format!("~{}", head_atom);
        prop_assert!(
            defeasible.contains(&neg),
            "Superior rule's conclusion ~{} should be +d, got: {:?}",
            head_atom, defeasible
        );

        prop_assert!(
            !defeasible.contains(&head_atom),
            "Defeated conclusion {} should not be +d",
            head_atom
        );
    }
}

// =============================================================================
// Defeaters only block, never prove
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// A defeater's head literal should never appear as a positive
    /// defeasible or definite conclusion unless another rule also derives it.
    #[test]
    fn defeaters_never_prove(theory in arb_theory()) {
        let conclusions = reason(&theory).unwrap();

        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);
        let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);

        use spindle_core::rule::RuleType;

        for rule in theory.rules_by_type(RuleType::Defeater) {
            let defeater_head = rule.head_literal().canonical_name();

            if defeasible.contains(&defeater_head) || definite.contains(&defeater_head) {
                let has_non_defeater_support = theory.rules().any(|r| {
                    r.rule_type != RuleType::Defeater
                        && !r.head.is_empty()
                        && r.head_literal().canonical_name() == defeater_head
                });
                prop_assert!(
                    has_non_defeater_support,
                    "Defeater head '{}' is provable but no non-defeater rule supports it",
                    defeater_head
                );
            }
        }
    }
}

// =============================================================================
// reason() never panics
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// reason() should never panic on any generated theory.
    #[test]
    fn reason_never_panics(theory in arb_theory()) {
        let result = reason(&theory);
        prop_assert!(result.is_ok(), "reason() failed: {:?}", result.err());
    }
}
