//! Expression dispatch and handler functions for SPL parsing.
//!
//! This module contains `process_expr_with_line()`, which dispatches top-level
//! S-expressions (given, always, normally, except, prefer, meta, claims,
//! trusts, decays, threshold) to their individual handler functions.
//!
//! Metadata-related handlers (meta, claims, trusts, decays, threshold) live in
//! the sibling `metadata` module.

use spindle_core::{Rule, RuleType, Theory};

use crate::ParseError;
use crate::error::ParserFormat;

use super::arith::{is_future_reserved_keyword, is_reserved_keyword};
use super::lexer::SExpr;
use super::lexer::source_line_text;
use super::literals::{parse_literal_with_line, parse_term_from_atom, parse_timepoint_with_line};
use super::metadata::{
    process_claims, process_decays, process_meta_with_line, process_threshold, process_trusts,
};
use super::rules::process_rule_with_line;

/// Enrich a `ParseError` with source line text if it is missing.
///
/// This is applied at the top-level expression dispatch so that errors from
/// sub-handlers (which may not have access to the cleaned input) still
/// include the offending source line in the error message.
fn enrich_error(err: ParseError, cleaned_input: &str) -> ParseError {
    match err {
        ParseError::ParserError {
            line,
            message,
            format,
            source_line: None,
        } => ParseError::ParserError {
            line,
            message,
            format,
            source_line: source_line_text(cleaned_input, line),
        },
        other => other,
    }
}

/// Process an s-expression into theory elements (with line number)
pub(crate) fn process_expr_with_line(
    theory: &mut Theory,
    expr: &SExpr,
    line: usize,
    cleaned_input: &str,
) -> Result<(), ParseError> {
    let list = expr.as_list().ok_or_else(|| ParseError::ParserError {
        line,
        message: "Expected list expression".to_string(),
        format: ParserFormat::Spl,
        source_line: source_line_text(cleaned_input, line),
    })?;

    if list.is_empty() {
        return Ok(());
    }

    let keyword = list[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "Expected keyword".to_string(),
        format: ParserFormat::Spl,
        source_line: source_line_text(cleaned_input, line),
    })?;

    let result = match keyword {
        "given" => process_fact_with_line(theory, &list[1..], line),
        "always" => process_rule_with_line(theory, RuleType::Strict, &list[1..], line),
        "normally" => process_rule_with_line(theory, RuleType::Defeasible, &list[1..], line),
        "except" => process_rule_with_line(theory, RuleType::Defeater, &list[1..], line),
        "during" => process_during_rule_with_line(theory, &list[1..], line, cleaned_input),
        "prefer" => process_prefer_with_line(theory, &list[1..], line),
        "predicate" => {
            super::predicate::process_predicate_declaration(theory, &list[1..], line, expr.offset())
        }
        "meta" => process_meta_with_line(theory, &list[1..], line),
        "claims" => process_claims(theory, &list[1..], line, cleaned_input),
        "trusts" => process_trusts(theory, &list[1..], line),
        "decays" => process_decays(theory, &list[1..], line),
        "threshold" => process_threshold(theory, &list[1..], line),
        "#lang" => Ok(()), // Ignore #lang directive
        _ => Err(ParseError::ParserError {
            line,
            message: format!("Unknown keyword: {keyword}"),
            format: ParserFormat::Spl,
            source_line: source_line_text(cleaned_input, line),
        }),
    };

    result.map_err(|e| enrich_error(e, cleaned_input))
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
            format: ParserFormat::Spl,
            source_line: None,
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
                let terms: Result<Vec<spindle_core::Term>, _> = args[1..]
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
                spindle_core::Literal::from_ids(
                    spindle_core::intern::intern(name),
                    false,
                    Default::default(),
                    Default::default(),
                    terms?,
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
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let labels: Result<Vec<&str>, _> = args
        .iter()
        .map(|a| {
            a.as_atom().ok_or_else(|| ParseError::ParserError {
                line,
                message: "prefer arguments must be atoms".to_string(),
                format: ParserFormat::Spl,
                source_line: None,
            })
        })
        .collect();
    let labels = labels?;

    // REQ-008: Validate that no reserved keywords are used as labels in prefer
    for label in &labels {
        if is_reserved_keyword(label) {
            return Err(ParseError::ParserError {
                line,
                message: format!(
                    "Reserved keyword '{label}' cannot be used as a rule label in prefer declaration (REQ-008)"
                ),
                format: ParserFormat::Spl,
                source_line: None,
            });
        }
        if is_future_reserved_keyword(label) {
            return Err(ParseError::ParserError {
                line,
                message: format!(
                    "'{label}' is reserved for future use and cannot be used as a rule label in prefer declaration (REQ-008)"
                ),
                format: ParserFormat::Spl,
                source_line: None,
            });
        }
    }

    // Create chain of superiorities: r1 > r2 > r3 ...
    for window in labels.windows(2) {
        theory.add_superiority(window[0], window[1]);
    }

    Ok(())
}

