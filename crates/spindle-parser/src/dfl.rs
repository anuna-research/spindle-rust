//! DFL (Defeasible Logic Format) parser
//!
//! Parses the textual DFL format into a Theory.
//!
//! # DFL Format
//!
//! ```text
//! # Comments start with #
//!
//! # Facts (>>)
//! f1: >> bird
//!
//! # Strict rules (->)
//! r1: bird -> animal
//!
//! # Defeasible rules (=>)
//! r2: bird => flies
//!
//! # Defeaters (~>)
//! d1: broken_wing ~> flies
//!
//! # Superiority (>)
//! r3 > r2
//!
//! # Negation (- or ~)
//! r3: penguin => -flies
//! ```

use nom::{
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, space0},
    combinator::{map, opt},
    multi::separated_list0,
    sequence::{delimited, terminated},
    IResult, Parser,
};

use spindle_core::{Literal, Rule, RuleType, Superiority, Theory};

use crate::ParseError;

/// Parse a DFL string into a Theory
pub fn parse_dfl(input: &str) -> Result<Theory, ParseError> {
    let mut theory = Theory::new();

    for (line_num, line) in input.lines().enumerate() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Try to parse as superiority first (no label)
        if let Ok((_, sup)) = parse_superiority(line) {
            theory.add_superiority(&sup.superior, &sup.inferior);
            continue;
        }

        // Try to parse as rule
        if let Ok((_, rule)) = parse_rule(line) {
            theory.add_rule(rule);
            continue;
        }

        return Err(ParseError::ParserError {
            line: line_num + 1,
            message: format!("could not parse: {line}"),
        });
    }

    Ok(theory)
}

/// Parse a rule label (identifier before colon)
fn parse_label(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-').parse(input)
}

/// Parse a literal name
fn parse_literal_name(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_' || c == '-').parse(input)
}

/// Parse a single literal (possibly negated)
fn parse_literal(input: &str) -> IResult<&str, Literal> {
    let (input, _) = space0.parse(input)?;
    let (input, negation) = opt(alt((char('-'), char('~'), char('¬')))).parse(input)?;
    let (input, name) = parse_literal_name(input)?;
    let (input, _) = space0.parse(input)?;

    Ok((
        input,
        if negation.is_some() {
            Literal::negated(name)
        } else {
            Literal::simple(name)
        },
    ))
}

/// Parse a comma-separated list of literals (the body)
fn parse_body(input: &str) -> IResult<&str, Vec<Literal>> {
    separated_list0(delimited(space0, char(','), space0), parse_literal).parse(input)
}

/// Parse the rule arrow and determine type
fn parse_arrow(input: &str) -> IResult<&str, RuleType> {
    let (input, _) = space0.parse(input)?;
    alt((
        map(tag(">>"), |_| RuleType::Fact),
        map(tag("->"), |_| RuleType::Strict),
        map(tag("=>"), |_| RuleType::Defeasible),
        map(tag("~>"), |_| RuleType::Defeater),
    ))
    .parse(input)
}

/// Parse a complete rule
fn parse_rule(input: &str) -> IResult<&str, Rule> {
    let (input, label) = parse_label(input)?;
    let (input, _) = space0.parse(input)?;
    let (input, _) = char(':').parse(input)?;
    let (input, _) = space0.parse(input)?;

    // Try to parse body (may be empty for facts)
    let (input, body) = opt(terminated(parse_body, space0)).parse(input)?;

    let (input, rule_type) = parse_arrow(input)?;
    let (input, _) = space0.parse(input)?;
    let (input, head) = parse_literal(input)?;

    let body = body.unwrap_or_default();

    Ok((input, Rule::new(label, rule_type, body, vec![head])))
}

/// Parse a superiority relation
fn parse_superiority(input: &str) -> IResult<&str, Superiority> {
    let (input, superior) = parse_label(input)?;
    let (input, _) = space0.parse(input)?;
    let (input, _) = char('>').parse(input)?;
    let (input, _) = space0.parse(input)?;
    let (input, inferior) = parse_label(input)?;

    Ok((input, Superiority::new(superior, inferior)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_fact() {
        let theory = parse_dfl("f1: >> bird").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_defeasible_rule() {
        let theory = parse_dfl("r1: bird => flies").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_negated_head() {
        let theory = parse_dfl("r1: penguin => -flies").unwrap();
        let rule = theory.rules().next().unwrap();
        assert!(rule.head_literal().is_negated());
    }

    #[test]
    fn test_parse_superiority() {
        let theory = parse_dfl("r1: >> a\nr2: >> b\nr2 > r1").unwrap();
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_parse_complex() {
        let input = r#"
# Penguin example
f1: >> bird
f2: >> penguin

r1: bird => flies
r2: penguin => -flies

r2 > r1
"#;
        let theory = parse_dfl(input).unwrap();
        assert_eq!(theory.rule_count(), 4);
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_parse_error_invalid_line() {
        // Line that can't be parsed as rule or superiority
        let result = parse_dfl("this is not valid dfl syntax");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{:?}", err).contains("could not parse"));
    }

    #[test]
    fn test_parse_error_invalid_line_with_valid() {
        // Valid lines followed by invalid
        let result = parse_dfl("f1: >> bird\ninvalid_line");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should report line 2
        assert!(format!("{:?}", err).contains("2"));
    }
}
