//! Literal, body, and timepoint parsing helpers for SPL.
//!
//! These functions convert S-expression trees into `Literal` and `TimePoint`
//! values, handling negation, modals, temporal annotations, and predicates.

use chrono::DateTime;
use rust_decimal::Decimal;
use spindle_core::Literal;
use spindle_core::arith::ArithConstraint;
use spindle_core::body::{BodyArg, BodyLiteral, BodyLogicLiteral};
use spindle_core::intern::intern;
use spindle_core::mode::Mode;
use spindle_core::temporal::{
    AllenConstraint, AllenRelation, StateQueryKind, Temporal, TemporalExpr, TemporalStateQuery,
    TimeExpr, TimePoint,
};
use spindle_core::term::{FiniteFloat, Term};

use crate::ParseError;
use crate::error::ParserFormat;

use super::arith::{is_arith_op, is_cmp_op, parse_arith_expr, parse_cmp_op};
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
            vec![BodyLiteral::Logic(parse_body_logic_literal_with_line(expr, line)?)],
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
                        body_literals.push(BodyLiteral::Logic(
                            parse_body_logic_literal_with_line(item, line)?,
                        ));
                    }
                }

                Ok((body_literals, constraints, state_queries))
            } else {
                // Single expression — could be an arithmetic constraint or a literal
                if let Some(arith) = try_parse_arith_constraint(expr, line)? {
                    Ok((vec![BodyLiteral::Arithmetic(arith)], vec![], vec![]))
                } else {
                    Ok((
                        vec![BodyLiteral::Logic(
                            parse_body_logic_literal_with_line(expr, line)?,
                        )],
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
                    let terms: Result<Vec<Term>, _> = items[1..]
                        .iter()
                        .map(|a| {
                            let s = a.as_atom().ok_or_else(|| ParseError::ParserError {
                                line,
                                message: "Expected atom argument".to_string(),
                                format: ParserFormat::Spl,
                                source_line: None,
                            })?;
                            parse_term_from_atom(s, line)
                        })
                        .collect();
                    Ok(Literal::from_ids(
                        first,
                        false,
                        Mode::empty(),
                        Temporal::empty(),
                        terms?,
                    ))
                }
            }
        }
    }
}

