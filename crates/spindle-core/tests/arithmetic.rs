//! Integration tests for arithmetic in the grounding pipeline.
//!
//! End-to-end: parse SPL → ground → reason → check conclusions.
//!
//! Covers TEST-003 (bind), TEST-004 (comparison), TEST-006 (temporal arithmetic),
//! TEST-007 (arithmetic not in superiority), TEST-010 (cross-type matching).

use spindle_core::conclusion::ConclusionType;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::reason::reason_prepared;
use spindle_parser::parse_spl;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse SPL, run pipeline + reasoning, return all conclusions.
fn reason_spl(input: &str) -> Vec<spindle_core::Conclusion> {
    let theory = parse_spl(input).expect("SPL parse failed");
    let opts = PrepareOptions::default();
    let result = prepare(&theory, opts).expect("pipeline failed");
    reason_prepared(&result.theory).expect("reasoning failed")
}

/// Check that a literal with the given name is defeasibly provable (+d).
fn has_defeasible(conclusions: &[spindle_core::Conclusion], name: &str) -> bool {
    conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == name
    })
}

/// Check that a literal with the given name and predicate args is defeasibly provable (+d).
fn has_defeasible_with_args(
    conclusions: &[spindle_core::Conclusion],
    name: &str,
    args: &[&str],
) -> bool {
    conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == name
            && c.literal
                .predicates()
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                == args
    })
}

// ===========================================================================
// TEST-003: `bind` Predicate Binding (7 scenarios)
// ===========================================================================

/// TEST-003.1: Basic bind with constants — (bind ?z (+ ?x ?y)) with ?x=3, ?y=4 → ?z=7
#[test]
fn test_003_1_bind_basic_addition() {
    let spl = r#"
        (given (val 3 4))
        (normally r1
          (and (val ?x ?y) (bind ?z (+ ?x ?y)))
          (total ?z))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible_with_args(&conclusions, "total", &["7"]),
        "Expected +d total(7) from bind ?z = 3+4, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-003.2: Unbound variable in expression — grounding discarded (no conclusion)
#[test]
fn test_003_2_bind_unbound_variable() {
    let spl = r#"
        (given (val 3))
        (normally r1
          (and (val ?x) (bind ?z (+ ?x ?y)))
          (total ?z))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "total"),
        "Expected no total conclusion when ?y is unbound, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-003.3: Division by zero — grounding discarded
#[test]
fn test_003_3_bind_division_by_zero() {
    let spl = r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?z (/ 10 0)))
          (result ?z))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "result"),
        "Expected no result conclusion on division by zero, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-003.4: Power within range — (** 2 62) fits in i64
#[test]
fn test_003_4_bind_power_in_range() {
    let spl = r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?z (** 2 62)))
          (result ?z))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible_with_args(&conclusions, "result", &["4611686018427387904"]),
        "Expected +d result(4611686018427387904) for 2^62, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-003.5: Power overflow — (** 2 63) overflows i64; grounding discarded
#[test]
fn test_003_5_bind_power_overflow() {
    let spl = r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?z (** 2 63)))
          (result ?z))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "result"),
        "Expected no result conclusion on 2^63 overflow, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-003.6: bind in rule head position → parse error
#[test]
fn test_003_6_bind_in_head_parse_error() {
    let spl = r#"
        (normally r1 body (bind ?x 5))
    "#;
    let result = parse_spl(spl);
    assert!(
        result.is_err(),
        "Expected parse error for bind in head position"
    );
}

/// TEST-003.7: bind with non-variable first argument → parse error
#[test]
fn test_003_7_bind_non_variable_parse_error() {
    let spl = r#"
        (normally r1 (and (p) (bind 5 (+ 2 3))) (q))
    "#;
    let result = parse_spl(spl);
    assert!(
        result.is_err(),
        "Expected parse error for bind with non-variable first arg"
    );
}

// ===========================================================================
// TEST-004: Comparison Predicates (10 scenarios)
// ===========================================================================

