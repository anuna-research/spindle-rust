//! Stats command implementation

use std::path::PathBuf;

use crate::cli::error::{CliError, Diagnostic};
use crate::cli::input::{load_theory_source, resolve_theory_source};
use crate::cli::output::CommandOutput;

#[derive(serde::Serialize)]
struct StatsOutput {
    stats: StatsPayload,
    diagnostics: Vec<Diagnostic>,
}

#[derive(serde::Serialize)]
struct StatsPayload {
    total_rules: usize,
    facts: usize,
    strict: usize,
    defeasible: usize,
    defeaters: usize,
    superiorities: usize,
}

pub(crate) fn run_stats(
    file: Option<&PathBuf>,
    stdin: bool,
    json: bool,
) -> Result<CommandOutput, CliError> {
    let source = resolve_theory_source(file, stdin)?;
    let theory = load_theory_source(&source)?;

    let facts = theory.facts().count();
    let strict = theory
        .rules_by_type(spindle_core::rule::RuleType::Strict)
        .count();
    let defeasible = theory
        .rules_by_type(spindle_core::rule::RuleType::Defeasible)
        .count();
    let defeaters = theory
        .rules_by_type(spindle_core::rule::RuleType::Defeater)
        .count();

    let total_rules = theory.rule_count();
    let superiorities = theory.superiorities().len();

    if json {
        CommandOutput::json(StatsOutput {
            stats: StatsPayload {
                total_rules,
                facts,
                strict,
                defeasible,
                defeaters,
                superiorities,
            },
            diagnostics: vec![],
        })
    } else {
        let mut text = String::new();
        text.push_str("Theory Statistics:\n");
        text.push_str(&format!("  Total rules: {total_rules}\n"));
        text.push_str(&format!("    Facts:      {facts}\n"));
        text.push_str(&format!("    Strict:     {strict}\n"));
        text.push_str(&format!("    Defeasible: {defeasible}\n"));
        text.push_str(&format!("    Defeaters:  {defeaters}\n"));
        text.push_str(&format!("  Superiorities: {superiorities}"));

        Ok(CommandOutput::text(text))
    }
}