/// Parse a body logic literal expression with line number tracking.
///
/// Like [`parse_literal_with_line`] but returns a [`BodyLogicLiteral`] directly,
/// recognising arithmetic expressions in predicate argument positions.
/// An argument like `(* ?p 2)` becomes `BodyArg::Arith(ArithExpr)` instead of
/// requiring it to be an atom.
fn parse_body_logic_literal_with_line(
    expr: &SExpr,
    line: usize,
) -> Result<BodyLogicLiteral, ParseError> {
    match expr {
        SExpr::Atom { value: s, .. } => {
            if let Some(name) = s.strip_prefix("~~") {
                if name.is_empty() {
                    return Err(ParseError::ParserError {
                        line,
                        message: "Double negation with empty name".to_string(),
                        format: ParserFormat::Spl,
                        source_line: None,
                    });
                }
                Ok(BodyLogicLiteral::simple(name))
            } else if let Some(name) = s.strip_prefix('~') {
                Ok(BodyLogicLiteral::negated(name))
            } else {
                Ok(BodyLogicLiteral::simple(s))
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
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "not takes exactly one argument".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    let mut inner = parse_body_logic_literal_with_line(&items[1], line)?;
                    inner.negation = !inner.negation;
                    Ok(inner)
                }
                "must" => {
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "must takes exactly one argument".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    let mut lit = parse_body_logic_literal_with_line(&items[1], line)?;
                    lit.mode = Mode::obligation();
                    Ok(lit)
                }
                "may" => {
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "may takes exactly one argument".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    let mut lit = parse_body_logic_literal_with_line(&items[1], line)?;
                    lit.mode = Mode::permission();
                    Ok(lit)
                }
                "forbidden" => {
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "forbidden takes exactly one argument".to_string(),
                            format: ParserFormat::Spl,
                            source_line: None,
                        });
                    }
                    let mut lit = parse_body_logic_literal_with_line(&items[1], line)?;
                    lit.mode = Mode::forbidden();
                    Ok(lit)
                }
                "during" => {
                    if items.len() == 3 {
                        if let Some(var_name) = items[2].as_atom()
                            && var_name.starts_with('?')
                        {
                            let mut lit =
                                parse_body_logic_literal_with_line(&items[1], line)?;
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
                    let mut lit = parse_body_logic_literal_with_line(&items[1], line)?;
                    let start = parse_timeexpr_with_line(&items[2], line)?;
                    let end = parse_timeexpr_with_line(&items[3], line)?;

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
                    // Each argument can be an atom (→ BodyArg::Term) or
                    // a list starting with an arith operator (→ BodyArg::Arith).
                    let args: Result<Vec<BodyArg>, _> =
                        items[1..].iter().map(|a| parse_body_arg(a, line)).collect();
                    Ok(BodyLogicLiteral::new(
                        first,
                        false,
                        Mode::empty(),
                        Temporal::empty(),
                        args?,
                    ))
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Numeric literal detection helpers
// ---------------------------------------------------------------------------

/// Returns `true` if `s` matches the integer pattern: `-?[0-9]+`.
fn is_integer_literal(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

/// Returns `true` if `s` matches the decimal pattern: `-?[0-9]+.[0-9]+`.
fn is_decimal_literal(s: &str) -> bool {
    let s = s.strip_prefix('-').unwrap_or(s);
    if let Some(dot_pos) = s.find('.') {
        let before = &s[..dot_pos];
        let after = &s[dot_pos + 1..];
        !before.is_empty()
            && before.bytes().all(|b| b.is_ascii_digit())
            && !after.is_empty()
            && after.bytes().all(|b| b.is_ascii_digit())
    } else {
        false
    }
}

/// Returns `true` if `s` matches the float pattern: `-?[0-9]+(.[0-9]+)?[eE]-?[0-9]+`.
fn is_float_literal(s: &str) -> bool {
    let e_pos = s.find(|c: char| c == 'e' || c == 'E');
    let e_pos = match e_pos {
        Some(pos) => pos,
        None => return false,
    };

    let mantissa = &s[..e_pos];
    let exponent = &s[e_pos + 1..];

    (is_integer_literal(mantissa) || is_decimal_literal(mantissa))
        && is_integer_literal(exponent)
}

/// Try to parse an atom as a numeric [`Term`].
///
/// Detection order: float (has e/E) → decimal (has .) → integer.
/// Returns `Ok(None)` if the atom does not match any numeric pattern (i.e., it's a symbol).
/// Returns `Err` if the atom matches a numeric pattern but the value is out of range.
fn try_parse_numeric_term(value: &str, line: usize) -> Result<Option<Term>, ParseError> {
    // Float (has e/E): checked first
    if is_float_literal(value) {
        let f = value.parse::<f64>().map_err(|_| ParseError::ParserError {
            line,
            message: format!("Float out of range: {value}"),
            format: ParserFormat::Spl,
            source_line: None,
        })?;
        let ff = FiniteFloat::new(f).ok_or_else(|| ParseError::ParserError {
            line,
            message: format!("Non-finite float value: {value}"),
            format: ParserFormat::Spl,
            source_line: None,
        })?;
        return Ok(Some(Term::Float(ff)));
    }

    // Decimal (has .): checked second
    if is_decimal_literal(value) {
        let d = value.parse::<Decimal>().map_err(|_| ParseError::ParserError {
            line,
            message: format!("Decimal out of range: {value}"),
            format: ParserFormat::Spl,
            source_line: None,
        })?;
        return Ok(Some(Term::Decimal(d)));
    }

    // Integer: checked third
    if is_integer_literal(value) {
        let n = value.parse::<i64>().map_err(|_| ParseError::ParserError {
            line,
            message: format!("Integer out of range: {value}"),
            format: ParserFormat::Spl,
            source_line: None,
        })?;
        return Ok(Some(Term::Integer(n)));
    }

    // Not numeric — it's a symbol
    Ok(None)
}

/// Parse an atom string as a [`Term`], detecting numeric literals.
///
/// If the atom matches a numeric pattern, returns the appropriate numeric Term.
/// Otherwise returns `Term::Symbol`.
fn parse_term_from_atom(value: &str, line: usize) -> Result<Term, ParseError> {
    if let Some(term) = try_parse_numeric_term(value, line)? {
        Ok(term)
    } else {
        Ok(Term::Symbol(intern(value)))
    }
}

/// Parse a single predicate argument as a [`BodyArg`].
///
/// - Atom → `BodyArg::Term(...)` (numeric literals detected automatically)
/// - List starting with arithmetic operator → `BodyArg::Arith(ArithExpr)`
/// - Other list → error
fn parse_body_arg(expr: &SExpr, line: usize) -> Result<BodyArg, ParseError> {
    match expr {
        SExpr::Atom { value, .. } => Ok(BodyArg::Term(parse_term_from_atom(value, line)?)),
        SExpr::List { items, .. } => {
            if let Some(head) = items.first().and_then(|i| i.as_atom()) {
                if is_arith_op(head) {
                    let arith = parse_arith_expr(expr, line)?;
                    return Ok(BodyArg::Arith(arith));
                }
            }
            Err(ParseError::ParserError {
                line,
                message: "Expected atom argument or arithmetic expression".to_string(),
                format: ParserFormat::Spl,
                source_line: None,
            })
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

    // =====================================================================
    // Body arith args — arithmetic expressions in predicate arguments
    // =====================================================================

    #[test]
    fn test_body_arith_arg_mul() {
        // (line-total ?i (* ?p 2))
        let mul = list(vec![atom("*"), atom("?p"), atom("2")]);
        let expr = list(vec![atom("line-total"), atom("?i"), mul]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 1);
        let lit = body[0].as_logic().expect("expected Logic variant");
        assert_eq!(lit.name(), "line-total");
        assert_eq!(lit.predicate_args().len(), 2);

        assert_eq!(
            lit.predicate_args()[0],
            BodyArg::Term(Term::Symbol(intern("?i")))
        );
        match &lit.predicate_args()[1] {
            BodyArg::Arith(ArithExpr::NaryOp { op, args }) => {
                assert_eq!(*op, NaryArithOp::Mul);
                assert_eq!(args.len(), 2);
                assert_eq!(args[0], ArithExpr::Var(intern("?p")));
                assert_eq!(args[1], ArithExpr::Lit(NumericValue::Integer(2)));
            }
            other => panic!("Expected BodyArg::Arith(NaryOp::Mul), got: {other:?}"),
        }
    }

    #[test]
    fn test_body_arith_arg_add() {
        // (cost ?item (+ ?base ?tax))
        let add = list(vec![atom("+"), atom("?base"), atom("?tax")]);
        let expr = list(vec![atom("cost"), atom("?item"), add]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 1);
        let lit = body[0].as_logic().unwrap();
        assert_eq!(lit.name(), "cost");
        assert!(lit.has_arith_args());
        assert!(matches!(
            &lit.predicate_args()[1],
            BodyArg::Arith(ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                ..
            })
        ));
    }

    #[test]
    fn test_body_arith_arg_nested() {
        // (result ?x (+ (* ?a ?b) ?c))
        let mul = list(vec![atom("*"), atom("?a"), atom("?b")]);
        let add = list(vec![atom("+"), mul, atom("?c")]);
        let expr = list(vec![atom("result"), atom("?x"), add]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        let lit = body[0].as_logic().unwrap();
        assert_eq!(lit.name(), "result");
        match &lit.predicate_args()[1] {
            BodyArg::Arith(ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                args,
            }) => {
                assert!(matches!(
                    &args[0],
                    ArithExpr::NaryOp {
                        op: NaryArithOp::Mul,
                        ..
                    }
                ));
                assert_eq!(args[1], ArithExpr::Var(intern("?c")));
            }
            other => panic!("Expected nested arith, got: {other:?}"),
        }
    }

    #[test]
    fn test_body_arith_arg_multiple() {
        // (tri ?a (+ ?b 1) (- ?c 2))
        let add = list(vec![atom("+"), atom("?b"), atom("1")]);
        let sub = list(vec![atom("-"), atom("?c"), atom("2")]);
        let expr = list(vec![atom("tri"), atom("?a"), add, sub]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        let lit = body[0].as_logic().unwrap();
        assert_eq!(lit.predicate_args().len(), 3);
        assert!(matches!(&lit.predicate_args()[0], BodyArg::Term(_)));
        assert!(matches!(
            &lit.predicate_args()[1],
            BodyArg::Arith(ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                ..
            })
        ));
        assert!(matches!(
            &lit.predicate_args()[2],
            BodyArg::Arith(ArithExpr::NaryOp {
                op: NaryArithOp::Sub,
                ..
            })
        ));
    }

    #[test]
    fn test_body_arith_arg_in_conjunction() {
        // (and (price ?item ?p) (line-total ?item (* ?p ?qty)))
        let price = list(vec![atom("price"), atom("?item"), atom("?p")]);
        let mul = list(vec![atom("*"), atom("?p"), atom("?qty")]);
        let total = list(vec![atom("line-total"), atom("?item"), mul]);
        let expr = list(vec![atom("and"), price, total]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        assert_eq!(body.len(), 2);

        let price_lit = body[0].as_logic().unwrap();
        assert_eq!(price_lit.name(), "price");
        assert!(!price_lit.has_arith_args());

        let total_lit = body[1].as_logic().unwrap();
        assert_eq!(total_lit.name(), "line-total");
        assert!(total_lit.has_arith_args());
    }

    #[test]
    fn test_body_arith_arg_negated() {
        // (not (cost ?item (* ?p 2)))
        let mul = list(vec![atom("*"), atom("?p"), atom("2")]);
        let cost = list(vec![atom("cost"), atom("?item"), mul]);
        let expr = list(vec![atom("not"), cost]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        let lit = body[0].as_logic().unwrap();
        assert!(lit.is_negated());
        assert_eq!(lit.name(), "cost");
        assert!(lit.has_arith_args());
    }

    #[test]
    fn test_body_arith_arg_all_operators() {
        // Test /, div, rem, **, abs in arg positions
        for (op_str, expected_check) in [
            ("/", "NaryOp"),
            ("div", "BinOp"),
            ("rem", "BinOp"),
            ("**", "BinOp"),
        ] {
            let arith = list(vec![atom(op_str), atom("?x"), atom("2")]);
            let expr = list(vec![atom("f"), arith]);
            let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();
            let lit = body[0].as_logic().unwrap();
            match &lit.predicate_args()[0] {
                BodyArg::Arith(_) => {} // expected
                other => panic!(
                    "Expected BodyArg::Arith for '{op_str}' ({expected_check}), got: {other:?}"
                ),
            }
        }

        // abs is unary
        let abs = list(vec![atom("abs"), atom("?x")]);
        let expr = list(vec![atom("f"), abs]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();
        assert!(matches!(
            &body[0].as_logic().unwrap().predicate_args()[0],
            BodyArg::Arith(_)
        ));
    }

    #[test]
    fn test_body_arith_arg_non_arith_list_error() {
        // (pred (unknown ?x)) — "unknown" is not an arith op
        let bad = list(vec![atom("unknown"), atom("?x")]);
        let expr = list(vec![atom("pred"), bad]);
        let err = parse_body_with_line(&expr, 1).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("atom argument or arithmetic expression"),
            "got: {msg}"
        );
    }

    #[test]
    fn test_body_no_arith_args_unchanged() {
        // (parent alice bob) — plain atom args, no arith
        let expr = list(vec![atom("parent"), atom("alice"), atom("bob")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();

        let lit = body[0].as_logic().unwrap();
        assert_eq!(lit.name(), "parent");
        assert!(!lit.has_arith_args());
        assert_eq!(
            lit.predicate_args()[0],
            BodyArg::Term(Term::Symbol(intern("alice")))
        );
        assert_eq!(
            lit.predicate_args()[1],
            BodyArg::Term(Term::Symbol(intern("bob")))
        );
    }

    // =====================================================================
    // Numeric literal detection helpers
    // =====================================================================

    #[test]
    fn test_is_integer_literal() {
        assert!(is_integer_literal("42"));
        assert!(is_integer_literal("0"));
        assert!(is_integer_literal("-7"));
        assert!(is_integer_literal("123456789"));
        assert!(!is_integer_literal(""));
        assert!(!is_integer_literal("-"));
        assert!(!is_integer_literal("abc"));
        assert!(!is_integer_literal("3.14"));
        assert!(!is_integer_literal("1e5"));
        assert!(!is_integer_literal("?x"));
    }

    #[test]
    fn test_is_decimal_literal() {
        assert!(is_decimal_literal("3.14"));
        assert!(is_decimal_literal("-0.5"));
        assert!(is_decimal_literal("100.00"));
        assert!(!is_decimal_literal("42"));
        assert!(!is_decimal_literal(".5"));
        assert!(!is_decimal_literal("5."));
        assert!(!is_decimal_literal("abc"));
        assert!(!is_decimal_literal("1e5"));
        assert!(!is_decimal_literal("1.5e2"));
    }

    #[test]
    fn test_is_float_literal() {
        assert!(is_float_literal("1e5"));
        assert!(is_float_literal("1E5"));
        assert!(is_float_literal("1.5e2"));
        assert!(is_float_literal("-1.5e-2"));
        assert!(is_float_literal("3e-10"));
        assert!(!is_float_literal("42"));
        assert!(!is_float_literal("3.14"));
        assert!(!is_float_literal("abc"));
        assert!(!is_float_literal("eE"));
        assert!(!is_float_literal("1e"));
        assert!(!is_float_literal("e5"));
    }

    // =====================================================================
    // Numeric literal parsing — parse_term_from_atom
    // =====================================================================

    #[test]
    fn test_parse_term_integer() {
        let t = parse_term_from_atom("42", 1).unwrap();
        assert_eq!(t, Term::Integer(42));
    }

    #[test]
    fn test_parse_term_negative_integer() {
        let t = parse_term_from_atom("-100", 1).unwrap();
        assert_eq!(t, Term::Integer(-100));
    }

    #[test]
    fn test_parse_term_zero() {
        let t = parse_term_from_atom("0", 1).unwrap();
        assert_eq!(t, Term::Integer(0));
    }

    #[test]
    fn test_parse_term_decimal() {
        let t = parse_term_from_atom("3.14", 1).unwrap();
        let expected = "3.14".parse::<Decimal>().unwrap();
        assert_eq!(t, Term::Decimal(expected));
    }

    #[test]
    fn test_parse_term_negative_decimal() {
        let t = parse_term_from_atom("-0.5", 1).unwrap();
        let expected = "-0.5".parse::<Decimal>().unwrap();
        assert_eq!(t, Term::Decimal(expected));
    }

    #[test]
    fn test_parse_term_float_scientific() {
        let t = parse_term_from_atom("1.5e2", 1).unwrap();
        assert_eq!(t, Term::Float(FiniteFloat::new(150.0).unwrap()));
    }

    #[test]
    fn test_parse_term_float_no_decimal() {
        let t = parse_term_from_atom("1e6", 1).unwrap();
        assert_eq!(t, Term::Float(FiniteFloat::new(1_000_000.0).unwrap()));
    }

    #[test]
    fn test_parse_term_float_negative_exponent() {
        let t = parse_term_from_atom("5e-3", 1).unwrap();
        assert_eq!(t, Term::Float(FiniteFloat::new(0.005).unwrap()));
    }

    #[test]
    fn test_parse_term_float_negative_mantissa() {
        let t = parse_term_from_atom("-2.5e3", 1).unwrap();
        assert_eq!(t, Term::Float(FiniteFloat::new(-2500.0).unwrap()));
    }

    #[test]
    fn test_parse_term_symbol() {
        let t = parse_term_from_atom("alice", 1).unwrap();
        assert_eq!(t, Term::Symbol(intern("alice")));
    }

    #[test]
    fn test_parse_term_variable() {
        // Variables like ?x should remain as symbols
        let t = parse_term_from_atom("?x", 1).unwrap();
        assert_eq!(t, Term::Symbol(intern("?x")));
    }

    #[test]
    fn test_parse_term_integer_out_of_range() {
        // i64::MAX + 1
        let err = parse_term_from_atom("9999999999999999999999", 5).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Integer out of range"),
            "Expected integer overflow error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_term_float_infinity() {
        // A value that parses as f64 infinity
        let err = parse_term_from_atom("1e999", 3).unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Non-finite float") || msg.contains("Float out of range"),
            "Expected float range error, got: {msg}"
        );
    }

    // =====================================================================
    // Numeric literals in head literal positions (parse_literal_with_line)
    // =====================================================================

    #[test]
    fn test_head_literal_integer_arg() {
        // (cost item 42) → predicate "cost" with args [Symbol("item"), Integer(42)]
        let expr = list(vec![atom("cost"), atom("item"), atom("42")]);
        let lit = parse_literal_with_line(&expr, 1).unwrap();
        assert_eq!(lit.name(), "cost");
        assert_eq!(lit.predicate_args().len(), 2);
        assert_eq!(lit.predicate_args()[0], Term::Symbol(intern("item")));
        assert_eq!(lit.predicate_args()[1], Term::Integer(42));
    }

    #[test]
    fn test_head_literal_decimal_arg() {
        // (price item 9.99) → predicate "price" with args [Symbol("item"), Decimal(9.99)]
        let expr = list(vec![atom("price"), atom("item"), atom("9.99")]);
        let lit = parse_literal_with_line(&expr, 1).unwrap();
        assert_eq!(lit.predicate_args()[1], Term::Decimal("9.99".parse::<Decimal>().unwrap()));
    }

    #[test]
    fn test_head_literal_float_arg() {
        // (sensor reading 1.5e3) → predicate with Float arg
        let expr = list(vec![atom("sensor"), atom("reading"), atom("1.5e3")]);
        let lit = parse_literal_with_line(&expr, 1).unwrap();
        assert_eq!(
            lit.predicate_args()[1],
            Term::Float(FiniteFloat::new(1500.0).unwrap())
        );
    }

    #[test]
    fn test_head_literal_negative_integer_arg() {
        // (temp sensor -40) → predicate with negative integer
        let expr = list(vec![atom("temp"), atom("sensor"), atom("-40")]);
        let lit = parse_literal_with_line(&expr, 1).unwrap();
        assert_eq!(lit.predicate_args()[1], Term::Integer(-40));
    }

    #[test]
    fn test_head_literal_mixed_args() {
        // (measurement device 42 3.14 1e2) — symbol, int, decimal, float
        let expr = list(vec![
            atom("measurement"),
            atom("device"),
            atom("42"),
            atom("3.14"),
            atom("1e2"),
        ]);
        let lit = parse_literal_with_line(&expr, 1).unwrap();
        assert_eq!(lit.predicate_args().len(), 4);
        assert_eq!(lit.predicate_args()[0], Term::Symbol(intern("device")));
        assert_eq!(lit.predicate_args()[1], Term::Integer(42));
        assert_eq!(lit.predicate_args()[2], Term::Decimal("3.14".parse::<Decimal>().unwrap()));
        assert_eq!(
            lit.predicate_args()[3],
            Term::Float(FiniteFloat::new(100.0).unwrap())
        );
    }

    // =====================================================================
    // Numeric literals in body literal positions (parse_body_arg)
    // =====================================================================

    #[test]
    fn test_body_arg_integer() {
        let expr = list(vec![atom("cost"), atom("item"), atom("42")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();
        let lit = body[0].as_logic().unwrap();
        assert_eq!(lit.predicate_args()[1], BodyArg::Term(Term::Integer(42)));
    }

    #[test]
    fn test_body_arg_decimal() {
        let expr = list(vec![atom("price"), atom("item"), atom("9.99")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();
        let lit = body[0].as_logic().unwrap();
        assert_eq!(
            lit.predicate_args()[1],
            BodyArg::Term(Term::Decimal("9.99".parse::<Decimal>().unwrap()))
        );
    }

    #[test]
    fn test_body_arg_float() {
        let expr = list(vec![atom("sensor"), atom("reading"), atom("1.5e3")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();
        let lit = body[0].as_logic().unwrap();
        assert_eq!(
            lit.predicate_args()[1],
            BodyArg::Term(Term::Float(FiniteFloat::new(1500.0).unwrap()))
        );
    }

    #[test]
    fn test_body_arg_negative_integer() {
        let expr = list(vec![atom("temp"), atom("sensor"), atom("-40")]);
        let (body, _, _) = parse_body_with_line(&expr, 1).unwrap();
        let lit = body[0].as_logic().unwrap();
        assert_eq!(
            lit.predicate_args()[1],
            BodyArg::Term(Term::Integer(-40))
        );
    }

    // =====================================================================
    // Detection order: float > decimal > integer
    // =====================================================================

    #[test]
    fn test_detection_order_float_over_decimal() {
        // "1.5e2" has both '.' and 'e', should be float (not decimal)
        let t = parse_term_from_atom("1.5e2", 1).unwrap();
        assert!(matches!(t, Term::Float(_)));
    }

    #[test]
    fn test_detection_order_float_over_integer() {
        // "1e5" has 'e', should be float (not integer)
        let t = parse_term_from_atom("1e5", 1).unwrap();
        assert!(matches!(t, Term::Float(_)));
    }

    #[test]
    fn test_detection_order_decimal_over_integer() {
        // "3.0" has '.', should be decimal (not integer)
        let t = parse_term_from_atom("3.0", 1).unwrap();
        assert!(matches!(t, Term::Decimal(_)));
    }

    // =====================================================================
    // Integration: numeric literals through parse_spl
    // =====================================================================

    #[test]
    fn test_spl_fact_with_integer_arg() {
        use crate::spl::parse_spl;
        let theory = parse_spl("(given (cost item 42))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert_eq!(lit.name(), "cost");
        assert_eq!(lit.predicate_args()[1], Term::Integer(42));
    }

    #[test]
    fn test_spl_fact_with_decimal_arg() {
        use crate::spl::parse_spl;
        let theory = parse_spl("(given (price item 9.99))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert_eq!(
            lit.predicate_args()[1],
            Term::Decimal("9.99".parse::<Decimal>().unwrap())
        );
    }

    #[test]
    fn test_spl_fact_with_float_arg() {
        use crate::spl::parse_spl;
        let theory = parse_spl("(given (sensor reading 1.5e3))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert_eq!(
            lit.predicate_args()[1],
            Term::Float(FiniteFloat::new(1500.0).unwrap())
        );
    }

    #[test]
    fn test_spl_integer_out_of_range_error() {
        use crate::spl::parse_spl;
        let err = parse_spl("(given (cost item 99999999999999999999))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Integer out of range"),
            "Expected integer overflow error, got: {msg}"
        );
    }

    #[test]
    fn test_spl_rule_with_numeric_body_args() {
        use crate::spl::parse_spl;
        let theory = parse_spl("(normally r1 (and (price ?item 100) (quality ?item high)) (buy ?item))").unwrap();
        let rule = theory.rules().next().unwrap();
        // Body literal "price" should have Integer(100) as second arg
        let price_body = &rule.body[0];
        let lit = price_body.as_logic().unwrap();
        assert_eq!(lit.name(), "price");
        assert_eq!(
            lit.predicate_args()[1],
            BodyArg::Term(Term::Integer(100))
        );
    }
}
