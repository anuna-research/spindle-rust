//! Integration tests for Allen interval constraints in SPL.
//!
//! Tests cover:
//! - SPL parser: all 13 Allen relations parse correctly
//! - Single-variable interval binding: `(during (p) ?T)` form
//! - Grounding constraint filtering
//! - End-to-end reasoning with Allen constraints
//! - Negative cases (constraint rejection)
//! - Multiple constraints

use spindle_core::grounding::ground_theory;
use spindle_core::reason::reason;
use spindle_core::temporal::{AllenRelation, Temporal, TimePoint};
use spindle_parser::parse_spl;

// =========================================================================
// PARSER TESTS — All 13 Allen relations
// =========================================================================

#[test]
fn test_parse_all_allen_relations() {
    let relations = [
        ("before", AllenRelation::Before),
        ("after", AllenRelation::After),
        ("meets", AllenRelation::Meets),
        ("met-by", AllenRelation::MetBy),
        ("overlaps", AllenRelation::Overlaps),
        ("overlapped-by", AllenRelation::OverlappedBy),
        ("starts", AllenRelation::Starts),
        ("started-by", AllenRelation::StartedBy),
        ("within", AllenRelation::During), // within maps to During
        ("contains", AllenRelation::Contains),
        ("finishes", AllenRelation::Finishes),
        ("finished-by", AllenRelation::FinishedBy),
        ("equals", AllenRelation::Equals),
    ];

    for (keyword, expected_relation) in relations {
        let input = format!(
            r#"
            (given (during (p a) 0 10))
            (given (during (q b) 20 30))
            (normally r1
              (and (during (p ?x) ?T) (during (q ?y) ?S) ({keyword} ?T ?S))
              (result ?x ?y))
            "#
        );

        let theory = parse_spl(&input).unwrap_or_else(|e| {
            panic!("Failed to parse Allen relation '{keyword}': {e}");
        });

        // Find the rule with constraints
        let rule = theory
            .rules()
            .find(|r| r.label == "r1")
            .unwrap_or_else(|| panic!("Rule r1 not found for '{keyword}'"));

        assert_eq!(
            rule.constraints.len(),
            1,
            "Expected 1 constraint for '{keyword}', got {}",
            rule.constraints.len()
        );
        assert_eq!(
            rule.constraints[0].relation, expected_relation,
            "'{keyword}' should map to {expected_relation:?}"
        );
    }
}

#[test]
fn test_within_maps_to_during() {
    // Specifically verify that "within" maps to AllenRelation::During
    let theory = parse_spl(
        r#"
        (given (during (p) 0 10))
        (given (during (q) 5 8))
        (normally r1 (and (during (p) ?T) (during (q) ?S) (within ?S ?T)) (result))
        "#,
    )
    .unwrap();

    let rule = theory.rules().find(|r| r.label == "r1").unwrap();
    assert_eq!(rule.constraints[0].relation, AllenRelation::During);
}

// =========================================================================
// SINGLE-VARIABLE INTERVAL BINDING
// =========================================================================

#[test]
fn test_parse_single_var_during() {
    // (during (p ?x) ?T) — single variable binds the entire interval
    let theory = parse_spl(
        r#"
        (given (during (p a) 100 200))
        (normally r1 (during (p ?x) ?T) (q ?x))
        "#,
    )
    .unwrap();

    let rule = theory.rules().find(|r| r.label == "r1").unwrap();
    assert_eq!(rule.body.len(), 1);
    assert!(
        rule.body[0].as_logic().unwrap().interval_var.is_some(),
        "Body literal should have interval_var set"
    );
    assert!(
        rule.body[0].has_temporal_variables(),
        "Should report having temporal variables"
    );
}

#[test]
fn test_single_var_during_grounds() {
    // Verify that single-var interval binding works during grounding
    let theory = parse_spl(
        r#"
        (given (during (p a) 100 200))
        (normally r1 (during (p ?x) ?T) (q ?x))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);

    // Should produce q(a)
    let has_q = grounded.rules().any(|r| {
        r.label.starts_with("r1_")
            && r.head
                .iter()
                .any(|h| h.name() == "q" && h.predicates() == vec!["a".to_string()])
    });
    assert!(has_q, "Grounding should produce q(a)");
}

