//! Query command implementation

use std::path::PathBuf;

use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::query::{QueryStatus, query};
use spindle_core::temporal::TimePoint;

use crate::cli::error::{CliError, Diagnostic};
use crate::cli::input::{load_theory_source, parse_literal_arg, resolve_theory_source};
use crate::cli::output::{CommandOutput, LiteralStructJson, TrustPayload};

#[derive(serde::Serialize)]
struct QueryOutput {
    schema_version: String,
    literal_spl: String,
    literal_struct: LiteralStructJson,
    status: String,
    conclusion_type: Option<String>,
    evaluated_at: Option<String>,
    trust: Option<TrustPayload>,
    diagnostics: Vec<Diagnostic>,
}

pub(crate) fn run_query(
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
    let result = query(&prepared.theory, &lit)
        .map_err(|e| CliError::execution("QUERY_ERROR", format!("Error querying literal: {e}")))?;

    if json {
        let status = match result.status {
            QueryStatus::Provable => "provable",
            QueryStatus::Refuted => "refuted",
            QueryStatus::Unknown => "unknown",
        };

        let conclusion_type = result.conclusion_type.map(|ct| ct.symbol().to_string());

        let output = QueryOutput {
            schema_version: "spindle.query.v1".to_string(),
            literal_spl: result.literal.to_spl(),
            literal_struct: LiteralStructJson::from(&result.literal),
            status: status.to_string(),
            conclusion_type,
            evaluated_at: prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
            trust: None,
            diagnostics: vec![],
        };

        CommandOutput::json(output)
    } else {
        let text = match result.status {
            QueryStatus::Provable => {
                let ct = result.conclusion_type.unwrap();
                format!("{} {}", ct.symbol(), result.literal)
            }
            QueryStatus::Refuted => {
                format!("Refuted: {}", result.literal)
            }
            QueryStatus::Unknown => {
                format!("Unknown: {}", result.literal)
            }
        };
        Ok(CommandOutput::text(text))
    }
}
