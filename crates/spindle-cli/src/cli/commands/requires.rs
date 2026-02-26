//! Requires command implementation

use std::path::PathBuf;

use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::query::{
    DEFAULT_MAX_RAW_CANDIDATES, RequiresOptions, RequiresSearchStatus, requires_with_options,
};
use spindle_core::temporal::TimePoint;

use crate::cli::error::{CliError, Diagnostic};
use crate::cli::input::{load_theory_source, parse_literal_arg, resolve_theory_source};
use crate::cli::output::{CommandOutput, LiteralStructJson, TrustPayload};

#[derive(serde::Serialize)]
struct RequiresOutput {
    schema_version: String,
    query: RequiresQuery,
    satisfied: bool,
    solutions: Vec<RequiresSolution>,
    verification_mode: String,
    search_status: String,
    verification: RequiresVerification,
    evaluated_at: Option<String>,
    trust: Option<TrustPayload>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncated: Option<TruncatedInfo>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(serde::Serialize)]
struct RequiresQuery {
    goal_spl: String,
    goal_struct: LiteralStructJson,
}

#[derive(serde::Serialize)]
struct RequiresSolution {
    facts: Vec<String>,
    score: f64,
}

#[derive(serde::Serialize)]
struct RequiresVerification {
    raw_examined: usize,
    accepted: usize,
    rejected: usize,
}

#[derive(serde::Serialize)]
struct TruncatedInfo {
    solutions: bool,
}

fn search_status_name(status: RequiresSearchStatus) -> &'static str {
    match status {
        RequiresSearchStatus::BoundedComplete => "BoundedComplete",
        RequiresSearchStatus::BudgetExhausted => "BudgetExhausted",
    }
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
        let satisfied = result.already_provable;

        let mut diagnostics = vec![];
        let mut truncated = None;

        // Check if we hit the max-solutions display limit.
        if solutions.len() > max {
            diagnostics.push(Diagnostic::warning(
                "SOLUTIONS_LIMIT_HIT",
                format!("Results limited to {max} solutions"),
            ));
            truncated = Some(TruncatedInfo { solutions: true });
            solutions.truncate(max);
        }

        if satisfied {
            solutions.clear();
        }

        let output = RequiresOutput {
            schema_version: "spindle.requires.v2".to_string(),
            query: RequiresQuery {
                goal_spl: lit.to_spl(),
                goal_struct: LiteralStructJson::from(&lit),
            },
            satisfied,
            solutions,
            verification_mode: "verified".to_string(),
            search_status: search_status_name(result.search_status).to_string(),
            verification: RequiresVerification {
                raw_examined: result.verification.raw_examined,
                accepted: result.verification.accepted,
                rejected: result.verification.rejected,
            },
            evaluated_at: prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
            trust: None,
            truncated,
            diagnostics,
        };

        CommandOutput::json(output)
    } else {
        let text = render_requires_text(&lit.to_spl(), &solutions, result.already_provable);
        Ok(CommandOutput::text(text))
    }
}
