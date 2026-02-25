//! Literal, body, and timepoint parsing helpers for SPL.
//!
//! These functions convert S-expression trees into `Literal` and `TimePoint`
//! values, handling negation, modals, temporal annotations, and predicates.

use chrono::DateTime;
use spindle_core::Literal;
use spindle_core::arith::ArithConstraint;
use spindle_core::body::BodyLiteral;
use spindle_core::intern::intern;
use spindle_core::mode::Mode;
use spindle_core::temporal::{
    AllenConstraint, AllenRelation, StateQueryKind, Temporal, TemporalExpr, TemporalStateQuery,
    TimeExpr, TimePoint,
};

use crate::ParseError;
use crate::error::ParserFormat;

use super::arith::{is_cmp_op, parse_arith_expr, parse_cmp_op};
use super::lexer::SExpr;

/// Parsed body components: (body literals, Allen constraints, state queries).
type BodyParseResult = (Vec<BodyLiteral>, Vec<AllenConstraint>, Vec<TemporalStateQuery>);

/// Parse a body expression with line number.
///
/// Returns `(body_literals, allen_constraints, state_queries)`. Constraints are only
/// recognized inside `(and ...)` conjunctions, where expressions like
/// `(before ?T ?S)` are parsed as interval constraints rather than literals.
///
/// Body literals include logic literals (predicate patterns) and arithmetic
/// constraints (`bind` and comparison operators).
pub(crate) fn parse_body_with_line(
    expr: &SExpr,
    line: usize,
) -> Result<BodyParseResult, ParseError> {
    match expr {
        SExpr::Atom { .. } => Ok((
            vec![BodyLiteral::Logic(parse_literal_with_line(expr, line)?.into())],
            vec![],
            vec![],
        )),
        SExpr::List { items, .. } => {
            if items.is_empty() {
                return Ok((vec![], vec![], vec![]));
            }

            // Check for (and ...)
            if let Some("and") = items[0].as_atom() {
                let mut body_literals = Vec::new();
                let mut constraints = Vec::new();
                let mut state_queries = Vec::new();

                for item in &items[1..] {
                    if let Some(constraint) = try_parse_allen_constraint(item, line)? {
                        constraints.push(constraint);
                    } else if let Some(sq) = try_parse_state_query(item, line)? {
                        state_queries.push(sq);
                    } else if let Some(arith) = try_parse_arith_constraint(item, line)? {
                        body_literals.push(BodyLiteral::Arithmetic(arith));
                    } else {
                        body_literals
                            .push(BodyLiteral::Logic(parse_literal_with_line(item, line)?.into()));
                    }
                }

                Ok((body_literals, constraints, state_queries))
            } else {
                // Single expression — could be an arithmetic constraint or a literal
                if let Some(arith) = try_parse_arith_constraint(expr, line)? {
                    Ok((vec![BodyLiteral::Arithmetic(arith)], vec![], vec![]))
                } else {
                    Ok((
                        vec![BodyLiteral::Logic(parse_literal_with_line(expr, line)?.into())],
                        vec![],
                        vec![],
                    ))
                }
            }
        }
    }
}

/// Try to parse an s-expression as an arithmetic constraint (`bind` or comparison).
///
/// Returns `Ok(Some(constraint))` if this is a bind or comparison form,
/// `Ok(None)` if it's not, or `Err` for malformed constraints.
fn try_parse_arith_constraint(
    expr: &SExpr,
    line: usize,
) -> Result<Option<ArithConstraint>, ParseError> {
    let items = match expr {
        SExpr::List { items, .. } if !items.is_empty() => items,
        _ => return Ok(None),
    };

    let keyword = match items[0].as_atom() {
        Some(kw) => kw,
        None => return Ok(None),
    };

    if keyword == "bind" {
        return try_parse_bind(items, line).map(Some);
    }

    if is_cmp_op(keyword) {
        return try_parse_compare(keyword, items, line).map(Some);
    }

    Ok(None)
}

