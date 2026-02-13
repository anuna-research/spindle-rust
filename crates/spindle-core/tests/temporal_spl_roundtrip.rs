//! Integration tests for temporal SPL features.
//!
//! Tests the full roundtrip: SPL parse -> reason -> format back to SPL.
//! Exercises: RFC3339 timestamps, temporal conclusions, state queries,
//! rule-level temporal bounds, and Allen constraints in output.

use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::temporal::TimePoint;
use spindle_parser::parse_spl;

/// Helper: parse SPL, run pipeline + reasoning, return conclusions as strings.
fn reason_spl(input: &str) -> Vec<String> {
    let theory = parse_spl(input).expect("SPL parse failed");
    let opts = PrepareOptions::default();
    let result = prepare(&theory, opts).expect("pipeline failed");
    let conclusions =
        spindle_core::reason::reason_prepared(&result.theory).expect("reasoning failed");
    conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| format!("{} {}", c.conclusion_type.symbol(), c.literal.to_spl()))
        .collect()
}

/// Helper: parse SPL, run pipeline with reference time, return conclusions.
fn reason_spl_at(input: &str, reference_time: TimePoint) -> Vec<String> {
    let theory = parse_spl(input).expect("SPL parse failed");
    let opts = PrepareOptions {
        reference_time: Some(reference_time),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("pipeline failed");
    let conclusions =
        spindle_core::reason::reason_prepared(&result.theory).expect("reasoning failed");
    conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| format!("{} {}", c.conclusion_type.symbol(), c.literal.to_spl()))
        .collect()
}

// =========================================================================
// RFC3339 Timestamp Round-Trip
// =========================================================================

#[test]
fn test_rfc3339_roundtrip_in_fact() {
    let input = r#"
        (given (during (employed alice acme) (moment "2024-01-01T00:00:00.000Z") (moment "2025-01-01T00:00:00.000Z")))
    "#;
    let theory = parse_spl(input).expect("parse failed");
    let fact = theory.facts().next().expect("should have a fact");
    let spl = fact.to_spl();

    // Should contain (moment "...") form, not raw milliseconds
    assert!(
        spl.contains("moment"),
        "Expected RFC3339 moment syntax, got: {spl}"
    );
    assert!(spl.contains("2024-01"), "Expected 2024 timestamp in: {spl}");
    assert!(spl.contains("2025-01"), "Expected 2025 timestamp in: {spl}");
}

#[test]
fn test_small_numeric_timepoints_stay_numeric() {
    let input = r#"
        (given (during (active) 100 200))
    "#;
    let theory = parse_spl(input).expect("parse failed");
    let fact = theory.facts().next().expect("should have a fact");
    let spl = fact.to_spl();

    // Small values should stay numeric, not become RFC3339
    assert!(
        spl.contains("100") && spl.contains("200"),
        "Expected numeric timepoints, got: {spl}"
    );
    assert!(
        !spl.contains("moment"),
        "Small values should not use moment syntax: {spl}"
    );
}

// =========================================================================
// Temporal Conclusions in Output
// =========================================================================

#[test]
fn test_temporal_fact_conclusions_carry_bounds() {
    let input = r#"
        (given (during bird 100 200))
    "#;
    let conclusions = reason_spl(input);

    // The fact's temporal bounds should be visible in the conclusion
    let bird = conclusions.iter().find(|c| c.contains("bird")).unwrap();
    assert!(
        bird.contains("during") && bird.contains("100") && bird.contains("200"),
        "Expected temporal bounds in conclusion, got: {bird}"
    );
}

#[test]
fn test_temporal_conclusions_with_rfc3339() {
    let input = r#"
        (given (during (contract) (moment "2024-06-01T00:00:00.000Z") (moment "2025-06-01T00:00:00.000Z")))
    "#;
    let conclusions = reason_spl(input);

    let contract = conclusions.iter().find(|c| c.contains("contract")).unwrap();
    assert!(
        contract.contains("moment") && contract.contains("2024"),
        "Expected RFC3339 in conclusion, got: {contract}"
    );
}

// =========================================================================
// State Query Predicates
// =========================================================================

#[test]
fn test_active_at_query_fires() {
    let input = r#"
        (given (during (contract alice) 100 200))
        (normally r1 (and (during (contract alice) ?T) (active-at ?T 150)) (valid-contract alice))
    "#;
    let conclusions = reason_spl(input);

    assert!(
        conclusions
            .iter()
            .any(|c| c.contains("valid-contract") && c.contains("alice")),
        "Expected valid-contract conclusion at time 150, got: {conclusions:?}"
    );
}

