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
    IResult, Parser,
    branch::alt,
    bytes::complete::{tag, take_while1},
    character::complete::{char, space0},
    combinator::{map, opt},
    multi::separated_list0,
    sequence::{delimited, terminated},
};

use spindle_core::{Literal, Rule, RuleType, Superiority, Theory};
use spindle_core::trust::DecayModel;

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

        // Handle directives
        if line.starts_with('@') {
            parse_directive(&mut theory, line, line_num + 1)?;
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
            format: crate::error::ParserFormat::Dfl,
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

/// Parse a directive line starting with @
fn parse_directive(theory: &mut Theory, line: &str, line_num: usize) -> Result<(), ParseError> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    match parts.first().copied() {
        Some("@trust") => {
            if parts.len() != 3 {
                return Err(ParseError::ParserError {
                    line: line_num,
                    message: "@trust requires: @trust <source> <value>".to_string(),
                    format: crate::error::ParserFormat::Dfl,
                });
            }
            let source = parts[1];
            let value: f64 = parts[2].parse().map_err(|_| ParseError::ParserError {
                line: line_num,
                message: format!("@trust value must be a number in [0.0, 1.0], got: {}", parts[2]),
                format: crate::error::ParserFormat::Dfl,
            })?;
            if !(0.0..=1.0).contains(&value) {
                return Err(ParseError::ParserError {
                    line: line_num,
                    message: format!("@trust value must be in [0.0, 1.0], got: {value}"),
                    format: crate::error::ParserFormat::Dfl,
                });
            }
            theory.trust_policy_mut().trust_map.insert(source.to_string(), value);
            Ok(())
        }
        Some("@decay") => {
            if parts.len() != 4 {
                return Err(ParseError::ParserError {
                    line: line_num,
                    message: "@decay requires: @decay <source> <model> <param>".to_string(),
                    format: crate::error::ParserFormat::Dfl,
                });
            }
            let source = parts[1];
            let model_name = parts[2];
            let param: f64 = parts[3].parse().map_err(|_| ParseError::ParserError {
                line: line_num,
                message: format!("@decay parameter must be a number, got: {}", parts[3]),
                format: crate::error::ParserFormat::Dfl,
            })?;
            let model = match model_name {
                "exponential" => DecayModel::Exponential { half_life_secs: param },
                "linear" => DecayModel::Linear { rate_per_sec: param },
                "step" => DecayModel::StepFunction { cutoff_secs: param },
                _ => {
                    return Err(ParseError::ParserError {
                        line: line_num,
                        message: format!("Unknown decay model: {model_name}. Expected: exponential, linear, or step"),
                        format: crate::error::ParserFormat::Dfl,
                    });
                }
            };
            theory.trust_policy_mut().decay_map.insert(source.to_string(), model);
            Ok(())
        }
        Some("@threshold") => {
            if parts.len() != 3 {
                return Err(ParseError::ParserError {
                    line: line_num,
                    message: "@threshold requires: @threshold <name> <value>".to_string(),
                    format: crate::error::ParserFormat::Dfl,
                });
            }
            let name = parts[1];
            let value: f64 = parts[2].parse().map_err(|_| ParseError::ParserError {
                line: line_num,
                message: format!("@threshold value must be a number in [0.0, 1.0], got: {}", parts[2]),
                format: crate::error::ParserFormat::Dfl,
            })?;
            if !(0.0..=1.0).contains(&value) {
                return Err(ParseError::ParserError {
                    line: line_num,
                    message: format!("@threshold value must be in [0.0, 1.0], got: {value}"),
                    format: crate::error::ParserFormat::Dfl,
                });
            }
            theory.trust_policy_mut().thresholds.insert(name.to_string(), value);
            Ok(())
        }
        Some(directive) => Err(ParseError::ParserError {
            line: line_num,
            message: format!("Unknown directive: {directive}"),
            format: crate::error::ParserFormat::Dfl,
        }),
        None => Ok(()),
    }
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
        assert!(format!("{err:?}").contains("could not parse"));
    }

    #[test]
    fn test_parse_error_invalid_line_with_valid() {
        // Valid lines followed by invalid
        let result = parse_dfl("f1: >> bird\ninvalid_line");
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Should report line 2
        assert!(format!("{err:?}").contains("2"));
    }

    // =========================================================================
    // Trust Directive Tests
    // =========================================================================

    #[test]
    fn test_dfl_trust_directive() {
        let input = "@trust agent:coder 0.9\nf1: >> bird";
        let theory = parse_dfl(input).unwrap();
        assert_eq!(theory.trust_policy().get_trust("agent:coder"), 0.9);
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_dfl_trust_multiple() {
        let input = "@trust agent:coder 0.9\n@trust agent:security 0.95\n@trust system:policy 1.0";
        let theory = parse_dfl(input).unwrap();
        assert_eq!(theory.trust_policy().get_trust("agent:coder"), 0.9);
        assert_eq!(theory.trust_policy().get_trust("agent:security"), 0.95);
        assert_eq!(theory.trust_policy().get_trust("system:policy"), 1.0);
    }

    #[test]
    fn test_dfl_trust_invalid_value() {
        let err = parse_dfl("@trust agent:coder 1.5").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("[0.0, 1.0]"), "Should reject out-of-range, got: {msg}");
    }

    #[test]
    fn test_dfl_threshold_directive() {
        let input = "@threshold action 0.7\n@threshold warn 0.5";
        let theory = parse_dfl(input).unwrap();
        assert_eq!(theory.trust_policy().is_above_threshold(0.8, "action"), Some(true));
        assert_eq!(theory.trust_policy().is_above_threshold(0.6, "action"), Some(false));
        assert_eq!(theory.trust_policy().is_above_threshold(0.6, "warn"), Some(true));
    }

    #[test]
    fn test_dfl_decay_exponential() {
        let input = "@decay agent:coder exponential 3600.0";
        let theory = parse_dfl(input).unwrap();
        assert!(theory.trust_policy().decay_map.contains_key("agent:coder"));
    }

    #[test]
    fn test_dfl_decay_linear() {
        let input = "@decay agent:sensor linear 0.001";
        let theory = parse_dfl(input).unwrap();
        assert!(theory.trust_policy().decay_map.contains_key("agent:sensor"));
    }

    #[test]
    fn test_dfl_decay_step() {
        let input = "@decay agent:temp step 86400.0";
        let theory = parse_dfl(input).unwrap();
        assert!(theory.trust_policy().decay_map.contains_key("agent:temp"));
    }

    #[test]
    fn test_dfl_decay_unknown_model() {
        let err = parse_dfl("@decay agent:x quadratic 1.0").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Unknown decay model"), "Got: {msg}");
    }

    #[test]
    fn test_dfl_trust_with_rules() {
        let input = r#"
@trust agent:coder 0.9
@threshold action 0.7
@decay agent:coder exponential 3600.0

f1: >> bird
r1: bird => flies
r2: penguin => -flies
r2 > r1
"#;
        let theory = parse_dfl(input).unwrap();
        assert_eq!(theory.rule_count(), 3);
        assert_eq!(theory.superiorities().len(), 1);
        assert_eq!(theory.trust_policy().get_trust("agent:coder"), 0.9);
        assert_eq!(theory.trust_policy().is_above_threshold(0.8, "action"), Some(true));
    }

    #[test]
    fn test_dfl_unknown_directive() {
        let err = parse_dfl("@unknown something").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Unknown directive"), "Got: {msg}");
    }

    #[test]
    fn test_dfl_trust_missing_args() {
        let err = parse_dfl("@trust agent:coder").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("@trust requires"), "Got: {msg}");
    }

    #[test]
    fn test_dfl_threshold_missing_args() {
        let err = parse_dfl("@threshold action").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("@threshold requires"), "Got: {msg}");
    }

    #[test]
    fn test_dfl_decay_missing_args() {
        let err = parse_dfl("@decay agent:coder exponential").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("@decay requires"), "Got: {msg}");
    }
}
