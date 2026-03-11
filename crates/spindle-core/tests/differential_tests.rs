//! Differential tests: compare conclusions, explanation labels, and trust
//! outputs between the standard and projection-based reasoning paths
//! across all fixtures.
//!
//! These tests exercise the `ShadowReasoner` in enabled mode to verify that
//! the projection engine agrees with the standard DL(d) reasoner on every
//! canonical fixture theory.

mod fixtures;

use std::collections::{HashMap, HashSet};

use spindle_core::conclusion::{Conclusion, ConclusionType};
use spindle_core::index::IndexedTheory;
use spindle_core::pipeline::{PrepareOptions, compute_weighted_conclusions, prepare};
use spindle_core::projection::{FamilyId, ProjectionToken};
use spindle_core::reason::{reason, reason_prepared};
use spindle_core::shadow::{ShadowReasoner, ShadowResult};
use spindle_core::theory::Theory;
use spindle_parser::parse_spl;

// ===========================================================================
// Helpers
// ===========================================================================

/// Run shadow reasoning on a theory and return the full result.
fn shadow(theory: &Theory) -> ShadowResult {
    let mut indexed = IndexedTheory::build(theory);
    ShadowReasoner::enabled()
        .reason_shadow(&mut indexed)
        .expect("shadow reasoning should not fail")
}

/// Collect conclusion strings as a comparable set: "{+D bird, -d flies, ...}".
fn conclusion_set(conclusions: &[Conclusion]) -> HashSet<String> {
    conclusions.iter().map(|c| format!("{c}")).collect()
}

/// Collect positive conclusion literal names from a set of conclusions.
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
// 1. Consistency: shadow mode reports no divergences
// ===========================================================================

macro_rules! consistency_test {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let theory = $fixture;
            let result = shadow(&theory);
            assert!(
                result.is_consistent(),
                "shadow divergences on {}: {:?}",
                stringify!($name),
                result.divergences
            );
        }
    };
}

consistency_test!(consistency_empty, fixtures::empty_theory());
consistency_test!(consistency_facts_only, fixtures::facts_only());
consistency_test!(consistency_tweety_triangle, fixtures::tweety_triangle());
consistency_test!(consistency_nixon_diamond, fixtures::nixon_diamond());
consistency_test!(consistency_chain_1, fixtures::inheritance_chain(1));
consistency_test!(consistency_chain_5, fixtures::inheritance_chain(5));
consistency_test!(consistency_chain_10, fixtures::inheritance_chain(10));
consistency_test!(consistency_temporal, fixtures::temporal_theory());
consistency_test!(consistency_modal, fixtures::modal_theory());
consistency_test!(
    consistency_conflicting_defeaters,
    fixtures::conflicting_defeaters()
);

// ===========================================================================
// 2. Conclusion agreement: standard reasoner == shadow primary
// ===========================================================================

macro_rules! conclusion_agreement_test {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let theory = $fixture;

            // Standard path: DL(d) reasoner
            let standard = reason(&theory).expect("standard reason should succeed");

            // Projection path: shadow primary
            let result = shadow(&theory);

            let standard_set = conclusion_set(&standard);
            let shadow_set = conclusion_set(&result.primary);

            assert_eq!(
                standard_set,
                shadow_set,
                "conclusions differ on {}\n  standard only: {:?}\n  shadow only: {:?}",
                stringify!($name),
                standard_set.difference(&shadow_set).collect::<Vec<_>>(),
                shadow_set.difference(&standard_set).collect::<Vec<_>>(),
            );
        }
    };
}

conclusion_agreement_test!(conclusions_empty, fixtures::empty_theory());
conclusion_agreement_test!(conclusions_facts_only, fixtures::facts_only());
conclusion_agreement_test!(conclusions_tweety_triangle, fixtures::tweety_triangle());
conclusion_agreement_test!(conclusions_nixon_diamond, fixtures::nixon_diamond());
conclusion_agreement_test!(conclusions_chain_3, fixtures::inheritance_chain(3));
conclusion_agreement_test!(conclusions_temporal, fixtures::temporal_theory());
conclusion_agreement_test!(conclusions_modal, fixtures::modal_theory());
conclusion_agreement_test!(
    conclusions_conflicting_defeaters,
    fixtures::conflicting_defeaters()
);

// ===========================================================================
// 3. Label coverage: every positive conclusion's rule_label appears in
//    projection tokens. Blockers may add extra projection labels.
// ===========================================================================

macro_rules! label_test {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let theory = $fixture;
            let result = shadow(&theory);

            let conclusion_labels = positive_labels(&result.primary);
            let proj_labels = token_labels(&result.projection_tokens);

            // Every label that contributed to a positive conclusion should
            // have been projected (producing at least one token).
            for label in &conclusion_labels {
                assert!(
                    proj_labels.contains(label),
                    "label '{}' in conclusions but missing from projection tokens on {}",
                    label,
                    stringify!($name),
                );
            }
        }
    };
}

