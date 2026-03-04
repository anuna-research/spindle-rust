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
//! | `+`, `*`     | 0        | ∞        | `Call`             |
//! | `-`, `/`     | 1        | ∞        | `Call`             |
//! | `min`, `max` | 1        | ∞        | `Call`             |
//! | `div`, `rem` | 2        | 2        | `Call`             |
//! | `**`         | 2        | 2        | `Call`             |
//! | `abs`        | 1        | 1        | `Call`             |

use rust_decimal::Decimal;
use spindle_core::arith::{ArithExpr, CmpOp};
use spindle_core::intern::intern;
use spindle_core::term::NumericValue;

use crate::ParseError;
use crate::error::ParserFormat;

use super::lexer::SExpr;

/// Reserved arithmetic operator names.
const ARITH_OPS: &[&str] = &[
    "+", "-", "*", "/", "div", "rem", "abs", "min", "max", "**", "round", "floor", "ceil",
];

/// Reserved comparison operator names.
const CMP_OPS: &[&str] = &["=", "!=", "<", ">", "<=", ">="];

/// Reserved keywords that cannot be used as predicate names or rule labels (REQ-008).
///
/// This includes arithmetic operators, comparison operators, and the `bind` keyword.
const RESERVED_KEYWORDS: &[&str] = &[
    "+", "-", "*", "/", "div", "rem", "abs", "min", "max", "**", "round", "floor", "ceil", "bind",
    "fold", "=", "!=", "<", ">", "<=", ">=",
];

/// Future-reserved keywords that cannot be used as predicate names or rule labels (REQ-008).
///
/// These are aggregate/math functions reserved for future use.
const FUTURE_RESERVED_KEYWORDS: &[&str] = &["sum", "count", "avg"];

/// Returns `true` if `name` is a reserved arithmetic operator.
pub(crate) fn is_arith_op(name: &str) -> bool {
    ARITH_OPS.contains(&name)
}

/// Returns `true` if `name` is a reserved comparison operator.
pub(crate) fn is_cmp_op(name: &str) -> bool {
    CMP_OPS.contains(&name)
}

/// Returns `true` if `name` is a reserved keyword (REQ-008).
pub(crate) fn is_reserved_keyword(name: &str) -> bool {
    RESERVED_KEYWORDS.contains(&name)
}

/// Returns `true` if `name` is a future-reserved keyword (REQ-008).
pub(crate) fn is_future_reserved_keyword(name: &str) -> bool {
    FUTURE_RESERVED_KEYWORDS.contains(&name)
}

/// Returns `true` if `name` is an arithmetic predicate (`bind` or comparison operator).
///
/// Used by guards that reject arithmetic predicates in head position (REQ-009).
pub(crate) fn is_arith_predicate(name: &str) -> bool {
    name == "bind" || is_cmp_op(name)
}

