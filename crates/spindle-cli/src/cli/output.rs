//! Output rendering and boundary function
//!
//! Centralized output boundary — the ONLY place that calls `std::process::exit`.

use super::error::{CliError, render_human, render_json};

// Re-export literal transport types from the contract crate
pub(crate) use spindle_contract::literal::LiteralStructJson;

/// Command output variants - either JSON-serializable or text
#[derive(Debug)]
pub(crate) enum CommandOutput {
    Json(serde_json::Value),
    Text(String),
}

impl CommandOutput {
    pub(crate) fn json(value: impl serde::Serialize) -> Result<Self, CliError> {
        serde_json::to_value(value).map(Self::Json).map_err(|e| {
            CliError::execution(
                "JSON_SERIALIZATION_ERROR",
                format!("Failed to serialize JSON response: {e}"),
            )
        })
    }

    pub(crate) fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }
}

/// Centralized output boundary - the ONLY place that calls std::process::exit
/// Per contract §6.1 and §8.3
pub(crate) fn emit_and_exit(
    result: Result<CommandOutput, CliError>,
    schema_version: Option<&'static str>,
    json: bool,
    debug_errors: bool,
) -> ! {
    /// Print a ProblemDetails-based JSON error envelope via ErrorReport.
    fn print_json_error(err: &CliError, schema_version: Option<&'static str>) {
        let report = err.to_error_report(schema_version);
        match render_json(&report) {
            Ok(serialized) => println!("{serialized}"),
            Err(_) => {
                // Last-resort fallback if ErrorReport serialization itself fails
                let sv_part = match schema_version {
                    Some(sv) => format!("\"schema_version\":\"{sv}\","),
                    None => String::new(),
                };
                let fallback = format!(
                    "{{{sv_part}\"diagnostics\":[],\"error\":{{\"code\":\"{}\",\"message\":\"{}\"}}}}",
                    err.code.replace('"', "\\\""),
                    err.message.replace('"', "\\\""),
                );
                println!("{fallback}");
            }
        }
    }

    /// Print a ProblemDetails-based human-readable error to stderr.
    fn print_human_error(err: &CliError, debug: bool) {
        eprint!("{}", render_human(&err.problem, debug));
    }

    match result {
        Ok(output) => match output {
            CommandOutput::Json(value) => match serde_json::to_string_pretty(&value) {
                Ok(serialized) => {
                    println!("{serialized}");
                    std::process::exit(0);
                }
                Err(e) => {
                    let err = CliError::execution(
                        "JSON_SERIALIZATION_ERROR",
                        format!("Failed to serialize successful JSON response: {e}"),
                    );
                    if json {
                        print_json_error(&err, schema_version);
                    } else {
                        print_human_error(&err, debug_errors);
                    }
                    std::process::exit(err.exit_code);
                }
            },
            CommandOutput::Text(text) => {
                if json {
                    let err = CliError::execution(
                        "JSON_OUTPUT_EXPECTED",
                        "Expected JSON output but command returned text",
                    );
                    print_json_error(&err, schema_version);
                    std::process::exit(err.exit_code);
                }
                println!("{text}");
                std::process::exit(0);
            }
        },
        Err(err) => {
            if json {
                print_json_error(&err, schema_version);
            } else {
                print_human_error(&err, debug_errors);
            }
            std::process::exit(err.exit_code);
        }
    }
}

// =============================================================================
// JSON serialization helpers
// =============================================================================

/// Trust payload structure
#[derive(serde::Serialize)]
pub(crate) struct TrustPayload {
    pub(crate) score: f64,
    pub(crate) contributors: Vec<TrustContributor>,
    pub(crate) explain: String,
}

#[derive(serde::Serialize)]
pub(crate) struct TrustContributor {
    pub(crate) source_id: String,
    pub(crate) weight: f64,
    pub(crate) impact: f64,
}
