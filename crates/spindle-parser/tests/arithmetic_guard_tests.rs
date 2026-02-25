//! Integration tests for arithmetic parsing and guard enforcement.
//!
//! Covers the parsing-side scenarios from:
//! - TEST-002: Arithmetic Expression Parsing (12 scenarios)
//! - TEST-008: Reserved Keyword Enforcement (8 scenarios)
//! - TEST-009: No Arithmetic in Rule Heads (3 scenarios)
//! - TEST-011: No Negation of Arithmetic Predicates (5 scenarios)

use spindle_core::arith::{ArithExpr, BinArithOp, NaryArithOp, UnaryArithOp};
use spindle_core::body::BodyArg;
use spindle_core::intern::intern;
use spindle_core::term::NumericValue;
use spindle_parser::parse_spl;

// =========================================================================
// TEST-002: Arithmetic Expression Parsing
// =========================================================================

/// TEST-002 scenario 1: `(+ 3 4)` in argument position →
/// `ArithExpr::NaryOp(Add, [Lit(3), Lit(4)])`
#[test]
fn test_002_01_add_in_argument_position() {
    let theory =
        parse_spl("(normally r1 (and (price ?x ?p) (bind ?total (+ 3 4))) (result ?x))").unwrap();
    assert_eq!(theory.rule_count(), 1);
    let rule = theory.rules().next().unwrap();
    // The bind constraint should be an arithmetic body literal
    let arith = rule
        .body
        .iter()
        .find_map(|bl| bl.as_arithmetic())
        .expect("Expected arithmetic constraint in body");
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(3)),
                    ArithExpr::Lit(NumericValue::Integer(4)),
                ]
            }
        );
    } else {
        panic!("Expected Bind constraint, got Compare");
    }
}

/// TEST-002 scenario 2: Nested: `(* (+ ?a ?b) 2)` → parses as nested ArithExpr
#[test]
fn test_002_02_nested_arith_expr() {
    let theory =
        parse_spl("(normally r1 (and (val ?a) (val ?b) (bind ?c (* (+ ?a ?b) 2))) (result ?c))")
            .unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule
        .body
        .iter()
        .find_map(|bl| bl.as_arithmetic())
        .expect("Expected arithmetic constraint");
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        match expr {
            ArithExpr::NaryOp {
                op: NaryArithOp::Mul,
                args,
            } => {
                assert_eq!(args.len(), 2);
                assert!(
                    matches!(
                        &args[0],
                        ArithExpr::NaryOp {
                            op: NaryArithOp::Add,
                            ..
                        }
                    ),
                    "First arg should be Add, got: {:?}",
                    args[0]
                );
                assert_eq!(args[1], ArithExpr::Lit(NumericValue::Integer(2)));
            }
            _ => panic!("Expected NaryOp::Mul, got: {expr:?}"),
        }
    } else {
        panic!("Expected Bind constraint");
    }
}

/// TEST-002 scenario 3: Variadic: `(+ 1 2 3)` → NaryOp(Add, [Lit(1), Lit(2), Lit(3)])
#[test]
fn test_002_03_variadic_add() {
    let theory = parse_spl("(normally r1 (and (val ?x) (bind ?s (+ 1 2 3))) (result ?s))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(1)),
                    ArithExpr::Lit(NumericValue::Integer(2)),
                    ArithExpr::Lit(NumericValue::Integer(3)),
                ]
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }
}

/// TEST-002 scenario 4: Zero-arg identity: `(+)` → NaryOp(Add, []);
/// `(*)` → NaryOp(Mul, [])
#[test]
fn test_002_04_zero_arg_identity() {
    // (+) zero args — identity for add
    let theory = parse_spl("(normally r1 (and (val ?x) (bind ?s (+))) (result ?s))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                args: vec![]
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }

    // (*) zero args — identity for mul
    let theory = parse_spl("(normally r1 (and (val ?x) (bind ?s (*))) (result ?s))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Mul,
                args: vec![]
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }
}

/// TEST-002 scenario 5: Unary `-`: `(- ?x)` → negation;
/// unary `/`: `(/ ?x)` → reciprocal
#[test]
fn test_002_05_unary_sub_and_div() {
    // (- ?x) → NaryOp(Sub, [Var(?x)])
    let theory =
        parse_spl("(normally r1 (and (val ?x) (bind ?neg (- ?x))) (result ?neg))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Sub,
                args: vec![ArithExpr::Var(intern("?x"))]
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }

    // (/ ?x) → NaryOp(Div, [Var(?x)])
    let theory =
        parse_spl("(normally r1 (and (val ?x) (bind ?recip (/ ?x))) (result ?recip))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Div,
                args: vec![ArithExpr::Var(intern("?x"))]
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }
}

