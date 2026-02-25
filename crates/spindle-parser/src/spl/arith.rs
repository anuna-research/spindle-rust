//! Arithmetic expression parsing for SPL.
//!
//! Recognises S-expressions whose head atom is a reserved arithmetic operator
//! (`+`, `-`, `*`, `/`, `div`, `rem`, `abs`, `min`, `max`, `**`) and
//! recursively parses them into [`ArithExpr`] trees.
//!
//! # Arity Rules
//!
//! | Operator     | Min args | Max args | AST variant       |
//! |--------------|----------|----------|--------------------|
//! | `+`, `*`     | 0        | ∞        | `NaryOp`           |
//! | `-`, `/`     | 1        | ∞        | `NaryOp`           |
//! | `min`, `max` | 1        | ∞        | `NaryOp`           |
//! | `div`, `rem` | 2        | 2        | `BinOp`            |
//! | `**`         | 2        | 2        | `BinOp`            |
//! | `abs`        | 1        | 1        | `UnaryOp`          |

use rust_decimal::Decimal;
use spindle_core::arith::{ArithExpr, BinArithOp, NaryArithOp, UnaryArithOp};
use spindle_core::intern::intern;
use spindle_core::term::NumericValue;

use crate::ParseError;
use crate::error::ParserFormat;

use super::lexer::SExpr;

/// Reserved arithmetic operator names.
const ARITH_OPS: &[&str] = &["+", "-", "*", "/", "div", "rem", "abs", "min", "max", "**"];

/// Returns `true` if `name` is a reserved arithmetic operator.
pub(crate) fn is_arith_op(name: &str) -> bool {
    ARITH_OPS.contains(&name)
}

/// Parse an S-expression as an arithmetic expression.
///
/// An arithmetic expression is one of:
/// - An atom that looks like a variable (`?x`) → `ArithExpr::Var`
/// - An atom that looks like a number → `ArithExpr::Lit`
/// - A list whose head is an arithmetic operator → recursive parse
pub(crate) fn parse_arith_expr(expr: &SExpr, line: usize) -> Result<ArithExpr, ParseError> {
    match expr {
        SExpr::Atom { value, .. } => parse_arith_atom(value, line),
        SExpr::List { items, .. } => {
            if items.is_empty() {
                return Err(ParseError::ParserError {
                    line,
                    message: "Empty list is not a valid arithmetic expression".to_string(),
                    format: ParserFormat::Spl,
                    source_line: None,
                });
            }

            let op_name = items[0].as_atom().ok_or_else(|| ParseError::ParserError {
                line,
                message: "Expected arithmetic operator atom".to_string(),
                format: ParserFormat::Spl,
                source_line: None,
            })?;

            if !is_arith_op(op_name) {
                return Err(ParseError::ParserError {
                    line,
                    message: format!(
                        "Unknown arithmetic operator: '{op_name}'. \
                         Expected one of: +, -, *, /, div, rem, abs, min, max, **"
                    ),
                    format: ParserFormat::Spl,
                    source_line: None,
                });
            }

            let arg_exprs = &items[1..];

            match op_name {
                // N-ary operators
                "+" => parse_nary(NaryArithOp::Add, arg_exprs, 0, op_name, line),
                "-" => parse_nary(NaryArithOp::Sub, arg_exprs, 1, op_name, line),
                "*" => parse_nary(NaryArithOp::Mul, arg_exprs, 0, op_name, line),
                "/" => parse_nary(NaryArithOp::Div, arg_exprs, 1, op_name, line),
                "min" => parse_nary(NaryArithOp::Min, arg_exprs, 1, op_name, line),
                "max" => parse_nary(NaryArithOp::Max, arg_exprs, 1, op_name, line),

                // Binary-only operators
                "div" => parse_binop(BinArithOp::IDiv, arg_exprs, op_name, line),
                "rem" => parse_binop(BinArithOp::Rem, arg_exprs, op_name, line),
                "**" => parse_binop(BinArithOp::Pow, arg_exprs, op_name, line),

                // Unary-only operator
                "abs" => parse_unary(UnaryArithOp::Abs, arg_exprs, op_name, line),

                _ => unreachable!("is_arith_op guard above"),
            }
        }
    }
}

