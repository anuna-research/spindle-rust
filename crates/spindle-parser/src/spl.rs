//! SPL (Spindle Lisp) Parser
//!
//! Parses the LISP-based SPL format into a Theory.
//!
//! # SPL Format
//!
//! ```lisp
//! ; Comments with semicolon
//!
//! ; Facts
//! (given bird)
//! (given (parent alice bob))
//!
//! ; Strict rules
//! (always r1 bird animal)
//!
//! ; Defeasible rules
//! (normally r1 bird flies)
//!
//! ; Defeaters
//! (except d1 broken-wing flies)
//!
//! ; Negation
//! (normally r2 penguin (not flies))
//!
//! ; Superiority
//! (prefer r2 r1)
//!
//! ; Conjunction
//! (normally r3 (and bird healthy) flies)
//! ```

use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::take_while1,
    character::complete::{char, multispace0},
    error::{Error, ErrorKind},
    multi::many0,
    sequence::{delimited, preceded},
};

use chrono::DateTime;
use spindle_core::mode::Mode;
use spindle_core::temporal::{Temporal, TimePoint};
use spindle_core::{Literal, MetaValue, Rule, RuleType, Theory};

use crate::ParseError;

/// Parse an SPL string into a Theory
pub fn parse_spl(input: &str) -> Result<Theory, ParseError> {
    // Remove comments
    let cleaned = remove_comments(input);

    let mut theory = Theory::new();

    // Parse all top-level expressions, tracking positions for line numbers
    let (remaining, expr_positions) = parse_expressions_with_positions(&cleaned).map_err(|e| {
        let line = line_of_from_error(&cleaned, &e);
        ParseError::ParserError {
            line,
            message: format!("SPL parse error: {e:?}"),
        }
    })?;

    if !remaining.trim().is_empty() {
        let line = line_of_offset(&cleaned, cleaned.len() - remaining.len());
        return Err(ParseError::ParserError {
            line,
            message: format!("Unparsed input remaining: {}", remaining.trim()),
        });
    }

    // Process expressions with line number tracking
    for (expr, offset) in expr_positions {
        let line = line_of_offset(&cleaned, offset);
        process_expr_with_line(&mut theory, &expr, line)?;
    }

    Ok(theory)
}

/// Calculate line number from byte offset in original input
fn line_of_offset(input: &str, offset: usize) -> usize {
    let clamped = offset.min(input.len());
    input[..clamped].chars().filter(|&c| c == '\n').count() + 1
}

/// Calculate line number from a nom parse error on cleaned input.
///
/// Extracts the remaining (unparsed) input length from the nom error and
/// computes the byte offset into the cleaned string.
fn line_of_from_error(cleaned: &str, err: &nom::Err<Error<&str>>) -> usize {
    let remaining_len = match err {
        nom::Err::Error(e) | nom::Err::Failure(e) => e.input.len(),
        nom::Err::Incomplete(_) => 0,
    };
    let offset = cleaned.len().saturating_sub(remaining_len);
    line_of_offset(cleaned, offset)
}

/// Remove semicolon comments and #lang directives from input, respecting quotes
fn remove_comments(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            // Skip #lang directives (Racket-specific)
            if trimmed.starts_with("#lang") {
                return "".to_string();
            }

            let mut result = String::new();
            let mut in_string = false;
            let mut escaped = false;

            for c in line.chars() {
                if in_string {
                    result.push(c);
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        in_string = false;
                    }
                } else {
                    if c == ';' {
                        break; // Comment starts
                    }
                    result.push(c);
                    if c == '"' {
                        in_string = true;
                    }
                }
            }
            result
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// S-expression representation
#[derive(Debug, Clone, PartialEq)]
enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

impl SExpr {
    fn as_atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom(s) => Some(s),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<&[SExpr]> {
        match self {
            SExpr::List(v) => Some(v),
            _ => None,
        }
    }
}

/// Parse multiple expressions, tracking byte offsets for each
fn parse_expressions_with_positions(input: &str) -> IResult<&str, Vec<(SExpr, usize)>> {
    let mut results = Vec::new();
    let mut remaining = input;

    loop {
        // Skip whitespace
        let (after_ws, _) = multispace0::<&str, Error<&str>>(remaining)?;
        if after_ws.is_empty() {
            break;
        }

        // Record offset before parsing the expression
        let offset = input.len() - after_ws.len();

        match parse_sexpr(after_ws) {
            Ok((rest, expr)) => {
                results.push((expr, offset));
                remaining = rest;
            }
            Err(_) => break,
        }
    }

    let final_remaining = remaining;
    // Skip trailing whitespace
    let (final_remaining, _) = multispace0::<&str, Error<&str>>(final_remaining)?;
    Ok((final_remaining, results))
}

