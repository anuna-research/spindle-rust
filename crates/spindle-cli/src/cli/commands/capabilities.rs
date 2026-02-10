//! Capabilities command implementation

use crate::cli::error::CliError;
use crate::cli::output::CommandOutput;

#[derive(serde::Serialize)]
struct CapabilitiesOutput {
    schema_version: String,
    commands: Vec<String>,
    features: FeaturesInfo,
    schemas: SchemasInfo,
}

#[derive(serde::Serialize)]
struct FeaturesInfo {
    stdin: bool,
    given_flags: bool,
    trust_overlay_v1: bool,
    trust_explain_v1: bool,
    at: bool,
    reason_json: bool,
}

#[derive(serde::Serialize)]
struct SchemasInfo {
    reason: String,
    query: String,
    requires: String,
    explain: String,
    why_not: String,
}

pub(crate) fn run_capabilities(json: bool) -> Result<CommandOutput, CliError> {
    if json {
        let output = CapabilitiesOutput {
            schema_version: "spindle.capabilities.v1".to_string(),
            commands: vec![
                "reason".to_string(),
                "query".to_string(),
                "requires".to_string(),
                "explain".to_string(),
                "why-not".to_string(),
            ],
            features: FeaturesInfo {
                stdin: true,
                given_flags: false,
                trust_overlay_v1: false,
                trust_explain_v1: false,
                at: true,
                reason_json: true,
            },
            schemas: SchemasInfo {
                reason: "spindle.reason.v1".to_string(),
                query: "spindle.query.v1".to_string(),
                requires: "spindle.requires.v1".to_string(),
                explain: "spindle.explain.v1".to_string(),
                why_not: "spindle.why_not.v1".to_string(),
            },
        };
        CommandOutput::json(output)
    } else {
        let mut text = String::new();
        text.push_str("Spindle Capabilities:\n\n");
        text.push_str("Commands: reason, query, requires, explain, why-not\n\n");
        text.push_str("Features:\n");
        text.push_str("  --stdin: yes\n");
        text.push_str("  --at: yes\n");
        text.push_str("  --json: yes\n");
        text.push_str("  Trust overlay: no\n");
        text.push_str("  Given flags: no\n\n");
        text.push_str("Schema versions:\n");
        text.push_str("  reason: spindle.reason.v1\n");
        text.push_str("  query: spindle.query.v1\n");
        text.push_str("  requires: spindle.requires.v1\n");
        text.push_str("  explain: spindle.explain.v1\n");
        text.push_str("  why-not: spindle.why_not.v1");
        Ok(CommandOutput::text(text))
    }
}
