//! Requires command implementation

use std::path::PathBuf;

use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::query::{DEFAULT_MAX_RAW_CANDIDATES, RequiresOptions, requires_with_options};

use crate::cli::error::CliError;
use crate::cli::input::{load_theory_source, parse_literal_arg, resolve_theory_source};
use crate::cli::output::CommandOutput;

#[derive(serde::Serialize)]
struct RequiresSolution {
    facts: Vec<String>,
    score: f64,
}

fn render_requires_text(goal_spl: &str, solutions: &[RequiresSolution], satisfied: bool) -> String {
    if satisfied {
        return format!("Already provable: {goal_spl}");
    }
    if solutions.is_empty() {
        return format!("No verified requirements found for {goal_spl}");
    }

    let mut out = format!("Verified requirements for {goal_spl}:\n");
    for (i, solution) in solutions.iter().enumerate() {
        out.push_str(&format!(
            "  {}. Add facts: {{{}}}\n",
            i + 1,
            solution.facts.join(", ")
        ));
    }
    out.trim_end().to_string()
}

pub(crate) fn run_requires(
    file: Option<&PathBuf>,
    literal: &str,
    max: usize,
    json: bool,
    stdin: bool,
    opts: PrepareOptions,
) -> Result<CommandOutput, CliError> {
    // Validate max parameter - must be at least 1 to satisfy contract
    if max == 0 {
        return Err(
            CliError::validation("INVALID_ARGUMENT", "--max must be at least 1").with_details(
                serde_json::json!({
                    "argument": "--max",
                    "provided": 0,
                    "minimum": 1
                }),
            ),
        );
    }

    let source = resolve_theory_source(file, stdin)?;
    let theory = load_theory_source(&source)?;

    let prepared = prepare(&theory, opts).map_err(|e| {
        CliError::execution(
            "PREPARATION_ERROR",
            format!("Error during preparation: {e}"),
        )
    })?;

    let lit = parse_literal_arg(literal)?;
    let options = RequiresOptions {
        // Keep a +1 sentinel in JSON mode to preserve SOLUTIONS_LIMIT_HIT behavior.
        max_solutions: if json { max.saturating_add(1) } else { max },
        max_raw_candidates: DEFAULT_MAX_RAW_CANDIDATES,
    };

    let result = requires_with_options(&prepared.theory, &lit, options).map_err(|e| {
        CliError::execution("REQUIRES_ERROR", format!("Error finding requirements: {e}"))
    })?;

    let mut solutions: Vec<_> = result
        .solutions
        .iter()
        .map(|s| {
            let mut facts: Vec<_> = s.facts.iter().map(|l| l.to_spl()).collect();
            facts.sort();
            RequiresSolution { facts, score: 1.0 }
        })
        .collect();
    solutions.sort_by(|a, b| match a.facts.len().cmp(&b.facts.len()) {
        std::cmp::Ordering::Equal => a.facts.cmp(&b.facts),
        other => other,
    });

    if json {
        CommandOutput::json(spindle_contract::query::requires_output(
            &lit,
            &result,
            max,
            prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
        ))
    } else {
        let text = render_requires_text(&lit.to_spl(), &solutions, result.already_provable);
        Ok(CommandOutput::text(text))
    }
}
