//! Aggregate expressions and explicit finite-domain declarations.
use super::arith::parse_value_expr;
use super::lexer::SExpr;
use super::literals::{parse_literal_with_line, parse_term_from_atom};
use crate::{ParseError, error::ParserFormat};
use spindle_core::{
    Theory,
    arith::{ArithExpr, FoldExpr},
};

fn error(line: usize, message: impl Into<String>) -> ParseError {
    ParseError::ParserError {
        line,
        message: message.into(),
        format: ParserFormat::Spl,
        source_line: None,
    }
}

pub(super) fn parse_fold(args: &[SExpr], line: usize) -> Result<ArithExpr, ParseError> {
    if args.len() < 5 {
        return Err(error(
            line,
            "fold expects reducer, extraction, :from pattern, and :initial value or :require-nonempty",
        ));
    }
    let reducer = args[0]
        .as_atom()
        .ok_or_else(|| error(line, "fold reducer must be a name"))?;
    if !["+", "sum", "min", "max"].contains(&reducer) {
        return Err(error(line, "fold reducer must be +/sum, min, or max"));
    }
    let extract = parse_value_expr(&args[1], line)?;
    let mut pattern = None;
    let mut initial = None;
    let mut empty_policy = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_atom() {
            Some(":from") if pattern.is_none() && i + 1 < args.len() => {
                pattern = Some(parse_literal_with_line(&args[i + 1], line)?);
                i += 2;
            }
            Some(":initial") if !empty_policy && i + 1 < args.len() => {
                initial = Some(parse_value_expr(&args[i + 1], line)?);
                empty_policy = true;
                i += 2;
            }
            Some(":require-nonempty") if !empty_policy => {
                empty_policy = true;
                i += 1;
            }
            _ => return Err(error(line, "invalid, duplicate, or incomplete fold option")),
        }
    }
    if !empty_policy {
        return Err(error(line, "fold requires an explicit empty-input policy"));
    }
    if extract.contains_fold() || initial.as_ref().is_some_and(ArithExpr::contains_fold) {
        return Err(error(
            line,
            "nested folds are not supported; use a separate rule and stratum",
        ));
    }
    Ok(ArithExpr::Fold(Box::new(FoldExpr {
        aggregator: None,
        reducer: reducer.into(),
        extract,
        initial,
        pattern: pattern.ok_or_else(|| error(line, "fold requires :from pattern"))?,
    })))
}

pub(super) fn parse_domain(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if theory.aggregate_domain().is_some() {
        return Err(error(line, "duplicate aggregate-domain declaration"));
    }
    let domain = args
        .iter()
        .map(|a| {
            let atom = a.as_atom().ok_or_else(|| {
                error(line, "aggregate-domain expects ground integers or symbols")
            })?;
            let term = parse_term_from_atom(atom, line)?;
            spindle_core::aggregation::source::term(&term)
                .map_err(|e| error(line, e))
                .and_then(|t| {
                    if matches!(t, spindle_core::aggregation::Term::Variable(_)) {
                        Err(error(line, "aggregate-domain must be ground"))
                    } else {
                        Ok(t)
                    }
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    theory.set_aggregate_domain(domain);
    Ok(())
}

/// A named aggregate is a direct output-binding premise.
pub(super) fn parse_agg(
    args: &[SExpr],
    line: usize,
) -> Result<spindle_core::arith::ArithConstraint, ParseError> {
    if args.len() != 4 {
        return Err(error(
            line,
            "agg expects (agg ?result name ?value (relation ...)); use a helper relation for joins",
        ));
    }
    let result = args[0]
        .as_atom()
        .filter(|v| v.starts_with('?') && v.len() > 1)
        .ok_or_else(|| error(line, "agg result must be a variable such as ?total"))?;
    let name = args[1]
        .as_atom()
        .filter(|v| !v.starts_with('?'))
        .ok_or_else(|| error(line, "agg aggregator must be a name such as sum"))?;
    let input = args[2]
        .as_atom()
        .filter(|v| v.starts_with('?') && v.len() > 1)
        .ok_or_else(|| error(line, "agg contribution must be a variable such as ?amount"))?;
    let pattern = parse_literal_with_line(&args[3], line)?;
    if !pattern
        .predicate_args()
        .iter()
        .any(|t| t.to_string() == input)
    {
        return Err(error(
            line,
            "agg contribution variable must occur in its row pattern",
        ));
    }
    Ok(spindle_core::arith::ArithConstraint::Bind {
        var: spindle_core::intern::intern(result),
        expr: ArithExpr::Fold(Box::new(FoldExpr {
            aggregator: Some(name.into()),
            reducer: String::new(),
            extract: ArithExpr::Var(spindle_core::intern::intern(input)),
            pattern,
            initial: None,
        })),
    })
}

#[cfg(test)]
mod tests {
    use crate::parse_spl;

    #[test]
    fn aggregate_premise_round_trips() {
        let theory =
            parse_spl("(normally r (agg ?total sum ?v (amount ?v)) (total ?total))").unwrap();
        let rule = theory.get_rule("r").unwrap();
        assert_eq!(rule.body[0].to_string(), "(agg ?total sum ?v (amount ?v))");
        let reparsed = parse_spl(&format!("(normally r {} (total ?total))", rule.body[0])).unwrap();
        assert_eq!(rule.body, reparsed.get_rule("r").unwrap().body);
    }

    #[test]
    fn aggregate_forms_reject_invalid_bindings_and_positions() {
        for source in [
            "(given (agg ?n sum ?v (amount ?v)))",
            "(normally r p (agg ?n sum ?v (amount ?v)))",
            "(normally r (not (agg ?n sum ?v (amount ?v))) p)",
            "(normally r (agg n sum ?v (amount ?v)) p)",
            "(normally r (agg ?n sum 1 (amount ?v)) p)",
            "(normally r (agg ?n sum ?missing (amount ?v)) p)",
            "(normally r (agg ?n sum ?v (amount ?v) (other ?v)) p)",
            "(normally r (agg ?n sum ?v) p)",
        ] {
            assert!(parse_spl(source).is_err(), "accepted {source}");
        }
    }
}
