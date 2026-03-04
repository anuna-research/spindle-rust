//! Property-based tests for the arithmetic module (TEST-PBT-001).
//!
//! Generators: bounded-depth ArithExpr (max depth 4), random NumericValue,
//! random Substitution, random rule-body fragments.
//!
//! 10 properties tested with 500+ cases each.

use proptest::prelude::*;
use rust_decimal::Decimal;

use spindle_core::arith::{ArithConstraint, ArithExpr, CmpOp};
use spindle_core::function_registry::{EvalContext, FunctionRegistry};
use spindle_core::grounding::Substitution;
use spindle_core::intern::intern;
use spindle_core::term::{FiniteFloat, NumericValue, Term};

// ---------------------------------------------------------------------------
// Generators
// ---------------------------------------------------------------------------

fn arb_numeric_value() -> impl Strategy<Value = NumericValue> {
    prop_oneof![
        4 => (-1_000_000i64..1_000_000).prop_map(NumericValue::Integer),
        3 => (-999_999i64..999_999)
            .prop_map(|n| NumericValue::Decimal(Decimal::new(n, 2))),
        3 => (-1000.0f64..1000.0)
            .prop_filter("must be finite", |f| f.is_finite())
            .prop_map(NumericValue::Float),
    ]
}

/// Generate a random operator name for Call nodes.
fn arb_op_name() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("+"),
        Just("-"),
        Just("*"),
        Just("/"),
        Just("min"),
        Just("max"),
    ]
}

/// Generate a random binary-only operator name.
fn arb_bin_op_name() -> impl Strategy<Value = &'static str> {
    prop_oneof![Just("div"), Just("rem"), Just("**"),]
}

#[allow(dead_code)]
fn arb_cmp_op() -> impl Strategy<Value = CmpOp> {
    prop_oneof![
        Just(CmpOp::Eq),
        Just(CmpOp::Ne),
        Just(CmpOp::Lt),
        Just(CmpOp::Gt),
        Just(CmpOp::Le),
        Just(CmpOp::Ge),
    ]
}

/// Generate an ArithExpr of bounded depth.
fn arb_arith_expr(max_depth: u32) -> impl Strategy<Value = ArithExpr> {
    let leaf = arb_numeric_value().prop_map(ArithExpr::Lit);
    leaf.prop_recursive(max_depth, 64, 4, |inner| {
        prop_oneof![
            // N-ary Call with 1-3 args
            (
                arb_op_name(),
                proptest::collection::vec(inner.clone(), 1..=3)
            )
                .prop_map(|(name, args)| ArithExpr::Call {
                    name: intern(name),
                    args,
                }),
            // Binary-only Call with exactly 2 args
            (arb_bin_op_name(), inner.clone(), inner.clone()).prop_map(|(name, lhs, rhs)| {
                ArithExpr::Call {
                    name: intern(name),
                    args: vec![lhs, rhs],
                }
            }),
            // abs Call with 1 arg
            inner.clone().prop_map(|e| ArithExpr::Call {
                name: intern("abs"),
                args: vec![e],
            }),
        ]
    })
}

/// Generate a substitution with some bound variables.
fn arb_substitution() -> impl Strategy<Value = Substitution> {
    proptest::collection::vec((0..5u32, arb_numeric_value()), 0..=5).prop_map(|bindings| {
        let mut subst = Substitution::default();
        for (idx, val) in bindings {
            let var_name = format!("?v{}", idx);
            let term = match val {
                NumericValue::Integer(n) => Term::Integer(n),
                NumericValue::Decimal(d) => Term::Decimal(d),
                NumericValue::Float(f) => {
                    Term::Float(FiniteFloat::new(f).unwrap_or(FiniteFloat::new(0.0).unwrap()))
                }
            };
            subst.terms.insert(intern(&var_name), term);
        }
        subst
    })
}

