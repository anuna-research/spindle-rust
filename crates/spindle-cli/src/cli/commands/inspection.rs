//! Vocabulary and hypothetical query entry points.
use crate::cli::{
    error::CliError,
    input::{load_theory_source, parse_literal_arg, resolve_theory_source},
    output::CommandOutput,
};
use spindle_core::{PrepareOptions, query};
use std::path::PathBuf;

pub(crate) fn vocabulary(
    file: Option<&PathBuf>,
    stdin: bool,
    json: bool,
) -> Result<CommandOutput, CliError> {
    let theory = load_theory_source(&resolve_theory_source(file, stdin)?)?;
    let report = spindle_core::Vocabulary::derive(&theory);
    let dto = spindle_contract::vocabulary::VocabularyReportDto::from(&report);
    if json {
        CommandOutput::json(dto)
    } else {
        // The complete report is useful in text mode too: no declarations or diagnostics are hidden.
        Ok(CommandOutput::text(
            serde_json::to_string_pretty(&dto)
                .map_err(|e| CliError::execution("SERIALIZATION_ERROR", e.to_string()))?,
        ))
    }
}

pub(crate) fn what_if(
    file: Option<&PathBuf>,
    stdin: bool,
    json: bool,
    goal: &str,
    given: &[String],
    options: PrepareOptions,
) -> Result<CommandOutput, CliError> {
    let theory = load_theory_source(&resolve_theory_source(file, stdin)?)?;
    let goal = parse_literal_arg(goal)?;
    let claims = given
        .iter()
        .map(|s| parse_literal_arg(s).map(query::HypotheticalClaim::new))
        .collect::<Result<Vec<_>, _>>()?;
    let result = query::what_if_with_options(&theory, claims, &goal, options)
        .map_err(|e| CliError::execution("WHAT_IF_ERROR", e.to_string()))?;
    let output = spindle_contract::query::what_if_output(&result);
    if json {
        CommandOutput::json(output)
    } else {
        Ok(CommandOutput::text(format!(
            "{}: {}\nNew conclusions: {}",
            if result.is_provable() {
                "Provable"
            } else {
                "Not provable"
            },
            goal.to_spl(),
            result
                .new_conclusions
                .iter()
                .map(|l| l.to_spl())
                .collect::<Vec<_>>()
                .join(", ")
        )))
    }
}

pub(crate) fn abduce(
    file: Option<&PathBuf>,
    stdin: bool,
    json: bool,
    goal: &str,
    max: usize,
    options: PrepareOptions,
) -> Result<CommandOutput, CliError> {
    if max == 0 {
        return Err(CliError::validation(
            "INVALID_ARGUMENT",
            "--max must be at least 1",
        ));
    }
    let theory = load_theory_source(&resolve_theory_source(file, stdin)?)?;
    let goal_input = goal;
    let goal = parse_literal_arg(goal)?;
    let prepared = spindle_core::pipeline::prepare(&theory, options)
        .map_err(|e| CliError::execution("PREPARATION_ERROR", e.to_string()))?;
    let result = query::abduce(&prepared.theory, &goal, max)
        .map_err(|e| CliError::execution("ABDUCTION_ERROR", e.to_string()))?;
    let output = spindle_contract::query::abduction_output(goal_input, &result);
    if json {
        CommandOutput::json(output)
    } else {
        Ok(CommandOutput::text(format!(
            "Unverified candidates for {}:\n{}",
            goal.to_spl(),
            serde_json::to_string_pretty(&output["solutions"])
                .map_err(|e| CliError::execution("SERIALIZATION_ERROR", e.to_string()))?
        )))
    }
}