/// Parse an atom as a leaf arithmetic expression (variable or numeric literal).
fn parse_arith_atom(name: &str, line: usize) -> Result<ArithExpr, ParseError> {
    // Variables start with ?
    if name.starts_with('?') {
        return Ok(ArithExpr::Var(intern(name)));
    }

    // Try integer first
    if let Ok(n) = name.parse::<i64>() {
        return Ok(ArithExpr::Lit(NumericValue::Integer(n)));
    }

    // Try decimal (contains '.' but no 'e'/'E' for scientific notation)
    if name.contains('.') && !name.contains('e') && !name.contains('E') {
        if let Ok(d) = name.parse::<Decimal>() {
            return Ok(ArithExpr::Lit(NumericValue::Decimal(d)));
        }
    }

    // Try float (scientific notation or other float formats)
    if let Ok(f) = name.parse::<f64>() {
        if f.is_finite() {
            return Ok(ArithExpr::Lit(NumericValue::Float(f)));
        }
        return Err(ParseError::ParserError {
            line,
            message: format!("Non-finite float value: {name}"),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    Err(ParseError::ParserError {
        line,
        message: format!(
            "Invalid arithmetic operand: '{name}'. \
             Expected a variable (?x), integer, decimal, or float"
        ),
        format: ParserFormat::Spl,
        source_line: None,
    })
}

/// Parse an n-ary operator with a minimum arity constraint.
fn parse_nary(
    op: NaryArithOp,
    args: &[SExpr],
    min_arity: usize,
    op_name: &str,
    line: usize,
) -> Result<ArithExpr, ParseError> {
    if args.len() < min_arity {
        let noun = if min_arity == 1 {
            "argument"
        } else {
            "arguments"
        };
        return Err(ParseError::ParserError {
            line,
            message: format!(
                "Arithmetic operator '{op_name}' requires at least {min_arity} {noun}, \
                 got {}",
                args.len()
            ),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let parsed: Vec<ArithExpr> = args
        .iter()
        .map(|a| parse_arith_expr(a, line))
        .collect::<Result<_, _>>()?;

    Ok(ArithExpr::NaryOp { op, args: parsed })
}

/// Parse a binary-only operator (exactly 2 arguments required).
fn parse_binop(
    op: BinArithOp,
    args: &[SExpr],
    op_name: &str,
    line: usize,
) -> Result<ArithExpr, ParseError> {
    if args.len() != 2 {
        return Err(ParseError::ParserError {
            line,
            message: format!(
                "Arithmetic operator '{op_name}' requires exactly 2 arguments, got {}",
                args.len()
            ),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let lhs = parse_arith_expr(&args[0], line)?;
    let rhs = parse_arith_expr(&args[1], line)?;

    Ok(ArithExpr::BinOp {
        op,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    })
}

/// Parse a unary-only operator (exactly 1 argument required).
fn parse_unary(
    op: UnaryArithOp,
    args: &[SExpr],
    op_name: &str,
    line: usize,
) -> Result<ArithExpr, ParseError> {
    if args.len() != 1 {
        return Err(ParseError::ParserError {
            line,
            message: format!(
                "Arithmetic operator '{op_name}' requires exactly 1 argument, got {}",
                args.len()
            ),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let inner = parse_arith_expr(&args[0], line)?;

    Ok(ArithExpr::UnaryOp {
        op,
        expr: Box::new(inner),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use spindle_core::arith::{BinArithOp, NaryArithOp, UnaryArithOp};
    use spindle_core::intern::intern;
    use spindle_core::term::NumericValue;

    /// Helper: build an SExpr atom.
    fn atom(s: &str) -> SExpr {
        SExpr::Atom {
            value: s.to_string(),
            offset: 0,
        }
    }

    /// Helper: build an SExpr list.
    fn list(items: Vec<SExpr>) -> SExpr {
        SExpr::List { items, offset: 0 }
    }

    // =====================================================================
    // is_arith_op
    // =====================================================================

    #[test]
    fn test_is_arith_op_all_operators() {
        for op in &["+", "-", "*", "/", "div", "rem", "abs", "min", "max", "**"] {
            assert!(is_arith_op(op), "Expected '{op}' to be an arith op");
        }
    }

    #[test]
    fn test_is_arith_op_non_operators() {
        for name in &["bird", "and", "not", "given", "?x", "42", "mod"] {
            assert!(!is_arith_op(name), "Expected '{name}' to NOT be an arith op");
        }
    }

    // =====================================================================
    // Leaf atoms: variables
    // =====================================================================

    #[test]
    fn test_parse_variable() {
        let expr = parse_arith_expr(&atom("?x"), 1).unwrap();
        assert_eq!(expr, ArithExpr::Var(intern("?x")));
    }

    #[test]
    fn test_parse_variable_long_name() {
        let expr = parse_arith_expr(&atom("?total_cost"), 1).unwrap();
        assert_eq!(expr, ArithExpr::Var(intern("?total_cost")));
    }

    // =====================================================================
    // Leaf atoms: integers
    // =====================================================================

    #[test]
    fn test_parse_integer_positive() {
        let expr = parse_arith_expr(&atom("42"), 1).unwrap();
        assert_eq!(expr, ArithExpr::Lit(NumericValue::Integer(42)));
    }

    #[test]
    fn test_parse_integer_zero() {
        let expr = parse_arith_expr(&atom("0"), 1).unwrap();
        assert_eq!(expr, ArithExpr::Lit(NumericValue::Integer(0)));
    }

    #[test]
    fn test_parse_integer_negative() {
        let expr = parse_arith_expr(&atom("-7"), 1).unwrap();
        assert_eq!(expr, ArithExpr::Lit(NumericValue::Integer(-7)));
    }

    // =====================================================================
    // Leaf atoms: decimals
    // =====================================================================

    #[test]
    fn test_parse_decimal() {
        let expr = parse_arith_expr(&atom("3.14"), 1).unwrap();
        let expected = "3.14".parse::<Decimal>().unwrap();
        assert_eq!(expr, ArithExpr::Lit(NumericValue::Decimal(expected)));
    }

    #[test]
    fn test_parse_decimal_negative() {
        let expr = parse_arith_expr(&atom("-0.5"), 1).unwrap();
        let expected = "-0.5".parse::<Decimal>().unwrap();
        assert_eq!(expr, ArithExpr::Lit(NumericValue::Decimal(expected)));
    }

    // =====================================================================
    // Leaf atoms: floats (scientific notation)
    // =====================================================================

    #[test]
    fn test_parse_float_scientific() {
        let expr = parse_arith_expr(&atom("1.5e2"), 1).unwrap();
        assert_eq!(expr, ArithExpr::Lit(NumericValue::Float(150.0)));
    }

    // =====================================================================
    // Leaf atoms: errors
    // =====================================================================

    #[test]
    fn test_parse_invalid_atom() {
        let err = parse_arith_expr(&atom("bird"), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Invalid arithmetic operand"),
            "got: {msg}"
        );
    }

    #[test]
    fn test_parse_empty_list() {
        let err = parse_arith_expr(&list(vec![]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Empty list"), "got: {msg}");
    }

    #[test]
    fn test_parse_unknown_operator() {
        let err = parse_arith_expr(&list(vec![atom("mod"), atom("5"), atom("3")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Unknown arithmetic operator"), "got: {msg}");
    }

    // =====================================================================
    // N-ary: + (0+ args)
    // =====================================================================

    #[test]
    fn test_add_zero_args() {
        let expr = parse_arith_expr(&list(vec![atom("+")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                args: vec![]
            }
        );
    }

    #[test]
    fn test_add_one_arg() {
        let expr = parse_arith_expr(&list(vec![atom("+"), atom("5")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                args: vec![ArithExpr::Lit(NumericValue::Integer(5))]
            }
        );
    }

    #[test]
    fn test_add_two_args() {
        let expr =
            parse_arith_expr(&list(vec![atom("+"), atom("1"), atom("2")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(1)),
                    ArithExpr::Lit(NumericValue::Integer(2)),
                ]
            }
        );
    }

    #[test]
    fn test_add_many_args() {
        let expr = parse_arith_expr(
            &list(vec![atom("+"), atom("1"), atom("2"), atom("3"), atom("4")]),
            1,
        )
        .unwrap();
        if let ArithExpr::NaryOp { op, args } = &expr {
            assert_eq!(*op, NaryArithOp::Add);
            assert_eq!(args.len(), 4);
        } else {
            panic!("Expected NaryOp, got: {expr:?}");
        }
    }

    // =====================================================================
    // N-ary: * (0+ args)
    // =====================================================================

    #[test]
    fn test_mul_zero_args() {
        let expr = parse_arith_expr(&list(vec![atom("*")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Mul,
                args: vec![]
            }
        );
    }

    #[test]
    fn test_mul_two_args() {
        let expr =
            parse_arith_expr(&list(vec![atom("*"), atom("3"), atom("4")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Mul,
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(3)),
                    ArithExpr::Lit(NumericValue::Integer(4)),
                ]
            }
        );
    }

    // =====================================================================
    // N-ary: - (1+ args)
    // =====================================================================

    #[test]
    fn test_sub_one_arg_negation() {
        let expr = parse_arith_expr(&list(vec![atom("-"), atom("5")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Sub,
                args: vec![ArithExpr::Lit(NumericValue::Integer(5))]
            }
        );
    }

    #[test]
    fn test_sub_two_args() {
        let expr =
            parse_arith_expr(&list(vec![atom("-"), atom("10"), atom("3")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Sub,
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(10)),
                    ArithExpr::Lit(NumericValue::Integer(3)),
                ]
            }
        );
    }

    #[test]
    fn test_sub_zero_args_error() {
        let err = parse_arith_expr(&list(vec![atom("-")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("at least 1 argument"), "got: {msg}");
    }

    // =====================================================================
    // N-ary: / (1+ args)
    // =====================================================================

    #[test]
    fn test_div_one_arg_reciprocal() {
        let expr = parse_arith_expr(&list(vec![atom("/"), atom("2")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Div,
                args: vec![ArithExpr::Lit(NumericValue::Integer(2))]
            }
        );
    }

    #[test]
    fn test_div_zero_args_error() {
        let err = parse_arith_expr(&list(vec![atom("/")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("at least 1 argument"), "got: {msg}");
    }

    // =====================================================================
    // N-ary: min, max (1+ args)
    // =====================================================================

    #[test]
    fn test_min_one_arg() {
        let expr = parse_arith_expr(&list(vec![atom("min"), atom("5")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Min,
                args: vec![ArithExpr::Lit(NumericValue::Integer(5))]
            }
        );
    }

    #[test]
    fn test_min_three_args() {
        let expr = parse_arith_expr(
            &list(vec![atom("min"), atom("5"), atom("3"), atom("7")]),
            1,
        )
        .unwrap();
        if let ArithExpr::NaryOp { op, args } = &expr {
            assert_eq!(*op, NaryArithOp::Min);
            assert_eq!(args.len(), 3);
        } else {
            panic!("Expected NaryOp");
        }
    }

    #[test]
    fn test_min_zero_args_error() {
        let err = parse_arith_expr(&list(vec![atom("min")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("at least 1 argument"), "got: {msg}");
    }

    #[test]
    fn test_max_two_args() {
        let expr =
            parse_arith_expr(&list(vec![atom("max"), atom("1"), atom("9")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Max,
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(1)),
                    ArithExpr::Lit(NumericValue::Integer(9)),
                ]
            }
        );
    }

    #[test]
    fn test_max_zero_args_error() {
        let err = parse_arith_expr(&list(vec![atom("max")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("at least 1 argument"), "got: {msg}");
    }

    // =====================================================================
    // Binary-only: div, rem, ** (exactly 2 args)
    // =====================================================================

    #[test]
    fn test_idiv_two_args() {
        let expr =
            parse_arith_expr(&list(vec![atom("div"), atom("10"), atom("3")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::BinOp {
                op: BinArithOp::IDiv,
                lhs: Box::new(ArithExpr::Lit(NumericValue::Integer(10))),
                rhs: Box::new(ArithExpr::Lit(NumericValue::Integer(3))),
            }
        );
    }

    #[test]
    fn test_idiv_one_arg_error() {
        let err = parse_arith_expr(&list(vec![atom("div"), atom("10")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("exactly 2 arguments"), "got: {msg}");
    }

    #[test]
    fn test_idiv_three_args_error() {
        let err = parse_arith_expr(
            &list(vec![atom("div"), atom("10"), atom("3"), atom("2")]),
            1,
        )
        .unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("exactly 2 arguments"), "got: {msg}");
    }

    #[test]
    fn test_rem_two_args() {
        let expr =
            parse_arith_expr(&list(vec![atom("rem"), atom("10"), atom("3")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::BinOp {
                op: BinArithOp::Rem,
                lhs: Box::new(ArithExpr::Lit(NumericValue::Integer(10))),
                rhs: Box::new(ArithExpr::Lit(NumericValue::Integer(3))),
            }
        );
    }

    #[test]
    fn test_rem_zero_args_error() {
        let err = parse_arith_expr(&list(vec![atom("rem")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("exactly 2 arguments"), "got: {msg}");
    }

    #[test]
    fn test_pow_two_args() {
        let expr =
            parse_arith_expr(&list(vec![atom("**"), atom("2"), atom("3")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::BinOp {
                op: BinArithOp::Pow,
                lhs: Box::new(ArithExpr::Lit(NumericValue::Integer(2))),
                rhs: Box::new(ArithExpr::Lit(NumericValue::Integer(3))),
            }
        );
    }

    #[test]
    fn test_pow_one_arg_error() {
        let err = parse_arith_expr(&list(vec![atom("**"), atom("2")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("exactly 2 arguments"), "got: {msg}");
    }

    // =====================================================================
    // Unary-only: abs (exactly 1 arg)
    // =====================================================================

    #[test]
    fn test_abs_one_arg() {
        let expr =
            parse_arith_expr(&list(vec![atom("abs"), atom("-5")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::UnaryOp {
                op: UnaryArithOp::Abs,
                expr: Box::new(ArithExpr::Lit(NumericValue::Integer(-5))),
            }
        );
    }

    #[test]
    fn test_abs_zero_args_error() {
        let err = parse_arith_expr(&list(vec![atom("abs")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("exactly 1 argument"), "got: {msg}");
    }

    #[test]
    fn test_abs_two_args_error() {
        let err =
            parse_arith_expr(&list(vec![atom("abs"), atom("1"), atom("2")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("exactly 1 argument"), "got: {msg}");
    }

    // =====================================================================
    // Recursive / nested expressions
    // =====================================================================

    #[test]
    fn test_nested_add_mul() {
        // (+ (* 2 3) 4)
        let inner = list(vec![atom("*"), atom("2"), atom("3")]);
        let expr = parse_arith_expr(&list(vec![atom("+"), inner, atom("4")]), 1).unwrap();

        assert_eq!(
            expr,
            ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                args: vec![
                    ArithExpr::NaryOp {
                        op: NaryArithOp::Mul,
                        args: vec![
                            ArithExpr::Lit(NumericValue::Integer(2)),
                            ArithExpr::Lit(NumericValue::Integer(3)),
                        ]
                    },
                    ArithExpr::Lit(NumericValue::Integer(4)),
                ]
            }
        );
    }

    #[test]
    fn test_nested_abs_in_add() {
        // (+ (abs -3) 1)
        let inner = list(vec![atom("abs"), atom("-3")]);
        let expr = parse_arith_expr(&list(vec![atom("+"), inner, atom("1")]), 1).unwrap();

        if let ArithExpr::NaryOp { op, args } = &expr {
            assert_eq!(*op, NaryArithOp::Add);
            assert_eq!(args.len(), 2);
            assert!(matches!(&args[0], ArithExpr::UnaryOp { op: UnaryArithOp::Abs, .. }));
        } else {
            panic!("Expected NaryOp");
        }
    }

    #[test]
    fn test_deeply_nested() {
        // (+ (- (* ?x 2) (div ?y 3)) (abs ?z))
        let mul = list(vec![atom("*"), atom("?x"), atom("2")]);
        let div = list(vec![atom("div"), atom("?y"), atom("3")]);
        let sub = list(vec![atom("-"), mul, div]);
        let abs = list(vec![atom("abs"), atom("?z")]);
        let expr = parse_arith_expr(&list(vec![atom("+"), sub, abs]), 1).unwrap();

        if let ArithExpr::NaryOp {
            op: NaryArithOp::Add,
            args,
        } = &expr
        {
            assert_eq!(args.len(), 2);
            assert!(matches!(
                &args[0],
                ArithExpr::NaryOp {
                    op: NaryArithOp::Sub,
                    ..
                }
            ));
            assert!(matches!(
                &args[1],
                ArithExpr::UnaryOp {
                    op: UnaryArithOp::Abs,
                    ..
                }
            ));
        } else {
            panic!("Expected NaryOp::Add, got: {expr:?}");
        }
    }

    #[test]
    fn test_nested_error_propagation() {
        // (+ (abs 1 2) 3) — inner abs has wrong arity
        let bad_abs = list(vec![atom("abs"), atom("1"), atom("2")]);
        let err =
            parse_arith_expr(&list(vec![atom("+"), bad_abs, atom("3")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("exactly 1 argument"), "got: {msg}");
    }

    // =====================================================================
    // Mixed types in arguments
    // =====================================================================

    #[test]
    fn test_mixed_integer_decimal_variable() {
        // (+ 1 3.14 ?x)
        let expr = parse_arith_expr(
            &list(vec![atom("+"), atom("1"), atom("3.14"), atom("?x")]),
            1,
        )
        .unwrap();

        if let ArithExpr::NaryOp { op, args } = &expr {
            assert_eq!(*op, NaryArithOp::Add);
            assert_eq!(args.len(), 3);
            assert!(matches!(args[0], ArithExpr::Lit(NumericValue::Integer(1))));
            assert!(matches!(args[1], ArithExpr::Lit(NumericValue::Decimal(_))));
            assert!(matches!(args[2], ArithExpr::Var(_)));
        } else {
            panic!("Expected NaryOp");
        }
    }

    // =====================================================================
    // Non-operator head in list → error
    // =====================================================================

    #[test]
    fn test_list_head_not_atom() {
        let inner = list(vec![atom("1")]);
        let err =
            parse_arith_expr(&list(vec![inner, atom("2")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Expected arithmetic operator atom"),
            "got: {msg}"
        );
    }
}