/// TEST-004.1: Greater than (true) — ?a=20, ?b=10, (> ?a ?b) retained
#[test]
fn test_004_1_gt_true() {
    let spl = r#"
        (given (val x 10))
        (given (val y 20))
        (normally r1
          (and (val y ?a) (val x ?b) (> ?a ?b))
          (y-greater))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible(&conclusions, "y-greater"),
        "Expected +d y-greater when 20 > 10, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-004.2: Greater than (false) — ?a=5, ?b=10, (> ?a ?b) discarded
#[test]
fn test_004_2_gt_false() {
    let spl = r#"
        (given (val x 10))
        (given (val y 5))
        (normally r1
          (and (val y ?a) (val x ?b) (> ?a ?b))
          (y-greater))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "y-greater"),
        "Expected no y-greater when 5 > 10 is false, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-004.3: Equals true and false
#[test]
fn test_004_3_eq_true_and_false() {
    // Equal case: ?a=10, ?b=10
    let spl_eq = r#"
        (given (val x 10))
        (given (val y 10))
        (normally r1
          (and (val x ?a) (val y ?b) (= ?a ?b))
          (equal))
    "#;
    let conclusions = reason_spl(spl_eq);
    assert!(
        has_defeasible(&conclusions, "equal"),
        "Expected +d equal when 10 = 10"
    );

    // Not equal case: ?a=10, ?b=11
    let spl_ne = r#"
        (given (val x 10))
        (given (val y 11))
        (normally r1
          (and (val x ?a) (val y ?b) (= ?a ?b))
          (equal))
    "#;
    let conclusions = reason_spl(spl_ne);
    assert!(
        !has_defeasible(&conclusions, "equal"),
        "Expected no equal when 10 != 11"
    );
}

/// TEST-004.4: Not equals
#[test]
fn test_004_4_ne() {
    // ?a=10, ?b=10 → discarded
    let spl_same = r#"
        (given (val x 10))
        (given (val y 10))
        (normally r1
          (and (val x ?a) (val y ?b) (!= ?a ?b))
          (different))
    "#;
    let conclusions = reason_spl(spl_same);
    assert!(
        !has_defeasible(&conclusions, "different"),
        "Expected no different when 10 != 10 is false"
    );

    // ?a=10, ?b=11 → retained
    let spl_diff = r#"
        (given (val x 10))
        (given (val y 11))
        (normally r1
          (and (val x ?a) (val y ?b) (!= ?a ?b))
          (different))
    "#;
    let conclusions = reason_spl(spl_diff);
    assert!(
        has_defeasible(&conclusions, "different"),
        "Expected +d different when 10 != 11"
    );
}

/// TEST-004.5: Less than or equals — ?a=10, ?b=10 → retained
#[test]
fn test_004_5_le() {
    let spl = r#"
        (given (val x 10))
        (given (val y 10))
        (normally r1
          (and (val x ?a) (val y ?b) (<= ?a ?b))
          (le-holds))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible(&conclusions, "le-holds"),
        "Expected +d le-holds when 10 <= 10"
    );
}

/// TEST-004.6: Mixed types with promotion — (> 10 9.5) integer promoted to decimal
#[test]
fn test_004_6_mixed_type_promotion() {
    let spl = r#"
        (given (ival 10))
        (given (dval 9.5))
        (normally r1
          (and (ival ?a) (dval ?b) (> ?a ?b))
          (promoted-gt))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible(&conclusions, "promoted-gt"),
        "Expected +d promoted-gt when Integer(10) > Decimal(9.5) with promotion"
    );
}

/// TEST-004.7: Comparison in rule head position → parse error
#[test]
fn test_004_7_comparison_in_head_parse_error() {
    let spl = r#"
        (normally r1 body (> ?x 0))
    "#;
    let result = parse_spl(spl);
    assert!(
        result.is_err(),
        "Expected parse error for comparison in head position"
    );
}

/// TEST-004.8: Unbound variable in comparison → grounding discarded
#[test]
fn test_004_8_unbound_variable_in_comparison() {
    let spl = r#"
        (given trigger)
        (normally r1
          (and (trigger) (> ?x 100))
          (big))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "big"),
        "Expected no big conclusion when ?x is unbound in comparison"
    );
}