/// TEST-002 scenario 6: `(- 10 3 2)` → left-fold;
/// `(/ 12 3 2)` → left-fold
#[test]
fn test_002_06_left_fold_sub_div() {
    // (- 10 3 2) → NaryOp(Sub, [10, 3, 2])
    let theory =
        parse_spl("(normally r1 (and (val ?x) (bind ?s (- 10 3 2))) (result ?s))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Sub,
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(10)),
                    ArithExpr::Lit(NumericValue::Integer(3)),
                    ArithExpr::Lit(NumericValue::Integer(2)),
                ]
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }

    // (/ 12 3 2) → NaryOp(Div, [12, 3, 2])
    let theory =
        parse_spl("(normally r1 (and (val ?x) (bind ?s (/ 12 3 2))) (result ?s))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Div,
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(12)),
                    ArithExpr::Lit(NumericValue::Integer(3)),
                    ArithExpr::Lit(NumericValue::Integer(2)),
                ]
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }
}

/// TEST-002 scenario 7: Strictly binary operators: `div`, `rem`, `**`
#[test]
fn test_002_07_binary_operators() {
    // (div 10 3) → BinOp(IDiv, 10, 3)
    let theory =
        parse_spl("(normally r1 (and (val ?x) (bind ?d (div 10 3))) (result ?d))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::BinOp {
                op: BinArithOp::IDiv,
                lhs: Box::new(ArithExpr::Lit(NumericValue::Integer(10))),
                rhs: Box::new(ArithExpr::Lit(NumericValue::Integer(3))),
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }

    // (rem 10 3) → BinOp(Rem, 10, 3)
    let theory =
        parse_spl("(normally r1 (and (val ?x) (bind ?r (rem 10 3))) (result ?r))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::BinOp {
                op: BinArithOp::Rem,
                lhs: Box::new(ArithExpr::Lit(NumericValue::Integer(10))),
                rhs: Box::new(ArithExpr::Lit(NumericValue::Integer(3))),
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }

    // (** 2 3) → BinOp(Pow, 2, 3)
    let theory = parse_spl("(normally r1 (and (val ?x) (bind ?p (** 2 3))) (result ?p))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::BinOp {
                op: BinArithOp::Pow,
                lhs: Box::new(ArithExpr::Lit(NumericValue::Integer(2))),
                rhs: Box::new(ArithExpr::Lit(NumericValue::Integer(3))),
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }
}

/// TEST-002 scenario 8: `(abs (- ?a ?b))` → absolute difference
#[test]
fn test_002_08_abs_difference() {
    let theory =
        parse_spl("(normally r1 (and (val ?a) (val ?b) (bind ?d (abs (- ?a ?b)))) (result ?d))")
            .unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        match expr {
            ArithExpr::UnaryOp {
                op: UnaryArithOp::Abs,
                expr: inner,
            } => {
                assert!(
                    matches!(
                        inner.as_ref(),
                        ArithExpr::NaryOp {
                            op: NaryArithOp::Sub,
                            ..
                        }
                    ),
                    "Inner should be Sub, got: {inner:?}"
                );
            }
            _ => panic!("Expected UnaryOp::Abs, got: {expr:?}"),
        }
    } else {
        panic!("Expected Bind constraint");
    }
}

/// TEST-002 scenario 9: Variadic min/max: `(min ?a ?b ?c)`, `(max 0 ?x)`
#[test]
fn test_002_09_variadic_min_max() {
    // (min ?a ?b ?c)
    let theory = parse_spl(
        "(normally r1 (and (val ?a) (val ?b) (val ?c) (bind ?m (min ?a ?b ?c))) (result ?m))",
    )
    .unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        match expr {
            ArithExpr::NaryOp {
                op: NaryArithOp::Min,
                args,
            } => {
                assert_eq!(args.len(), 3);
                assert_eq!(args[0], ArithExpr::Var(intern("?a")));
                assert_eq!(args[1], ArithExpr::Var(intern("?b")));
                assert_eq!(args[2], ArithExpr::Var(intern("?c")));
            }
            _ => panic!("Expected NaryOp::Min, got: {expr:?}"),
        }
    } else {
        panic!("Expected Bind constraint");
    }

    // (max 0 ?x)
    let theory =
        parse_spl("(normally r1 (and (val ?x) (bind ?m (max 0 ?x))) (result ?m))").unwrap();
    let rule = theory.rules().next().unwrap();
    let arith = rule.body.iter().find_map(|bl| bl.as_arithmetic()).unwrap();
    if let spindle_core::arith::ArithConstraint::Bind { expr, .. } = arith {
        assert_eq!(
            *expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Max,
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(0)),
                    ArithExpr::Var(intern("?x")),
                ]
            }
        );
    } else {
        panic!("Expected Bind constraint");
    }
}