#[test]
fn test_single_var_during_propagates_temporal() {
    // When the head also has (during ... ?T), the interval should propagate
    let theory = parse_spl(
        r#"
        (given (during (p a) 100 200))
        (normally r1 (during (p ?x) ?T) (during (q ?x) ?T))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);

    // Should produce q(a)[100, 200]
    let has_q_temporal = grounded.rules().any(|r| {
        r.label.starts_with("r1_")
            && r.head.iter().any(|h| {
                h.name() == "q"
                    && h.predicates() == vec!["a".to_string()]
                    && h.temporal.start == TimePoint::Moment(100)
                    && h.temporal.end == TimePoint::Moment(200)
            })
    });
    assert!(
        has_q_temporal,
        "Grounding should propagate interval to q(a)[100, 200]"
    );
}

#[test]
fn test_reason_pipeline_keeps_interval_bindings_after_wildcard_rewrite() {
    let theory = parse_spl(
        r#"
        (given (during (p a) 0 10))
        (given (during (q b) 20 30))
        (normally r1
          (and (during (p ?x) ?T) (during (q ?y) ?S) (before ?T ?S))
          (result ?x ?y))
        "#,
    )
    .unwrap();

    let conclusions = reason(&theory).unwrap();

    assert!(
        conclusions.iter().any(|c| {
            c.is_positive()
                && c.literal.name() == "result"
                && c.literal.predicates() == vec!["a".to_string(), "b".to_string()]
        }),
        "reason() should derive result(a,b) when Allen constraints are satisfiable"
    );
}

// =========================================================================
// GROUNDING CONSTRAINT FILTERING
// =========================================================================

#[test]
fn test_before_constraint_accepts() {
    // Facts: p(a)[0,10], q(b)[20,30]
    // Rule: if p and q, and p.interval before q.interval, then result
    // [0,10] before [20,30] → true → should derive result
    let theory = parse_spl(
        r#"
        (given (during (p a) 0 10))
        (given (during (q b) 20 30))
        (normally r1
          (and (during (p ?x) ?T) (during (q ?y) ?S) (before ?T ?S))
          (result ?x ?y))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);

    let has_result = grounded.rules().any(|r| {
        r.head.iter().any(|h| {
            h.name() == "result" && h.predicates() == vec!["a".to_string(), "b".to_string()]
        })
    });
    assert!(
        has_result,
        "before constraint should accept [0,10] before [20,30]"
    );
}

#[test]
fn test_before_constraint_rejects() {
    // Facts: p(a)[20,30], q(b)[0,10]
    // Rule: if p before q → should NOT derive result
    // [20,30] before [0,10] → false → no result
    let theory = parse_spl(
        r#"
        (given (during (p a) 20 30))
        (given (during (q b) 0 10))
        (normally r1
          (and (during (p ?x) ?T) (during (q ?y) ?S) (before ?T ?S))
          (result ?x ?y))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);

    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        !has_result,
        "before constraint should reject [20,30] NOT before [0,10]"
    );
}

