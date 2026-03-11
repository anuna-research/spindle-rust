//! Property-based tests verifying the redesigned (projection-based) engine
//! matches the current (standard DL(d)) engine on randomly generated
//! non-temporal theories.
//!
//! These tests use `ShadowReasoner` to run both engines in parallel and
//! verify parity across conclusions, projection tokens, compiled bodies,
//! diagnostics, and deterministic snapshots.

mod proptest_helpers;

use std::collections::HashSet;

use proptest::prelude::*;

use spindle_core::conclusion::{Conclusion, ConclusionType};
use spindle_core::index::IndexedTheory;
use spindle_core::projection::{FamilyId, ProjectionToken};
use spindle_core::reason::reason;
use spindle_core::shadow::{ShadowReasoner, ShadowResult};

use proptest_helpers::{
    arb_conflicting_theory, arb_resolved_conflict_theory, arb_theory, conclusion_set,
};

// ===========================================================================
// Helpers
// ===========================================================================

/// Run shadow reasoning and return the full result.
fn shadow(theory: &spindle_core::theory::Theory) -> ShadowResult {
    let mut indexed = IndexedTheory::build(theory);
    ShadowReasoner::enabled()
        .reason_shadow(&mut indexed)
        .expect("shadow reasoning should not fail")
}

/// Collect conclusion strings as a comparable set.
fn conclusion_str_set(conclusions: &[Conclusion]) -> HashSet<String> {
    conclusions.iter().map(|c| format!("{c}")).collect()
}

/// Collect positive conclusion literal names.
#[allow(dead_code)]
fn positive_names(conclusions: &[Conclusion]) -> HashSet<String> {
    conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| c.literal.name().to_string())
        .collect()
}

/// Collect rule labels from positive conclusions.
fn positive_labels(conclusions: &[Conclusion]) -> HashSet<String> {
    conclusions
        .iter()
        .filter(|c| c.is_positive())
        .filter_map(|c| c.rule_label.clone())
        .collect()
}

/// Collect rule labels from projection tokens.
fn token_labels(tokens: &[ProjectionToken]) -> HashSet<String> {
    tokens
        .iter()
        .map(|t| match t {
            ProjectionToken::Exact(es) => es.rule_label.clone(),
            ProjectionToken::Family(fs) => fs.rule_label.clone(),
            ProjectionToken::Attack(fa) => fa.rule_label.clone(),
        })
        .collect()
}

/// Collect family IDs from family-level projection tokens.
fn token_families(tokens: &[ProjectionToken]) -> HashSet<FamilyId> {
    tokens
        .iter()
        .filter_map(|t| match t {
            ProjectionToken::Family(fs) => Some(fs.family_id.clone()),
            ProjectionToken::Attack(fa) => Some(fa.family_id.clone()),
            ProjectionToken::Exact(_) => None,
        })
        .collect()
}

// ===========================================================================
// 1. Consistency: shadow mode reports zero divergences on random theories
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn shadow_consistent_on_random_theories(theory in arb_theory()) {
        let result = shadow(&theory);
        prop_assert!(
            result.is_consistent(),
            "shadow divergences: {:?}",
            result.divergences
        );
    }
}

// ===========================================================================
// 2. Conclusion agreement: standard reason() == shadow primary conclusions
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    #[test]
    fn conclusions_agree_on_random_theories(theory in arb_theory()) {
        let standard = reason(&theory).expect("standard reason should succeed");
        let result = shadow(&theory);

        let standard_set = conclusion_str_set(&standard);
        let shadow_set = conclusion_str_set(&result.primary);

        prop_assert_eq!(
            standard_set.clone(),
            shadow_set.clone(),
            "conclusions differ\n  standard only: {:?}\n  shadow only: {:?}",
            standard_set.difference(&shadow_set).collect::<Vec<_>>(),
            shadow_set.difference(&standard_set).collect::<Vec<_>>(),
        );
    }
}