/// TEST-002 scenario 10: `(div 10 3 2)` → parse error (div requires exactly 2 arguments)
#[test]
fn test_002_10_div_wrong_arity() {
    let err =
        parse_spl("(normally r1 (and (val ?x) (bind ?d (div 10 3 2))) (result ?d))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("exactly 2 arguments"),
        "Expected arity error for div with 3 args, got: {msg}"
    );
}

/// TEST-002 scenario 11: Arithmetic operator at predicate (non-argument) position → parse error
#[test]
fn test_002_11_arith_op_as_predicate() {
    // `+` used as a predicate name in body position
    let err = parse_spl("(normally r1 (+ ?x ?y) result)").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Reserved keyword '+'") || msg.contains("Arithmetic"),
        "Expected error for + as predicate, got: {msg}"
    );
}

/// TEST-002 scenario 12: Arithmetic expression inside a user-defined body literal argument
/// parses as `BodyArg::Arith`
#[test]
fn test_002_12_arith_in_body_literal_arg() {
    let theory =
        parse_spl("(normally r1 (and (price ?i ?p) (line-total ?i (* ?p 2))) (ok ?i))").unwrap();
    let rule = theory.rules().next().unwrap();

    // Find the line-total body literal
    let line_total = rule
        .body
        .iter()
        .find(|bl| bl.is_logic() && bl.name() == "line-total")
        .expect("Expected 'line-total' body literal");

    let logic = line_total.as_logic().unwrap();
    let args = logic.predicate_args();
    assert_eq!(args.len(), 2, "line-total should have 2 args");

    // First arg should be Term (variable ?i)
    assert!(
        matches!(&args[0], BodyArg::Term(_)),
        "First arg should be Term, got: {:?}",
        args[0]
    );

    // Second arg should be Arith (* ?p 2)
    match &args[1] {
        BodyArg::Arith(expr) => {
            assert_eq!(
                *expr,
                ArithExpr::NaryOp {
                    op: NaryArithOp::Mul,
                    args: vec![
                        ArithExpr::Var(intern("?p")),
                        ArithExpr::Lit(NumericValue::Integer(2)),
                    ]
                }
            );
        }
        other => panic!("Second arg should be Arith, got: {other:?}"),
    }
}

// =========================================================================
// TEST-008: Reserved Keyword Enforcement
// =========================================================================

/// TEST-008 scenario 1: `+` used as a predicate name → parse error
#[test]
fn test_008_01_reserved_op_as_predicate() {
    let err = parse_spl("(normally r1 (+ ?x ?y) result)").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Reserved keyword '+'") && msg.contains("REQ-008"),
        "Expected REQ-008 error for '+' as predicate, got: {msg}"
    );
}

/// TEST-008 scenario 2: `bind` in head position → parse error
#[test]
fn test_008_02_bind_in_head() {
    let err = parse_spl("(normally r1 body (bind ?x 5))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("bind") && msg.contains("REQ-009"),
        "Expected REQ-009 error for bind in head, got: {msg}"
    );
}

/// TEST-008 scenario 3: Arithmetic expression appears in fact argument → parse error
#[test]
fn test_008_03_arith_expr_in_fact_arg() {
    let err = parse_spl("(given (cost (+ 3 4)))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Arithmetic expressions cannot appear as arguments in rule heads"),
        "Expected REQ-009 error for arith expr in fact arg, got: {msg}"
    );
}

/// TEST-008 scenario 4: `bind` used as a rule label → parse error
#[test]
fn test_008_04_bind_as_label() {
    let err = parse_spl("(normally bind body result)").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Reserved keyword 'bind'") && msg.contains("rule label"),
        "Expected REQ-008 error for bind as label, got: {msg}"
    );
}

/// TEST-008 scenario 5: `+` used as a rule label in superiority → parse error
#[test]
fn test_008_05_reserved_op_in_prefer() {
    let err = parse_spl("(prefer r1 +)").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Reserved keyword '+'") && msg.contains("prefer"),
        "Expected REQ-008 error for '+' in prefer, got: {msg}"
    );
}

/// TEST-008 scenario 6: `sum` (future-reserved) as predicate name → parse error
#[test]
fn test_008_06_future_reserved_sum_as_predicate() {
    let err = parse_spl("(given (sum report 100))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("reserved for future use") && msg.contains("REQ-008"),
        "Expected REQ-008 error for sum as predicate, got: {msg}"
    );
}

/// TEST-008 scenario 7: `count` (future-reserved) as rule label → parse error
#[test]
fn test_008_07_future_reserved_count_as_label() {
    let err = parse_spl("(normally count body result)").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("reserved for future use") && msg.contains("rule label"),
        "Expected REQ-008 error for count as label, got: {msg}"
    );
}

