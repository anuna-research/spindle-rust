//! Tests for the rounding functions: round, floor, ceil.
//!
//! round(value, dp) — Banker's rounding (half-to-even) via Decimal::round_dp_with_strategy.
//! floor(value)     — Largest integer <= value. Returns Integer.
//! ceil(value)      — Smallest integer >= value. Returns Integer.

use rust_decimal::Decimal;
use spindle_core::arith::{ArithError, ArithExpr};
use spindle_core::function_registry::{EvalContext, FunctionRegistry};
use spindle_core::grounding::Substitution;
use spindle_core::intern::intern;
use spindle_core::term::NumericValue;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn lit_int(n: i64) -> ArithExpr {
    ArithExpr::Lit(NumericValue::Integer(n))
}

fn lit_dec(n: i64, scale: u32) -> ArithExpr {
    ArithExpr::Lit(NumericValue::Decimal(Decimal::new(n, scale)))
}

fn lit_float(f: f64) -> ArithExpr {
    ArithExpr::Lit(NumericValue::Float(f))
}

fn call(name: &str, args: Vec<ArithExpr>) -> ArithExpr {
    ArithExpr::Call {
        name: intern(name),
        args,
    }
}

fn eval(expr: &ArithExpr) -> Result<NumericValue, ArithError> {
    let reg = FunctionRegistry::with_prelude();
    let ctx = EvalContext::with_registry(&reg);
    expr.eval_with_context(&Substitution::default(), &ctx)
}

// =====================================================================
// round(value, dp) — Banker's rounding (half-to-even)
// =====================================================================

#[test]
fn round_half_to_even_2_5() {
    // round(2.5, 0) = 2 (half-to-even: rounds to nearest even)
    let result = eval(&call("round", vec![lit_dec(25, 1), lit_int(0)])).unwrap();
    assert_eq!(result, NumericValue::Decimal(Decimal::new(2, 0)));
}

#[test]
fn round_half_to_even_3_5() {
    // round(3.5, 0) = 4 (half-to-even: rounds to nearest even)
    let result = eval(&call("round", vec![lit_dec(35, 1), lit_int(0)])).unwrap();
    assert_eq!(result, NumericValue::Decimal(Decimal::new(4, 0)));
}

#[test]
fn round_half_to_even_4_5() {
    // round(4.5, 0) = 4
    let result = eval(&call("round", vec![lit_dec(45, 1), lit_int(0)])).unwrap();
    assert_eq!(result, NumericValue::Decimal(Decimal::new(4, 0)));
}

#[test]
fn round_half_to_even_5_5() {
    // round(5.5, 0) = 6
    let result = eval(&call("round", vec![lit_dec(55, 1), lit_int(0)])).unwrap();
    assert_eq!(result, NumericValue::Decimal(Decimal::new(6, 0)));
}

#[test]
fn round_to_2dp() {
    // round(3.14159, 2) = 3.14
    let result = eval(&call("round", vec![lit_dec(314159, 5), lit_int(2)])).unwrap();
    assert_eq!(result, NumericValue::Decimal(Decimal::new(314, 2)));
}

#[test]
fn round_negative_half_to_even() {
    // round(-2.5, 0) = -2 (half-to-even)
    let result = eval(&call("round", vec![lit_dec(-25, 1), lit_int(0)])).unwrap();
    assert_eq!(result, NumericValue::Decimal(Decimal::new(-2, 0)));
}

#[test]
fn round_negative_half_to_even_odd() {
    // round(-3.5, 0) = -4
    let result = eval(&call("round", vec![lit_dec(-35, 1), lit_int(0)])).unwrap();
    assert_eq!(result, NumericValue::Decimal(Decimal::new(-4, 0)));
}

#[test]
fn round_integer_input() {
    // round(42, 0) = 42.0 (integer promoted to Decimal)
    let result = eval(&call("round", vec![lit_int(42), lit_int(0)])).unwrap();
    assert_eq!(result, NumericValue::Decimal(Decimal::new(42, 0)));
}

#[test]
fn round_integer_input_with_dp() {
    // round(42, 2) = 42.00
    let result = eval(&call("round", vec![lit_int(42), lit_int(2)])).unwrap();
    assert_eq!(result, NumericValue::Decimal(Decimal::new(4200, 2)));
}

#[test]
fn round_negative_dp_error() {
    // round(3.14, -1) → error
    let err = eval(&call("round", vec![lit_dec(314, 2), lit_int(-1)])).unwrap_err();
    assert!(
        matches!(err, ArithError::TypeMismatch { op: "round", .. }),
        "expected TypeMismatch, got: {err:?}"
    );
}

#[test]
fn round_non_integer_dp_error() {
    // round(3.14, 2.0) → error
    let err = eval(&call("round", vec![lit_dec(314, 2), lit_dec(20, 1)])).unwrap_err();
    assert!(
        matches!(err, ArithError::TypeMismatch { op: "round", .. }),
        "expected TypeMismatch, got: {err:?}"
    );
}

#[test]
fn round_float_input() {
    // round(2.5e0, 0) should work (float promoted to decimal for rounding)
    let result = eval(&call("round", vec![lit_float(2.5), lit_int(0)])).unwrap();
    // Result is Decimal
    assert!(matches!(result, NumericValue::Decimal(_)));
}

// =====================================================================
// floor(value) — Largest integer <= value
// =====================================================================

#[test]
fn floor_positive_fractional() {
    // floor(3.7) = 3
    let result = eval(&call("floor", vec![lit_dec(37, 1)])).unwrap();
    assert_eq!(result, NumericValue::Integer(3));
}