/// Parse a `(bind ?var expr)` constraint.
///
/// Requirements:
/// - Exactly 2 arguments (3 items total including `bind`)
/// - First argument must be a `?`-prefixed variable
/// - Second argument must be a valid arithmetic expression
fn try_parse_bind(items: &[SExpr], line: usize) -> Result<ArithConstraint, ParseError> {
    if items.len() != 3 {
        return Err(ParseError::ParserError {
            line,
            message: format!(
                "'bind' requires exactly 2 arguments (variable and expression), got {}",
                items.len() - 1
            ),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let var_name = items[1].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "'bind' first argument must be a variable (e.g. ?x), got a list".to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    if !var_name.starts_with('?') {
        return Err(ParseError::ParserError {
            line,
            message: format!(
                "'bind' first argument must be a variable (starting with ?), got '{var_name}'"
            ),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let expr = parse_arith_expr(&items[2], line)?;

    Ok(ArithConstraint::Bind {
        var: intern(var_name),
        expr,
    })
}

/// Parse a `(<cmp> expr1 expr2)` comparison constraint.
///
/// Requirements:
/// - Exactly 2 arguments (3 items total including operator)
/// - Both arguments must be valid arithmetic expressions
fn try_parse_compare(
    op_name: &str,
    items: &[SExpr],
    line: usize,
) -> Result<ArithConstraint, ParseError> {
    if items.len() != 3 {
        return Err(ParseError::ParserError {
            line,
            message: format!(
                "Comparison operator '{op_name}' requires exactly 2 arguments, got {}",
                items.len() - 1
            ),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let op = parse_cmp_op(op_name).expect("is_cmp_op guard above");
    let lhs = parse_arith_expr(&items[1], line)?;
    let rhs = parse_arith_expr(&items[2], line)?;

    Ok(ArithConstraint::Compare { op, lhs, rhs })
}

/// Try to parse an s-expression as an Allen interval constraint.
///
/// An Allen constraint has the form `(relation ?T ?S)` where:
/// - `relation` is one of the 13 Allen relation keywords
/// - `?T` and `?S` are interval variables (atoms starting with `?`)
///
/// Returns `Ok(Some(constraint))` if this is an Allen constraint,
/// `Ok(None)` if it's not (should be parsed as a literal instead),
/// or `Err` for malformed Allen constraints.
fn try_parse_allen_constraint(
    expr: &SExpr,
    line: usize,
) -> Result<Option<AllenConstraint>, ParseError> {
    let items = match expr {
        SExpr::List { items, .. } if items.len() == 3 => items,
        _ => return Ok(None),
    };

    let keyword = match items[0].as_atom() {
        Some(kw) => kw,
        None => return Ok(None),
    };

    let relation = match AllenRelation::from_keyword(keyword) {
        Some(r) => r,
        None => return Ok(None),
    };

    // Both arguments must be ?-prefixed interval variables
    let var1 = items[1].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: format!("Allen relation '{keyword}' requires interval variable arguments"),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let var2 = items[2].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: format!("Allen relation '{keyword}' requires interval variable arguments"),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    if !var1.starts_with('?') || !var2.starts_with('?') {
        return Err(ParseError::ParserError {
            line,
            message: format!(
                "Allen relation '{keyword}' arguments must be interval variables (starting with ?), got '{var1}' and '{var2}'"
            ),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    Ok(Some(AllenConstraint::new(
        relation,
        intern(var1),
        intern(var2),
    )))
}

/// Try to parse an s-expression as a temporal state query.
///
/// A state query has the form `(kind ?T timepoint)` where:
/// - `kind` is one of `active-at`, `past-at`, `future-at`
/// - `?T` is an interval variable (atom starting with `?`)
/// - `timepoint` is a concrete timepoint or temporal variable
///
/// Returns `Ok(Some(query))` if this is a state query,
/// `Ok(None)` if it's not (should be parsed as a literal instead),
/// or `Err` for malformed state queries.
fn try_parse_state_query(
    expr: &SExpr,
    line: usize,
) -> Result<Option<TemporalStateQuery>, ParseError> {
    let items = match expr {
        SExpr::List { items, .. } if items.len() == 3 => items,
        _ => return Ok(None),
    };

    let keyword = match items[0].as_atom() {
        Some(kw) => kw,
        None => return Ok(None),
    };

    let kind = match keyword {
        "active-at" => StateQueryKind::ActiveAt,
        "past-at" => StateQueryKind::PastAt,
        "future-at" => StateQueryKind::FutureAt,
        _ => return Ok(None),
    };

    // First argument must be a ?-prefixed interval variable
    let var = items[1].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: format!("State query '{keyword}' requires an interval variable as first argument"),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    if !var.starts_with('?') {
        return Err(ParseError::ParserError {
            line,
            message: format!(
                "State query '{keyword}' first argument must be an interval variable (starting with ?), got '{var}'"
            ),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    // Second argument is a time expression (concrete timepoint or variable)
    let time = parse_timeexpr_with_line(&items[2], line)?;

    Ok(Some(TemporalStateQuery::new(kind, intern(var), time)))
}

/// Parse a literal expression with line number tracking
pub(crate) fn parse_literal_with_line(expr: &SExpr, line: usize) -> Result<Literal, ParseError> {
    match expr {
        SExpr::Atom { value: s, .. } => {
            // Handle double-negation: ~~name -> positive name
            if let Some(name) = s.strip_prefix("~~") {
                if name.is_empty() {
                    return Err(ParseError::ParserError {
                        line,
                        message: "Double negation with empty name".to_string(),
                        format: ParserFormat::Spl,
                        source_line: None,
                    });
                }
                // ~~name = not(not(name)) = name (positive)
                Ok(Literal::simple(name))
            } else if let Some(name) = s.strip_prefix('~') {
                // Handle single negation prefix
                Ok(Literal::negated(name))
            } else {
                Ok(Literal::simple(s))
            }
        }
        SExpr::List { items, .. } => {
            if items.is_empty() {
                return Err(ParseError::ParserError {
                    line,
                    message: "Empty list is not a valid literal".to_string(),
                    format: ParserFormat::Spl,
                    source_line: None,
                });
            }

            let first = items[0].as_atom().ok_or_else(|| ParseError::ParserError {
                line,
                message: "Expected atom in literal".to_string(),
                format: ParserFormat::Spl,
                source_line: None,
            })?;

            match first {
                "not" => {
                    // (not literal)
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "not takes exactly one argument".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    let inner = parse_literal_with_line(&items[1], line)?;
                    Ok(inner.complement())
                }
                "must" => {
                    // (must literal)
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "must takes exactly one argument".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    let mut lit = parse_literal_with_line(&items[1], line)?;
                    lit.mode = Mode::obligation();
                    Ok(lit)
                }
                "may" => {
                    // (may literal)
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "may takes exactly one argument".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    let mut lit = parse_literal_with_line(&items[1], line)?;
                    lit.mode = Mode::permission();
                    Ok(lit)
                }
                "forbidden" => {
                    // (forbidden literal)
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "forbidden takes exactly one argument".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    let mut lit = parse_literal_with_line(&items[1], line)?;
                    lit.mode = Mode::forbidden();
                    Ok(lit)
                }
                "during" => {
                    // Two forms:
                    //   (during literal start end)  — two-endpoint form
                    //   (during literal ?T)         — single interval variable form
                    if items.len() == 3 {
                        // Check for single interval variable form: (during literal ?T)
                        if let Some(var_name) = items[2].as_atom()
                            && var_name.starts_with('?')
                        {
                            let mut lit = parse_literal_with_line(&items[1], line)?;
                            lit.interval_var = Some(intern(var_name));
                            return Ok(lit);
                        }
                        return Err(ParseError::ParserError {
                            line,
                            message: "during takes either (during literal start end) or (during literal ?var)".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    if items.len() != 4 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "during takes either (during literal start end) or (during literal ?var)".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    let mut lit = parse_literal_with_line(&items[1], line)?;
                    let start = parse_timeexpr_with_line(&items[2], line)?;
                    let end = parse_timeexpr_with_line(&items[3], line)?;

                    // If both endpoints are concrete, use concrete temporal directly.
                    // Otherwise, store as temporal_expr for grounding to resolve.
                    match (&start, &end) {
                        (TimeExpr::Const(s), TimeExpr::Const(e)) => {
                            lit.temporal = Temporal::new(*s, *e);
                        }
                        _ => {
                            lit.temporal_expr = Some(TemporalExpr::new(start, end));
                        }
                    }
                    Ok(lit)
                }
                _ => {
                    // Predicate: (name arg1 arg2 ...)
                    let predicates: Result<Vec<String>, _> = items[1..]
                        .iter()
                        .map(|a| {
                            a.as_atom().map(|s| s.to_string()).ok_or_else(|| {
                                ParseError::ParserError {
                                    line,
                                    message: "Expected atom argument".to_string(),
                                    format: ParserFormat::Spl,
                                    source_line: None,
                                }
                            })
                        })
                        .collect();
                    Ok(Literal::new(
                        first,
                        false,
                        Default::default(),
                        Default::default(),
                        predicates?,
                    ))
                }
            }
        }
    }
}

/// Parse a time expression that may be a concrete timepoint or a temporal variable.
///
/// Temporal variables start with `?` (e.g., `?t1`, `?start`).
/// Concrete values are parsed by [`parse_timepoint_with_line`].
pub(crate) fn parse_timeexpr_with_line(expr: &SExpr, line: usize) -> Result<TimeExpr, ParseError> {
    // Check for temporal variable first
    if let SExpr::Atom { value: s, .. } = expr
        && s.starts_with('?')
    {
        return Ok(TimeExpr::var(s));
    }
    // Fall back to concrete timepoint parsing
    parse_timepoint_with_line(expr, line).map(TimeExpr::Const)
}

/// Parse a timepoint expression with line number tracking
pub(crate) fn parse_timepoint_with_line(
    expr: &SExpr,
    line: usize,
) -> Result<TimePoint, ParseError> {
    match expr {
        SExpr::Atom { value: s, .. } => {
            if s == "-inf" {
                Ok(TimePoint::NegInf)
            } else if s == "inf" || s == "+inf" {
                Ok(TimePoint::PosInf)
            } else if let Ok(n) = s.parse::<i64>() {
                Ok(TimePoint::from_millis(n))
            } else {
                Err(ParseError::ParserError {
                    line,
                    message: format!("Invalid timepoint: {s}"),
                    format: ParserFormat::Spl,
                    source_line: None,
                })
            }
        }
        SExpr::List { items, .. } => {
            // (moment "RFC3339") only
            if items.len() == 2 && items[0].as_atom() == Some("moment") {
                if let Some(s) = items[1].as_atom() {
                    // RFC3339 parsing using chrono
                    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                        return Ok(TimePoint::from_millis(dt.timestamp_millis()));
                    }

                    Err(ParseError::ParserError {
                        line,
                        message: format!("Invalid RFC3339 timepoint for moment: {s}"),
                        format: ParserFormat::Spl,
                        source_line: None,
                    })
                } else {
                    Err(ParseError::ParserError {
                        line,
                        message: "moment argument must be atom".to_string(),
                        format: ParserFormat::Spl,
                        source_line: None,
                    })
                }
            } else if items.len() >= 4 && items[0].as_atom() == Some("moment") {
                Err(ParseError::ParserError {
                    line,
                    message: "Multi-arity moment (YYYY MM DD ...) not yet supported".to_string(),
                    format: ParserFormat::Spl,
                    source_line: None,
                })
            } else {
                Err(ParseError::ParserError {
                    line,
                    message: "Invalid timepoint expression".to_string(),
                    format: ParserFormat::Spl,
                    source_line: None,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use spindle_core::arith::{ArithExpr, CmpOp, NaryArithOp};
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
    // bind — happy paths
    // =====================================================================

    #[test]
    fn test_bind_variable_to_literal() {
        // (bind ?x 42)
        let expr = list(vec![atom("bind"), atom("?x"), atom("42")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 1);
        match &body[0] {
            BodyLiteral::Arithmetic(ArithConstraint::Bind { var, expr }) => {
                assert_eq!(*var, intern("?x"));
                assert_eq!(*expr, ArithExpr::Lit(NumericValue::Integer(42)));
            }
            other => panic!("Expected Arithmetic(Bind), got: {other:?}"),
        }
    }

    #[test]
    fn test_bind_variable_to_expression() {
        // (bind ?total (+ ?a ?b))
        let add_expr = list(vec![atom("+"), atom("?a"), atom("?b")]);
        let expr = list(vec![atom("bind"), atom("?total"), add_expr]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 1);
        match &body[0] {
            BodyLiteral::Arithmetic(ArithConstraint::Bind { var, expr }) => {
                assert_eq!(*var, intern("?total"));
                assert!(matches!(
                    expr,
                    ArithExpr::NaryOp {
                        op: NaryArithOp::Add,
                        ..
                    }
                ));
            }
            other => panic!("Expected Arithmetic(Bind), got: {other:?}"),
        }
    }

    #[test]
    fn test_bind_in_and_conjunction() {
        // (and (bird ?x) (bind ?y 10))
        let bird = list(vec![atom("bird"), atom("?x")]);
        let bind = list(vec![atom("bind"), atom("?y"), atom("10")]);
        let expr = list(vec![atom("and"), bird, bind]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 2);
        assert!(matches!(&body[0], BodyLiteral::Logic(_)));
        assert!(matches!(
            &body[1],
            BodyLiteral::Arithmetic(ArithConstraint::Bind { .. })
        ));
    }

    // =====================================================================
    // bind — error cases
    // =====================================================================

    #[test]
    fn test_bind_wrong_arity_one_arg() {
        let expr = list(vec![atom("bind"), atom("?x")]);
        let err = parse_body_with_line(&expr, 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("2 arguments"), "got: {msg}");
    }

    #[test]
    fn test_bind_wrong_arity_three_args() {
        let expr = list(vec![atom("bind"), atom("?x"), atom("1"), atom("2")]);
        let err = parse_body_with_line(&expr, 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("2 arguments"), "got: {msg}");
    }

    #[test]
    fn test_bind_first_arg_not_variable() {
        let expr = list(vec![atom("bind"), atom("x"), atom("42")]);
        let err = parse_body_with_line(&expr, 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("variable"), "got: {msg}");
        assert!(msg.contains("starting with ?"), "got: {msg}");
    }

    #[test]
    fn test_bind_first_arg_is_list() {
        let expr = list(vec![
            atom("bind"),
            list(vec![atom("?x")]),
            atom("42"),
        ]);
        let err = parse_body_with_line(&expr, 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("variable"), "got: {msg}");
    }

    #[test]
    fn test_bind_invalid_arith_expr() {
        // (bind ?x bird) — "bird" is not a valid arith operand
        let expr = list(vec![atom("bind"), atom("?x"), atom("bird")]);
        let err = parse_body_with_line(&expr, 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Invalid arithmetic operand"), "got: {msg}");
    }

    // =====================================================================
    // compare — happy paths
    // =====================================================================

    #[test]
    fn test_compare_eq() {
        let expr = list(vec![atom("="), atom("?x"), atom("5")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 1);
        match &body[0] {
            BodyLiteral::Arithmetic(ArithConstraint::Compare { op, lhs, rhs }) => {
                assert_eq!(*op, CmpOp::Eq);
                assert_eq!(*lhs, ArithExpr::Var(intern("?x")));
                assert_eq!(*rhs, ArithExpr::Lit(NumericValue::Integer(5)));
            }
            other => panic!("Expected Arithmetic(Compare), got: {other:?}"),
        }
    }

    #[test]
    fn test_compare_ne() {
        let expr = list(vec![atom("!="), atom("?x"), atom("0")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 1);
        match &body[0] {
            BodyLiteral::Arithmetic(ArithConstraint::Compare { op, .. }) => {
                assert_eq!(*op, CmpOp::Ne);
            }
            other => panic!("Expected Arithmetic(Compare), got: {other:?}"),
        }
    }

    #[test]
    fn test_compare_lt() {
        let expr = list(vec![atom("<"), atom("?a"), atom("?b")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        match &body[0] {
            BodyLiteral::Arithmetic(ArithConstraint::Compare { op, .. }) => {
                assert_eq!(*op, CmpOp::Lt);
            }
            other => panic!("Expected Arithmetic(Compare), got: {other:?}"),
        }
    }

    #[test]
    fn test_compare_gt() {
        let expr = list(vec![atom(">"), atom("?x"), atom("100")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        match &body[0] {
            BodyLiteral::Arithmetic(ArithConstraint::Compare { op, .. }) => {
                assert_eq!(*op, CmpOp::Gt);
            }
            other => panic!("Expected Arithmetic(Compare), got: {other:?}"),
        }
    }

    #[test]
    fn test_compare_le() {
        let expr = list(vec![atom("<="), atom("?x"), atom("?y")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        match &body[0] {
            BodyLiteral::Arithmetic(ArithConstraint::Compare { op, .. }) => {
                assert_eq!(*op, CmpOp::Le);
            }
            other => panic!("Expected Arithmetic(Compare), got: {other:?}"),
        }
    }

    #[test]
    fn test_compare_ge() {
        let expr = list(vec![atom(">="), atom("?x"), atom("?y")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        match &body[0] {
            BodyLiteral::Arithmetic(ArithConstraint::Compare { op, .. }) => {
                assert_eq!(*op, CmpOp::Ge);
            }
            other => panic!("Expected Arithmetic(Compare), got: {other:?}"),
        }
    }

    #[test]
    fn test_compare_with_arith_exprs() {
        // (< (+ ?x 1) (* ?y 2))
        let lhs = list(vec![atom("+"), atom("?x"), atom("1")]);
        let rhs = list(vec![atom("*"), atom("?y"), atom("2")]);
        let expr = list(vec![atom("<"), lhs, rhs]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 1);
        match &body[0] {
            BodyLiteral::Arithmetic(ArithConstraint::Compare { op, lhs, rhs }) => {
                assert_eq!(*op, CmpOp::Lt);
                assert!(matches!(
                    lhs,
                    ArithExpr::NaryOp {
                        op: NaryArithOp::Add,
                        ..
                    }
                ));
                assert!(matches!(
                    rhs,
                    ArithExpr::NaryOp {
                        op: NaryArithOp::Mul,
                        ..
                    }
                ));
            }
            other => panic!("Expected Arithmetic(Compare), got: {other:?}"),
        }
    }

    #[test]
    fn test_compare_in_and_conjunction() {
        // (and (price ?item ?p) (> ?p 100))
        let price = list(vec![atom("price"), atom("?item"), atom("?p")]);
        let cmp = list(vec![atom(">"), atom("?p"), atom("100")]);
        let expr = list(vec![atom("and"), price, cmp]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 2);
        assert!(matches!(&body[0], BodyLiteral::Logic(_)));
        assert!(matches!(
            &body[1],
            BodyLiteral::Arithmetic(ArithConstraint::Compare { op: CmpOp::Gt, .. })
        ));
    }

    // =====================================================================
    // compare — error cases
    // =====================================================================

    #[test]
    fn test_compare_wrong_arity_one_arg() {
        let expr = list(vec![atom("<"), atom("?x")]);
        let err = parse_body_with_line(&expr, 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("2 arguments"), "got: {msg}");
    }

    #[test]
    fn test_compare_wrong_arity_three_args() {
        let expr = list(vec![atom("="), atom("?x"), atom("?y"), atom("?z")]);
        let err = parse_body_with_line(&expr, 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("2 arguments"), "got: {msg}");
    }

    #[test]
    fn test_compare_invalid_arith_operand() {
        // (= ?x bird) — "bird" is not a valid arith operand
        let expr = list(vec![atom("="), atom("?x"), atom("bird")]);
        let err = parse_body_with_line(&expr, 1).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Invalid arithmetic operand"), "got: {msg}");
    }

    // =====================================================================
    // Mixed: bind + compare + logic in conjunction
    // =====================================================================

    #[test]
    fn test_mixed_body_literals() {
        // (and (price ?item ?p) (bind ?tax (* ?p 0.1)) (> ?p 50))
        let price = list(vec![atom("price"), atom("?item"), atom("?p")]);
        let mul = list(vec![atom("*"), atom("?p"), atom("0.1")]);
        let bind = list(vec![atom("bind"), atom("?tax"), mul]);
        let cmp = list(vec![atom(">"), atom("?p"), atom("50")]);
        let expr = list(vec![atom("and"), price, bind, cmp]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 3);
        assert!(matches!(&body[0], BodyLiteral::Logic(_)));
        assert!(matches!(
            &body[1],
            BodyLiteral::Arithmetic(ArithConstraint::Bind { .. })
        ));
        assert!(matches!(
            &body[2],
            BodyLiteral::Arithmetic(ArithConstraint::Compare { op: CmpOp::Gt, .. })
        ));
    }
}
