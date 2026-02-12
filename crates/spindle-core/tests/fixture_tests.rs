//! Integration tests that exercise the test fixture library.
//!
//! This file imports `fixtures` and runs the built-in structural
//! assertions plus a few smoke tests that reason over the canonical
//! theories.

mod fixtures;

use spindle_core::conclusion::ConclusionType;
use spindle_core::reason::reason;

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
    // Should have at least the base fact conclusion
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

    // All conclusions from facts should be definite
    let definite_count = conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
        .count();
    assert!(
        definite_count > 0,
        "facts should produce definite conclusions"
    );
}