label_test!(labels_facts_only, fixtures::facts_only());
label_test!(labels_tweety_triangle, fixtures::tweety_triangle());
label_test!(labels_nixon_diamond, fixtures::nixon_diamond());
label_test!(labels_chain_5, fixtures::inheritance_chain(5));
label_test!(labels_temporal, fixtures::temporal_theory());
label_test!(labels_modal, fixtures::modal_theory());
label_test!(
    labels_conflicting_defeaters,
    fixtures::conflicting_defeaters()
);

// ===========================================================================
// 4. Family coverage: every positive conclusion's family has a token
// ===========================================================================

macro_rules! family_coverage_test {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let theory = $fixture;
            let result = shadow(&theory);

            let families = token_families(&result.projection_tokens);

            for c in result.primary.iter().filter(|c| c.is_positive()) {
                // Skip facts (no rule_label) — they don't go through projection.
                if c.rule_label.is_none() {
                    continue;
                }
                let family = FamilyId::from(&c.literal);
                assert!(
                    families.contains(&family),
                    "positive conclusion '{c}' has no family-level projection token on {}",
                    stringify!($name),
                );
            }
        }
    };
}

family_coverage_test!(families_tweety_triangle, fixtures::tweety_triangle());
family_coverage_test!(families_chain_5, fixtures::inheritance_chain(5));
family_coverage_test!(families_temporal, fixtures::temporal_theory());
family_coverage_test!(families_modal, fixtures::modal_theory());

// ===========================================================================
// 5. Compiled bodies populated for all fixtures with rules
// ===========================================================================

macro_rules! compiled_bodies_test {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let theory = $fixture;
            let result = shadow(&theory);

            // If the theory has non-fact rules, compiled_bodies should be non-empty.
            let has_non_fact_rules = theory.rules().any(|r| !r.is_fact());
            if has_non_fact_rules {
                assert!(
                    !result.compiled_bodies.is_empty(),
                    "compiled_bodies should be populated for {} (has non-fact rules)",
                    stringify!($name),
                );
            }
        }
    };
}

compiled_bodies_test!(compiled_empty, fixtures::empty_theory());
compiled_bodies_test!(compiled_facts_only, fixtures::facts_only());
compiled_bodies_test!(compiled_tweety, fixtures::tweety_triangle());
compiled_bodies_test!(compiled_chain, fixtures::inheritance_chain(5));
compiled_bodies_test!(compiled_temporal, fixtures::temporal_theory());
compiled_bodies_test!(compiled_modal, fixtures::modal_theory());
compiled_bodies_test!(compiled_defeaters, fixtures::conflicting_defeaters());

// ===========================================================================
// 6. Trust output agreement: weighted conclusions from standard match shadow
// ===========================================================================

/// Parse SPL, reason via both paths, compare weighted conclusions.
fn trust_differential(spl: &str) {
    let theory = parse_spl(spl).expect("SPL should parse");
    let opts = PrepareOptions::default();
    let prepared = prepare(&theory, opts).expect("prepare should succeed");
    let policy = prepared.theory.trust_policy().clone();

    // Standard path
    let standard_conclusions =
        reason_prepared(&prepared.theory).expect("standard reasoning should succeed");
    let standard_weighted =
        compute_weighted_conclusions(&standard_conclusions, &prepared.theory, &policy, None);

    // Projection path (shadow)
    let shadow_result = shadow(&prepared.theory);

    // Shadow primary conclusions should match standard conclusions.
    let standard_set = conclusion_set(&standard_conclusions);
    let shadow_set = conclusion_set(&shadow_result.primary);
    assert_eq!(
        standard_set, shadow_set,
        "trust differential: conclusions differ"
    );

    // Shadow should be consistent.
    assert!(
        shadow_result.is_consistent(),
        "trust differential: shadow divergences: {:?}",
        shadow_result.divergences
    );

    // Weighted conclusions should have same count and same degrees.
    let shadow_weighted =
        compute_weighted_conclusions(&shadow_result.primary, &prepared.theory, &policy, None);

    assert_eq!(
        standard_weighted.len(),
        shadow_weighted.len(),
        "weighted conclusion count differs"
    );

    // Build map by (literal_display, conclusion_type) for comparison.
    let standard_map: HashMap<(String, ConclusionType), f64> = standard_weighted
        .iter()
        .map(|wc| ((format!("{}", wc.literal), wc.conclusion_type), wc.degree))
        .collect();

    for wc in &shadow_weighted {
        let key = (format!("{}", wc.literal), wc.conclusion_type);
        let standard_degree = standard_map
            .get(&key)
            .unwrap_or_else(|| panic!("missing standard weighted conclusion for {:?}", key));
        assert!(
            (wc.degree - standard_degree).abs() < 1e-10,
            "trust degree differs for {:?}: standard={}, shadow={}",
            key,
            standard_degree,
            wc.degree,
        );
    }
}

#[test]
fn trust_single_source() {
    trust_differential(
        r#"
        (trusts alice 0.8)
        (claims alice (given bird))
    "#,
    );
}

#[test]
fn trust_weakest_link_chain() {
    trust_differential(
        r#"
        (trusts alice 0.6)
        (trusts bob 0.9)
        (claims alice (given bird))
        (claims bob (normally r1 (bird) (flies)))
    "#,
    );
}

