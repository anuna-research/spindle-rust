//! Metadata and claims parsing for SPL.
//!
//! This module contains handler functions for SPL directives that manage
//! metadata, provenance, trust, decay, and threshold configuration:
//! `meta`, `claims`, `trusts`, `decays`, `threshold`.

use spindle_core::trust::DecayModel;
use spindle_core::{MetaValue, Theory};

use crate::ParseError;
use crate::error::ParserFormat;

use super::lexer::{SExpr, line_of_offset};

/// Process a claims block: (claims source [:at "timestamp"] [:sig "signature"] [:id "block-id"] [:note "annotation"] (expr1) (expr2) ...)
pub(crate) fn process_claims(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
    cleaned_input: &str,
) -> Result<(), ParseError> {
    if args.is_empty() {
        return Err(ParseError::ParserError {
            line,
            message: "claims requires a source".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let source = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "claims source must be an atom".to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let mut timestamp: Option<String> = None;
    let mut signature: Option<String> = None;
    let mut id: Option<String> = None;
    let mut note: Option<String> = None;
    let mut body_start = 1;

    // Parse optional keyword fields: :at, :sig, :id, :note
    while body_start < args.len() {
        if let Some(kw) = args[body_start].as_atom() {
            let (field_name, target) = match kw {
                ":at" => ("timestamp", &mut timestamp),
                ":sig" => ("signature", &mut signature),
                ":id" => ("id", &mut id),
                ":note" => ("note", &mut note),
                _ => break,
            };
            if body_start + 1 >= args.len() {
                return Err(ParseError::ParserError {
                    line,
                    message: format!("claims {kw} requires a {field_name} atom"),
                    format: ParserFormat::Spl,
                    source_line: None,
                });
            }
            let val = args[body_start + 1]
                .as_atom()
                .ok_or_else(|| ParseError::ParserError {
                    line,
                    message: format!("claims {kw} requires a {field_name} atom"),
                    format: ParserFormat::Spl,
                    source_line: None,
                })?;
            *target = Some(val.to_string());
            body_start += 2;
            continue;
        }
        break;
    }

    // Process each claimed expression using its true source offset for line tracking.
    for expr in &args[body_start..] {
        let expr_line = line_of_offset(cleaned_input, expr.offset());
        let labels_before: std::collections::HashSet<String> =
            theory.rules().map(|r| r.label.clone()).collect();

        // Each expression inside claims is processed normally but gets source metadata
        super::expressions::process_expr_with_line(theory, expr, expr_line, cleaned_input)?;

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
            if let Some(ref sig) = signature {
                theory.add_meta_string(&label, "signature", sig);
            }
            if let Some(ref id) = id {
                theory.add_meta_string(&label, "id", id);
            }
            if let Some(ref n) = note {
                theory.add_meta_string(&label, "note", n);
            }
        }
    }

    Ok(())
}

/// Process a trusts directive: (trusts source value)
pub(crate) fn process_trusts(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if args.len() != 2 {
        return Err(ParseError::ParserError {
            line,
            message: "trusts requires exactly two arguments: source and value".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let source = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "trusts source must be an atom".to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let value_str = args[1].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "trusts value must be a number".to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let value: f64 = value_str.parse().map_err(|_| ParseError::ParserError {
        line,
        message: format!("trusts value must be a number in [0.0, 1.0], got: {value_str}"),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    if !(0.0..=1.0).contains(&value) {
        return Err(ParseError::ParserError {
            line,
            message: format!("trusts value must be in [0.0, 1.0], got: {value}"),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    theory
        .trust_policy_mut()
        .trust_map
        .insert(source.to_string(), value);
    Ok(())
}

/// Process a decays directive: (decays source model params...)
/// Models: exponential half_life_secs, linear rate_per_sec, step cutoff_secs
pub(crate) fn process_decays(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if args.len() < 3 {
        return Err(ParseError::ParserError {
            line,
            message: "decays requires at least: source, model, parameter".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let source = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "decays source must be an atom".to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let model_name = args[1].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "decays model must be an atom".to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let param_str = args[2].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "decays parameter must be a number".to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let param: f64 = param_str.parse().map_err(|_| ParseError::ParserError {
        line,
        message: format!("decays parameter must be a number, got: {param_str}"),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let model = match model_name {
        "exponential" => DecayModel::Exponential {
            half_life_secs: param,
        },
        "linear" => DecayModel::Linear {
            rate_per_sec: param,
        },
        "step" => DecayModel::StepFunction { cutoff_secs: param },
        _ => {
            return Err(ParseError::ParserError {
                line,
                message: format!(
                    "Unknown decay model: {model_name}. Expected: exponential, linear, or step"
                ),
                format: ParserFormat::Spl,
                source_line: None,
            });
        }
    };

    theory
        .trust_policy_mut()
        .decay_map
        .insert(source.to_string(), model);
    Ok(())
}

/// Process a threshold directive: (threshold name value)
pub(crate) fn process_threshold(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if args.len() != 2 {
        return Err(ParseError::ParserError {
            line,
            message: "threshold requires exactly two arguments: name and value".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    let name = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "threshold name must be an atom".to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let value_str = args[1].as_atom().ok_or_else(|| ParseError::ParserError {
        line,
        message: "threshold value must be a number".to_string(),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    let value: f64 = value_str.parse().map_err(|_| ParseError::ParserError {
        line,
        message: format!("threshold value must be a number in [0.0, 1.0], got: {value_str}"),
        format: ParserFormat::Spl,
        source_line: None,
    })?;

    if !(0.0..=1.0).contains(&value) {
        return Err(ParseError::ParserError {
            line,
            message: format!("threshold value must be in [0.0, 1.0], got: {value}"),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    theory
        .trust_policy_mut()
        .thresholds
        .insert(name.to_string(), value);
    Ok(())
}

/// Process meta with line number
pub(crate) fn process_meta_with_line(
    theory: &mut Theory,
    args: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    if args.is_empty() {
        return Err(ParseError::ParserError {
            line,
            message: "meta requires a label".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        });
    }

    // Resolve the target: a bare label atom or a structured
    // (predicate functor arity) target (SPEC-024 REQ-017).
    let target = super::predicate::parse_meta_target(&args[0], line)?;

    apply_meta_properties(theory, &target, &args[1..], line)
}

/// Apply `(key "value")` / `(key ("v1" "v2"))` properties to a metadata target.
///
/// Shared by the `meta` statement and by inline predicate-declaration metadata,
/// so both forms use identical parsing and the same `Theory` metadata store
/// (SPEC-024 CON-008, ADR-008).
///
/// Every property must be exactly a `(key value)` list; a bare atom or a list
/// of any other length is rejected rather than silently skipped, so typos in
/// the recognized form surface as parse errors.
pub(crate) fn apply_meta_properties(
    theory: &mut Theory,
    target: &spindle_core::vocabulary::MetaTarget,
    props: &[SExpr],
    line: usize,
) -> Result<(), ParseError> {
    for prop in props {
        let prop_list = prop.as_list().ok_or_else(|| ParseError::ParserError {
            line,
            message: "each meta property must be a (key value) list".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        })?;
        if prop_list.len() != 2 {
            return Err(ParseError::ParserError {
                line,
                message: "each meta property must be exactly (key value)".to_string(),
                format: ParserFormat::Spl,
                source_line: None,
            });
        }

        let key = prop_list[0]
            .as_atom()
            .ok_or_else(|| ParseError::ParserError {
                line,
                message: "meta property key must be an atom".to_string(),
                format: ParserFormat::Spl,
                source_line: None,
            })?;

        // Check if value is a list or single value
        let value = match &prop_list[1] {
            SExpr::Atom { value: s, .. } => MetaValue::String(s.clone()),
            SExpr::List { items, .. } => {
                // List of strings
                let strings: Result<Vec<String>, _> = items
                    .iter()
                    .map(|item| {
                        item.as_atom().map(|s| s.to_string()).ok_or_else(|| {
                            ParseError::ParserError {
                                line,
                                message: "meta list values must be atoms".to_string(),
                                format: ParserFormat::Spl,
                                source_line: None,
                            }
                        })
                    })
                    .collect();
                MetaValue::List(strings?)
            }
        };

        theory.add_meta_target(target.clone(), key, value);
    }

    Ok(())
}