// ===========================================================================
// 3. Conclusion agreement on conflicting theories
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn conclusions_agree_on_conflicting_theories(theory in arb_conflicting_theory()) {
        let standard = reason(&theory).expect("standard reason should succeed");
        let result = shadow(&theory);

        let standard_set = conclusion_str_set(&standard);
        let shadow_set = conclusion_str_set(&result.primary);

        prop_assert_eq!(standard_set, shadow_set);
        prop_assert!(
            result.is_consistent(),
            "shadow divergences on conflicting theory: {:?}",
            result.divergences
        );
    }
}

// ===========================================================================
// 4. Conclusion agreement on resolved-conflict theories
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn conclusions_agree_on_resolved_conflicts(
        (theory, _head) in arb_resolved_conflict_theory()
    ) {
        let standard = reason(&theory).expect("standard reason should succeed");
        let result = shadow(&theory);

        let standard_set = conclusion_str_set(&standard);
        let shadow_set = conclusion_str_set(&result.primary);

        prop_assert_eq!(standard_set, shadow_set);
        prop_assert!(
            result.is_consistent(),
            "shadow divergences on resolved conflict: {:?}",
            result.divergences
        );
    }
}

// ===========================================================================
// 5. Label coverage: every positive conclusion's rule_label appears in
//    projection tokens. Blockers may add extra projection labels.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn label_parity_on_random_theories(theory in arb_theory()) {
        let result = shadow(&theory);
        let conclusion_labels = positive_labels(&result.primary);
        let proj_labels = token_labels(&result.projection_tokens);

        for label in &conclusion_labels {
            prop_assert!(
                proj_labels.contains(label),
                "label '{}' in conclusions but missing from projection tokens",
                label,
            );
        }

    }
}

// ===========================================================================
// 6. Family coverage: every non-fact positive conclusion has a family token
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn family_coverage_on_random_theories(theory in arb_theory()) {
        let result = shadow(&theory);
        let families = token_families(&result.projection_tokens);

        for c in result.primary.iter().filter(|c| c.is_positive()) {
            // Facts don't go through projection.
            if c.rule_label.is_none() {
                continue;
            }
            let family = FamilyId::from(&c.literal);
            prop_assert!(
                families.contains(&family),
                "positive conclusion '{}' has no family-level projection token",
                c,
            );
        }
    }
}

// ===========================================================================
// 7. Compiled bodies: non-fact rules produce compiled body entries
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn compiled_bodies_populated_for_rules(theory in arb_theory()) {
        let result = shadow(&theory);
        let has_non_fact_rules = theory.rules().any(|r| !r.is_fact());

        if has_non_fact_rules {
            prop_assert!(
                !result.compiled_bodies.is_empty(),
                "compiled_bodies should be populated when theory has non-fact rules",
            );
        }
    }
}

// ===========================================================================
// 8. Determinism: shadow produces identical results across two runs
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn shadow_deterministic_on_random_theories(theory in arb_theory()) {
        let r1 = shadow(&theory);
        let r2 = shadow(&theory);

        prop_assert_eq!(
            conclusion_str_set(&r1.primary),
            conclusion_str_set(&r2.primary),
            "non-deterministic conclusions"
        );

        prop_assert_eq!(
            r1.projection_tokens.len(),
            r2.projection_tokens.len(),
            "non-deterministic token count"
        );

        prop_assert_eq!(
            r1.divergence_count(),
            r2.divergence_count(),
            "non-deterministic divergence count"
        );
    }
}

