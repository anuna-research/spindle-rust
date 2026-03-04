//! Integration tests for Phase 1 extension function mechanism.
//!
//! Tests cover:
//! - Basic call: parse, register, ground, reason
//! - Composition: Call nested in arithmetic
//! - Multi-arg function
//! - Validation: unknown function, arity mismatch, no registry
//! - Eval failure: function returns error → substitution path discarded
//! - No registry, no calls: programs without Call nodes work as before

use rust_decimal::Decimal;
use spindle_core::conclusion::ConclusionType;
use spindle_core::function_registry::{
    Arity, EvalContext, EvalError, ExtensionFunction, FunctionRegistry, FunctionSignature,
};
use spindle_core::grounding::ground_theory_with_limit;
use spindle_core::intern::intern;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::reason::reason_with_options;
use spindle_core::term::{FiniteFloat, NumericValue, Term};
use spindle_parser::parse_spl;

// ---------------------------------------------------------------------------
// Test extension functions
// ---------------------------------------------------------------------------

/// Doubles an integer.
struct DoubleFunction(FunctionSignature);

impl DoubleFunction {
    fn new() -> Self {
        Self(FunctionSignature {
            name: intern("double"),
            arity: Arity::Fixed(1),
            description: "doubles an integer",
        })
    }
}

impl ExtensionFunction for DoubleFunction {
    fn signature(&self) -> &FunctionSignature {
        &self.0
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        match &args[0] {
            Term::Integer(n) => Ok(Term::Integer(n * 2)),
            other => Err(EvalError::TypeError(format!(
                "double: expected integer, got {other:?}"
            ))),
        }
    }
}

/// Adds three integers.
struct Add3Function(FunctionSignature);

impl Add3Function {
    fn new() -> Self {
        Self(FunctionSignature {
            name: intern("add3"),
            arity: Arity::Fixed(3),
            description: "adds three integers",
        })
    }
}

impl ExtensionFunction for Add3Function {
    fn signature(&self) -> &FunctionSignature {
        &self.0
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        let a = match &args[0] {
            Term::Integer(n) => *n,
            other => {
                return Err(EvalError::TypeError(format!(
                    "add3: expected integer, got {other:?}"
                )));
            }
        };
        let b = match &args[1] {
            Term::Integer(n) => *n,
            other => {
                return Err(EvalError::TypeError(format!(
                    "add3: expected integer, got {other:?}"
                )));
            }
        };
        let c = match &args[2] {
            Term::Integer(n) => *n,
            other => {
                return Err(EvalError::TypeError(format!(
                    "add3: expected integer, got {other:?}"
                )));
            }
        };
        Ok(Term::Integer(a + b + c))
    }
}

/// Always fails.
struct FailFunction(FunctionSignature);

impl FailFunction {
    fn new() -> Self {
        Self(FunctionSignature {
            name: intern("fail_fn"),
            arity: Arity::Fixed(1),
            description: "always fails",
        })
    }
}

impl ExtensionFunction for FailFunction {
    fn signature(&self) -> &FunctionSignature {
        &self.0
    }

    fn eval(&self, _args: &[Term]) -> Result<Term, EvalError> {
        Err(EvalError::EvalFailed("intentional failure".into()))
    }
}

/// Sums 1–3 integer arguments (range arity).
struct SumUpTo3Function(FunctionSignature);

impl SumUpTo3Function {
    fn new() -> Self {
        Self(FunctionSignature {
            name: intern("sum_up_to_3"),
            arity: Arity::Range(1, 3),
            description: "sums 1 to 3 integers",
        })
    }
}

impl ExtensionFunction for SumUpTo3Function {
    fn signature(&self) -> &FunctionSignature {
        &self.0
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        let mut total = 0i64;
        for arg in args {
            match arg {
                Term::Integer(n) => total += n,
                other => {
                    return Err(EvalError::TypeError(format!(
                        "sum_up_to_3: expected integer, got {other:?}"
                    )));
                }
            }
        }
        Ok(Term::Integer(total))
    }
}

/// Returns a symbol (non-numeric Term).
struct SymbolFunction(FunctionSignature);

impl SymbolFunction {
    fn new() -> Self {
        Self(FunctionSignature {
            name: intern("to_symbol"),
            arity: Arity::Fixed(1),
            description: "returns a symbol",
        })
    }
}

