//! Integration tests for temporal bridging (TEST-005, TEST-006, TEST-011, TEST-012, TEST-017).
//!
//! These tests verify end-to-end temporal bridging, cross-window blocking,
//! defeasible defeat through bridge, and non-temporal backward compatibility
//! as specified in SPEC-018.

mod fixtures;

use spindle_core::conclusion::ConclusionType;
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::pipeline::{Pipeline, PrepareOptions, prepare};
use spindle_core::query::{QueryStatus, matches_query_temporal, query};
use spindle_core::reason::{reason_prepared, reason_with_options};
use spindle_core::rule::Rule;
use spindle_core::temporal::{Temporal, TimePoint};
use spindle_core::theory::Theory;

// ===========================================================================
// Helpers
// ===========================================================================

/// Create a temporal fact `>> name [start, end].`
fn temporal_fact(label: &str, name: &str, start: i64, end: i64) -> Rule {
    let head = Literal::new(
        name,
        false,
        Mode::empty(),
        Temporal::new(TimePoint::from_millis(start), TimePoint::from_millis(end)),
        vec![],
    );
    Rule::fact(label, head)
}

/// Create a defeasible rule with a temporal body literal:
/// `body_name [start, end] => head_name.`
fn temporal_body_defeasible_rule(
    label: &str,
    body_name: &str,
    start: i64,
    end: i64,
    head_name: &str,
) -> Rule {
    let body = vec![Literal::new(
        body_name,
        false,
        Mode::empty(),
        Temporal::new(TimePoint::from_millis(start), TimePoint::from_millis(end)),
        vec![],
    )];
    Rule::defeasible(label, body, Literal::simple(head_name))
}

// ===========================================================================
// TEST-005: No double-decrement with two temporal facts
// Trace: REQ-003
// ===========================================================================

/// Two temporal facts `p[1,10]` and `p[20,30]` should bridge to a single
/// base `p`, allowing `p => q` to fire exactly once. The temporal bounds
/// in AtomKey ensure each temporal fact is a distinct atom, preventing
/// double-decrement of the body counter.
#[test]
fn test_005_no_double_decrement_with_two_temporal_facts() {
    // >> p [1, 10].  >> p [20, 30].  p => q.
    let mut theory = Theory::new();
    theory.add_rule(temporal_fact("f1", "p", 1, 10));
    theory.add_rule(temporal_fact("f2", "p", 20, 30));
    theory.add_defeasible_rule(&["p"], "q");

    let result = prepare(&theory, PrepareOptions::default()).unwrap();
    let conclusions = reason_prepared(&result.theory).unwrap();

    // q must be +d (defeasibly provable) exactly once — not duplicated by double-decrement
    let q_positive: Vec<_> = conclusions
        .iter()
        .filter(|c| {
            c.literal.name() == "q"
                && c.conclusion_type == ConclusionType::DefeasiblyProvable
        })
        .collect();
    assert_eq!(
        q_positive.len(),
        1,
        "q should be +d exactly once, got {}",
        q_positive.len()
    );

    // Also verify p (base, atemporal) is +D (definitely provable via bridge)
    let p_base_definite = conclusions.iter().any(|c| {
        c.literal.name() == "p"
            && c.literal.temporal.is_empty()
            && c.conclusion_type == ConclusionType::DefinitelyProvable
    });
    assert!(p_base_definite, "p (base) should be +D via bridging");
}

// ===========================================================================
// TEST-006: Cross-window satisfaction blocked
// Trace: REQ-003
// ===========================================================================

/// A fact `p[1,10]` must NOT satisfy a rule body requiring `p[20,30]`.
/// Temporal bounds are part of atom identity, so different windows
/// are different atoms.
#[test]
fn test_006_cross_window_satisfaction_blocked() {
    // >> p [1, 10].  p [20, 30] => q.
    let mut theory = Theory::new();
    theory.add_rule(temporal_fact("f1", "p", 1, 10));
    theory.add_rule(temporal_body_defeasible_rule("r1", "p", 20, 30, "q"));

    let result = prepare(&theory, PrepareOptions::default()).unwrap();
    let conclusions = reason_prepared(&result.theory).unwrap();

    // q must NOT be derived: p[1,10] ≠ p[20,30]
    let q_positive = conclusions.iter().any(|c| {
        c.literal.name() == "q" && c.conclusion_type.is_positive()
    });
    assert!(
        !q_positive,
        "q should NOT be derived from wrong temporal window"
    );
}

// ===========================================================================
// TEST-011: Temporal fact proves base via bridging
// Trace: REQ-005
// ===========================================================================

