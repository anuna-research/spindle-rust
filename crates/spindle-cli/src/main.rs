//! Spindle CLI - Command-line interface for defeasible logic reasoning

#[cfg(test)]
mod tests;

use std::fs;
use std::path::PathBuf;

use chrono::DateTime;
use clap::error::ErrorKind;
use clap::{Parser, Subcommand};
use spindle_core::conclusion::ConclusionType;
use spindle_core::explanation::explain;
use spindle_core::literal::Literal;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::query::{QueryStatus, abduce, query, why_not};
use spindle_core::temporal::TimePoint;
use spindle_parser::spl::parse_spl as parse_spl_str;
use spindle_parser::{parse_dfl, parse_spl};

// =============================================================================
// CENTRALIZED CONTRACT TYPES (NEW)
// =============================================================================

/// Theory source enum - exactly one source per invocation (per contract §5.1)
#[derive(Debug)]
enum TheorySource {
    File(PathBuf),
    Stdin,
}

/// Structured CLI error with contract-compliant exit codes
/// Per contract §8.1: 2=user, 3=execution, 4=resource
#[derive(Debug)]
struct CliError {
    exit_code: i32,
    code: String,
    message: String,
    details: serde_json::Value,
    diagnostics: Vec<Diagnostic>,
}

impl CliError {
    /// Exit code 2: validation/user input error (per contract §8.1)
    fn validation(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        Self {
            exit_code: 2,
            code: code.clone(),
            message: message.clone(),
            details: serde_json::json!({}),
            diagnostics: vec![Diagnostic::error(&code, &message)],
        }
    }

    /// Exit code 2: parse error (per contract §8.1)
    fn parse(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        Self {
            exit_code: 2,
            code: code.clone(),
            message: message.clone(),
            details: serde_json::json!({}),
            diagnostics: vec![Diagnostic::error(&code, &message)],
        }
    }

    /// Exit code 3: execution/internal error (per contract §8.1)
    fn execution(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        Self {
            exit_code: 3,
            code: code.clone(),
            message: message.clone(),
            details: serde_json::json!({}),
            diagnostics: vec![Diagnostic::error(&code, &message)],
        }
    }

    /// Exit code 4: resource/limit error (per contract §8.1)
    #[allow(dead_code)]
    fn resource(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        Self {
            exit_code: 4,
            code: code.clone(),
            message: message.clone(),
            details: serde_json::json!({}),
            diagnostics: vec![Diagnostic::error(&code, &message)],
        }
    }

    /// Add details to the error (must be an object per contract §8.3)
    fn with_details(mut self, details: serde_json::Value) -> Self {
        // Ensure details is an object
        if !details.is_object() {
            self.details = serde_json::json!({});
        } else {
            self.details = details;
        }
        self
    }
}

/// Command output variants - either JSON-serializable or text
#[derive(Debug)]
enum CommandOutput {
    Json(serde_json::Value),
    Text(String),
}

impl CommandOutput {
    fn json(value: impl serde::Serialize) -> Result<Self, CliError> {
        serde_json::to_value(value).map(Self::Json).map_err(|e| {
            CliError::execution(
                "JSON_SERIALIZATION_ERROR",
                format!("Failed to serialize JSON response: {e}"),
            )
        })
    }

    fn text(s: impl Into<String>) -> Self {
        Self::Text(s.into())
    }
}