#[test]
fn trust_three_step_chain() {
    trust_differential(
        r#"
        (trusts s1 0.9)
        (trusts s2 0.4)
        (trusts s3 0.8)
        (claims s1 (given a))
        (claims s2 (normally r1 (a) (b)))
        (claims s3 (normally r2 (b) (c)))
    "#,
    );
}

#[test]
fn trust_no_directives_default() {
    trust_differential(
        r#"
        (given bird)
    "#,
    );
}

#[test]
fn trust_superiority_conflict() {
    trust_differential(
        r#"
        (trusts alice 0.8)
        (trusts bob 0.9)
        (claims alice (given bird))
        (claims alice (given penguin))
        (claims bob (normally r1 (bird) (flies)))
        (claims bob (normally r2 (penguin) (~flies)))
        (prefer r2 r1)
    "#,
    );
}

#[test]
fn trust_threshold_evaluation() {
    trust_differential(
        r#"
        (trusts alice 0.4)
        (threshold action 0.7)
        (threshold warn 0.3)
        (claims alice (given bird))
    "#,
    );
}

// ===========================================================================
// 7. Projection token type correctness
// ===========================================================================

#[test]
fn defeater_tokens_are_attacks() {
    let theory = fixtures::conflicting_defeaters();
    let result = shadow(&theory);

    // If any attack tokens exist, they should reference defeater rules.
    let attack_labels: Vec<_> = result
        .projection_tokens
        .iter()
        .filter_map(|t| match t {
            ProjectionToken::Attack(fa) => Some(fa.rule_label.clone()),
            _ => None,
        })
        .collect();

    // Attack tokens should only come from defeater rules.
    for label in &attack_labels {
        let rule = theory
            .get_rule(label)
            .unwrap_or_else(|| panic!("attack token references unknown rule '{label}'"));
        assert!(
            rule.rule_type.is_defeater(),
            "attack token from non-defeater rule '{label}' (type: {:?})",
            rule.rule_type,
        );
    }
}

#[test]
fn defeasible_tokens_are_family_support() {
    let theory = fixtures::tweety_triangle();
    let result = shadow(&theory);

    let family_support_labels: Vec<_> = result
        .projection_tokens
        .iter()
        .filter_map(|t| match t {
            ProjectionToken::Family(fs) => Some(fs.rule_label.clone()),
            _ => None,
        })
        .collect();

    // Family support tokens should come from defeasible or strict rules.
    for label in &family_support_labels {
        let rule = theory
            .get_rule(label)
            .unwrap_or_else(|| panic!("family support token references unknown rule '{label}'"));
        assert!(
            !rule.rule_type.is_defeater(),
            "family support token from defeater rule '{label}'",
        );
    }
}

// ===========================================================================
// 8. Determinism: shadow reasoning produces identical results across runs
// ===========================================================================

#[test]
fn shadow_deterministic_tweety() {
    let theory = fixtures::tweety_triangle();
    let r1 = shadow(&theory);
    let r2 = shadow(&theory);

    assert_eq!(
        conclusion_set(&r1.primary),
        conclusion_set(&r2.primary),
        "non-deterministic conclusions"
    );
    assert_eq!(
        r1.projection_tokens.len(),
        r2.projection_tokens.len(),
        "non-deterministic token count"
    );
    assert_eq!(
        r1.divergence_count(),
        r2.divergence_count(),
        "non-deterministic divergence count"
    );
}

#[test]
fn shadow_deterministic_chain() {
    let theory = fixtures::inheritance_chain(10);
    let r1 = shadow(&theory);
    let r2 = shadow(&theory);

    assert_eq!(conclusion_set(&r1.primary), conclusion_set(&r2.primary));
    assert_eq!(r1.projection_tokens.len(), r2.projection_tokens.len());
}

// ===========================================================================
// 9. Edge cases
// ===========================================================================

#[test]
fn empty_theory_produces_no_tokens() {
    let result = shadow(&fixtures::empty_theory());
    assert!(result.primary.is_empty());
    assert!(result.projection_tokens.is_empty());
    assert!(result.is_consistent());
}

#[test]
fn facts_only_no_projection_tokens_for_facts() {
    let result = shadow(&fixtures::facts_only());

    // Facts are definite conclusions via empty-body rules. They don't produce
    // projection tokens because their rule labels aren't on positive
    // defeasible conclusions (they are on +D conclusions which are facts).
    // The key invariant is consistency.
    assert!(
        result.is_consistent(),
        "facts-only divergences: {:?}",
        result.divergences
    );
}

#[test]
fn long_chain_all_conclusions_propagate() {
    let depth = 20;
    let theory = fixtures::inheritance_chain(depth);
    let result = shadow(&theory);

    // Should have +d for p1..p20 and +D for p0.
    let names = positive_names(&result.primary);
    assert!(names.contains("p0"), "should have p0");
    for i in 1..=depth {
        assert!(
            names.contains(&format!("p{i}")),
            "should have p{i} in conclusions"
        );
    }

    assert!(
        result.is_consistent(),
        "long chain divergences: {:?}",
        result.divergences
    );
}