/// Parse a single s-expression
fn parse_sexpr(input: &str) -> IResult<&str, SExpr> {
    alt((parse_list, parse_string, parse_atom)).parse(input)
}

/// Parse a list: (...)
fn parse_list(input: &str) -> IResult<&str, SExpr> {
    let (input, items) = delimited(
        char('('),
        many0(preceded(multispace0, parse_sexpr)),
        preceded(multispace0, char(')')),
    )
    .parse(input)?;
    Ok((input, SExpr::List(items)))
}

/// Parse a string: "..."
fn parse_string(input: &str) -> IResult<&str, SExpr> {
    let (input, _) = char('"').parse(input)?;
    let mut escaped = false;
    let mut out = String::new();
    let mut end_idx = None;

    for (idx, c) in input.char_indices() {
        if escaped {
            out.push(c);
            escaped = false;
            continue;
        }

        if c == '\\' {
            escaped = true;
            continue;
        }

        if c == '"' {
            end_idx = Some(idx);
            break;
        }

        out.push(c);
    }

    let end_idx = match end_idx {
        Some(idx) => idx,
        None => {
            return Err(nom::Err::Error(Error::new(input, ErrorKind::Char)));
        }
    };

    let remaining = &input[end_idx + 1..];
    Ok((remaining, SExpr::Atom(out)))
}

/// Parse an atom (identifier, number, variable)
fn parse_atom(input: &str) -> IResult<&str, SExpr> {
    let (input, s) = take_while1(|c: char| {
        c.is_alphanumeric()
            || c == '-'
            || c == '_'
            || c == '?'
            || c == '~'
            || c == ':'
            || c == '.'
            || c == '+'
    })
    .parse(input)?;
    Ok((input, SExpr::Atom(s.to_string())))
}

/// Process an s-expression into theory elements (with line number)
fn process_expr_with_line(
    theory: &mut Theory,
    expr: &SExpr,
    line: usize,
) -> Result<(), ParseError> {
    let list = expr.as_list().ok_or_else(|| ParseError::ParserError {
        line,
        message: "Expected list expression".to_string(),
    })?;

    if list.is_empty() {
        return Ok(());
    }

    let keyword = list[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "Expected keyword".to_string(),
    })?;

    match keyword {
        "given" => process_fact_with_line(theory, &list[1..], line),
        "always" => process_rule_with_line(theory, RuleType::Strict, &list[1..], line),
        "normally" => process_rule_with_line(theory, RuleType::Defeasible, &list[1..], line),
        "except" => process_rule_with_line(theory, RuleType::Defeater, &list[1..], line),
        "prefer" => process_prefer_with_line(theory, &list[1..], line),
        "meta" => process_meta_with_line(theory, &list[1..], line),
        "claims" => process_claims(theory, &list[1..], line),
        "#lang" => Ok(()), // Ignore #lang directive
        _ => Err(ParseError::ParserError {
            line,
            message: format!("Unknown keyword: {keyword}"),
        }),
    }
}

/// Process a claims block: (claims source :at "timestamp" (expr1) (expr2) ...)
fn process_claims(theory: &mut Theory, args: &[SExpr], line: usize) -> Result<(), ParseError> {
    if args.is_empty() {
        return Err(ParseError::ParserError {
            line,
            message: "claims requires a source".to_string(),
        });
    }

    let source = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "claims source must be an atom".to_string(),
    })?;

    let mut timestamp: Option<String> = None;
    let mut body_start = 1;

    // Parse optional :at "timestamp"
    while body_start < args.len() {
        if let Some(kw) = args[body_start].as_atom()
            && kw == ":at"
        {
            if body_start + 1 >= args.len() {
                return Err(ParseError::ParserError {
                    line,
                    message: "claims :at requires a timestamp atom".to_string(),
                });
            }
            let ts = args[body_start + 1]
                .as_atom()
                .ok_or_else(|| ParseError::ParserError {
                    line,
                    message: "claims :at requires a timestamp atom".to_string(),
                })?;
            timestamp = Some(ts.to_string());
            body_start += 2;
            continue;
        }
        break;
    }

    // Process each claimed expression.
    // We propagate a best-effort per-expression line by advancing from the
    // claims line; this preserves useful diagnostics for multi-line blocks.
    for (expr_idx, expr) in args[body_start..].iter().enumerate() {
        let expr_line = line + expr_idx + 1;
        let labels_before: std::collections::HashSet<String> =
            theory.rules().map(|r| r.label.clone()).collect();

        // Each expression inside claims is processed normally but gets source metadata
        process_expr_with_line(theory, expr, expr_line)?;

        // Find newly added rule labels, then attach metadata
        let new_labels: Vec<String> = theory
            .rules()
            .filter(|r| !labels_before.contains(&r.label))
            .map(|r| r.label.clone())
            .collect();

        for label in new_labels {
            theory.add_meta_string(&label, "source", source);
            if let Some(ref ts) = timestamp {
                theory.add_meta_string(&label, "timestamp", ts);
            }
        }
    }

    Ok(())
}

