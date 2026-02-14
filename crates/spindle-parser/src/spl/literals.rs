//! Literal, body, and timepoint parsing helpers for SPL.
//!
//! These functions convert S-expression trees into `Literal` and `TimePoint`
//! values, handling negation, modals, temporal annotations, and predicates.

use chrono::DateTime;
use spindle_core::Literal;
use spindle_core::intern::intern;
use spindle_core::mode::Mode;
use spindle_core::temporal::{
    AllenConstraint, AllenRelation, StateQueryKind, Temporal, TemporalExpr, TemporalStateQuery,
    TimeExpr, TimePoint,
};

use crate::ParseError;
use crate::error::ParserFormat;

use super::lexer::SExpr;

/// Parsed body components: (literals, Allen constraints, state queries).
type BodyParseResult = (Vec<Literal>, Vec<AllenConstraint>, Vec<TemporalStateQuery>);

/// Parse a body expression with line number.
///
/// Returns `(literals, allen_constraints, state_queries)`. Constraints are only
/// recognized inside `(and ...)` conjunctions, where expressions like
/// `(before ?T ?S)` are parsed as interval constraints rather than literals.
pub(crate) fn parse_body_with_line(
    expr: &SExpr,
    line: usize,
) -> Result<BodyParseResult, ParseError> {
    match expr {
        SExpr::Atom { .. } => Ok((vec![parse_literal_with_line(expr, line)?], vec![], vec![])),
        SExpr::List { items, .. } => {
            if items.is_empty() {
                return Ok((vec![], vec![], vec![]));
            }

            // Check for (and ...)
            if let Some("and") = items[0].as_atom() {
                let mut literals = Vec::new();
                let mut constraints = Vec::new();
                let mut state_queries = Vec::new();

                for item in &items[1..] {
                    if let Some(constraint) = try_parse_allen_constraint(item, line)? {
                        constraints.push(constraint);
                    } else if let Some(sq) = try_parse_state_query(item, line)? {
                        state_queries.push(sq);
                    } else {
                        literals.push(parse_literal_with_line(item, line)?);
                    }
                }

                Ok((literals, constraints, state_queries))
            } else {
                // Single complex literal
                Ok((vec![parse_literal_with_line(expr, line)?], vec![], vec![]))
            }
        }
    }
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
