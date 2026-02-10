//! Output rendering and boundary function
//!
//! Centralized output boundary — the ONLY place that calls `std::process::exit`.

use super::error::CliError;

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
    schema_version: Option<&str>,
    json: bool,
) -> ! {
    fn print_json_error_envelope(err: &CliError, schema_version: Option<&str>) {
        let mut envelope = serde_json::Map::new();

        if let Some(sv) = schema_version {
            envelope.insert(
                "schema_version".to_string(),
                serde_json::Value::String(sv.to_string()),
            );
        }

        let mut error_obj = serde_json::Map::new();
        error_obj.insert(
            "code".to_string(),
            serde_json::Value::String(err.code.clone()),
        );
        error_obj.insert(
            "message".to_string(),
            serde_json::Value::String(err.message.clone()),
        );
        error_obj.insert("details".to_string(), err.details.clone());
        envelope.insert("error".to_string(), serde_json::Value::Object(error_obj));

        let diagnostics: Vec<serde_json::Value> = err
            .diagnostics
            .iter()
            .map(|d| {
                serde_json::to_value(d).unwrap_or_else(|_| {
                    serde_json::json!({
                        "severity": "error",
                        "code": "DIAGNOSTIC_SERIALIZATION_ERROR",
                        "message": "Failed to serialize diagnostic"
                    })
                })
            })
            .collect();
        envelope.insert(
            "diagnostics".to_string(),
            serde_json::Value::Array(diagnostics),
        );

        match serde_json::to_string_pretty(&envelope) {
            Ok(serialized) => println!("{serialized}"),
            Err(_) => {
                let mut fallback = serde_json::json!({
                    "error": {
                        "code": "JSON_SERIALIZATION_ERROR",
                        "message": "Failed to serialize JSON error envelope",
                        "details": {}
                    },
                    "diagnostics": [{
                        "severity": "error",
                        "code": "JSON_SERIALIZATION_ERROR",
                        "message": "Failed to serialize JSON error envelope"
                    }]
                });
                if let Some(sv) = schema_version {
                    fallback["schema_version"] = serde_json::Value::String(sv.to_string());
                }
                let fallback_str = serde_json::to_string(&fallback).unwrap_or_else(|_| {
                    "{\"error\":{\"code\":\"JSON_SERIALIZATION_ERROR\",\"message\":\"Failed to serialize JSON error envelope\",\"details\":{}},\"diagnostics\":[]}".to_string()
                });
                println!("{fallback_str}");
            }
        }
    }

    fn print_non_json_error(err: &CliError) {
        eprintln!("Error: {}", err.message);
        if let Some(hint) = err.details.get("hint").and_then(|v| v.as_str()) {
            eprintln!("Hint: {hint}");
        }
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
                        print_json_error_envelope(&err, schema_version);
                    } else {
                        print_non_json_error(&err);
                    }
                    std::process::exit(err.exit_code);
                }
            },
            CommandOutput::Text(text) => {
                if json {
                    let err = CliError::execution(
                        "JSON_OUTPUT_EXPECTED",
                        "Expected JSON output but command returned text",
                    )
                    .with_details(serde_json::json!({
                        "hint": "This is an internal CLI bug. Please report it."
                    }));
                    print_json_error_envelope(&err, schema_version);
                    std::process::exit(err.exit_code);
                }
                println!("{text}");
                std::process::exit(0);
            }
        },
        Err(err) => {
            if json {
                print_json_error_envelope(&err, schema_version);
            } else {
                print_non_json_error(&err);
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