/// Process a fact with line number: (given literal)
fn process_fact_with_line(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if args.is_empty() {
        return Err(ParseError::ParserError {
            line,
            message: "fact requires at least one argument".to_string(),
        });
    }

    let lit = parse_literal_with_line(&args[0], line)?;

    // Handle flat predicate: (given pred arg1 arg2 ...)
    // Only if the first arg is an atom and there are more args
    let lit = if args.len() > 1 && args[0].as_atom().is_some() {
        let name = args[0].as_atom().unwrap();
        // Check if name is a reserved keyword that should be parsed as a nested literal instead
        match name {
            "during" | "must" | "may" | "forbidden" | "not" => lit,
            _ => {
                let predicates: Result<Vec<String>, _> = args[1..]
                    .iter()
                    .map(|a| {
                        a.as_atom()
                            .map(|s| s.to_string())
                            .ok_or_else(|| ParseError::ParserError {
                                line,
                                message: "Expected atom argument".to_string(),
                            })
                    })
                    .collect();
                Literal::new(
                    name,
                    false,
                    Default::default(),
                    Default::default(),
                    predicates?,
                )
            }
        }
    } else {
        lit
    };

    let label = generate_unique_label(theory, "f");
    let rule = Rule::fact(label, lit);
    theory.add_rule(rule);
    Ok(())
}

/// Process a rule with line number: (always/normally/except [label] body head)
fn process_rule_with_line(
    theory: &mut Theory,
    rule_type: RuleType,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if args.len() < 2 {
        return Err(ParseError::ParserError {
            line,
            message: "rule requires at least body and head".to_string(),
        });
    }

    // Check if first arg is a label (simple atom, not a list)
    let (label, body_expr, head_expr) = if args.len() >= 3 {
        // Could be labeled
        if let Some(label_str) = args[0].as_atom() {
            // Check if it looks like a label (not a keyword like "and")
            if label_str != "and" && label_str != "not" && !label_str.starts_with('(') {
                (Some(label_str.to_string()), &args[1], &args[2])
            } else {
                (None, &args[0], &args[1])
            }
        } else {
            (None, &args[0], &args[1])
        }
    } else {
        (None, &args[0], &args[1])
    };

    let body = parse_body_with_line(body_expr, line)?;
    let head = parse_literal_with_line(head_expr, line)?;

    let prefix = match rule_type {
        RuleType::Fact => "f",
        RuleType::Strict => "s",
        RuleType::Defeasible => "r",
        RuleType::Defeater => "d",
    };
    let final_label = label.unwrap_or_else(|| generate_unique_label(theory, prefix));

    // Detect label collision with existing rules
    if theory.get_rule(&final_label).is_some() {
        return Err(ParseError::ParserError {
            line,
            message: format!("Duplicate rule label: {final_label}"),
        });
    }

    let rule = Rule::new(final_label, rule_type, body, vec![head]);
    theory.add_rule(rule);
    Ok(())
}

/// Parse a body expression with line number
fn parse_body_with_line(expr: &SExpr, line: usize) -> Result<Vec<Literal>, ParseError> {
    match expr {
        SExpr::Atom(_) => Ok(vec![parse_literal_with_line(expr, line)?]),
        SExpr::List(items) => {
            if items.is_empty() {
                return Ok(vec![]);
            }

            // Check for (and ...)
            if let Some("and") = items[0].as_atom() {
                items[1..]
                    .iter()
                    .map(|item| parse_literal_with_line(item, line))
                    .collect()
            } else {
                // Single complex literal
                Ok(vec![parse_literal_with_line(expr, line)?])
            }
        }
    }
}