/// Parse a comparison operator name into a [`CmpOp`].
///
/// Returns `None` if `name` is not a recognised comparison operator.
pub(crate) fn parse_cmp_op(name: &str) -> Option<CmpOp> {
    match name {
        "=" => Some(CmpOp::Eq),
        "!=" => Some(CmpOp::Ne),
        "<" => Some(CmpOp::Lt),
        ">" => Some(CmpOp::Gt),
        "<=" => Some(CmpOp::Le),
        ">=" => Some(CmpOp::Ge),
        _ => None,
    }
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

            let arg_exprs = &items[1..];

            // All operators (built-in and unknown) emit Call nodes.
            // Built-in operators get parse-time arity checking for good error messages.
            if is_arith_op(op_name) {
                match op_name {
                    "+" | "*" => parse_known_call(op_name, arg_exprs, 0, line),
                    "-" | "/" | "min" | "max" => parse_known_call(op_name, arg_exprs, 1, line),
                    "div" | "rem" | "**" | "round" => {
                        parse_known_call_exact(op_name, arg_exprs, 2, line)
                    }
                    "abs" | "floor" | "ceil" => parse_known_call_exact(op_name, arg_exprs, 1, line),
                    _ => unreachable!("is_arith_op guard above"),
                }
            } else {
                // Unknown operators become extension function Call nodes,
                // validated later against the function registry.
                let args = arg_exprs
                    .iter()
                    .map(|e| parse_arith_expr(e, line))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ArithExpr::Call {
                    name: intern(op_name),
                    args,
                })
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
    if name.contains('.')
        && !name.contains('e')
        && !name.contains('E')
        && let Ok(d) = name.parse::<Decimal>()
    {
        return Ok(ArithExpr::Lit(NumericValue::Decimal(d)));
    }

    // Try float (scientific notation or other float formats).
    // Only attempt f64 parse if the token syntactically looks like a float
    // (contains '.', 'e', or 'E'). This prevents oversized bare integers
    // from silently losing precision via f64 conversion.
    if (name.contains('.') || name.contains('e') || name.contains('E'))
        && let Ok(f) = name.parse::<f64>()
    {
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

/// Parse a known operator as a Call node with minimum-arity checking.
fn parse_known_call(
    op_name: &str,
    args: &[SExpr],
    min_arity: usize,
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

    Ok(ArithExpr::Call {
        name: intern(op_name),
        args: parsed,
    })
}

/// Parse a known operator as a Call node with exact-arity checking.
fn parse_known_call_exact(
    op_name: &str,
    args: &[SExpr],
    exact_arity: usize,
    line: usize,
) -> Result<ArithExpr, ParseError> {
    if args.len() != exact_arity {
        let noun = if exact_arity == 1 {
            "argument"
        } else {
            "arguments"
        };
        return Err(ParseError::ParserError {
            line,
            message: format!(
                "Arithmetic operator '{op_name}' requires exactly {exact_arity} {noun}, got {}",
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

    Ok(ArithExpr::Call {
        name: intern(op_name),
        args: parsed,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use spindle_core::arith::ArithExpr;
    use spindle_core::intern::{intern, resolve};
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
        for op in &[
            "+", "-", "*", "/", "div", "rem", "abs", "min", "max", "**", "round", "floor", "ceil",
        ] {
            assert!(is_arith_op(op), "Expected '{op}' to be an arith op");
        }
    }

    #[test]
    fn test_is_arith_op_non_operators() {
        for name in &["bird", "and", "not", "given", "?x", "42", "mod"] {
            assert!(
                !is_arith_op(name),
                "Expected '{name}' to NOT be an arith op"
            );
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
        assert!(msg.contains("Invalid arithmetic operand"), "got: {msg}");
    }

    #[test]
    fn test_parse_empty_list() {
        let err = parse_arith_expr(&list(vec![]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Empty list"), "got: {msg}");
    }

    #[test]
    fn test_parse_unknown_operator_becomes_call() {
        let expr = parse_arith_expr(&list(vec![atom("mod"), atom("5"), atom("3")]), 1).unwrap();
        match expr {
            ArithExpr::Call { name, args } => {
                assert_eq!(resolve(name), "mod");
                assert_eq!(args.len(), 2);
            }
            other => panic!("expected Call, got: {other:?}"),
        }
    }

    // =====================================================================
    // N-ary: + (0+ args)
    // =====================================================================

    #[test]
    fn test_add_zero_args() {
        let expr = parse_arith_expr(&list(vec![atom("+")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("+"),
                args: vec![]
            }
        );
    }

    #[test]
    fn test_add_one_arg() {
        let expr = parse_arith_expr(&list(vec![atom("+"), atom("5")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("+"),
                args: vec![ArithExpr::Lit(NumericValue::Integer(5))]
            }
        );
    }

    #[test]
    fn test_add_two_args() {
        let expr = parse_arith_expr(&list(vec![atom("+"), atom("1"), atom("2")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("+"),
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
        if let ArithExpr::Call { name, args } = &expr {
            assert_eq!(resolve(*name), "+");
            assert_eq!(args.len(), 4);
        } else {
            panic!("Expected Call, got: {expr:?}");
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
            ArithExpr::Call {
                name: intern("*"),
                args: vec![]
            }
        );
    }

    #[test]
    fn test_mul_two_args() {
        let expr = parse_arith_expr(&list(vec![atom("*"), atom("3"), atom("4")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("*"),
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
            ArithExpr::Call {
                name: intern("-"),
                args: vec![ArithExpr::Lit(NumericValue::Integer(5))]
            }
        );
    }

    #[test]
    fn test_sub_two_args() {
        let expr = parse_arith_expr(&list(vec![atom("-"), atom("10"), atom("3")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("-"),
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
            ArithExpr::Call {
                name: intern("/"),
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
            ArithExpr::Call {
                name: intern("min"),
                args: vec![ArithExpr::Lit(NumericValue::Integer(5))]
            }
        );
    }

    #[test]
    fn test_min_three_args() {
        let expr =
            parse_arith_expr(&list(vec![atom("min"), atom("5"), atom("3"), atom("7")]), 1).unwrap();
        if let ArithExpr::Call { name, args } = &expr {
            assert_eq!(resolve(*name), "min");
            assert_eq!(args.len(), 3);
        } else {
            panic!("Expected Call");
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
        let expr = parse_arith_expr(&list(vec![atom("max"), atom("1"), atom("9")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("max"),
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
        let expr = parse_arith_expr(&list(vec![atom("div"), atom("10"), atom("3")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("div"),
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(10)),
                    ArithExpr::Lit(NumericValue::Integer(3)),
                ]
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
        let expr = parse_arith_expr(&list(vec![atom("rem"), atom("10"), atom("3")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("rem"),
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(10)),
                    ArithExpr::Lit(NumericValue::Integer(3)),
                ]
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
        let expr = parse_arith_expr(&list(vec![atom("**"), atom("2"), atom("3")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("**"),
                args: vec![
                    ArithExpr::Lit(NumericValue::Integer(2)),
                    ArithExpr::Lit(NumericValue::Integer(3)),
                ]
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
        let expr = parse_arith_expr(&list(vec![atom("abs"), atom("-5")]), 1).unwrap();
        assert_eq!(
            expr,
            ArithExpr::Call {
                name: intern("abs"),
                args: vec![ArithExpr::Lit(NumericValue::Integer(-5))],
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
        let err = parse_arith_expr(&list(vec![atom("abs"), atom("1"), atom("2")]), 1).unwrap_err();
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
            ArithExpr::Call {
                name: intern("+"),
                args: vec![
                    ArithExpr::Call {
                        name: intern("*"),
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

        if let ArithExpr::Call { name, args } = &expr {
            assert_eq!(resolve(*name), "+");
            assert_eq!(args.len(), 2);
            if let ArithExpr::Call {
                name: inner_name, ..
            } = &args[0]
            {
                assert_eq!(resolve(*inner_name), "abs");
            } else {
                panic!("Expected Call for abs");
            }
        } else {
            panic!("Expected Call");
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

        if let ArithExpr::Call { name, args } = &expr {
            assert_eq!(resolve(*name), "+");
            assert_eq!(args.len(), 2);
            if let ArithExpr::Call { name: sub_name, .. } = &args[0] {
                assert_eq!(resolve(*sub_name), "-");
            } else {
                panic!("Expected Call for -");
            }
            if let ArithExpr::Call { name: abs_name, .. } = &args[1] {
                assert_eq!(resolve(*abs_name), "abs");
            } else {
                panic!("Expected Call for abs");
            }
        } else {
            panic!("Expected Call for +, got: {expr:?}");
        }
    }

    #[test]
    fn test_nested_error_propagation() {
        // (+ (abs 1 2) 3) — inner abs has wrong arity
        let bad_abs = list(vec![atom("abs"), atom("1"), atom("2")]);
        let err = parse_arith_expr(&list(vec![atom("+"), bad_abs, atom("3")]), 1).unwrap_err();
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

        if let ArithExpr::Call { name, args } = &expr {
            assert_eq!(resolve(*name), "+");
            assert_eq!(args.len(), 3);
            assert!(matches!(args[0], ArithExpr::Lit(NumericValue::Integer(1))));
            assert!(matches!(args[1], ArithExpr::Lit(NumericValue::Decimal(_))));
            assert!(matches!(args[2], ArithExpr::Var(_)));
        } else {
            panic!("Expected Call");
        }
    }

    // =====================================================================
    // Non-operator head in list → error
    // =====================================================================

    #[test]
    fn test_list_head_not_atom() {
        let inner = list(vec![atom("1")]);
        let err = parse_arith_expr(&list(vec![inner, atom("2")]), 1).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Expected arithmetic operator atom"),
            "got: {msg}"
        );
    }
}