/// TEST-004.9: Chained order-dependent constraints (valid order) —
/// bind before comparison, ?x visible at comparison time
#[test]
fn test_004_9_chained_valid_order() {
    let spl = r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?x (+ 2 3)) (> ?x 4))
          (chained-ok))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible(&conclusions, "chained-ok"),
        "Expected +d chained-ok when bind ?x=5 then (> 5 4), got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-004.10: Reversed dependent order — comparison before bind,
/// ?x unbound at comparison time → grounding discarded
#[test]
fn test_004_10_chained_reversed_order() {
    let spl = r#"
        (given trigger)
        (normally r1
          (and (trigger) (> ?x 4) (bind ?x (+ 2 3)))
          (chained-bad))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "chained-bad"),
        "Expected no chained-bad when ?x unbound at comparison time (reversed order)"
    );
}

// ===========================================================================
// TEST-006: Arithmetic in Temporal Rules (2 scenarios)
// ===========================================================================

/// TEST-006.1: Rule with `during` literal and `>` comparison — fires only for
/// intervals where comparison holds (worked example §9.3: salary bands)
#[test]
fn test_006_1_temporal_arithmetic_salary_bands() {
    let spl = r#"
        (given (during (salary alice 95000) 0 100))
        (given (during (salary alice 110000) 100 200))
        (given (threshold senior 100000))

        (normally r1
          (and (during (salary ?emp ?amt) ?s ?e)
               (threshold senior ?t)
               (> ?amt ?t))
          (during (senior-earner ?emp) ?s ?e))
    "#;
    let conclusions = reason_spl(spl);

    // The 110000 period [100, 200] qualifies (110000 > 100000)
    let has_senior_earner = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "senior-earner"
            && c.literal.predicates().iter().any(|s| s == "alice")
    });
    assert!(
        has_senior_earner,
        "Expected +d senior-earner(alice) for the 110000 period, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );

    // The 95000 period should NOT produce senior-earner (95000 < 100000).
    // We verify there is exactly one senior-earner conclusion (the 110000 one).
    let senior_count = conclusions
        .iter()
        .filter(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "senior-earner"
        })
        .count();
    assert_eq!(
        senior_count, 1,
        "Expected exactly one senior-earner conclusion (only 110000 qualifies), got {senior_count}"
    );
}

/// TEST-006.2: Temporal/interval variable in arithmetic expression —
/// (+ ?T 1) fails because ?T is temporal, not numeric
#[test]
fn test_006_2_temporal_variable_in_arithmetic() {
    let spl = r#"
        (given (during (event) 100 200))
        (normally r1
          (and (during (event) ?T) (bind ?z (+ ?T 1)))
          (shifted ?z))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "shifted"),
        "Expected no shifted conclusion when ?T is a temporal variable used in arithmetic"
    );
}

// ===========================================================================
// TEST-007: Arithmetic Predicates Not in Superiority (2 scenarios)
// ===========================================================================

/// TEST-007.1: Arithmetic keyword as rule label in prefer → parse error
#[test]
fn test_007_1_reserved_keyword_in_prefer() {
    let spl = r#"
        (normally r1 (a) (b))
        (normally r2 (a) (c))
        (prefer bind r1)
    "#;
    let result = parse_spl(spl);
    assert!(
        result.is_err(),
        "Expected parse error when using 'bind' as a rule label in prefer"
    );
}