impl ExtensionFunction for SymbolFunction {
    fn signature(&self) -> &FunctionSignature {
        &self.0
    }

    fn eval(&self, _args: &[Term]) -> Result<Term, EvalError> {
        Ok(Term::Symbol(intern("hello")))
    }
}

/// Zero-arg constant function returning 42.
struct TheAnswerFunction(FunctionSignature);

impl TheAnswerFunction {
    fn new() -> Self {
        Self(FunctionSignature {
            name: intern("the-answer"),
            arity: Arity::Fixed(0),
            description: "returns 42",
        })
    }
}

impl ExtensionFunction for TheAnswerFunction {
    fn signature(&self) -> &FunctionSignature {
        &self.0
    }

    fn eval(&self, _args: &[Term]) -> Result<Term, EvalError> {
        Ok(Term::Integer(42))
    }
}

fn make_registry_with_double() -> FunctionRegistry {
    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(DoubleFunction::new()));
    reg
}

// ---------------------------------------------------------------------------
// Basic call: parse, ground, check output
// ---------------------------------------------------------------------------

#[test]
fn basic_call_double() {
    let spl = r#"
        (given (val 5))
        (normally r1
          (and (val ?x) (bind ?y (double ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse should succeed");

    let opts = PrepareOptions {
        function_registry: Some(make_registry_with_double()),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare should succeed");

    // After grounding, we should have (result 10) since double(5) = 10
    let has_result_10 = result.theory.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "result" && h.predicate_args() == [Term::Integer(10)])
    });
    assert!(has_result_10, "expected (result 10) in grounded theory");
}

// ---------------------------------------------------------------------------
// Composition: Call nested in arithmetic
// ---------------------------------------------------------------------------

#[test]
fn call_nested_in_arithmetic() {
    // (bind ?z (+ 1 (double ?x))) should produce 1 + 2*x
    let spl = r#"
        (given (val 3))
        (normally r1
          (and (val ?x) (bind ?z (+ 1 (double ?x))))
          (result ?z))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let opts = PrepareOptions {
        function_registry: Some(make_registry_with_double()),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    // double(3) = 6, 1 + 6 = 7
    let has_result_7 = result.theory.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "result" && h.predicate_args() == [Term::Integer(7)])
    });
    assert!(has_result_7, "expected (result 7) in grounded theory");
}

// ---------------------------------------------------------------------------
// Multi-arg function
// ---------------------------------------------------------------------------

#[test]
fn multi_arg_function() {
    let spl = r#"
        (given (vals 1 2 3))
        (normally r1
          (and (vals ?a ?b ?c) (bind ?sum (add3 ?a ?b ?c)))
          (total ?sum))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(Add3Function::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    // add3(1, 2, 3) = 6
    let has_total_6 = result.theory.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "total" && h.predicate_args() == [Term::Integer(6)])
    });
    assert!(has_total_6, "expected (total 6) in grounded theory");
}

// ---------------------------------------------------------------------------
// Validation: unknown function (no registry)
// ---------------------------------------------------------------------------