/// A temporal fact `p[1,10]` should be bridged to base `p` via a strict
/// rule, enabling `p => q` to fire. This verifies the core bridging
/// mechanism works end-to-end.
#[test]
fn test_011_temporal_fact_proves_base_via_bridging() {
    // >> p [1, 10].  p => q.
    let mut theory = Theory::new();
    theory.add_rule(temporal_fact("f1", "p", 1, 10));
    theory.add_defeasible_rule(&["p"], "q");

    let result = prepare(&theory, PrepareOptions::default()).unwrap();
    let conclusions = reason_prepared(&result.theory).unwrap();

    // q should be +d (defeasibly provable via temporal bridging)
    let q_proved = conclusions.iter().any(|c| {
        c.literal.name() == "q"
            && c.conclusion_type == ConclusionType::DefeasiblyProvable
    });
    assert!(
        q_proved,
        "q should be defeasibly provable via temporal bridging"
    );
}

// ===========================================================================
// TEST-012: Defeasible defeat works through bridge
// Trace: REQ-005
// ===========================================================================

/// Defeasible rules fired via bridging still participate in ambiguity
/// blocking. `p[1,10]` bridges to `p`, enabling `p => q`, but `>> ~q`
/// opposes it. With no superiority, `q` should be ambiguity-blocked.
#[test]
fn test_012_defeasible_defeat_works_through_bridge() {
    // >> p [1, 10].  p => q.  >> ~q.
    let mut theory = Theory::new();
    theory.add_rule(temporal_fact("f1", "p", 1, 10));
    theory.add_defeasible_rule(&["p"], "q");
    theory.add_fact("~q");

    let result = prepare(&theory, PrepareOptions::default()).unwrap();
    let conclusions = reason_prepared(&result.theory).unwrap();

    // Positive q should be blocked (-d q) because >> ~q opposes p => q
    let q_pos_defeasible = conclusions.iter().any(|c| {
        c.literal.name() == "q"
            && !c.literal.negation
            && c.conclusion_type == ConclusionType::DefeasiblyProvable
    });
    assert!(!q_pos_defeasible, "q (positive) should be blocked by ~q");

    // ~q should still be +d (it's a fact)
    let q_neg_defeasible = conclusions.iter().any(|c| {
        c.literal.name() == "q"
            && c.literal.negation
            && c.conclusion_type == ConclusionType::DefeasiblyProvable
    });
    assert!(q_neg_defeasible, "~q should be +d (it is a fact)");
}

// ===========================================================================
// TEST-017: Non-temporal theory unchanged
// Trace: REQ-008
// ===========================================================================

/// The temporal bridging mechanism must be a no-op for non-temporal
/// theories, ensuring backward compatibility. Uses the Nixon Diamond
/// as a canonical non-temporal example.
#[test]
fn test_017_non_temporal_theory_unchanged() {
    // >> republican. >> quaker. republican => ~pacifist. quaker => pacifist.
    let mut theory = Theory::new();
    theory.add_fact("republican");
    theory.add_fact("quaker");
    theory.add_defeasible_rule(&["republican"], "~pacifist");
    theory.add_defeasible_rule(&["quaker"], "pacifist");

    let result = prepare(&theory, PrepareOptions::default()).unwrap();
    let conclusions = reason_prepared(&result.theory).unwrap();

    // Verify bridge stage added zero rules (no temporal atoms)
    assert!(
        result
            .theory
            .rules()
            .all(|r| !r.label.starts_with("__bridge::")),
        "No bridge rules should exist in a non-temporal theory"
    );

    // pacifist should be ambiguity-blocked: neither +d pacifist nor +d ~pacifist
    let pacifist_pos = conclusions.iter().any(|c| {
        c.literal.name() == "pacifist"
            && !c.literal.negation
            && c.conclusion_type == ConclusionType::DefeasiblyProvable
    });
    let neg_pacifist_pos = conclusions.iter().any(|c| {
        c.literal.name() == "pacifist"
            && c.literal.negation
            && c.conclusion_type == ConclusionType::DefeasiblyProvable
    });
    assert!(!pacifist_pos, "pacifist should be blocked in Nixon diamond");
    assert!(
        !neg_pacifist_pos,
        "~pacifist should be blocked in Nixon diamond"
    );

    // republican and quaker should be +D (definite facts)
    let republican_definite = conclusions.iter().any(|c| {
        c.literal.name() == "republican"
            && c.conclusion_type == ConclusionType::DefinitelyProvable
    });
    let quaker_definite = conclusions.iter().any(|c| {
        c.literal.name() == "quaker"
            && c.conclusion_type == ConclusionType::DefinitelyProvable
    });
    assert!(republican_definite, "republican should be +D");
    assert!(quaker_definite, "quaker should be +D");
}

