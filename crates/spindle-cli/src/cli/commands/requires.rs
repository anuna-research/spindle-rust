//! Requires command implementation

use std::path::PathBuf;

use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::query::abduce;
use spindle_core::temporal::TimePoint;

use crate::cli::error::{CliError, Diagnostic};
use crate::cli::input::{load_theory_source, parse_literal_arg, resolve_theory_source};
use crate::cli::output::{CommandOutput, LiteralStructJson, TrustPayload};

#[derive(serde::Serialize)]
struct RequiresOutput {
    schema_version: String,
    goal_spl: String,
    goal_struct: LiteralStructJson,
    satisfied: bool,
    solutions: Vec<RequiresSolution>,
    evaluated_at: Option<String>,
    trust: Option<TrustPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncated: Option<TruncatedInfo>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(serde::Serialize)]
struct RequiresSolution {
    facts: Vec<String>,
    score: f64,
}

#[derive(serde::Serialize)]
struct TruncatedInfo {
    solutions: bool,
}

pub(crate) fn run_requires(
    file: Option<&PathBuf>,
    literal: &str,
    max: usize,
    json: bool,
    stdin: bool,
    reference_time: Option<TimePoint>,
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

    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let prepared = prepare(&theory, opts).map_err(|e| {
        CliError::execution(
            "PREPARATION_ERROR",
            format!("Error during preparation: {e}"),
        )
    })?;

    let lit = parse_literal_arg(literal)?;
    let abduce_limit = if json { max.saturating_add(1) } else { max };
    let result = abduce(&prepared.theory, &lit, abduce_limit).map_err(|e| {
        CliError::execution(
            "ABDUCTION_ERROR",
            format!("Error finding requirements: {e}"),
        )
    })?;

    if json {
        // Per contract §6.3: satisfied=true => solutions=[], satisfied=false => solutions non-empty
        let satisfied = result.is_already_provable();

        let mut diagnostics = vec![];
        let mut truncated = None;

        // Check if we hit the limit
        let solutions_limit_hit = result.solutions.len() > max;
        let solutions_to_show: Vec<_> = if solutions_limit_hit {
            diagnostics.push(Diagnostic::warning(
                "SOLUTIONS_LIMIT_HIT",
                format!("Results limited to {max} solutions"),
            ));
            truncated = Some(TruncatedInfo { solutions: true });
            result.solutions.iter().take(max).collect()
        } else {
            result.solutions.iter().collect()
        };

        // Build solutions - only include if not satisfied
        let solutions: Vec<_> = if satisfied {
            vec![]
        } else {
            solutions_to_show
                .iter()
                .map(|s| {
                    let facts: Vec<_> = s.facts.iter().map(|l| l.to_spl()).collect();
                    // Sort facts lexically for determinism (per spec §7)
                    let mut facts = facts;
                    facts.sort();

                    RequiresSolution {
                        facts,
                        score: s.confidence,
                    }
                })
                .collect()
        };

        // Sort solutions by set size then lexical order (per spec §7)
        let mut solutions = solutions;
        solutions.sort_by(|a, b| match a.facts.len().cmp(&b.facts.len()) {
            std::cmp::Ordering::Equal => a.facts.cmp(&b.facts),
            other => other,
        });

        let output = RequiresOutput {
            schema_version: "spindle.requires.v1".to_string(),
            goal_spl: result.goal.to_spl(),
            goal_struct: LiteralStructJson::from(&result.goal),
            satisfied,
            solutions,
            evaluated_at: prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
            trust: None,
            truncated,
            diagnostics,
        };

        CommandOutput::json(output)
    } else {
        Ok(CommandOutput::text(format!("{result}")))
    }
}