/// A diagnostic message for the output envelope (per contract §6.1)
#[derive(serde::Serialize, Debug, Clone)]
struct Diagnostic {
    severity: String,
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

impl Diagnostic {
    fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "warning".to_string(),
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    #[allow(dead_code)]
    fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "info".to_string(),
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
}

// =============================================================================
// CENTRALIZED BOUNDARY FUNCTION (NEW)
// Single place where all output and exit happens
// =============================================================================

/// Centralized output boundary - the ONLY place that calls std::process::exit
/// Per contract §6.1 and §8.3
fn emit_and_exit(
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
// SHARED THEORY SOURCE RESOLVER (NEW)
// Per contract §5.1: exactly one theory source per invocation
// =============================================================================

/// Resolve theory source with mutual exclusivity validation
/// Returns error if both file and stdin provided, or neither provided
fn resolve_theory_source(file: Option<&PathBuf>, stdin: bool) -> Result<TheorySource, CliError> {
    match (file, stdin) {
        (Some(f), true) => Err(CliError::validation(
            "CONFLICTING_INPUT_SOURCES",
            format!("Cannot specify both file '{}' and --stdin", f.display()),
        )
        .with_details(serde_json::json!({
            "file": f.to_string_lossy().to_string(),
            "stdin": true,
            "hint": "Use either a file path or --stdin, but not both."
        }))),
        (Some(f), false) => Ok(TheorySource::File(f.clone())),
        (None, true) => Ok(TheorySource::Stdin),
        (None, false) => Err(CliError::validation(
            "MISSING_INPUT_SOURCE",
            "Must specify either a file or --stdin",
        )
        .with_details(serde_json::json!({
            "hint": "Provide a file path or use --stdin to read theory from standard input"
        }))),
    }
}

/// Load theory from resolved source
fn load_theory_source(source: &TheorySource) -> Result<spindle_core::Theory, CliError> {
    match source {
        TheorySource::File(path) => load_theory_from_file(path),
        TheorySource::Stdin => load_theory_from_stdin(),
    }
}

fn load_theory_from_file(file: &PathBuf) -> Result<spindle_core::Theory, CliError> {
    let content = fs::read_to_string(file).map_err(|e| {
        CliError::validation(
            "FILE_READ_ERROR",
            format!("Error reading file '{}': {}", file.display(), e),
        )
    })?;
    parse_theory_content(&content, Some(file))
}

fn load_theory_from_stdin() -> Result<spindle_core::Theory, CliError> {
    use std::io::{self, Read};

    let mut content = String::new();
    io::stdin().read_to_string(&mut content).map_err(|e| {
        CliError::validation("STDIN_READ_ERROR", format!("Error reading stdin: {e}"))
    })?;
    parse_theory_content(&content, None)
}

fn parse_theory_content(
    content: &str,
    file: Option<&PathBuf>,
) -> Result<spindle_core::Theory, CliError> {
    // Auto-detect SPL vs DFL based on file extension or content
    let is_spl = file
        .map(|f| f.extension().is_some_and(|ext| ext == "spl"))
        .unwrap_or(false)
        || content.trim().starts_with("#lang")
        || content.trim().starts_with('(')
        || content.trim().starts_with(';');

    if is_spl {
        parse_spl(content)
            .map_err(|e| CliError::parse("SPL_PARSE_ERROR", format!("SPL parse error: {e}")))
    } else {
        parse_dfl(content)
            .map_err(|e| CliError::parse("DFL_PARSE_ERROR", format!("DFL parse error: {e}")))
    }
}

// =============================================================================
// CLI STRUCTS
// =============================================================================

/// Spindle - Defeasible Logic Reasoning Engine
#[derive(Parser, Debug)]
#[command(name = "spindle")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output in JSON format
    #[arg(long, global = true)]
    json: bool,

    /// Reference time for "as-of" reasoning (ISO 8601)
    #[arg(long, global = true)]
    at: Option<String>,

    /// Read theory from stdin
    #[arg(long, global = true)]
    stdin: bool,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show reasoning conclusions from a theory file
    Reason {
        /// Input file (mutually exclusive with --stdin)
        file: Option<PathBuf>,
        /// Use scalable mode
        #[arg(long)]
        scalable: bool,
        /// Only show positive conclusions
        #[arg(long)]
        positive: bool,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Validate a theory file
    Validate {
        /// Input file (mutually exclusive with --stdin)
        file: Option<PathBuf>,
        /// Read theory from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// Show theory statistics
    Stats {
        /// Input file (mutually exclusive with --stdin)
        file: Option<PathBuf>,
        /// Read theory from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// Query if a literal holds in the theory
    Query {
        /// Literal to query
        literal: String,
        /// Input file (mutually exclusive with --stdin)
        file: Option<PathBuf>,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Explain why a conclusion holds
    Explain {
        /// Literal to explain
        literal: String,
        /// Input file (mutually exclusive with --stdin)
        file: Option<PathBuf>,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Explain why a conclusion does NOT hold
    WhyNot {
        /// Literal to check
        literal: String,
        /// Input file (mutually exclusive with --stdin)
        file: Option<PathBuf>,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Find facts needed to derive a literal
    Requires {
        /// Goal literal
        literal: String,
        /// Input file (mutually exclusive with --stdin)
        file: Option<PathBuf>,
        /// Max solutions
        #[arg(long, default_value = "10")]
        max: usize,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Show spindle capabilities
    Capabilities {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
}

// =============================================================================
// TRUST AND JSON STRUCTS
// =============================================================================

/// Trust payload structure
#[derive(serde::Serialize)]
struct TrustPayload {
    score: f64,
    contributors: Vec<TrustContributor>,
    explain: String,
}

#[derive(serde::Serialize)]
struct TrustContributor {
    source_id: String,
    weight: f64,
    impact: f64,
}

/// JSON-serializable literal structure (contract-compliant)
#[derive(serde::Serialize)]
struct LiteralStructJson {
    mode: ModeJson,
    negated: bool,
    functor: String,
    args: Vec<String>,
    temporal: TemporalJson,
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
struct ModeJson {
    name: Option<String>,
    negation: bool,
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
struct TemporalJson {
    start: Option<i64>,
    end: Option<i64>,
}

impl From<&spindle_core::temporal::Temporal> for TemporalJson {
    fn from(temporal: &spindle_core::temporal::Temporal) -> Self {
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

fn parse_literal_arg(s: &str) -> Result<Literal, CliError> {
    // If it looks like an SPL expression (starts with paren), try to parse it as a dummy fact
    if s.trim().starts_with('(') {
        let dummy_spl = format!("(given {s})");
        if let Ok(theory) = parse_spl_str(&dummy_spl)
            && let Some(fact) = theory.facts().next()
            && let Some(head) = fact.head.first()
        {
            return Ok(head.clone());
        }
    }

    // Fallback to simple parsing logic
    if s.starts_with("(not ") && s.ends_with(')') {
        let inner = &s[5..s.len() - 1];
        Ok(Literal::negated(inner))
    } else if let Some(stripped) = s.strip_prefix('~') {
        Ok(Literal::negated(stripped))
    } else {
        Ok(Literal::simple(s))
    }
}

// =============================================================================
// REASON COMMAND
// =============================================================================

#[derive(serde::Serialize)]
struct ReasonOutput {
    schema_version: String,
    evaluated_at: Option<String>,
    grounding: GroundingStats,
    conclusions: Vec<ConclusionStruct>,
    diagnostics: Vec<Diagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stats: Option<TheoryStats>,
}

#[derive(serde::Serialize)]
struct GroundingStats {
    performed: bool,
    had_variables: bool,
    instances: usize,
    limit_hit: bool,
}

#[derive(serde::Serialize)]
struct ConclusionStruct {
    conclusion_type: String,
    literal_spl: String,
    literal_struct: LiteralStructJson,
    positive: bool,
}

#[derive(serde::Serialize)]
struct TheoryStats {
    rule_count: usize,
    fact_count: usize,
}

fn run_reason(
    file: Option<&PathBuf>,
    scalable: bool,
    positive_only: bool,
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

    let pipeline_result = prepare(&theory, opts).map_err(|e| {
        CliError::execution(
            "PREPARATION_ERROR",
            format!("Error during preparation: {e}"),
        )
    })?;

    let conclusions = if scalable {
        let indexed = spindle_core::index::IndexedTheory::build(&pipeline_result.theory);
        let result = spindle_core::scalable::reason_scalable(&indexed);
        result.to_conclusions(&indexed)
    } else {
        spindle_core::reason::reason(&pipeline_result.theory).map_err(|e| {
            CliError::execution("REASONING_ERROR", format!("Error during reasoning: {e}"))
        })?
    };

    if json {
        let mut output_conclusions: Vec<ConclusionStruct> = conclusions
            .into_iter()
            .filter(|c| !positive_only || c.is_positive())
            .map(|c| ConclusionStruct {
                conclusion_type: c.conclusion_type.symbol().to_string(),
                literal_spl: c.literal.to_spl(),
                literal_struct: LiteralStructJson::from(&c.literal),
                positive: c.is_positive(),
            })
            .collect();

        // Sort conclusions for deterministic output (per spec §7)
        output_conclusions.sort_by(|a, b| match a.literal_spl.cmp(&b.literal_spl) {
            std::cmp::Ordering::Equal => a.conclusion_type.cmp(&b.conclusion_type),
            other => other,
        });

        let output = ReasonOutput {
            schema_version: "spindle.reason.v1".to_string(),
            evaluated_at: pipeline_result
                .evaluated_at
                .and_then(|t: TimePoint| t.to_rfc3339()),
            grounding: GroundingStats {
                performed: pipeline_result.grounding_report.performed,
                had_variables: pipeline_result.grounding_report.had_variables,
                instances: pipeline_result.grounding_report.instances,
                limit_hit: pipeline_result.grounding_report.limit_hit,
            },
            conclusions: output_conclusions,
            diagnostics: vec![],
            stats: Some(TheoryStats {
                rule_count: pipeline_result.theory.rule_count(),
                fact_count: pipeline_result.theory.facts().count(),
            }),
        };

        CommandOutput::json(output)
    } else {
        let mut text = String::new();
        text.push_str("Conclusions:\n\n");

        for c in conclusions {
            if positive_only && !c.is_positive() {
                continue;
            }

            let symbol = match c.conclusion_type {
                ConclusionType::DefinitelyProvable => "+D",
                ConclusionType::DefinitelyNotProvable => "-D",
                ConclusionType::DefeasiblyProvable => "+d",
                ConclusionType::DefeasiblyNotProvable => "-d",
            };

            text.push_str(&format!("  {} {}\n", symbol, c.literal));
        }

        Ok(CommandOutput::text(text))
    }
}

// =============================================================================
// VALIDATE COMMAND
// =============================================================================

#[derive(serde::Serialize)]
struct ValidateOutput {
    valid: bool,
    diagnostics: Vec<Diagnostic>,
}

fn run_validate(
    file: Option<&PathBuf>,
    stdin: bool,
    json: bool,
) -> Result<CommandOutput, CliError> {
    let source = resolve_theory_source(file, stdin)?;
    let _theory = load_theory_source(&source)?;

    if json {
        CommandOutput::json(ValidateOutput {
            valid: true,
            diagnostics: vec![],
        })
    } else {
        Ok(CommandOutput::text("Valid theory file"))
    }
}

// =============================================================================
// STATS COMMAND
// =============================================================================

#[derive(serde::Serialize)]
struct StatsOutput {
    stats: StatsPayload,
    diagnostics: Vec<Diagnostic>,
}

#[derive(serde::Serialize)]
struct StatsPayload {
    total_rules: usize,
    facts: usize,
    strict: usize,
    defeasible: usize,
    defeaters: usize,
    superiorities: usize,
}

fn run_stats(file: Option<&PathBuf>, stdin: bool, json: bool) -> Result<CommandOutput, CliError> {
    let source = resolve_theory_source(file, stdin)?;
    let theory = load_theory_source(&source)?;

    let facts = theory.facts().count();
    let strict = theory
        .rules_by_type(spindle_core::rule::RuleType::Strict)
        .count();
    let defeasible = theory
        .rules_by_type(spindle_core::rule::RuleType::Defeasible)
        .count();
    let defeaters = theory
        .rules_by_type(spindle_core::rule::RuleType::Defeater)
        .count();

    let total_rules = theory.rule_count();
    let superiorities = theory.superiorities().len();

    if json {
        CommandOutput::json(StatsOutput {
            stats: StatsPayload {
                total_rules,
                facts,
                strict,
                defeasible,
                defeaters,
                superiorities,
            },
            diagnostics: vec![],
        })
    } else {
        let mut text = String::new();
        text.push_str("Theory Statistics:\n");
        text.push_str(&format!("  Total rules: {total_rules}\n"));
        text.push_str(&format!("    Facts:      {facts}\n"));
        text.push_str(&format!("    Strict:     {strict}\n"));
        text.push_str(&format!("    Defeasible: {defeasible}\n"));
        text.push_str(&format!("    Defeaters:  {defeaters}\n"));
        text.push_str(&format!("  Superiorities: {superiorities}"));

        Ok(CommandOutput::text(text))
    }
}

// =============================================================================
// QUERY COMMAND
// =============================================================================

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

fn run_query(
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

// =============================================================================
// EXPLAIN COMMAND
// =============================================================================

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

fn run_explain(
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

// =============================================================================
// WHY-NOT COMMAND
// =============================================================================

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

fn run_why_not(
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

// =============================================================================
// REQUIRES COMMAND
// =============================================================================

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

fn run_requires(
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

// =============================================================================
// CAPABILITIES COMMAND
// =============================================================================

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

fn run_capabilities(json: bool) -> Result<CommandOutput, CliError> {
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

// =============================================================================
// MAIN ENTRY POINT
// =============================================================================

fn main() {
    let json_requested_in_raw_args = std::env::args_os()
        .skip(1)
        .any(|arg| arg == std::ffi::OsStr::new("--json"));

    let cli =
        match Cli::try_parse() {
            Ok(cli) => cli,
            Err(err) => match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    // Keep help/version textual and user-friendly.
                    let _ = err.print();
                    std::process::exit(0);
                }
                kind => {
                    if json_requested_in_raw_args {
                        emit_and_exit(
                            Err(CliError::validation("CLI_PARSE_ERROR", err.to_string())
                                .with_details(serde_json::json!({
                                    "kind": format!("{kind:?}")
                                }))),
                            None,
                            true,
                        );
                    } else {
                        err.exit();
                    }
                }
            },
        };

    // Determine schema version and json flag based on the command before we move out of cli.command
    let (schema_version, json_flag) = match &cli.command {
        Commands::Reason { json, .. } => (Some("spindle.reason.v1"), *json || cli.json),
        Commands::Query { json, .. } => (Some("spindle.query.v1"), *json || cli.json),
        Commands::Explain { json, .. } => (Some("spindle.explain.v1"), *json || cli.json),
        Commands::WhyNot { json, .. } => (Some("spindle.why_not.v1"), *json || cli.json),
        Commands::Requires { json, .. } => (Some("spindle.requires.v1"), *json || cli.json),
        Commands::Capabilities { json, .. } => (Some("spindle.capabilities.v1"), *json || cli.json),
        Commands::Validate { .. } | Commands::Stats { .. } => (None, cli.json), // Non-schema commands
    };

    // Parse reference time if provided (after determining json_flag so errors use correct format)
    let reference_time = if let Some(ref s) = cli.at {
        match DateTime::parse_from_rfc3339(s) {
            Ok(dt) => Some(TimePoint::from_millis(dt.timestamp_millis())),
            Err(e) => {
                emit_and_exit(
                    Err(CliError::parse(
                        "INVALID_TIME_FORMAT",
                        format!("Error parsing time '{s}': {e}"),
                    )),
                    schema_version,
                    json_flag,
                );
            }
        }
    } else {
        None
    };

    let result: Result<CommandOutput, CliError> = match cli.command {
        Commands::Reason {
            file,
            scalable,
            positive,
            json: _,
        } => run_reason(
            file.as_ref(),
            scalable,
            positive,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::Validate { file, stdin } => {
            run_validate(file.as_ref(), stdin || cli.stdin, json_flag)
        }
        Commands::Stats { file, stdin } => run_stats(file.as_ref(), stdin || cli.stdin, json_flag),
        Commands::Query {
            literal,
            file,
            json: _,
        } => run_query(
            file.as_ref(),
            &literal,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::Explain {
            literal,
            file,
            json: _,
        } => run_explain(
            file.as_ref(),
            &literal,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::WhyNot {
            literal,
            file,
            json: _,
        } => run_why_not(
            file.as_ref(),
            &literal,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::Requires {
            literal,
            file,
            max,
            json: _,
        } => run_requires(
            file.as_ref(),
            &literal,
            max,
            json_flag,
            cli.stdin,
            reference_time,
        ),
        Commands::Capabilities { json: _ } => run_capabilities(json_flag),
    };

    emit_and_exit(result, schema_version, json_flag);
}
