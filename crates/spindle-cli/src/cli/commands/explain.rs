//! Explain command implementation

use std::path::PathBuf;

use spindle_core::explanation::explain;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::query::{QueryStatus, query};
use spindle_core::temporal::TimePoint;

use crate::cli::error::{CliError, Diagnostic};
use crate::cli::input::{load_theory_source, parse_literal_arg, resolve_theory_source};
use crate::cli::output::{CommandOutput, LiteralStructJson, TrustPayload};

#[derive(serde::Serialize)]
struct ExplainOutput {
    schema_version: String,
    literal_spl: String,
    literal_struct: LiteralStructJson,
    status: String,
    proof_tree: Option<serde_json::Value>,
    evaluated_at: Option<String>,
    trust: Option<TrustPayload>,
    diagnostics: Vec<Diagnostic>,
}

pub(crate) fn run_explain(
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

    // First query to get the status
    let query_result = query(&prepared.theory, &lit)
        .map_err(|e| CliError::execution("QUERY_ERROR", format!("Error querying literal: {e}")))?;

    let status = match query_result.status {
        QueryStatus::Provable => "provable",
        QueryStatus::Refuted => "refuted",
        QueryStatus::Unknown => "unknown",
    };

    match explain(&prepared.theory, &lit) {
        Ok(Some(explanation)) => {
            if json {
                let output = ExplainOutput {
                    schema_version: "spindle.explain.v1".to_string(),
                    literal_spl: lit.to_spl(),
                    literal_struct: LiteralStructJson::from(&lit),
                    status: status.to_string(),
                    proof_tree: Some(explanation.to_json()),
                    evaluated_at: prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
                    trust: None,
                    diagnostics: vec![],
                };
                CommandOutput::json(output)
            } else {
                Ok(CommandOutput::text(explanation.to_natural_language()))
            }
        }
        Ok(None) => {
            if json {
                // Per contract §8.2: explain with no proof tree is exit code 0
                let diagnostics = vec![Diagnostic::warning(
                    "NOT_PROVABLE",
                    format!("Literal {lit} is not provable"),
                )];

                let output = ExplainOutput {
                    schema_version: "spindle.explain.v1".to_string(),
                    literal_spl: lit.to_spl(),
                    literal_struct: LiteralStructJson::from(&lit),
                    status: status.to_string(),
                    proof_tree: None,
                    evaluated_at: prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
                    trust: None,
                    diagnostics,
                };
                CommandOutput::json(output)
            } else {
                let text = format!("{lit} is not provable.\nUse 'spindle why-not' to see why.");
                Ok(CommandOutput::text(text))
            }
        }
        Err(e) => Err(CliError::execution(
            "EXPLANATION_ERROR",
            format!("Error explaining literal: {e}"),
        )),
    }
}