/// Parse a literal expression with line number tracking
fn parse_literal_with_line(expr: &SExpr, line: usize) -> Result<Literal, ParseError> {
    match expr {
        SExpr::Atom(s) => {
            // Handle double-negation: ~~name -> positive name
            if let Some(name) = s.strip_prefix("~~") {
                if name.is_empty() {
                    return Err(ParseError::ParserError {
                        line,
                        message: "Double negation with empty name".to_string(),
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
        SExpr::List(items) => {
            if items.is_empty() {
                return Err(ParseError::ParserError {
                    line,
                    message: "Empty list is not a valid literal".to_string(),
                });
            }

            let first = items[0].as_atom().ok_or_else(|| ParseError::ParserError {
                line,
                message: "Expected atom in literal".to_string(),
            })?;

            match first {
                "not" => {
                    // (not literal)
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "not takes exactly one argument".to_string(),
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
                        });
                    }
                    let mut lit = parse_literal_with_line(&items[1], line)?;
                    lit.mode = Mode::forbidden();
                    Ok(lit)
                }
                "during" => {
                    // (during literal start end)
                    if items.len() != 4 {
                        return Err(ParseError::ParserError {
                            line,
                            message: "during takes exactly three arguments: literal, start, end"
                                .to_string(),
                        });
                    }
                    let mut lit = parse_literal_with_line(&items[1], line)?;
                    let start = parse_timepoint_with_line(&items[2], line)?;
                    let end = parse_timepoint_with_line(&items[3], line)?;
                    lit.temporal = Temporal::new(start, end);
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

fn parse_timepoint_with_line(expr: &SExpr, line: usize) -> Result<TimePoint, ParseError> {
    match expr {
        SExpr::Atom(s) => {
            if s == "-inf" {
                Ok(TimePoint::NegInf)
            } else if s == "inf" || s == "+inf" {
                Ok(TimePoint::PosInf)
            } else if let Ok(n) = s.parse::<i64>() {
                Ok(TimePoint::from_millis(n))
            } else {
                Err(ParseError::ParserError {
                    line,
                    message: format!("Invalid timepoint: {}", s),
                })
            }
        }
        SExpr::List(items) => {
            // (moment "RFC3339") only
            if items.len() == 2 && items[0].as_atom() == Some("moment") {
                if let Some(s) = items[1].as_atom() {
                    // RFC3339 parsing using chrono
                    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
                        return Ok(TimePoint::from_millis(dt.timestamp_millis()));
                    }

                    Err(ParseError::ParserError {
                        line,
                        message: format!("Invalid RFC3339 timepoint for moment: {}", s),
                    })
                } else {
                    Err(ParseError::ParserError {
                        line,
                        message: "moment argument must be atom".to_string(),
                    })
                }
            } else if items.len() >= 4 && items[0].as_atom() == Some("moment") {
                Err(ParseError::ParserError {
                    line,
                    message: "Multi-arity moment (YYYY MM DD ...) not yet supported".to_string(),
                })
            } else {
                Err(ParseError::ParserError {
                    line,
                    message: "Invalid timepoint expression".to_string(),
                })
            }
        }
    }
}

/// Process meta with line number
fn process_meta_with_line(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if args.is_empty() {
        return Err(ParseError::ParserError {
            line,
            message: "meta requires a label".to_string(),
        });
    }

    let label = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "meta label must be an atom".to_string(),
    })?;

    // Process each property: (key "value") or (key ("v1" "v2"))
    for prop in &args[1..] {
        if let Some(prop_list) = prop.as_list()
            && prop_list.len() >= 2
        {
            let key = prop_list[0]
                .as_atom()
                .ok_or_else(|| ParseError::ParserError {
                    line,
                    message: "meta property key must be an atom".to_string(),
                })?;

            // Check if value is a list or single value
            let value = match &prop_list[1] {
                SExpr::Atom(s) => MetaValue::String(s.clone()),
                SExpr::List(items) => {
                    // List of strings
                    let strings: Result<Vec<String>, _> = items
                        .iter()
                        .map(|item| {
                            item.as_atom().map(|s| s.to_string()).ok_or_else(|| {
                                ParseError::ParserError {
                                    line,
                                    message: "meta list values must be atoms".to_string(),
                                }
                            })
                        })
                        .collect();
                    MetaValue::List(strings?)
                }
            };

            theory.add_meta(label, key, value);
        }
    }

    Ok(())
}

/// Process prefer with line number
fn process_prefer_with_line(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if args.len() < 2 {
        return Err(ParseError::ParserError {
            line,
            message: "prefer requires at least two labels".to_string(),
        });
    }

    let labels: Result<Vec<&str>, _> = args
        .iter()
        .map(|a| {
            a.as_atom().ok_or_else(|| ParseError::ParserError {
                line,
                message: "prefer arguments must be atoms".to_string(),
            })
        })
        .collect();
    let labels = labels?;

    // Create chain of superiorities: r1 > r2 > r3 ...
    for window in labels.windows(2) {
        theory.add_superiority(window[0], window[1]);
    }

    Ok(())
}

/// Generate a unique label that doesn't collide with existing labels in the theory.
/// Uses a simple counter approach: prefix1, prefix2, prefix3, etc.
/// If a label already exists, keeps incrementing until a free slot is found.
fn generate_unique_label(theory: &Theory, prefix: &str) -> String {
    let mut counter = theory.rule_count() + 1;
    loop {
        let label = format!("{prefix}{counter}");
        if theory.get_rule(&label).is_none() {
            return label;
        }
        counter += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // BASIC PARSING TESTS (existing)
    // =========================================================================

    #[test]
    fn test_parse_simple_fact() {
        let theory = parse_spl("(given bird)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_predicate_fact() {
        let theory = parse_spl("(given (parent alice bob))").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_flat_predicate_fact() {
        let theory = parse_spl("(given employed alice acme)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_defeasible_rule() {
        let theory = parse_spl("(normally r1 bird flies)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_strict_rule() {
        let theory = parse_spl("(always r1 bird animal)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_negated_head() {
        let theory = parse_spl("(normally r1 penguin (not flies))").unwrap();
        let rule = theory.rules().next().unwrap();
        assert!(rule.head_literal().is_negated());
    }

    #[test]
    fn test_parse_superiority() {
        let theory = parse_spl("(given a)\n(given b)\n(prefer r2 r1)").unwrap();
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_parse_superiority_chain() {
        let theory = parse_spl("(prefer r3 r2 r1)").unwrap();
        assert_eq!(theory.superiorities().len(), 2);
    }

    #[test]
    fn test_parse_conjunction() {
        let theory = parse_spl("(normally r1 (and bird healthy) flies)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.body.len(), 2);
    }

    #[test]
    fn test_parse_nested_during() {
        let theory = parse_spl("(given (during (employed alice) 10 20))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert_eq!(lit.name(), "employed");
        assert!(lit.is_temporal());
        assert_eq!(lit.temporal.start, TimePoint::from_millis(10));
        assert_eq!(lit.temporal.end, TimePoint::from_millis(20));
    }

    #[test]
    fn test_parse_nested_must() {
        let theory = parse_spl("(given (must (pay alice)))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert_eq!(lit.name(), "pay");
        assert!(lit.is_modal());
        assert_eq!(lit.mode.name, Some("O".to_string()));
    }

    #[test]
    fn test_parse_moment_rfc3339() {
        // 2024-01-01T00:00:00Z is 1704067200000 millis
        let theory = parse_spl("(given (during p (moment \"2024-01-01T00:00:00Z\") inf))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();

        if let TimePoint::Moment(millis) = lit.temporal.start {
            assert_eq!(millis, 1704067200000);
        } else {
            panic!(
                "Expected Moment for start time, got {:?}",
                lit.temporal.start
            );
        }
    }

    #[test]
    fn test_reject_ambiguous_moment_numeric() {
        let err = parse_spl("(given (during p (moment 2020) inf))").unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("moment"),
            "Expected error mentioning moment for ambiguous numeric form"
        );
    }

    // =========================================================================
    // REGRESSION TESTS - Bug Hunt Fixes
    // =========================================================================

    #[test]
    fn test_claims_keyword_basic() {
        // Regression: SPL parser should handle 'claims' keyword
        let input = r#"(claims agent:alice :at "2024-01-01T00:00:00Z" (given bird))"#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 1);

        // Should have source metadata
        let fact = theory.facts().next().unwrap();
        let meta = theory.get_meta(&fact.label);
        assert!(meta.is_some(), "Claims should attach source metadata");
    }

    #[test]
    fn test_claims_keyword_multiple_facts() {
        let input = r#"(claims agent:bob (given sunny) (given warm))"#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);
    }

    #[test]
    fn test_auto_label_no_collision() {
        // Regression: auto-labels should not collide with user-provided labels
        let input = "(normally r3 bird flies)\n(normally bird2 animal)";
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);

        // Both rules should have distinct labels
        let labels: Vec<_> = theory.rules().map(|r| r.label.clone()).collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(labels.len(), unique.len(), "Labels should be unique");
    }

    #[test]
    fn test_double_negation_stripped() {
        // Regression: ~~name should be treated as positive literal
        let theory = parse_spl("(given ~~bird)").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert!(
            !lit.is_negated(),
            "~~bird should be positive (double negation cancels)"
        );
        assert_eq!(lit.name(), "bird");
    }

    #[test]
    fn test_double_negation_in_rule() {
        let theory = parse_spl("(normally r1 ~~p q)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert!(!rule.body[0].is_negated(), "~~p in body should be positive");
    }

    #[test]
    fn test_line_numbers_in_errors() {
        // Regression: parser errors should report actual line numbers, not always line 1
        let input = "(given bird)\n(given fish)\n(bogus_keyword something)";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        // The error is on line 3
        assert!(
            message.contains("3") || message.contains("bogus_keyword"),
            "Error should reference line 3 or the bad keyword, got: {message}"
        );
    }

    #[test]
    fn test_line_numbers_not_always_one() {
        // More specific test: an error late in the file should NOT say line 1
        let input = "(given a)\n(given b)\n(given c)\n(given d)\n(bad)";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        // Should mention line 5 (or at least not line 1)
        assert!(
            !message.contains("line 1") || message.contains("line 5"),
            "Error on line 5 should not report line 1, got: {message}"
        );
    }

    // =========================================================================
    // Reviewer Fix Tests
    // =========================================================================

    #[test]
    fn test_claims_timestamp_attached() {
        // Fix 2: verify "timestamp" metadata key is set when :at is provided
        let input = r#"(claims agent:alice :at "2024-06-15T12:00:00Z" (given sunny))"#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 1);

        let fact = theory.facts().next().unwrap();
        let meta = theory.get_meta(&fact.label);
        assert!(meta.is_some(), "Should have metadata");

        let meta_map = meta.unwrap();
        assert!(
            meta_map.properties.contains_key("timestamp"),
            "Should have 'timestamp' metadata key"
        );
        let ts_val = &meta_map.properties["timestamp"];
        match ts_val {
            MetaValue::String(s) => assert_eq!(s, "2024-06-15T12:00:00Z"),
            other => panic!("Expected String metadata for timestamp, got: {:?}", other),
        }

        assert!(
            meta_map.properties.contains_key("source"),
            "Should have 'source' metadata key"
        );
    }

    #[test]
    fn test_claims_prefer_no_wrong_metadata() {
        // Fix 3: claims block with prefer inside — metadata should only attach to actual rules
        let input = r#"(claims agent:bob (given bird) (normally r1 bird flies) (prefer r1 r2) (given fish))"#;
        let theory = parse_spl(input).unwrap();

        // Should have 3 rules: bird fact, r1, fish fact
        assert_eq!(theory.rule_count(), 3);

        // All 3 rules should have source metadata
        for rule in theory.rules() {
            let meta = theory.get_meta(&rule.label);
            assert!(
                meta.is_some() && meta.unwrap().properties.contains_key("source"),
                "Rule {} should have source metadata",
                rule.label
            );
        }

        // The prefer expression should NOT have attached metadata to the wrong rule.
        // Specifically, r1's source should be "agent:bob" (from claims), not duplicated.
        let r1_meta = theory.get_meta("r1").unwrap();
        match &r1_meta.properties["source"] {
            MetaValue::String(s) => assert_eq!(s, "agent:bob"),
            other => panic!("Expected String source for r1, got: {:?}", other),
        }
    }

    #[test]
    fn test_claims_at_requires_atom_timestamp() {
        let err = parse_spl("(claims agent:alice :at (given bird))").unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("claims :at requires a timestamp atom"),
            "Expected malformed :at value to fail, got: {message}"
        );
    }

    #[test]
    fn test_line_numbers_with_lang_directive() {
        let input = "#lang spindle\n(bogus something)";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("line 2"),
            "Expected error to report line 2 after #lang, got: {message}"
        );
    }

    #[test]
    fn test_claims_inner_expression_line_numbers() {
        let input = "(claims agent:alice\n  (given bird)\n  (bogus something))";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("line 3"),
            "Expected bad inner claims expression to report line 3, got: {message}"
        );
    }
}
