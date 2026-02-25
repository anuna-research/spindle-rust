//! Rule parsing for SPL: always, normally, except forms.

use spindle_core::rule::RuleBody;
use spindle_core::{Rule, RuleType, Theory};

use crate::ParseError;
use crate::error::ParserFormat;

use super::expressions::generate_unique_label;
use super::lexer::SExpr;
use super::literals::{parse_body_with_line, parse_literal_with_line};

/// Process a rule with line number: (always/normally/except [label] body head)
pub(crate) fn process_rule_with_line(
    theory: &mut Theory,
    rule_type: RuleType,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if args.len() < 2 {
        return Err(ParseError::ParserError {
            line,
            message: "rule requires at least body and head".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
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

    let (body, constraints, state_queries) = parse_body_with_line(body_expr, line)?;
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
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let body: RuleBody = body.into_iter().collect();
    let mut rule = Rule::new(final_label, rule_type, body, vec![head]);
    rule.constraints = constraints;
    rule.state_queries = state_queries;
    theory.add_rule(rule);
    Ok(())
}