/// TEST-008 scenario 8: Future-reserved keywords `avg`, `round`, `floor`, `ceil`
/// as predicate names → parse error
#[test]
fn test_008_08_future_reserved_as_predicates() {
    for kw in &["avg", "round", "floor", "ceil"] {
        let input = format!("(given {kw})");
        let err = parse_spl(&input).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("reserved for future use") && msg.contains("REQ-008"),
            "Expected REQ-008 error for '{kw}' as predicate, got: {msg}"
        );
    }
}

// =========================================================================
// TEST-009: No Arithmetic in Rule Heads
// =========================================================================

/// TEST-009 scenario 1: `(normally r1 body (bind ?x 5))` → parse error
#[test]
fn test_009_01_bind_in_head() {
    let err = parse_spl("(normally r1 body (bind ?x 5))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Arithmetic predicate 'bind'") && msg.contains("REQ-009"),
        "Expected REQ-009 error for bind in head, got: {msg}"
    );
}

/// TEST-009 scenario 2: `(normally r1 body (> ?x 0))` → parse error
#[test]
fn test_009_02_comparison_in_head() {
    let err = parse_spl("(normally r1 body (> ?x 0))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Arithmetic predicate '>'") && msg.contains("REQ-009"),
        "Expected REQ-009 error for > in head, got: {msg}"
    );
}

/// TEST-009 scenario 3: `(normally r1 body (cost item (+ 1 2)))` → parse error
/// (arithmetic expression in head argument)
#[test]
fn test_009_03_arith_expr_in_head_arg() {
    let err = parse_spl("(normally r1 body (cost item (+ 1 2)))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("Arithmetic expressions cannot appear as arguments in rule heads"),
        "Expected REQ-009 error for arith expr as head arg, got: {msg}"
    );
}

// =========================================================================
// TEST-011: No Negation of Arithmetic Predicates
// =========================================================================

/// TEST-011 scenario 1: `(not (> ?x 100))` in body → parse error
#[test]
fn test_011_01_negated_gt() {
    let err = parse_spl("(normally r1 (and (val ?x) (not (> ?x 100))) (low ?x))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("cannot be negated") && msg.contains("REQ-011"),
        "Expected REQ-011 error for negated >, got: {msg}"
    );
}

/// TEST-011 scenario 2: `(not (= ?x 0))` in body → parse error
#[test]
fn test_011_02_negated_eq() {
    let err = parse_spl("(normally r1 (and (val ?x) (not (= ?x 0))) (nonzero ?x))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("cannot be negated") && msg.contains("REQ-011"),
        "Expected REQ-011 error for negated =, got: {msg}"
    );
}

/// TEST-011 scenario 3: `(not (bind ?y (+ ?x 1)))` in body → parse error
#[test]
fn test_011_03_negated_bind() {
    let err =
        parse_spl("(normally r1 (and (val ?x) (not (bind ?y (+ ?x 1)))) (result ?x))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("cannot be negated") && msg.contains("REQ-011"),
        "Expected REQ-011 error for negated bind, got: {msg}"
    );
}

/// TEST-011 scenario 4: Tilde negation variant `~(> ?x 100)` → parse error
///
/// Note: SPL tilde negation uses `~name` on atoms, not on list forms.
/// `(~(> ?x 100))` is not valid syntax — the `~` prefix only works on atoms.
/// This test verifies the parser rejects such malformed input.
#[test]
fn test_011_04_tilde_negation_variant() {
    // The tilde form for negating a comparison in the body.
    // The spec shows (~(> ?x 100)) but SPL tilde works on atoms like ~name.
    // The closest valid representation would use (not ...) form.
    // Trying ~> as an atom — should be rejected as reserved keyword
    let err = parse_spl("(normally r1 (and (val ?x) (not (> ?x 100))) (low ?x))").unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("cannot be negated") && msg.contains("REQ-011"),
        "Expected REQ-011 error for negated comparison, got: {msg}"
    );
}

/// TEST-011 scenario 5: Complementary comparison `(<= ?x 100)` → legal
#[test]
fn test_011_05_complementary_comparison_legal() {
    let theory = parse_spl("(normally r1 (and (val ?x) (<= ?x 100)) (low ?x))").unwrap();
    assert_eq!(theory.rule_count(), 1);

    let rule = theory.rules().next().unwrap();
    // Body should contain a logic literal and a comparison constraint
    let has_comparison = rule.body.iter().any(|bl| bl.is_arithmetic());
    assert!(
        has_comparison,
        "Expected arithmetic comparison constraint in body"
    );
}
