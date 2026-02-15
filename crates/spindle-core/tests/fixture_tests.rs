//! Integration tests that exercise the test fixture library.
//!
//! This is the single home for fixture structural assertions and smoke
//! tests that reason over the canonical theories. Previously the
//! structural tests lived inside `fixtures/mod.rs` as `#[cfg(test)]`
//! self-tests, which caused them to run in every integration test binary
//! that declared `mod fixtures`.

mod fixtures;

use spindle_core::conclusion::ConclusionType;
use spindle_core::reason::reason;
use spindle_core::rule::RuleType;

// =============================================================================
// Structural assertions (moved from fixtures/mod.rs)
// =============================================================================

#[test]
fn fixture_tweety_triangle_structure() {
    let t = fixtures::tweety_triangle();
    // 2 facts + 1 strict + 2 defeasible = 5 rules
    assert_eq!(t.rule_count(), 5);
    assert_eq!(t.facts().count(), 2);
    assert_eq!(t.superiorities().len(), 1);
}

#[test]
fn fixture_nixon_diamond_structure() {
    let t = fixtures::nixon_diamond();
    // 2 facts + 2 defeasible = 4 rules
    assert_eq!(t.rule_count(), 4);
    assert_eq!(t.facts().count(), 2);
    assert_eq!(t.superiorities().len(), 0);
}

#[test]
fn fixture_inheritance_chain_depth_zero() {
    let t = fixtures::inheritance_chain(0);
    // Just the base fact
    assert_eq!(t.rule_count(), 1);
    assert_eq!(t.facts().count(), 1);
}

#[test]
fn fixture_inheritance_chain_depth_five() {
    let t = fixtures::inheritance_chain(5);
    // 1 fact + 5 rules = 6
    assert_eq!(t.rule_count(), 6);
    assert_eq!(t.facts().count(), 1);
}

#[test]
fn fixture_inheritance_chain_depth_one() {
    let t = fixtures::inheritance_chain(1);
    // 1 fact + 1 rule = 2
    assert_eq!(t.rule_count(), 2);
}

#[test]
fn fixture_temporal_theory_structure() {
    let t = fixtures::temporal_theory();
    assert_eq!(t.rule_count(), 5);
    assert_eq!(t.superiorities().len(), 1);

    let on_leave = t.get_rule("t_f1").expect("on_leave fact should exist");
    assert!(on_leave.head_literal().temporal.has_info());

    let r2 = t.get_rule("t_r2").expect("t_r2 should exist");
    assert!(r2.temporal.has_info());

    let r3 = t.get_rule("t_r3").expect("t_r3 should exist");
    assert!(r3.temporal.has_info());
}

#[test]
fn fixture_modal_theory_structure() {
    let t = fixtures::modal_theory();
    assert_eq!(t.rule_count(), 7);
    assert_eq!(t.facts().count(), 2);

    let r1 = t.get_rule("m_r1").expect("m_r1 should exist");
    assert!(!r1.head_literal().mode.is_empty());

    let r4 = t.get_rule("m_r4").expect("m_r4 should exist");
    assert_eq!(r4.head_literal().mode.name, Some("F".to_string()));
}

#[test]
fn fixture_conflicting_defeaters_structure() {
    let t = fixtures::conflicting_defeaters();
    assert_eq!(t.rule_count(), 8);
    assert_eq!(t.facts().count(), 3);
    assert_eq!(t.superiorities().len(), 0);

    let defeater_count = t.rules_by_type(RuleType::Defeater).count();
    assert_eq!(defeater_count, 3);
}

#[test]
fn fixture_empty_theory_structure() {
    let t = fixtures::empty_theory();
    assert_eq!(t.rule_count(), 0);
    assert_eq!(t.facts().count(), 0);
    assert_eq!(t.superiorities().len(), 0);
}

#[test]
fn fixture_facts_only_structure() {
    let t = fixtures::facts_only();
    assert_eq!(t.rule_count(), 3);
    assert_eq!(t.facts().count(), 3);
    assert_eq!(t.superiorities().len(), 0);
}

// =============================================================================
// Reasoning smoke tests
// =============================================================================

#[test]
fn fixture_tweety_triangle_reasons() {
    let theory = fixtures::tweety_triangle();
    let conclusions = reason(&theory).unwrap();
    assert!(
        !conclusions.is_empty(),
        "tweety triangle should produce conclusions"
    );
}

#[test]
fn fixture_nixon_diamond_reasons() {
    let theory = fixtures::nixon_diamond();
    let conclusions = reason(&theory).unwrap();
    assert!(
        !conclusions.is_empty(),
        "nixon diamond should produce conclusions"
    );
}

#[test]
fn fixture_inheritance_chain_reasons() {
    let theory = fixtures::inheritance_chain(3);
    let conclusions = reason(&theory).unwrap();
    assert!(!conclusions.is_empty(), "chain should produce conclusions");
}

#[test]
fn fixture_temporal_theory_reasons() {
    let theory = fixtures::temporal_theory();
    let conclusions = reason(&theory).unwrap();
    assert!(
        !conclusions.is_empty(),
        "temporal theory should produce conclusions"
    );
}

#[test]
fn fixture_modal_theory_reasons() {
    let theory = fixtures::modal_theory();
    let conclusions = reason(&theory).unwrap();
    assert!(
        !conclusions.is_empty(),
        "modal theory should produce conclusions"
    );
}

#[test]
fn fixture_conflicting_defeaters_reasons() {
    let theory = fixtures::conflicting_defeaters();
    let conclusions = reason(&theory).unwrap();
    assert!(
        !conclusions.is_empty(),
        "conflicting defeaters should produce conclusions"
    );
}

#[test]
fn fixture_empty_theory_reasons() {
    let theory = fixtures::empty_theory();
    let conclusions = reason(&theory).unwrap();
    assert!(
        conclusions.is_empty(),
        "empty theory should produce zero conclusions"
    );
}

#[test]
fn fixture_facts_only_produces_definite() {
    let theory = fixtures::facts_only();
    let conclusions = reason(&theory).unwrap();
    assert!(
        !conclusions.is_empty(),
        "facts-only theory should produce conclusions"
    );

    let definite_count = conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
        .count();
    assert!(
        definite_count > 0,
        "facts should produce definite conclusions"
    );
}
