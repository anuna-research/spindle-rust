//! Worked examples from SPEC-017 Section 9, implemented as integration tests.
//!
//! Each test parses SPL, runs the full pipeline, and checks expected conclusions exactly.

use spindle_core::conclusion::ConclusionType;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::reason::reason_prepared;
use spindle_parser::parse_spl;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn reason_spl(input: &str) -> Vec<spindle_core::Conclusion> {
    let theory = parse_spl(input).expect("SPL parse failed");
    let opts = PrepareOptions::default();
    let result = prepare(&theory, opts).expect("pipeline failed");
    reason_prepared(&result.theory).expect("reasoning failed")
}

fn has_defeasible_with_args(
    conclusions: &[spindle_core::Conclusion],
    name: &str,
    args: &[&str],
) -> bool {
    conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == name
            && c.literal.predicates() == args
    })
}

// ===========================================================================
// Section 9.1: Order Total with Tax
// ===========================================================================
//
// An e-commerce order: item price * quantity, tax rate applied, total computed.
// Exercises bind, comparison, and decimal arithmetic.

#[test]
fn section_9_1_order_total_with_tax() {
    let spl = r#"
        (given (item order-001 widget 25 4))
        (given (tax-rate standard 0.08))

        ; Compute line total = price * quantity
        (normally r-line-total
          (and (item ?order ?product ?price ?qty)
               (bind ?line-total (* ?price ?qty)))
          (line-total ?order ?product ?line-total))

        ; Compute tax = line-total * tax-rate
        (normally r-tax
          (and (line-total ?order ?product ?subtotal)
               (tax-rate standard ?rate)
               (bind ?tax (* ?subtotal ?rate)))
          (tax ?order ?tax))

        ; Compute grand total = subtotal + tax
        (normally r-grand-total
          (and (line-total ?order ?product ?subtotal)
               (tax ?order ?tax)
               (bind ?total (+ ?subtotal ?tax)))
          (grand-total ?order ?total))
    "#;
    let conclusions = reason_spl(spl);

    // line-total(order-001, widget, 100) — Integer(25) * Integer(4) = Integer(100)
    assert!(
        has_defeasible_with_args(&conclusions, "line-total", &["order-001", "widget", "100"]),
        "Expected +d line-total(order-001, widget, 100), got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive())
            .collect::<Vec<_>>()
    );

    // tax(order-001, 8.00) — Integer(100) * Decimal(0.08) = Decimal(8.00)
    assert!(
        has_defeasible_with_args(&conclusions, "tax", &["order-001", "8.00"]),
        "Expected +d tax(order-001, 8.00) (Decimal, not Float), got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive() && c.literal.name() == "tax")
            .collect::<Vec<_>>()
    );

    // grand-total(order-001, 108.00) — Integer(100) + Decimal(8.00) = Decimal(108.00)
    assert!(
        has_defeasible_with_args(&conclusions, "grand-total", &["order-001", "108.00"]),
        "Expected +d grand-total(order-001, 108.00) (Decimal), got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive() && c.literal.name() == "grand-total")
            .collect::<Vec<_>>()
    );
}

// ===========================================================================
// Section 9.2: Defeasible Classification with Override
// ===========================================================================
//
// Classify employees as "standard" or "high-earner" based on salary thresholds.
// A specific override rule can reclassify. Exercises bind, comparison, and
// defeasible superiority.

#[test]
fn section_9_2_defeasible_classification_with_override() {
    let spl = r#"
        (given (employee alice 95000))
        (given (employee bob 110000))
        (given (threshold high 100000))

        ; Default: everyone is standard
        (normally r-standard
          (employee ?emp ?sal)
          (class ?emp standard))

        ; Override: salary > threshold → high-earner
        (normally r-high
          (and (employee ?emp ?sal)
               (threshold high ?t)
               (> ?sal ?t))
          (class ?emp high-earner))

        ; high-earner classification beats standard
        (prefer r-high r-standard)
    "#;
    let conclusions = reason_spl(spl);

    // Alice earns 95000 < 100000 → class(alice, standard), no class(alice, high-earner)
    assert!(
        has_defeasible_with_args(&conclusions, "class", &["alice", "standard"]),
        "Expected +d class(alice, standard)"
    );
    assert!(
        !has_defeasible_with_args(&conclusions, "class", &["alice", "high-earner"]),
        "Expected no class(alice, high-earner) — 95000 < 100000"
    );

    // Bob earns 110000 > 100000 → class(bob, high-earner) beats class(bob, standard)
    assert!(
        has_defeasible_with_args(&conclusions, "class", &["bob", "high-earner"]),
        "Expected +d class(bob, high-earner) via superiority"
    );
}

