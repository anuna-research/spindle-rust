//! TEST-PBT-002: Property-based test for non-temporal equivalence.
//!
//! Verifies: non-temporal theories produce identical conclusions with and
//! without the TemporalBridge pipeline stage.
//!
//! Since non-temporal theories contain no temporal head literals, the bridge
//! stage should be a no-op — generating zero bridging rules — and therefore
//! reasoning results must be identical regardless of whether the stage is
//! present in the pipeline.

mod proptest_helpers;

use proptest::prelude::*;
use std::collections::BTreeSet;

use spindle_core::conclusion::{Conclusion, ConclusionType};
use spindle_core::pipeline::{
    Ground, Pipeline, TemporalBridge, TemporalVarValidation, Validate, WildcardRewrite,
};
use spindle_core::reason::reason_prepared;

use proptest_helpers::arb_theory;

/// Canonical representation of a conclusion for comparison.
/// Uses (canonical_name, conclusion_type) so ordering is deterministic.
fn conclusion_key(c: &Conclusion) -> (String, u8) {
    let type_ord = match c.conclusion_type {
        ConclusionType::DefinitelyProvable => 0,
        ConclusionType::DefinitelyNotProvable => 1,
        ConclusionType::DefeasiblyProvable => 2,
        ConclusionType::DefeasiblyNotProvable => 3,
    };
    (c.literal.canonical_name(), type_ord)
}

/// Run the pipeline with TemporalBridge included (matches default pipeline).
fn run_with_bridge(theory: &spindle_core::theory::Theory) -> BTreeSet<(String, u8)> {
    let pipeline = Pipeline::builder()
        .stage(Validate::default())
        .stage(WildcardRewrite)
        .stage(Ground::default())
        .stage(TemporalBridge)
        .stage(TemporalVarValidation::default())
        .build();

    let (prepared, _ctx) = pipeline.run(theory.clone()).unwrap();
    let conclusions = reason_prepared(&prepared).unwrap();
    conclusions.iter().map(conclusion_key).collect()
}

/// Run the pipeline without TemporalBridge.
fn run_without_bridge(theory: &spindle_core::theory::Theory) -> BTreeSet<(String, u8)> {
    let pipeline = Pipeline::builder()
        .stage(Validate::default())
        .stage(WildcardRewrite)
        .stage(Ground::default())
        .stage(TemporalVarValidation::default())
        .build();

    let (prepared, _ctx) = pipeline.run(theory.clone()).unwrap();
    let conclusions = reason_prepared(&prepared).unwrap();
    conclusions.iter().map(conclusion_key).collect()
}

// =============================================================================
// Core property: with-bridge ≡ without-bridge for non-temporal theories
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// For any non-temporal theory, reasoning with the TemporalBridge stage
    /// must produce exactly the same conclusions as reasoning without it.
    #[test]
    fn non_temporal_bridge_equivalence(theory in arb_theory()) {
        let with = run_with_bridge(&theory);
        let without = run_without_bridge(&theory);

        prop_assert_eq!(
            &with, &without,
            "Non-temporal theory produced different conclusions with vs without TemporalBridge.\n\
             Only in with-bridge: {:?}\n\
             Only in without-bridge: {:?}",
            with.difference(&without).collect::<Vec<_>>(),
            without.difference(&with).collect::<Vec<_>>(),
        );
    }
}

// =============================================================================
// Auxiliary: bridge stage generates zero rules for non-temporal theories
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// The TemporalBridge stage must not add any rules to a non-temporal theory.
    #[test]
    fn bridge_adds_no_rules_to_non_temporal(theory in arb_theory()) {
        let pipeline_no_bridge = Pipeline::builder()
            .stage(Validate::default())
            .stage(WildcardRewrite)
            .stage(Ground::default())
            .build();

        let pipeline_with_bridge = Pipeline::builder()
            .stage(Validate::default())
            .stage(WildcardRewrite)
            .stage(Ground::default())
            .stage(TemporalBridge)
            .build();

        let (no_bridge_theory, _) = pipeline_no_bridge.run(theory.clone()).unwrap();
        let (with_bridge_theory, _) = pipeline_with_bridge.run(theory).unwrap();

        prop_assert_eq!(
            no_bridge_theory.rule_count(),
            with_bridge_theory.rule_count(),
            "TemporalBridge added {} rules to a non-temporal theory",
            with_bridge_theory.rule_count().saturating_sub(no_bridge_theory.rule_count()),
        );
    }
}

// =============================================================================
// Auxiliary: conclusion count is identical
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// The number of conclusions must be identical with and without the bridge.
    #[test]
    fn same_conclusion_count(theory in arb_theory()) {
        let with = run_with_bridge(&theory);
        let without = run_without_bridge(&theory);

        prop_assert_eq!(
            with.len(),
            without.len(),
            "Conclusion count differs: with_bridge={}, without_bridge={}",
            with.len(),
            without.len(),
        );
    }
}