/// TEST-007.2: Arithmetic guard literals do not produce conclusions —
/// conclusions include `q` but never include a literal named `>` or `bind`
#[test]
fn test_007_2_arithmetic_guards_no_conclusions() {
    let spl = r#"
        (given (p))
        (normally r1
          (and (p) (> 1 0))
          (q))
    "#;
    let conclusions = reason_spl(spl);

    // q should be derived
    assert!(
        has_defeasible(&conclusions, "q"),
        "Expected +d q from rule with arithmetic guard, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );

    // No conclusion should have a literal named ">" or "bind"
    let has_arith_literal = conclusions.iter().any(|c| {
        let name = c.literal.name();
        name == ">"
            || name == "<"
            || name == "="
            || name == "!="
            || name == ">="
            || name == "<="
            || name == "bind"
    });
    assert!(
        !has_arith_literal,
        "Arithmetic guard literals must not produce conclusions"
    );
}

// ===========================================================================
// TEST-010: Cross-Type Numeric Matching (6 unit + 1 end-to-end)
// ===========================================================================

/// TEST-010.1: Integer fact vs Decimal binding — Integer(8) matches Decimal(8.00)
/// via (bind ?val (* 100 0.08))
#[test]
fn test_010_1_integer_fact_decimal_bind() {
    let spl = r#"
        (given (threshold 8))
        (given trigger)
        (normally r1
          (and (trigger) (bind ?val (* 100 0.08)) (threshold ?val))
          (matched))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible(&conclusions, "matched"),
        "Expected +d matched: Integer(8) should match Decimal(8.00) via promotion, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-010.2: Integer fact vs Float binding — Integer(100) matches Float(100.0)
/// via (bind ?val (* 50.0e0 2.0e0))
#[test]
fn test_010_2_integer_fact_float_bind() {
    let spl = r#"
        (given (limit 100))
        (given trigger)
        (normally r1
          (and (trigger) (bind ?val (* 50.0e0 2.0e0)) (limit ?val))
          (matched))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible(&conclusions, "matched"),
        "Expected +d matched: Integer(100) should match Float(100.0) via promotion, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}

/// TEST-010.3: Decimal fact vs Integer binding (value mismatch) —
/// Decimal(0.5) does not match Integer(1)
#[test]
fn test_010_3_decimal_fact_integer_bind_mismatch() {
    let spl = r#"
        (given (rate 0.5))
        (given trigger)
        (normally r1
          (and (trigger) (bind ?val (+ 0 1)) (rate ?val))
          (matched))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "matched"),
        "Expected no matched: Decimal(0.5) != Integer(1) after promotion"
    );
}

/// TEST-010.4: Symbol fact vs numeric binding — Symbol never matches numeric
#[test]
fn test_010_4_symbol_vs_numeric_no_match() {
    let spl = r#"
        (given (name alice))
        (given trigger)
        (normally r1
          (and (trigger) (bind ?val (+ 50 50)) (name ?val))
          (matched))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "matched"),
        "Expected no matched: Symbol(alice) never matches Integer(100)"
    );
}

/// TEST-010.5: Integer fact vs Decimal binding (equal values) —
/// Integer(95) matches Decimal(95.00)
#[test]
fn test_010_5_integer_matches_equal_decimal() {
    let spl = r#"
        (given (score 95))
        (given trigger)
        (normally r1
          (and (trigger) (bind ?val (* 95 1.0)) (score ?val))
          (matched))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible(&conclusions, "matched"),
        "Expected +d matched: Integer(95) should match Decimal(95.00) via promotion"
    );
}

/// TEST-010.6: Integer fact vs Decimal binding (unequal values) —
/// Integer(95) does not match Decimal(95.01)
#[test]
fn test_010_6_integer_does_not_match_unequal_decimal() {
    let spl = r#"
        (given (score 95))
        (given trigger)
        (normally r1
          (and (trigger) (bind ?val (+ 95.0 0.01)) (score ?val))
          (matched))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "matched"),
        "Expected no matched: Integer(95) != Decimal(95.01)"
    );
}

/// TEST-010 End-to-end: Full cross-type matching pipeline
/// (given (threshold 8)), (given (rate standard 0.08)),
/// rule binds ?val = (* 100 ?r) where ?r=0.08, producing Decimal(8.00),
/// matches Integer(8) fact → +d threshold-met
#[test]
fn test_010_end_to_end_cross_type_matching() {
    let spl = r#"
        (given (threshold 8))
        (given (rate standard 0.08))

        (normally r1
          (and (rate standard ?r) (bind ?val (* 100 ?r)) (threshold ?val))
          (threshold-met))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible(&conclusions, "threshold-met"),
        "Expected +d threshold-met via cross-type matching: \
         bind ?val = (* 100 0.08) = Decimal(8.00), matches fact (threshold 8) Integer(8), got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );
}