// ===========================================================================
// TEST-017b: Non-temporal reasoning identical with and without bridge stage
// Trace: REQ-008
// ===========================================================================

/// Verify that reasoning results are identical whether or not the
/// TemporalBridge stage is in the pipeline, for a non-temporal theory.
#[test]
fn test_017b_non_temporal_reasoning_identical_with_and_without_bridge() {
    use spindle_core::pipeline::{
        Ground, Validate, WildcardRewrite,
    };
    use std::collections::BTreeSet;

    let theory = fixtures::nixon_diamond();

    // Path A: pipeline WITHOUT TemporalBridge
    let pipeline_no_bridge = Pipeline::builder()
        .stage(Validate::default())
        .stage(WildcardRewrite)
        .stage(Ground::default())
        .build();
    let (theory_a, _) = pipeline_no_bridge.run(theory.clone()).unwrap();
    let conclusions_a = reason_prepared(&theory_a).unwrap();

    // Path B: default pipeline (WITH TemporalBridge)
    let (theory_b, _) = Pipeline::default_pipeline()
        .run(theory)
        .unwrap();
    let conclusions_b = reason_prepared(&theory_b).unwrap();

    let norm_a: BTreeSet<String> = conclusions_a.iter().map(|c| format!("{c}")).collect();
    let norm_b: BTreeSet<String> = conclusions_b.iter().map(|c| format!("{c}")).collect();

    assert_eq!(
        norm_a, norm_b,
        "Non-temporal theory should produce identical conclusions with and without bridge stage"
    );
}

// ===========================================================================
// TEST-016: matches_query_temporal filters conclusion list correctly
// Trace: REQ-007
// ===========================================================================

/// Wildcard query (empty temporal) combined with `matches_query_temporal`
/// should match all temporal variants of a literal in the conclusion list.
/// The scalar `query()` API returns Provable for the base literal via
/// bridging, while manual filtering over conclusions picks up all variants.
#[test]
fn test_016_matches_query_temporal_filters_conclusion_list() {
    // >> p [1, 10]. >> p [20, 30]. p => q.
    let mut theory = Theory::new();
    theory.add_rule(temporal_fact("f1", "p", 1, 10));
    theory.add_rule(temporal_fact("f2", "p", 20, 30));
    theory.add_defeasible_rule(&["p"], "q");

    // Scalar query: p (base) is provable via bridging
    let result = query(&theory, &Literal::simple("p")).unwrap();
    assert_eq!(result.status, QueryStatus::Provable);

    // Full conclusion list filtering via matches_query_temporal
    let conclusions = reason_with_options(&theory, PrepareOptions::default()).unwrap();
    let query_lit = Literal::simple("p"); // wildcard temporal
    let p_matches: Vec<_> = conclusions
        .iter()
        .filter(|c| {
            c.literal == query_lit
                && matches_query_temporal(&query_lit.temporal, &c.literal.temporal)
                && c.is_positive()
        })
        .collect();
    // Should match p[1,10], p[20,30], and p (base)
    assert!(
        p_matches.len() >= 3,
        "Temporal wildcard should match all variants, got {}",
        p_matches.len()
    );
}

// ===========================================================================
// TEST-016b: Bounded query rejects wrong temporal window
// Trace: REQ-007
// ===========================================================================

/// A temporally-qualified query must only match conclusions with the exact
/// same temporal window. This tests the critical false-positive bug:
/// querying `p[1,10]` when only `p[20,30]` exists must return Unknown,
/// not Provable.
#[test]
fn test_016b_bounded_query_rejects_wrong_temporal_window() {
    // >> p [20, 30].
    let mut theory = Theory::new();
    theory.add_rule(temporal_fact("f1", "p", 20, 30));

    // Query for p[1,10] — should be Unknown (no fact for that window)
    let query_lit = Literal::new(
        "p",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::from_millis(1), TimePoint::from_millis(10)),
        vec![],
    );
    let result = query(&theory, &query_lit).unwrap();
    assert_eq!(
        result.status,
        QueryStatus::Unknown,
        "p[1,10] should be Unknown when only p[20,30] exists"
    );

    // Query for p[20,30] — should be Provable
    let query_lit2 = Literal::new(
        "p",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::from_millis(20), TimePoint::from_millis(30)),
        vec![],
    );
    let result2 = query(&theory, &query_lit2).unwrap();
    assert_eq!(
        result2.status,
        QueryStatus::Provable,
        "p[20,30] should be Provable"
    );

    // Query for p (base, wildcard) — should be Provable via bridge
    let result3 = query(&theory, &Literal::simple("p")).unwrap();
    assert_eq!(
        result3.status,
        QueryStatus::Provable,
        "p (base) should be Provable via bridging"
    );
}