// ===========================================================================
// Section 9.3: Temporal Arithmetic (Salary Bands Over Time)
// ===========================================================================
//
// Salary changes over time; classification should apply only in the appropriate
// temporal interval.

#[test]
fn section_9_3_temporal_arithmetic_salary_bands() {
    let spl = r#"
        (given (during (salary alice 95000) 0 100))
        (given (during (salary alice 110000) 100 200))
        (given (threshold senior 100000))

        (normally r-senior
          (and (during (salary ?emp ?amt) ?s ?e)
               (threshold senior ?t)
               (> ?amt ?t))
          (during (senior-earner ?emp) ?s ?e))
    "#;
    let conclusions = reason_spl(spl);

    // Only the 110000 period [100, 200] qualifies
    let senior_conclusions: Vec<_> = conclusions
        .iter()
        .filter(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "senior-earner"
        })
        .collect();

    assert_eq!(
        senior_conclusions.len(),
        1,
        "Expected exactly one senior-earner conclusion (only 110000 qualifies), got {}",
        senior_conclusions.len()
    );

    let senior = &senior_conclusions[0];
    assert!(
        senior.literal.predicates().contains(&"alice"),
        "Expected senior-earner(alice)"
    );
}

// ===========================================================================
// Section 9.4: Decimal Precision and Rounding
// ===========================================================================
//
// Verifies that decimal arithmetic produces exact results, not floating-point
// approximations.

#[test]
fn section_9_4_decimal_precision_exact() {
    // 0.1 + 0.2 should be exactly 0.3 (Decimal), not 0.30000000000000004 (Float)
    let spl = r#"
        (given trigger)
        (normally r-sum
          (and (trigger) (bind ?val (+ 0.1 0.2)))
          (result ?val))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible_with_args(&conclusions, "result", &["0.3"]),
        "Expected +d result(0.3) exact (Decimal), not 0.30000000000000004, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive() && c.literal.name() == "result")
            .collect::<Vec<_>>()
    );
}

#[test]
fn section_9_4_decimal_precision_tax_calculation() {
    // Tax on 100 at 0.08 should be Decimal(8.00), not Float
    let spl = r#"
        (given trigger)
        (normally r-tax
          (and (trigger) (bind ?tax (* 100 0.08)))
          (result ?tax))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible_with_args(&conclusions, "result", &["8.00"]),
        "Expected +d result(8.00) (Decimal with scale), got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive() && c.literal.name() == "result")
            .collect::<Vec<_>>()
    );
}

#[test]
fn section_9_4_decimal_precision_subtraction() {
    // 1.0 - 0.9 should be exactly 0.1
    let spl = r#"
        (given trigger)
        (normally r-diff
          (and (trigger) (bind ?val (- 1.0 0.9)))
          (result ?val))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        has_defeasible_with_args(&conclusions, "result", &["0.1"]),
        "Expected +d result(0.1) exact (Decimal), got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive() && c.literal.name() == "result")
            .collect::<Vec<_>>()
    );
}

#[test]
fn section_9_4_decimal_precision_division() {
    // 1 / 3 should be Decimal (not float) since both are integers,
    // integer division yields Decimal per promotion rule
    let spl = r#"
        (given trigger)
        (normally r-div
          (and (trigger) (bind ?val (/ 10 4)))
          (result ?val))
    "#;
    let conclusions = reason_spl(spl);
    // 10 / 4 = Decimal(2.50) — rust_decimal preserves scale from division
    assert!(
        has_defeasible_with_args(&conclusions, "result", &["2.50"]),
        "Expected +d result(2.50) from integer division promoting to Decimal, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.is_positive() && c.literal.name() == "result")
            .collect::<Vec<_>>()
    );
}
