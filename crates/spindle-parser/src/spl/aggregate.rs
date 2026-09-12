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