#[test]
fn floor_negative_fractional() {
    // floor(-3.2) = -4 (rounds toward negative infinity)
    let result = eval(&call("floor", vec![lit_dec(-32, 1)])).unwrap();
    assert_eq!(result, NumericValue::Integer(-4));
}

#[test]
fn floor_exact_integer_value() {
    // floor(5.0) = 5
    let result = eval(&call("floor", vec![lit_dec(50, 1)])).unwrap();
    assert_eq!(result, NumericValue::Integer(5));
}

#[test]
fn floor_integer_passthrough() {
    // floor(42) = 42
    let result = eval(&call("floor", vec![lit_int(42)])).unwrap();
    assert_eq!(result, NumericValue::Integer(42));
}

#[test]
fn floor_negative_exact() {
    // floor(-5.0) = -5
    let result = eval(&call("floor", vec![lit_dec(-50, 1)])).unwrap();
    assert_eq!(result, NumericValue::Integer(-5));
}

#[test]
fn floor_float_input() {
    // floor(3.7e0) = 3
    let result = eval(&call("floor", vec![lit_float(3.7)])).unwrap();
    assert_eq!(result, NumericValue::Integer(3));
}

#[test]
fn floor_negative_float() {
    // floor(-3.2e0) = -4
    let result = eval(&call("floor", vec![lit_float(-3.2)])).unwrap();
    assert_eq!(result, NumericValue::Integer(-4));
}

// =====================================================================
// ceil(value) — Smallest integer >= value
// =====================================================================

#[test]
fn ceil_positive_fractional() {
    // ceil(3.2) = 4
    let result = eval(&call("ceil", vec![lit_dec(32, 1)])).unwrap();
    assert_eq!(result, NumericValue::Integer(4));
}

#[test]
fn ceil_negative_fractional() {
    // ceil(-3.7) = -3 (rounds toward positive infinity)
    let result = eval(&call("ceil", vec![lit_dec(-37, 1)])).unwrap();
    assert_eq!(result, NumericValue::Integer(-3));
}

#[test]
fn ceil_exact_integer_value() {
    // ceil(5.0) = 5
    let result = eval(&call("ceil", vec![lit_dec(50, 1)])).unwrap();
    assert_eq!(result, NumericValue::Integer(5));
}

#[test]
fn ceil_integer_passthrough() {
    // ceil(42) = 42
    let result = eval(&call("ceil", vec![lit_int(42)])).unwrap();
    assert_eq!(result, NumericValue::Integer(42));
}

#[test]
fn ceil_negative_exact() {
    // ceil(-5.0) = -5
    let result = eval(&call("ceil", vec![lit_dec(-50, 1)])).unwrap();
    assert_eq!(result, NumericValue::Integer(-5));
}

#[test]
fn ceil_float_input() {
    // ceil(3.2e0) = 4
    let result = eval(&call("ceil", vec![lit_float(3.2)])).unwrap();
    assert_eq!(result, NumericValue::Integer(4));
}

#[test]
fn ceil_negative_float() {
    // ceil(-3.7e0) = -3
    let result = eval(&call("ceil", vec![lit_float(-3.7)])).unwrap();
    assert_eq!(result, NumericValue::Integer(-3));
}

// =====================================================================
// End-to-end pipeline tests
// =====================================================================

#[test]
fn round_in_bind_expression() {
    // (bind ?pay (round (* 25.50 8) 2)) → 204.00
    let spl = r#"
        (given (rate 25.50))
        (given (hours 8))
        (normally r1
          (and (rate ?rate) (hours ?hours) (bind ?pay (round (* ?rate ?hours) 2)))
          (result ?pay))
    "#;
    let theory = spindle_parser::parse_spl(spl).unwrap();
    let result = spindle_core::pipeline::prepare(&theory, Default::default()).unwrap();
    let conclusions = spindle_core::reason::reason_prepared(&result.theory).unwrap();
    let has_result = conclusions.iter().any(|c| {
        c.conclusion_type == spindle_core::conclusion::ConclusionType::DefeasiblyProvable
            && c.literal.name() == "result"
    });
    assert!(has_result, "Expected +d result(?pay) in conclusions");
    // Find the result conclusion and check value
    let result_conc = conclusions
        .iter()
        .find(|c| c.literal.name() == "result")
        .unwrap();
    let pay = &result_conc.literal.predicates()[0];
    assert_eq!(pay, "204.00", "Expected 204.00, got {pay}");
}

#[test]
fn floor_in_bind_expression() {
    // (bind ?n (floor 3.7)) → 3
    let spl = r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?n (floor 3.7)))
          (result ?n))
    "#;
    let theory = spindle_parser::parse_spl(spl).unwrap();
    let result = spindle_core::pipeline::prepare(&theory, Default::default()).unwrap();
    let conclusions = spindle_core::reason::reason_prepared(&result.theory).unwrap();
    let result_conc = conclusions
        .iter()
        .find(|c| c.literal.name() == "result")
        .unwrap();
    let n = &result_conc.literal.predicates()[0];
    assert_eq!(n, "3", "Expected 3, got {n}");
}

#[test]
fn ceil_in_bind_expression() {
    // (bind ?n (ceil 3.2)) → 4
    let spl = r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?n (ceil 3.2)))
          (result ?n))
    "#;
    let theory = spindle_parser::parse_spl(spl).unwrap();
    let result = spindle_core::pipeline::prepare(&theory, Default::default()).unwrap();
    let conclusions = spindle_core::reason::reason_prepared(&result.theory).unwrap();
    let result_conc = conclusions
        .iter()
        .find(|c| c.literal.name() == "result")
        .unwrap();
    let n = &result_conc.literal.predicates()[0];
    assert_eq!(n, "4", "Expected 4, got {n}");
}
