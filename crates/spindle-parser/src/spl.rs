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
    branch::alt,
    bytes::complete::take_while1,
    character::complete::{char, multispace0},
    multi::many0,
    sequence::{delimited, preceded},
    IResult, Parser,
};

use spindle_core::{Literal, MetaValue, Rule, RuleType, Theory};

use crate::ParseError;

/// Parse an SPL string into a Theory
pub fn parse_spl(input: &str) -> Result<Theory, ParseError> {
    // Remove comments
    let cleaned = remove_comments(input);

    let mut theory = Theory::new();

    // Parse all top-level expressions
    let (remaining, exprs) = parse_expressions(&cleaned).map_err(|e| ParseError::ParserError {
        line: 1,
        message: format!("SPL parse error: {:?}", e),
    })?;

    if !remaining.trim().is_empty() {
        return Err(ParseError::ParserError {
            line: 1,
            message: format!("Unparsed input remaining: {}", remaining.trim()),
        });
    }

    // Process expressions
    for expr in exprs {
        process_expr(&mut theory, &expr)?;
    }

    Ok(theory)
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

/// Parse multiple expressions
fn parse_expressions(input: &str) -> IResult<&str, Vec<SExpr>> {
    many0(preceded(multispace0, parse_sexpr)).parse(input)
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
    let (input, content) = take_while1(|c| c != '"').parse(input)?;
    let (input, _) = char('"').parse(input)?;
    Ok((input, SExpr::Atom(content.to_string())))
}

/// Parse an atom (identifier, number, variable)
fn parse_atom(input: &str) -> IResult<&str, SExpr> {
    let (input, s) = take_while1(|c: char| {
        c.is_alphanumeric() || c == '-' || c == '_' || c == '?' || c == '~' || c == ':' || c == '.'
    })
    .parse(input)?;
    Ok((input, SExpr::Atom(s.to_string())))
}

/// Process an s-expression into theory elements
fn process_expr(theory: &mut Theory, expr: &SExpr) -> Result<(), ParseError> {
    let list = expr.as_list().ok_or_else(|| ParseError::ParserError {
        line: 1,
        message: "Expected list expression".to_string(),
    })?;

    if list.is_empty() {
        return Ok(());
    }

    let keyword = list[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line: 1,
        message: "Expected keyword".to_string(),
    })?;

    match keyword {
        "given" => process_fact(theory, &list[1..]),
        "always" => process_rule(theory, RuleType::Strict, &list[1..]),
        "normally" => process_rule(theory, RuleType::Defeasible, &list[1..]),
        "except" => process_rule(theory, RuleType::Defeater, &list[1..]),
        "prefer" => process_prefer(theory, &list[1..]),
        "meta" => process_meta(theory, &list[1..]),
        "#lang" => Ok(()), // Ignore #lang directive
        _ => Err(ParseError::ParserError {
            line: 1,
            message: format!("Unknown keyword: {}", keyword),
        }),
    }
}

