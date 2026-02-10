//! Output rendering and boundary function
//!
//! Centralized output boundary — the ONLY place that calls `std::process::exit`.

use spindle_core::literal::Literal;
use spindle_core::temporal::Temporal;

use super::error::CliError;

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

/// JSON-serializable literal structure (contract-compliant)
#[derive(serde::Serialize)]
pub(crate) struct LiteralStructJson {
    pub(crate) mode: ModeJson,
    pub(crate) negated: bool,
    pub(crate) functor: String,
    pub(crate) args: Vec<String>,
    pub(crate) temporal: TemporalJson,
}

impl From<&Literal> for LiteralStructJson {
    fn from(literal: &Literal) -> Self {
        Self {
            mode: ModeJson::from(&literal.mode),
            negated: literal.negation,
            functor: literal.name().to_string(),
            args: literal.predicates().iter().map(|s| s.to_string()).collect(),
            temporal: TemporalJson::from(&literal.temporal),
        }
    }
}

/// JSON-serializable mode structure
#[derive(serde::Serialize)]
pub(crate) struct ModeJson {
    pub(crate) name: Option<String>,
    pub(crate) negation: bool,
}

impl From<&spindle_core::mode::Mode> for ModeJson {
    fn from(mode: &spindle_core::mode::Mode) -> Self {
        Self {
            name: mode.name.clone(),
            negation: mode.negation,
        }
    }
}

/// JSON-serializable temporal structure
/// Maps NegInf/PosInf to null per contract schema
#[derive(serde::Serialize)]
pub(crate) struct TemporalJson {
    pub(crate) start: Option<i64>,
    pub(crate) end: Option<i64>,
}

impl From<&Temporal> for TemporalJson {
    fn from(temporal: &Temporal) -> Self {
        use spindle_core::temporal::TimePoint;
        Self {
            start: match temporal.start {
                TimePoint::Moment(v) => Some(v),
                TimePoint::NegInf | TimePoint::PosInf => None,
            },
            end: match temporal.end {
                TimePoint::Moment(v) => Some(v),
                TimePoint::NegInf | TimePoint::PosInf => None,
            },
        }
    }
}
