//! Why-not command implementation

use std::path::PathBuf;

use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::query::{QueryStatus, query, why_not};
use spindle_core::temporal::TimePoint;

use crate::cli::error::{CliError, Diagnostic};
use crate::cli::input::{load_theory_source, parse_literal_arg, resolve_theory_source};
use crate::cli::output::{CommandOutput, LiteralStructJson, TrustPayload};

#[derive(serde::Serialize)]
struct WhyNotOutput {
    schema_version: String,
    literal_spl: String,
    literal_struct: LiteralStructJson,
    status: String,
    blocked_by: Vec<BlockedByItem>,
    evaluated_at: Option<String>,
    trust: Option<TrustPayload>,
    diagnostics: Vec<Diagnostic>,
}

#[derive(serde::Serialize)]
struct BlockedByItem {
    #[serde(rename = "type")]
    blocking_type: String,
    rule: String,
    explanation: String,
}

pub(crate) fn run_why_not(
    file: Option<&PathBuf>,
    literal: &str,
    json: bool,
    stdin: bool,
    reference_time: Option<TimePoint>,
) -> Result<CommandOutput, CliError> {
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

    // Query to get the actual status
    let query_result = query(&prepared.theory, &lit)
        .map_err(|e| CliError::execution("QUERY_ERROR", format!("Error querying literal: {e}")))?;

    let status = match query_result.status {
        QueryStatus::Provable => "provable",
        QueryStatus::Refuted => "refuted",
        QueryStatus::Unknown => "unknown",
    };

    let result = why_not(&prepared.theory, &lit).map_err(|e| {
        CliError::execution("WHY_NOT_ERROR", format!("Error checking why-not: {e}"))
    })?;

    if json {
        let blockers: Vec<_> = result
            .blocked_by
            .iter()
            .map(|b| BlockedByItem {
                blocking_type: b.blocking_type.to_string(),
                rule: b.rule_label.clone(),
                explanation: b.explanation.clone(),
            })
            .collect();

        let output = WhyNotOutput {
            schema_version: "spindle.why_not.v1".to_string(),
            literal_spl: result.literal.to_spl(),
            literal_struct: LiteralStructJson::from(&result.literal),
            status: status.to_string(),
            blocked_by: blockers,
            evaluated_at: prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
            trust: None,
            diagnostics: vec![],
        };
        CommandOutput::json(output)
    } else {
        Ok(CommandOutput::text(format!("{result}")))
    }
}
