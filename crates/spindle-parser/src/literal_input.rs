//! Literal input shared by the command-line and JavaScript interfaces.

use spindle_core::Literal;

/// Parse one ground literal in SPL or legacy `p(a, b)` notation.
/// SPL preserves numeric types, quoted symbols, modes and temporal windows.
/// Legacy comma notation accepts atomic arguments; use SPL for nested forms.
pub fn parse_literal_input(input: &str) -> Result<Literal, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("expected a literal".into());
    }
    let expression = if let Some(rest) = input.strip_prefix('~').or_else(|| input.strip_prefix('-'))
    {
        format!("(not {})", legacy_expression(rest)?)
    } else {
        legacy_expression(input)?
    };
    let cleaned = crate::spl::lexer::remove_comments(&expression);
    let (remaining, expressions) = crate::spl::lexer::parse_expressions_with_positions(&cleaned)
        .map_err(|e| format!("invalid literal: {e}"))?;
    if !remaining.trim().is_empty() || expressions.len() != 1 {
        return Err("expected exactly one complete literal".into());
    }
    let literal = crate::spl::literals::parse_literal_with_line(&expressions[0].0, 1)
        .map_err(|e| e.to_string())?;
    if spindle_core::GroundLiteral::try_from(literal.clone()).is_err() {
        return Err("expected a ground literal, not a variable pattern".into());
    }
    Ok(literal)
}

fn legacy_expression(input: &str) -> Result<String, String> {
    if !input.starts_with(['(', '"'])
        && let Some(pos) = input.find('(')
    {
        let args = input[pos + 1..]
            .strip_suffix(')')
            .ok_or("unclosed literal")?;
        // Split commas only outside quoted strings.
        let mut quoted = false;
        let mut escaped = false;
        let mut converted = String::new();
        for c in args.chars() {
            if escaped {
                escaped = false;
            } else if c == '\\' && quoted {
                escaped = true;
            } else if c == '"' {
                quoted = !quoted;
            } else if c == ',' && !quoted {
                converted.push(' ');
                continue;
            }
            converted.push(c);
        }
        return Ok(format!("({} {converted})", &input[..pos]));
    }
    Ok(input.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use spindle_core::Term;

    #[test]
    fn preserves_types_and_windows() {
        let literal = parse_literal_input("(during (p 2) 100 200)").unwrap();
        assert_eq!(literal.predicate_args(), vec![Term::Integer(2)]);
        assert_eq!(
            literal.temporal.start,
            spindle_core::TimePoint::from_millis(100)
        );
        assert_eq!(
            parse_literal_input("p(2)").unwrap().predicate_args(),
            vec![Term::Integer(2)]
        );
        assert!(parse_literal_input("~p(2)").unwrap().negation);
        assert_eq!(parse_literal_input("\"a(b\"").unwrap().name(), "a(b");
        assert_eq!(
            parse_literal_input("p(\"a,b\", 2)")
                .unwrap()
                .predicates()
                .len(),
            2
        );
    }

    #[test]
    fn rejects_malformed_or_multiple_literals() {
        for input in [
            "",
            "(p",
            "p) (given q",
            "(p ?x)",
            "p) (predicate x ()",
            "p q",
        ] {
            assert!(parse_literal_input(input).is_err(), "{input}");
        }
    }
}