#[test]
fn test_meets_constraint() {
    // [0,10] meets [10,20] → true
    let theory = parse_spl(
        r#"
        (given (during (p) 0 10))
        (given (during (q) 10 20))
        (normally r1
          (and (during (p) ?T) (during (q) ?S) (meets ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);
    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        has_result,
        "meets constraint should accept [0,10] meets [10,20]"
    );
}

#[test]
fn test_meets_constraint_rejects() {
    // [0,10] meets [15,20] → false (gap)
    let theory = parse_spl(
        r#"
        (given (during (p) 0 10))
        (given (during (q) 15 20))
        (normally r1
          (and (during (p) ?T) (during (q) ?S) (meets ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);
    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        !has_result,
        "meets constraint should reject [0,10] not-meets [15,20]"
    );
}

#[test]
fn test_within_constraint() {
    // within = During: [3,7] during [0,10] → true
    let theory = parse_spl(
        r#"
        (given (during (inner) 3 7))
        (given (during (outer) 0 10))
        (normally r1
          (and (during (inner) ?T) (during (outer) ?S) (within ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);
    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        has_result,
        "within constraint should accept [3,7] within [0,10]"
    );
}

#[test]
fn test_contains_constraint() {
    // [0,10] contains [3,7] → true
    let theory = parse_spl(
        r#"
        (given (during (outer) 0 10))
        (given (during (inner) 3 7))
        (normally r1
          (and (during (outer) ?T) (during (inner) ?S) (contains ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);
    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        has_result,
        "contains constraint should accept [0,10] contains [3,7]"
    );
}

#[test]
fn test_overlaps_constraint() {
    // [0,7] overlaps [5,15] → true
    let theory = parse_spl(
        r#"
        (given (during (a) 0 7))
        (given (during (b) 5 15))
        (normally r1
          (and (during (a) ?T) (during (b) ?S) (overlaps ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);
    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        has_result,
        "overlaps constraint should accept [0,7] overlaps [5,15]"
    );
}

#[test]
fn test_equals_constraint() {
    // [0,10] equals [0,10] → true
    let theory = parse_spl(
        r#"
        (given (during (a) 0 10))
        (given (during (b) 0 10))
        (normally r1
          (and (during (a) ?T) (during (b) ?S) (equals ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);
    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        has_result,
        "equals constraint should accept [0,10] equals [0,10]"
    );
}

#[test]
fn test_equals_constraint_rejects() {
    // [0,10] equals [0,15] → false
    let theory = parse_spl(
        r#"
        (given (during (a) 0 10))
        (given (during (b) 0 15))
        (normally r1
          (and (during (a) ?T) (during (b) ?S) (equals ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);
    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        !has_result,
        "equals constraint should reject [0,10] not-equals [0,15]"
    );
}

// =========================================================================
// MULTIPLE CONSTRAINTS
// =========================================================================

#[test]
fn test_multiple_constraints() {
    // Three intervals: T before S, S meets U
    // a=[0,10], b=[20,30], c=[30,40]
    // T=a before S=b ✓  (10 < 20)
    // S=b meets U=c ✓  (30 == 30)
    let theory = parse_spl(
        r#"
        (given (during (a) 0 10))
        (given (during (b) 20 30))
        (given (during (c) 30 40))
        (normally r1
          (and (during (a) ?T) (during (b) ?S) (during (c) ?U)
               (before ?T ?S) (meets ?S ?U))
          (result))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);
    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        has_result,
        "Multiple constraints (before + meets) should accept"
    );
}

#[test]
fn test_multiple_constraints_partial_fail() {
    // Same setup but S does NOT meet U (gap)
    // a=[0,10], b=[20,30], c=[35,40]
    // T=a before S=b ✓  (10 < 20)
    // S=b meets U=c ✗  (30 != 35)
    let theory = parse_spl(
        r#"
        (given (during (a) 0 10))
        (given (during (b) 20 30))
        (given (during (c) 35 40))
        (normally r1
          (and (during (a) ?T) (during (b) ?S) (during (c) ?U)
               (before ?T ?S) (meets ?S ?U))
          (result))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);
    let has_result = grounded
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(
        !has_result,
        "Should reject when second constraint (meets) fails"
    );
}

// =========================================================================
// MULTIPLE FACTS — ALL VALID COMBINATIONS
// =========================================================================

#[test]
fn test_selects_valid_combinations_from_multiple_facts() {
    // p has two temporal instances, q has one
    // p(a)[0,10], p(a)[50,60], q(b)[20,30]
    // Rule: before ?T ?S
    // [0,10] before [20,30] → accept ✓
    // [50,60] before [20,30] → reject ✗
    let theory = parse_spl(
        r#"
        (given (during (p a) 0 10))
        (given (during (p a) 50 60))
        (given (during (q b) 20 30))
        (normally r1
          (and (during (p ?x) ?T) (during (q ?y) ?S) (before ?T ?S))
          (result ?x ?y))
        "#,
    )
    .unwrap();

    let grounded = ground_theory(&theory);

    let result_count = grounded
        .rules()
        .filter(|r| r.head.iter().any(|h| h.name() == "result"))
        .count();
    assert_eq!(
        result_count, 1,
        "Only one combination should satisfy before: [0,10] before [20,30]"
    );
}

// =========================================================================
// ALLEN RELATION KEYWORD HELPERS
// =========================================================================

#[test]
fn test_allen_relation_from_keyword() {
    assert_eq!(
        AllenRelation::from_keyword("before"),
        Some(AllenRelation::Before)
    );
    assert_eq!(
        AllenRelation::from_keyword("after"),
        Some(AllenRelation::After)
    );
    assert_eq!(
        AllenRelation::from_keyword("meets"),
        Some(AllenRelation::Meets)
    );
    assert_eq!(
        AllenRelation::from_keyword("met-by"),
        Some(AllenRelation::MetBy)
    );
    assert_eq!(
        AllenRelation::from_keyword("overlaps"),
        Some(AllenRelation::Overlaps)
    );
    assert_eq!(
        AllenRelation::from_keyword("overlapped-by"),
        Some(AllenRelation::OverlappedBy)
    );
    assert_eq!(
        AllenRelation::from_keyword("starts"),
        Some(AllenRelation::Starts)
    );
    assert_eq!(
        AllenRelation::from_keyword("started-by"),
        Some(AllenRelation::StartedBy)
    );
    assert_eq!(
        AllenRelation::from_keyword("within"),
        Some(AllenRelation::During)
    );
    assert_eq!(
        AllenRelation::from_keyword("contains"),
        Some(AllenRelation::Contains)
    );
    assert_eq!(
        AllenRelation::from_keyword("finishes"),
        Some(AllenRelation::Finishes)
    );
    assert_eq!(
        AllenRelation::from_keyword("finished-by"),
        Some(AllenRelation::FinishedBy)
    );
    assert_eq!(
        AllenRelation::from_keyword("equals"),
        Some(AllenRelation::Equals)
    );
    assert_eq!(AllenRelation::from_keyword("unknown"), None);
}

#[test]
fn test_allen_relation_is_keyword() {
    assert!(AllenRelation::is_allen_keyword("before"));
    assert!(AllenRelation::is_allen_keyword("within"));
    assert!(!AllenRelation::is_allen_keyword("during")); // "during" is NOT an Allen keyword
    assert!(!AllenRelation::is_allen_keyword("not"));
}

#[test]
fn test_allen_relation_holds() {
    let t1 = Temporal::from_bounds(0, 10);
    let t2 = Temporal::from_bounds(20, 30);

    assert!(AllenRelation::Before.holds(&t1, &t2));
    assert!(!AllenRelation::After.holds(&t1, &t2));
    assert!(AllenRelation::After.holds(&t2, &t1));
}

// =========================================================================
// ERROR CASES
// =========================================================================

#[test]
fn test_allen_constraint_non_variable_args_error() {
    // Allen constraints require ?-prefixed arguments
    let err = parse_spl(
        r#"
        (normally r1
          (and (p) (before foo bar))
          (result))
        "#,
    );
    assert!(
        err.is_err(),
        "Non-variable Allen constraint args should error"
    );
}

#[test]
fn test_during_single_var_non_variable_error() {
    // (during literal nonvar) where nonvar doesn't start with ? should error
    let err = parse_spl("(given (during p foo))");
    assert!(err.is_err(), "Non-variable single during arg should error");
}

#[test]
fn test_constraint_only_body_is_not_unconditional() {
    let theory = parse_spl(
        r#"
        (normally r1
          (and (before ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let conclusions = reason(&theory).unwrap();
    assert!(
        !conclusions
            .iter()
            .any(|c| c.is_positive() && c.literal.name() == "result"),
        "Unbound constraints must not allow rule to fire"
    );
}

// =========================================================================
// TO_SPL ROUNDTRIP
// =========================================================================

#[test]
fn test_interval_var_to_spl() {
    let theory = parse_spl(
        r#"
        (given (during (p a) 0 10))
        (normally r1 (during (p ?x) ?T) (q ?x))
        "#,
    )
    .unwrap();

    let rule = theory.rules().find(|r| r.label == "r1").unwrap();
    let body_spl = rule.body[0].to_spl();
    assert!(
        body_spl.contains("?T"),
        "to_spl should include interval var: {body_spl}"
    );
    assert!(
        body_spl.starts_with("(during"),
        "to_spl should wrap in during: {body_spl}"
    );
}

#[test]
fn test_rule_to_spl_roundtrip_preserves_allen_constraint() {
    let theory = parse_spl(
        r#"
        (normally r1
          (and (during (p) ?T) (during (q) ?S) (within ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let rule = theory.rules().find(|r| r.label == "r1").unwrap();
    let rendered = rule.to_spl();
    assert!(
        rendered.contains("(within ?T ?S)"),
        "Rendered SPL must keep Allen constraint: {rendered}"
    );

    let reparsed = parse_spl(&rendered).unwrap();
    let reparsed_rule = reparsed.rules().find(|r| r.label == "r1").unwrap();
    assert_eq!(reparsed_rule.constraints.len(), 1);
    assert_eq!(reparsed_rule.constraints[0].relation, AllenRelation::During);
}

// =========================================================================
// CONSTRAINT DISPLAY
// =========================================================================

#[test]
fn test_allen_constraint_display() {
    let theory = parse_spl(
        r#"
        (given (during (p) 0 10))
        (given (during (q) 20 30))
        (normally r1
          (and (during (p) ?T) (during (q) ?S) (before ?T ?S))
          (result))
        "#,
    )
    .unwrap();

    let rule = theory.rules().find(|r| r.label == "r1").unwrap();
    let display = format!("{}", rule.constraints[0]);
    assert!(
        display.contains("before"),
        "Display should show relation: {display}"
    );
    assert!(
        display.contains("?T"),
        "Display should show first var: {display}"
    );
    assert!(
        display.contains("?S"),
        "Display should show second var: {display}"
    );
}