#[test]
fn validation_error_unknown_function_no_user_registry() {
    let spl = r#"
        (given (val 5))
        (normally r1
          (and (val ?x) (bind ?y (double ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    // No user registry provided (prelude is auto-injected but lacks 'double')
    let opts = PrepareOptions::default();
    let result = prepare(&theory, opts);

    match result {
        Ok(_) => panic!("should fail validation for unknown function"),
        Err(err) => {
            let msg = format!("{err}");
            assert!(
                msg.contains("Unknown extension function") && msg.contains("double"),
                "error should mention unknown function 'double', got: {msg}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Validation: unknown function (registry without the function)
// ---------------------------------------------------------------------------

#[test]
fn validation_error_unknown_function() {
    let spl = r#"
        (given (val 5))
        (normally r1
          (and (val ?x) (bind ?y (unknown_fn ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    // Registry exists but doesn't have 'unknown_fn'
    let opts = PrepareOptions {
        function_registry: Some(FunctionRegistry::new()),
        ..Default::default()
    };
    let result = prepare(&theory, opts);

    match result {
        Ok(_) => panic!("should fail validation for unknown function"),
        Err(err) => {
            let msg = format!("{err}");
            assert!(
                msg.contains("Unknown extension function 'unknown_fn'"),
                "got: {msg}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Validation: arity mismatch
// ---------------------------------------------------------------------------

#[test]
fn validation_error_arity_mismatch() {
    // double expects 1 arg, but we pass 2
    let spl = r#"
        (given (val 5))
        (normally r1
          (and (val ?x) (bind ?y (double ?x ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let opts = PrepareOptions {
        function_registry: Some(make_registry_with_double()),
        ..Default::default()
    };
    let result = prepare(&theory, opts);

    match result {
        Ok(_) => panic!("should fail for arity mismatch"),
        Err(err) => {
            let msg = format!("{err}");
            assert!(msg.contains("expects 1 argument(s), got 2"), "got: {msg}");
        }
    }
}

// ---------------------------------------------------------------------------
// Eval failure: function returns error → path discarded
// ---------------------------------------------------------------------------

#[test]
fn eval_failure_discards_path() {
    let spl = r#"
        (given (val 5))
        (normally r1
          (and (val ?x) (bind ?y (fail_fn ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(FailFunction::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result =
        prepare(&theory, opts).expect("prepare should succeed (error silently discards path)");

    // No (result ...) should be generated since the function always fails
    let has_result = result
        .theory
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(!has_result, "should not produce any result rules");
}

// ---------------------------------------------------------------------------
// No registry, no calls: existing programs work exactly as before
// ---------------------------------------------------------------------------

#[test]
fn no_calls_no_registry_works() {
    let spl = r#"
        (given bird)
        (given penguin)
        (normally r1 bird flies)
        (normally r2 penguin ~flies)
        (prefer r2 r1)
    "#;
    let theory = parse_spl(spl).expect("parse");

    let conclusions = reason_with_options(&theory, PrepareOptions::default());
    assert!(
        conclusions.is_ok(),
        "reasoning should succeed without registry"
    );
}

// ---------------------------------------------------------------------------
// Direct grounding API with EvalContext
// ---------------------------------------------------------------------------

#[test]
fn ground_theory_with_eval_context() {
    let spl = r#"
        (given (val 4))
        (normally r1
          (and (val ?x) (bind ?y (double ?x)))
          (doubled ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let reg = make_registry_with_double();
    let ctx = EvalContext::with_registry(&reg);
    let (grounded, _limit_hit) = ground_theory_with_limit(&theory, 100, 10000, &ctx);

    let has_doubled_8 = grounded.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "doubled" && h.predicate_args() == [Term::Integer(8)])
    });
    assert!(has_doubled_8, "expected (doubled 8) in grounded theory");
}

// ---------------------------------------------------------------------------
// Extension function call in comparison position
// ---------------------------------------------------------------------------

#[test]
fn call_in_comparison() {
    let spl = r#"
        (given (val 3))
        (normally r1
          (and (val ?x) (> (double ?x) 5))
          (big ?x))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let opts = PrepareOptions {
        function_registry: Some(make_registry_with_double()),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    // double(3) = 6 > 5, so (big ...) should be derived
    let has_big = result
        .theory
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "big"));
    assert!(has_big, "expected (big ...) in grounded theory");
}

// ---------------------------------------------------------------------------
// 3a. Range arity integration test
// ---------------------------------------------------------------------------

#[test]
fn range_arity_1_arg() {
    let spl = r#"
        (given (val 5))
        (normally r1
          (and (val ?x) (bind ?y (sum_up_to_3 ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(SumUpTo3Function::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    let has_result_5 = result.theory.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "result" && h.predicate_args() == [Term::Integer(5)])
    });
    assert!(has_result_5, "expected (result 5) with 1 arg");
}

#[test]
fn range_arity_2_args() {
    let spl = r#"
        (given (vals 2 3))
        (normally r1
          (and (vals ?a ?b) (bind ?y (sum_up_to_3 ?a ?b)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(SumUpTo3Function::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    let has_result_5 = result.theory.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "result" && h.predicate_args() == [Term::Integer(5)])
    });
    assert!(has_result_5, "expected (result 5) with 2 args");
}

#[test]
fn range_arity_3_args() {
    let spl = r#"
        (given (vals 1 2 3))
        (normally r1
          (and (vals ?a ?b ?c) (bind ?y (sum_up_to_3 ?a ?b ?c)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(SumUpTo3Function::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    let has_result_6 = result.theory.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "result" && h.predicate_args() == [Term::Integer(6)])
    });
    assert!(has_result_6, "expected (result 6) with 3 args");
}

#[test]
fn range_arity_too_many_args_fails_validation() {
    // sum_up_to_3 accepts 1..3 args, so 4 should fail
    let spl = r#"
        (given (vals 1 2 3 4))
        (normally r1
          (and (vals ?a ?b ?c ?d) (bind ?y (sum_up_to_3 ?a ?b ?c ?d)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(SumUpTo3Function::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts);

    match result {
        Ok(_) => panic!("should fail for arity mismatch with range"),
        Err(err) => {
            let msg = format!("{err}");
            assert!(
                msg.contains("expects 1..3 argument(s), got 4"),
                "got: {msg}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3b. Non-numeric return from extension function
// ---------------------------------------------------------------------------

#[test]
fn non_numeric_return_discards_path() {
    let spl = r#"
        (given (val 5))
        (normally r1
          (and (val ?x) (bind ?y (to_symbol ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(SymbolFunction::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts)
        .expect("prepare should succeed (non-numeric silently discards path)");

    let has_result = result
        .theory
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "result"));
    assert!(!has_result, "should not produce any result rules");
}

// ---------------------------------------------------------------------------
// 3c. Nested Call in Call
// ---------------------------------------------------------------------------

#[test]
fn nested_call_in_call() {
    // double(double(3)) = double(6) = 12
    let spl = r#"
        (given (val 3))
        (normally r1
          (and (val ?x) (bind ?z (double (double ?x))))
          (result ?z))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let opts = PrepareOptions {
        function_registry: Some(make_registry_with_double()),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    let has_result_12 = result.theory.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "result" && h.predicate_args() == [Term::Integer(12)])
    });
    assert!(has_result_12, "expected (result 12) from double(double(3))");
}

// ---------------------------------------------------------------------------
// 3d. Call inside predicate argument (BodyLiteral::Logic path)
// ---------------------------------------------------------------------------

#[test]
fn call_in_body_predicate_argument() {
    // An extension function call nested inside a built-in arithmetic
    // expression in a body predicate argument position. This exercises
    // the validate_expr_calls path through BodyLiteral::Logic → BodyArg::Arith,
    // and the grounding path that evaluates BodyArg::Arith in body logic literals.
    // (The `+` wrapper is needed because parse_body_arg only recognises
    //  built-in operators at the top level of a predicate argument.)
    let spl = r#"
        (given (val 3))
        (given (target 6))
        (normally r1
          (and (val ?x) (target (+ 0 (double ?x))))
          (matched ?x))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let opts = PrepareOptions {
        function_registry: Some(make_registry_with_double()),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    // double(3) = 6, 0 + 6 = 6, matches (target 6), so (matched 3) should be derived
    let has_matched = result.theory.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "matched" && h.predicate_args() == [Term::Integer(3)])
    });
    assert!(
        has_matched,
        "expected (matched 3) from call in body predicate arg"
    );
}

// ---------------------------------------------------------------------------
// 3e. Zero-arg extension function
// ---------------------------------------------------------------------------

#[test]
fn zero_arg_function() {
    let spl = r#"
        (given start)
        (normally r1
          (and start (bind ?x (the-answer)))
          (result ?x))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(TheAnswerFunction::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    let has_result_42 = result.theory.rules().any(|r| {
        r.head
            .iter()
            .any(|h| h.name() == "result" && h.predicate_args() == [Term::Integer(42)])
    });
    assert!(has_result_42, "expected (result 42) from zero-arg function");
}

// ---------------------------------------------------------------------------
// End-to-end reasoning with extension functions
// ---------------------------------------------------------------------------

#[test]
fn end_to_end_reasoning_with_extension_function() {
    // Full pipeline: parse → prepare (validate + ground) → reason → check conclusions
    let spl = r#"
        (given (val 5))
        (normally r1
          (and (val ?x) (bind ?y (double ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let opts = PrepareOptions {
        function_registry: Some(make_registry_with_double()),
        ..Default::default()
    };
    let conclusions = reason_with_options(&theory, opts).expect("reasoning should succeed");

    // Should derive +d (result 10)
    let has_result_10 = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "result"
            && c.literal.predicate_args() == [Term::Integer(10)]
    });
    assert!(
        has_result_10,
        "expected +d (result 10) in conclusions, got: {:?}",
        conclusions
            .iter()
            .filter(|c| c.literal.name() == "result")
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------------
// Extension function returning Decimal
// ---------------------------------------------------------------------------

/// Halves a numeric value, returning a Decimal.
struct HalfFunction(FunctionSignature);

impl HalfFunction {
    fn new() -> Self {
        Self(FunctionSignature {
            name: intern("half"),
            arity: Arity::Fixed(1),
            description: "halves a numeric value (returns Decimal)",
        })
    }
}

impl ExtensionFunction for HalfFunction {
    fn signature(&self) -> &FunctionSignature {
        &self.0
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        let val: Decimal = match &args[0] {
            Term::Integer(n) => Decimal::from(*n),
            Term::Decimal(d) => *d,
            other => {
                return Err(EvalError::TypeError(format!(
                    "half: expected numeric, got {other:?}"
                )));
            }
        };
        let two = Decimal::from(2);
        val.checked_div(two)
            .map(Term::Decimal)
            .ok_or_else(|| EvalError::EvalFailed("decimal overflow in half".into()))
    }
}

#[test]
fn decimal_return_from_extension_function() {
    let spl = r#"
        (given (val 7))
        (normally r1
          (and (val ?x) (bind ?y (half ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(HalfFunction::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    // half(7) = 3.5
    let expected = Decimal::from(7).checked_div(Decimal::from(2)).unwrap();
    let has_result = result.theory.rules().any(|r| {
        r.head.iter().any(|h| {
            h.name() == "result"
                && h.predicate_args() == [Term::try_from(NumericValue::Decimal(expected)).unwrap()]
        })
    });
    assert!(has_result, "expected (result 3.5) from half(7)");
}

// ---------------------------------------------------------------------------
// Extension function returning Float
// ---------------------------------------------------------------------------

/// Square root via f64.
struct SqrtFunction(FunctionSignature);

impl SqrtFunction {
    fn new() -> Self {
        Self(FunctionSignature {
            name: intern("sqrt"),
            arity: Arity::Fixed(1),
            description: "square root (returns Float)",
        })
    }
}

impl ExtensionFunction for SqrtFunction {
    fn signature(&self) -> &FunctionSignature {
        &self.0
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        let val: f64 = match &args[0] {
            Term::Integer(n) => *n as f64,
            Term::Float(f) => f.value(),
            Term::Decimal(d) => {
                use rust_decimal::prelude::ToPrimitive;
                d.to_f64().unwrap_or(f64::NAN)
            }
            other => {
                return Err(EvalError::TypeError(format!(
                    "sqrt: expected numeric, got {other:?}"
                )));
            }
        };
        let result = val.sqrt();
        match FiniteFloat::new(result) {
            Some(ff) => Ok(Term::Float(ff)),
            None => Err(EvalError::EvalFailed(
                "sqrt produced non-finite result".into(),
            )),
        }
    }
}

#[test]
fn float_return_from_extension_function() {
    // sqrt(4) = 2.0
    let spl = r#"
        (given (val 4))
        (normally r1
          (and (val ?x) (bind ?y (sqrt ?x)))
          (result ?y))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(SqrtFunction::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    let has_result = result.theory.rules().any(|r| {
        r.head.iter().any(|h| {
            h.name() == "result"
                && h.predicate_args() == [Term::Float(FiniteFloat::new(2.0).unwrap())]
        })
    });
    assert!(has_result, "expected (result 2.0) from sqrt(4)");
}

// ---------------------------------------------------------------------------
// Multiple extension functions in one theory
// ---------------------------------------------------------------------------

#[test]
fn multiple_extension_functions_in_one_theory() {
    let spl = r#"
        (given (val 5))
        (normally r1
          (and (val ?x) (bind ?y (double ?x)) (bind ?z (half ?y)))
          (result ?z))
    "#;
    let theory = parse_spl(spl).expect("parse");

    let mut reg = FunctionRegistry::new();
    reg.register(Box::new(DoubleFunction::new()));
    reg.register(Box::new(HalfFunction::new()));

    let opts = PrepareOptions {
        function_registry: Some(reg),
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare");

    // double(5) = 10, half(10) = 5.0 (Decimal)
    let expected = Decimal::from(10).checked_div(Decimal::from(2)).unwrap();
    let has_result = result.theory.rules().any(|r| {
        r.head.iter().any(|h| {
            h.name() == "result"
                && h.predicate_args() == [Term::try_from(NumericValue::Decimal(expected)).unwrap()]
        })
    });
    assert!(has_result, "expected (result 5.0) from half(double(5))");
}