// ===========================================================================
// 10. Diagnostics parity: token counts match actual token vectors
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn diagnostics_counts_match_tokens(theory in arb_theory()) {
        let result = shadow(&theory);
        let diag = &result.diagnostics;

        let actual_exact = result
            .projection_tokens
            .iter()
            .filter(|t| matches!(t, ProjectionToken::Exact(_)))
            .count();
        let actual_family = result
            .projection_tokens
            .iter()
            .filter(|t| matches!(t, ProjectionToken::Family(_)))
            .count();
        let actual_attack = result
            .projection_tokens
            .iter()
            .filter(|t| matches!(t, ProjectionToken::Attack(_)))
            .count();

        prop_assert_eq!(diag.exact_supports, actual_exact,
            "exact_supports diagnostic mismatch");
        prop_assert_eq!(diag.family_supports, actual_family,
            "family_supports diagnostic mismatch");
        prop_assert_eq!(diag.family_attacks, actual_attack,
            "family_attacks diagnostic mismatch");
    }
}

// ===========================================================================
// 11. Snapshot determinism: two runs produce identical snapshots
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    #[test]
    fn snapshot_deterministic(theory in arb_theory()) {
        let r1 = shadow(&theory);
        let r2 = shadow(&theory);

        prop_assert_eq!(
            format!("{:?}", r1.snapshot),
            format!("{:?}", r2.snapshot),
            "snapshot should be deterministic across runs"
        );
    }
}

// ===========================================================================
// 12. Conclusion-type parity: per-type conclusion sets match
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn per_type_conclusion_parity(theory in arb_theory()) {
        let standard = reason(&theory).expect("reason should succeed");
        let result = shadow(&theory);

        for ct in &[
            ConclusionType::DefinitelyProvable,
            ConclusionType::DefinitelyNotProvable,
            ConclusionType::DefeasiblyProvable,
            ConclusionType::DefeasiblyNotProvable,
        ] {
            let standard_set = conclusion_set(&standard, *ct);
            let shadow_set = conclusion_set(&result.primary, *ct);
            prop_assert_eq!(
                standard_set.clone(),
                shadow_set.clone(),
                "conclusion type {:?} differs\n  standard only: {:?}\n  shadow only: {:?}",
                ct,
                standard_set.difference(&shadow_set).collect::<Vec<_>>(),
                shadow_set.difference(&standard_set).collect::<Vec<_>>(),
            );
        }
    }
}

// ===========================================================================
// 13. Attack tokens only from defeater rules
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn attack_tokens_from_defeaters_only(theory in arb_theory()) {
        let result = shadow(&theory);

        for token in &result.projection_tokens {
            if let ProjectionToken::Attack(fa) = token {
                let rule = theory.get_rule(&fa.rule_label);
                prop_assert!(
                    rule.is_some(),
                    "attack token references unknown rule '{}'",
                    fa.rule_label,
                );
                prop_assert!(
                    rule.unwrap().rule_type.is_defeater(),
                    "attack token from non-defeater rule '{}'",
                    fa.rule_label,
                );
            }
        }
    }
}

// ===========================================================================
// 14. Family support tokens never from defeater rules
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn family_support_not_from_defeaters(theory in arb_theory()) {
        let result = shadow(&theory);

        for token in &result.projection_tokens {
            if let ProjectionToken::Family(fs) = token {
                let rule = theory.get_rule(&fs.rule_label);
                prop_assert!(
                    rule.is_some(),
                    "family support token references unknown rule '{}'",
                    fs.rule_label,
                );
                prop_assert!(
                    !rule.unwrap().rule_type.is_defeater(),
                    "family support token from defeater rule '{}'",
                    fs.rule_label,
                );
            }
        }
    }
}

// ===========================================================================
// 15. Contributing labels in diagnostics cover positive conclusion labels.
//     Blockers may add extra labels that never appear on +D/+d conclusions.
// ===========================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn contributing_labels_match_conclusions(theory in arb_theory()) {
        let result = shadow(&theory);
        let conclusion_labels = positive_labels(&result.primary);
        let diag_labels: HashSet<String> = result
            .diagnostics
            .contributing_labels
            .iter()
            .cloned()
            .collect();

        for label in &conclusion_labels {
            prop_assert!(
                diag_labels.contains(label),
                "positive conclusion label '{}' missing from diagnostics",
                label,
            );
        }
    }
}