/// Process a temporal wrapper around a rule or fact:
/// `(during (normally label body head) start end)`
/// `(during (given literal) start end)`
fn process_during_rule_with_line(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
    cleaned_input: &str,
) -> Result<(), ParseError> {
    // Need at least 3 args: inner-expr, start, end
    if args.len() != 3 {
        return Err(ParseError::ParserError {
            line,
            message: "temporal rule wrapper (during ...) requires exactly 3 arguments: rule-expr start end".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let inner = &args[0];
    let inner_list = inner.as_list().ok_or_else(|| ParseError::ParserError {
        line,
        message: "(during ...) first argument must be a rule expression like (normally ...)"
            .to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    if inner_list.is_empty() {
        return Err(ParseError::ParserError {
            line,
            message: "(during ...) inner expression cannot be empty".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let inner_keyword = inner_list[0]
        .as_atom()
        .ok_or_else(|| ParseError::ParserError {
            line,
            message: "(during ...) inner expression must start with a keyword".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        })?;

    // Parse start and end timepoints
    let start = parse_timepoint_with_line(&args[1], line)?;
    let end = parse_timepoint_with_line(&args[2], line)?;
    let temporal = spindle_core::temporal::Temporal::new(start, end);

    // Collect existing labels so we can find the newly added one
    let existing_labels: std::collections::HashSet<String> =
        theory.rules().map(|r| r.label.clone()).collect();

    match inner_keyword {
        "given" => process_fact_with_line(theory, &inner_list[1..], line)?,
        "always" => process_rule_with_line(theory, RuleType::Strict, &inner_list[1..], line)?,
        "normally" => process_rule_with_line(theory, RuleType::Defeasible, &inner_list[1..], line)?,
        "except" => process_rule_with_line(theory, RuleType::Defeater, &inner_list[1..], line)?,
        _ => {
            return Err(ParseError::ParserError {
                line,
                message: format!(
                    "(during ...) inner expression must be given/always/normally/except, got '{inner_keyword}'"
                ),
                format: ParserFormat::Spl,
                source_line: source_line_text(cleaned_input, line),
            });
        }
    }

    // Find and update the newly added rule's temporal bounds
    let new_label = theory
        .rules()
        .find(|r| !existing_labels.contains(&r.label))
        .map(|r| r.label.clone());
    if let Some(label) = new_label
        && let Some(rule) = theory.get_rule_mut(&label)
    {
        rule.temporal = temporal;
    }

    Ok(())
}

/// Generate a unique label that doesn't collide with existing labels in the theory.
/// Uses a simple counter approach: prefix1, prefix2, prefix3, etc.
/// If a label already exists, keeps incrementing until a free slot is found.
pub(super) fn generate_unique_label(theory: &Theory, prefix: &str) -> String {
    let mut counter = theory.rule_count() + 1;
    loop {
        let label = format!("{prefix}{counter}");
        if theory.get_rule(&label).is_none() {
            return label;
        }
        counter += 1;
    }
}