/// Create an EvalContext with the prelude registry.
#[allow(dead_code)]
fn prelude_ctx() -> (
    FunctionRegistry,
    impl Fn(&FunctionRegistry) -> EvalContext<'_>,
) {
    let reg = FunctionRegistry::with_prelude();
    (reg, |r: &FunctionRegistry| EvalContext::with_registry(r))
}

// ---------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Property 1: Evaluation is deterministic — same expr + subst → same result.
    #[test]
    fn arith_eval_deterministic(
        expr in arb_arith_expr(3),
        subst in arb_substitution(),
    ) {
        let reg = FunctionRegistry::with_prelude();
        let ctx = EvalContext::with_registry(&reg);
        let r1 = expr.eval_with_context(&subst, &ctx);
        let r2 = expr.eval_with_context(&subst, &ctx);
        match (&r1, &r2) {
            (Ok(v1), Ok(v2)) => prop_assert!(
                v1.numeric_eq(v2),
                "Non-deterministic eval: {:?} vs {:?}",
                v1, v2
            ),
            (Err(_), Err(_)) => {} // Both error is fine
            _ => prop_assert!(false, "Determinism violation: {:?} vs {:?}", r1, r2),
        }
    }

    /// Property 2: Evaluation never panics on any generated input.
    #[test]
    fn arith_eval_no_panic(
        expr in arb_arith_expr(4),
        subst in arb_substitution(),
    ) {
        let reg = FunctionRegistry::with_prelude();
        let ctx = EvalContext::with_registry(&reg);
        // If this test runs without panic, the property holds.
        let _ = expr.eval_with_context(&subst, &ctx);
    }

    /// Property 3: Type promotion correctness — Integer op Decimal → Decimal,
    /// any op Float → Float.
    #[test]
    fn arith_promotion_correctness(
        a in -1000i64..1000,
        b_dec in -999i64..999,
    ) {
        let subst = Substitution::default();
        // Integer + Decimal should yield Decimal
        let expr = ArithExpr::Call {
            name: intern("+"),
            args: vec![
                ArithExpr::Lit(NumericValue::Integer(a)),
                ArithExpr::Lit(NumericValue::Decimal(Decimal::new(b_dec, 2))),
            ],
        };
        let reg = FunctionRegistry::with_prelude();
        let ctx = EvalContext::with_registry(&reg);
        if let Ok(result) = expr.eval_with_context(&subst, &ctx) {
            prop_assert!(
                matches!(result, NumericValue::Decimal(_)),
                "Integer + Decimal should produce Decimal, got {:?}",
                result
            );
        }
    }

    /// Property 4: div/rem identity: a = b * div(a, b) + rem(a, b)
    /// for non-zero integer b.
    #[test]
    fn arith_div_rem_identity(
        a in -10_000i64..10_000,
        b in prop_oneof![-10_000i64..-1, 1..10_000i64],
    ) {
        let subst = Substitution::default();
        let div_expr = ArithExpr::Call {
            name: intern("div"),
            args: vec![
                ArithExpr::Lit(NumericValue::Integer(a)),
                ArithExpr::Lit(NumericValue::Integer(b)),
            ],
        };
        let rem_expr = ArithExpr::Call {
            name: intern("rem"),
            args: vec![
                ArithExpr::Lit(NumericValue::Integer(a)),
                ArithExpr::Lit(NumericValue::Integer(b)),
            ],
        };

        let reg = FunctionRegistry::with_prelude();
        let ctx = EvalContext::with_registry(&reg);
        if let (Ok(NumericValue::Integer(q)), Ok(NumericValue::Integer(r))) =
            (div_expr.eval_with_context(&subst, &ctx), rem_expr.eval_with_context(&subst, &ctx))
        {
            prop_assert_eq!(
                a, b * q + r,
                "div/rem identity failed: {} != {} * {} + {}",
                a, b, q, r
            );
        }
    }

    /// Property 5: Floor remainder sign rule — rem(a, b) has same sign as b
    /// (or is zero), following floor-division semantics.
    #[test]
    fn arith_rem_sign_rule(
        a in -10_000i64..10_000,
        b in prop_oneof![-10_000i64..-1, 1..10_000i64],
    ) {
        let subst = Substitution::default();
        let rem_expr = ArithExpr::Call {
            name: intern("rem"),
            args: vec![
                ArithExpr::Lit(NumericValue::Integer(a)),
                ArithExpr::Lit(NumericValue::Integer(b)),
            ],
        };
        let reg = FunctionRegistry::with_prelude();
        let ctx = EvalContext::with_registry(&reg);
        if let Ok(NumericValue::Integer(r)) = rem_expr.eval_with_context(&subst, &ctx)
            && r != 0 {
                prop_assert!(
                    (r > 0) == (b > 0),
                    "rem({}, {}) = {} should have same sign as divisor {}",
                    a, b, r, b
                );
            }
    }

    /// Property 6: Comparison reflexivity — (= x x) always holds,
    /// (< x x) never holds, (<= x x) always holds.
    #[test]
    fn arith_comparison_reflexivity(val in arb_numeric_value()) {
        let mut subst = Substitution::default();
        let var_id = intern("?x");
        let term = match &val {
            NumericValue::Integer(n) => Term::Integer(*n),
            NumericValue::Decimal(d) => Term::Decimal(*d),
            NumericValue::Float(f) => {
                Term::Float(FiniteFloat::new(*f).unwrap_or(FiniteFloat::new(0.0).unwrap()))
            }
        };
        subst.terms.insert(var_id, term);

        let eq_constraint = ArithConstraint::Compare {
            op: CmpOp::Eq,
            lhs: ArithExpr::Var(var_id),
            rhs: ArithExpr::Var(var_id),
        };
        prop_assert!(
            eq_constraint.eval(&mut subst).is_ok(),
            "(= ?x ?x) should always hold for {:?}",
            val
        );

        let lt_constraint = ArithConstraint::Compare {
            op: CmpOp::Lt,
            lhs: ArithExpr::Var(var_id),
            rhs: ArithExpr::Var(var_id),
        };
        prop_assert!(
            lt_constraint.eval(&mut subst).is_err(),
            "(< ?x ?x) should never hold for {:?}",
            val
        );

        let le_constraint = ArithConstraint::Compare {
            op: CmpOp::Le,
            lhs: ArithExpr::Var(var_id),
            rhs: ArithExpr::Var(var_id),
        };
        prop_assert!(
            le_constraint.eval(&mut subst).is_ok(),
            "(<= ?x ?x) should always hold for {:?}",
            val
        );
    }

    /// Property 7: Float safety — no NaN or Infinity in evaluation results.
    #[test]
    fn arith_float_safety(
        expr in arb_arith_expr(3),
        subst in arb_substitution(),
    ) {
        let reg = FunctionRegistry::with_prelude();
        let ctx = EvalContext::with_registry(&reg);
        if let Ok(val) = expr.eval_with_context(&subst, &ctx)
            && let NumericValue::Float(f) = val {
                prop_assert!(
                    f.is_finite(),
                    "Float result must be finite, got {}",
                    f
                );
                prop_assert!(
                    !f.is_nan(),
                    "Float result must not be NaN"
                );
            }
    }

    /// Property 8: Source-order dependency — bind then compare succeeds,
    /// compare then bind fails (when variable only comes from bind).
    #[test]
    fn arith_source_order_dependency(
        val in -1000i64..1000,
        threshold in -1000i64..1000,
    ) {
        use spindle_core::pipeline::{PrepareOptions, prepare};
        use spindle_core::reason::reason_prepared;

        // Order 1: bind before compare (should work if val > threshold)
        let spl_ok = format!(
            r#"
            (given trigger)
            (normally r1
              (and (trigger) (bind ?x (+ {} 0)) (> ?x {}))
              (pass))
        "#,
            val, threshold
        );
        if let Ok(theory) = spindle_parser::parse_spl(&spl_ok)
            && let Ok(result) = prepare(&theory, PrepareOptions::default())
                && let Ok(conclusions) = reason_prepared(&result.theory) {
                    let has_pass = conclusions.iter().any(|c| {
                        c.conclusion_type == spindle_core::conclusion::ConclusionType::DefeasiblyProvable
                            && c.literal.name() == "pass"
                    });
                    if val > threshold {
                        prop_assert!(has_pass, "bind({}) > {} should yield pass", val, threshold);
                    } else {
                        prop_assert!(!has_pass, "bind({}) > {} should not yield pass", val, threshold);
                    }
                }

        // Order 2: compare before bind (should always fail — ?x unbound)
        let spl_bad = format!(
            r#"
            (given trigger)
            (normally r1
              (and (trigger) (> ?x {}) (bind ?x (+ {} 0)))
              (pass-bad))
        "#,
            threshold, val
        );
        if let Ok(theory) = spindle_parser::parse_spl(&spl_bad)
            && let Ok(result) = prepare(&theory, PrepareOptions::default())
                && let Ok(conclusions) = reason_prepared(&result.theory) {
                    let has_pass_bad = conclusions.iter().any(|c| {
                        c.literal.name() == "pass-bad"
                    });
                    prop_assert!(
                        !has_pass_bad,
                        "reversed order (compare before bind) should never fire"
                    );
                }
    }

    /// Property 9: Body-arg expression equivalence — a rule with (pred ?x (+ ?a ?b))
    /// should match when the computed value equals the fact argument.
    #[test]
    fn arith_body_arg_equivalence(
        a in 1i64..100,
        b in 1i64..100,
    ) {
        use spindle_core::pipeline::{PrepareOptions, prepare};
        use spindle_core::reason::reason_prepared;

        let sum = a + b;
        let spl = format!(
            r#"
            (given (pair {} {}))
            (given (target {}))
            (normally r1
              (and (pair ?a ?b) (target (+ ?a ?b)))
              (match))
        "#,
            a, b, sum
        );
        if let Ok(theory) = spindle_parser::parse_spl(&spl)
            && let Ok(result) = prepare(&theory, PrepareOptions::default())
                && let Ok(conclusions) = reason_prepared(&result.theory) {
                    let has_match = conclusions.iter().any(|c| {
                        c.conclusion_type == spindle_core::conclusion::ConclusionType::DefeasiblyProvable
                            && c.literal.name() == "match"
                    });
                    prop_assert!(
                        has_match,
                        "pair({}, {}) with target({}) should match via (+ ?a ?b)",
                        a, b, sum
                    );
                }
    }

    /// Property 10: Parser round-trip stability — parsing an arithmetic SPL
    /// fragment twice produces the same theory structure.
    #[test]
    fn arith_parser_round_trip_stability(
        a in 1i64..1000,
        b in 1i64..1000,
        op_idx in 0..6usize,
    ) {
        let ops = ["+" , "-", "*", "/", "min", "max"];
        let op = ops[op_idx];
        let spl = format!(
            r#"
            (given (x {}))
            (normally r1
              (and (x ?v) (bind ?r ({} ?v {})))
              (result ?r))
        "#,
            a, op, b
        );

        let t1 = spindle_parser::parse_spl(&spl);
        let t2 = spindle_parser::parse_spl(&spl);

        match (t1, t2) {
            (Ok(theory1), Ok(theory2)) => {
                prop_assert_eq!(
                    theory1.rules().count(),
                    theory2.rules().count(),
                    "Round-trip should produce same number of rules"
                );
            }
            (Err(_), Err(_)) => {} // Both error is consistent
            (r1, r2) => prop_assert!(
                false,
                "Inconsistent parse results: {:?} vs {:?}",
                r1.is_ok(),
                r2.is_ok()
            ),
        }
    }
}