/// Process a fact: (given literal)
fn process_fact(theory: &mut Theory, args: &[SExpr]) -> Result<(), ParseError> {
    if args.is_empty() {
        return Err(ParseError::ParserError {
            line: 1,
            message: "fact requires at least one argument".to_string(),
        });
    }

    let lit = parse_literal(&args[0])?;

    // Handle flat predicate: (given pred arg1 arg2 ...)
    let lit = if args.len() > 1 {
        let name = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
            line: 1,
            message: "Expected predicate name".to_string(),
        })?;
        let predicates: Result<Vec<String>, _> = args[1..]
            .iter()
            .map(|a| {
                a.as_atom()
                    .map(|s| s.to_string())
                    .ok_or_else(|| ParseError::ParserError {
                        line: 1,
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
    } else {
        lit
    };

    let label = theory.next_fact_label();
    let rule = Rule::fact(label, lit);
    theory.add_rule(rule);
    Ok(())
}

/// Process a rule: (always/normally/except [label] body head)
fn process_rule(
    theory: &mut Theory,
    rule_type: RuleType,
    args: &[SExpr],
) -> Result<(), ParseError> {
    if args.len() < 2 {
        return Err(ParseError::ParserError {
            line: 1,
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

    let body = parse_body(body_expr)?;
    let head = parse_literal(head_expr)?;

    let final_label = label.unwrap_or_else(|| theory.next_rule_label(rule_type));
    let rule = Rule::new(final_label, rule_type, body, vec![head]);
    theory.add_rule(rule);
    Ok(())
}

/// Parse a body expression (single literal or conjunction)
fn parse_body(expr: &SExpr) -> Result<Vec<Literal>, ParseError> {
    match expr {
        SExpr::Atom(_) => Ok(vec![parse_literal(expr)?]),
        SExpr::List(items) => {
            if items.is_empty() {
                return Ok(vec![]);
            }

            // Check for (and ...)
            if let Some("and") = items[0].as_atom() {
                items[1..].iter().map(parse_literal).collect()
            } else {
                // Single complex literal
                Ok(vec![parse_literal(expr)?])
            }
        }
    }
}

/// Parse a literal expression
fn parse_literal(expr: &SExpr) -> Result<Literal, ParseError> {
    match expr {
        SExpr::Atom(s) => {
            // Handle negation prefix
            if let Some(name) = s.strip_prefix('~') {
                Ok(Literal::negated(name))
            } else {
                Ok(Literal::simple(s))
            }
        }
        SExpr::List(items) => {
            if items.is_empty() {
                return Err(ParseError::ParserError {
                    line: 1,
                    message: "Empty list is not a valid literal".to_string(),
                });
            }

            let first = items[0].as_atom().ok_or_else(|| ParseError::ParserError {
                line: 1,
                message: "Expected atom in literal".to_string(),
            })?;

            match first {
                "not" => {
                    // (not literal)
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line: 1,
                            message: "not takes exactly one argument".to_string(),
                        });
                    }
                    let inner = parse_literal(&items[1])?;
                    Ok(inner.complement())
                }
                _ => {
                    // Predicate: (name arg1 arg2 ...)
                    let predicates: Result<Vec<String>, _> = items[1..]
                        .iter()
                        .map(|a| {
                            a.as_atom().map(|s| s.to_string()).ok_or_else(|| {
                                ParseError::ParserError {
                                    line: 1,
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

/// Process meta: (meta label (key "value") (key2 "value2") ...)
fn process_meta(theory: &mut Theory, args: &[SExpr]) -> Result<(), ParseError> {
    if args.is_empty() {
        return Err(ParseError::ParserError {
            line: 1,
            message: "meta requires a label".to_string(),
        });
    }

    let label = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line: 1,
        message: "meta label must be an atom".to_string(),
    })?;

    // Process each property: (key "value") or (key ("v1" "v2"))
    for prop in &args[1..] {
        if let Some(prop_list) = prop.as_list()
            && prop_list.len() >= 2 {
            let key = prop_list[0]
                .as_atom()
                .ok_or_else(|| ParseError::ParserError {
                    line: 1,
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
                                    line: 1,
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

/// Process prefer: (prefer r1 r2 ...)
fn process_prefer(theory: &mut Theory, args: &[SExpr]) -> Result<(), ParseError> {
    if args.len() < 2 {
        return Err(ParseError::ParserError {
            line: 1,
            message: "prefer requires at least two labels".to_string(),
        });
    }

    let labels: Result<Vec<&str>, _> = args
        .iter()
        .map(|a| {
            a.as_atom().ok_or_else(|| ParseError::ParserError {
                line: 1,
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

// Helper trait for Theory to generate labels
trait TheoryLabelExt {
    fn next_fact_label(&mut self) -> String;
    fn next_rule_label(&mut self, rule_type: RuleType) -> String;
}

impl TheoryLabelExt for Theory {
    fn next_fact_label(&mut self) -> String {
        let count = self.facts().count() + 1;
        format!("f{}", count)
    }

    fn next_rule_label(&mut self, rule_type: RuleType) -> String {
        let prefix = match rule_type {
            RuleType::Fact => "f",
            RuleType::Strict => "s",
            RuleType::Defeasible => "r",
            RuleType::Defeater => "d",
        };
        let count = self.rules_by_type(rule_type).count() + 1;
        format!("{}{}", prefix, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_parse_complete_theory() {
        let input = r#"
            ; Penguin example
            (given bird)
            (given penguin)
            (normally r1 bird flies)
            (normally r2 penguin (not flies))
            (prefer r2 r1)
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 4);
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_parse_with_variables() {
        let theory = parse_spl("(given (parent ?x ?y))").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.head_literal().predicates().len(), 2);
        assert!(rule.head_literal().predicates()[0].starts_with('?'));
    }
}