#[test]
fn test_active_at_query_rejects() {
    let input = r#"
        (given (during (contract alice) 100 200))
        (normally r1 (and (during (contract alice) ?T) (active-at ?T 300)) (valid-contract alice))
    "#;
    let conclusions = reason_spl(input);

    assert!(
        !conclusions.iter().any(|c| c.contains("valid-contract")),
        "Should NOT conclude valid-contract at time 300 (outside interval), got: {conclusions:?}"
    );
}

#[test]
fn test_past_at_query() {
    let input = r#"
        (given (during (event) 100 200))
        (normally r1 (and (during (event) ?T) (past-at ?T 300)) (event-completed))
    "#;
    let conclusions = reason_spl(input);

    assert!(
        conclusions.iter().any(|c| c.contains("event-completed")),
        "Event [100,200] should be past at 300, got: {conclusions:?}"
    );
}

#[test]
fn test_future_at_query() {
    let input = r#"
        (given (during (event) 100 200))
        (normally r1 (and (during (event) ?T) (future-at ?T 50)) (event-upcoming))
    "#;
    let conclusions = reason_spl(input);

    assert!(
        conclusions.iter().any(|c| c.contains("event-upcoming")),
        "Event [100,200] should be future at 50, got: {conclusions:?}"
    );
}

// =========================================================================
// Rule-Level Temporal Bounds
// =========================================================================

#[test]
fn test_rule_level_temporal_parse_and_render() {
    let input = r#"
        (during (normally r1 bird flies) 100 500)
    "#;
    let theory = parse_spl(input).expect("parse failed");
    let rule = theory.get_rule("r1").expect("should find r1");

    // Rule should have temporal bounds
    assert!(
        !rule.temporal.is_empty(),
        "Rule should have temporal bounds"
    );

    // to_spl() should include the (during ...) wrapper
    let spl = rule.to_spl();
    assert!(
        spl.contains("during") && spl.contains("100") && spl.contains("500"),
        "Expected temporal wrapper in SPL, got: {spl}"
    );
}

#[test]
fn test_rule_level_temporal_with_at_filter() {
    let input = r#"
        (given bird)
        (during (normally r1 bird flies) 100 500)
    "#;

    // At time 250 (within bounds), rule should fire
    let at_250 = reason_spl_at(input, TimePoint::from_millis(250));
    assert!(
        at_250.iter().any(|c| c.contains("flies")),
        "Rule should fire at time 250, got: {at_250:?}"
    );

    // At time 600 (outside bounds), rule should NOT fire
    let at_600 = reason_spl_at(input, TimePoint::from_millis(600));
    assert!(
        !at_600.iter().any(|c| c.contains("flies")),
        "Rule should NOT fire at time 600, got: {at_600:?}"
    );
}

#[test]
fn test_fact_level_temporal_with_during_wrapper() {
    let input = r#"
        (during (given bird) 100 500)
    "#;
    let theory = parse_spl(input).expect("parse failed");
    let fact = theory.facts().next().expect("should have a fact");

    assert!(
        !fact.temporal.is_empty(),
        "Fact should have temporal bounds from during wrapper"
    );
}

// =========================================================================
// Allen Constraints in Rule Output
// =========================================================================

#[test]
fn test_allen_constraint_in_rule_to_spl() {
    let input = r#"
        (normally r1 (and (during (event1) ?T) (during (event2) ?S) (before ?T ?S)) (sequenced))
    "#;
    let theory = parse_spl(input).expect("parse failed");
    let rule = theory.get_rule("r1").expect("should find r1");

    let spl = rule.to_spl();
    assert!(
        spl.contains("before"),
        "Expected Allen constraint in SPL output, got: {spl}"
    );
    assert!(
        spl.contains("?T") && spl.contains("?S"),
        "Expected interval variables in SPL output, got: {spl}"
    );
}

#[test]
fn test_state_query_in_rule_to_spl() {
    let input = r#"
        (normally r1 (and (during (contract) ?T) (active-at ?T 150)) (valid))
    "#;
    let theory = parse_spl(input).expect("parse failed");
    let rule = theory.get_rule("r1").expect("should find r1");

    let spl = rule.to_spl();
    assert!(
        spl.contains("active-at"),
        "Expected state query in SPL output, got: {spl}"
    );
}

// =========================================================================
// Combined: full round-trip parse -> reason -> format
// =========================================================================

#[test]
fn test_full_temporal_roundtrip() {
    let input = r#"
        (given (during (project alpha) 100 300))
        (given (during (project beta) 400 600))
        (normally r1
            (and (during (project alpha) ?T) (during (project beta) ?S) (before ?T ?S))
            (sequential alpha beta))
    "#;
    let conclusions = reason_spl(input);

    // Should derive the sequential conclusion
    assert!(
        conclusions.iter().any(|c| c.contains("sequential")),
        "Expected sequential conclusion from Allen constraint, got: {conclusions:?}"
    );
}
