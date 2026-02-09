//! Spindle CLI - Command-line interface for defeasible logic reasoning

#[cfg(test)]
mod tests;

use std::fs;
use std::path::PathBuf;

use chrono::DateTime;
use clap::{Parser, Subcommand};
use spindle_core::conclusion::ConclusionType;
use spindle_core::explanation::explain;
use spindle_core::literal::Literal;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::query::{QueryStatus, abduce, query, why_not};
use spindle_core::temporal::TimePoint;
use spindle_parser::spl::parse_spl as parse_spl_str;
use spindle_parser::{parse_dfl, parse_spl};

/// Spindle - Defeasible Logic Reasoning Engine
#[derive(Parser, Debug)]
#[command(name = "spindle")]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Input file (DFL format)
    #[arg(value_name = "FILE")]
    input: Option<PathBuf>,

    /// Use scalable reasoning mode (DL(d||))
    #[arg(long)]
    scalable: bool,

    /// Output only positive conclusions
    #[arg(long)]
    positive: bool,

    /// Output in JSON format
    #[arg(long)]
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
        /// Input file
        file: PathBuf,
    },
    /// Show theory statistics
    Stats {
        /// Input file
        file: PathBuf,
    },
    /// Query if a literal holds in the theory
    Query {
        /// Input file (use "-" as placeholder with --stdin)
        file: PathBuf,
        /// Literal to query
        literal: String,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Explain why a conclusion holds
    Explain {
        /// Input file (use "-" as placeholder with --stdin)
        file: PathBuf,
        /// Literal to explain
        literal: String,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Explain why a conclusion does NOT hold
    WhyNot {
        /// Input file (use "-" as placeholder with --stdin)
        file: PathBuf,
        /// Literal to check
        literal: String,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Find facts needed to derive a literal
    Requires {
        /// Input file (use "-" as placeholder with --stdin)
        file: PathBuf,
        /// Goal literal
        literal: String,
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

/// A diagnostic message for the output envelope
#[derive(serde::Serialize)]
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

    #[allow(dead_code)]
    fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
}

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

// =============================================================================
// CLI-specific JSON structs for contract-compliant serialization
// These convert core types to schema-compliant JSON (e.g., NegInf/PosInf -> null)
// =============================================================================

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

fn main() {
    let cli = Cli::parse();

    // Parse reference time if provided
    let reference_time = if let Some(ref s) = cli.at {
        match DateTime::parse_from_rfc3339(s) {
            Ok(dt) => Some(TimePoint::from_millis(dt.timestamp_millis())),
            Err(e) => {
                eprintln!("Error parsing time '{s}': {e}");
                std::process::exit(2);
            }
        }
    } else {
        None
    };

    match cli.command {
        Some(Commands::Reason {
            file,
            scalable,
            positive,
            json,
        }) => {
            run_reason(
                file.as_ref(),
                scalable,
                positive,
                json,
                cli.stdin,
                reference_time,
            );
        }
        Some(Commands::Validate { file }) => {
            run_validate(&file);
        }
        Some(Commands::Stats { file }) => {
            run_stats(&file);
        }
        Some(Commands::Query {
            file,
            literal,
            json,
        }) => {
            run_query(Some(&file), &literal, json, cli.stdin, reference_time);
        }
        Some(Commands::Explain {
            file,
            literal,
            json,
        }) => {
            run_explain(Some(&file), &literal, json, cli.stdin, reference_time);
        }
        Some(Commands::WhyNot {
            file,
            literal,
            json,
        }) => {
            run_why_not(Some(&file), &literal, json, cli.stdin, reference_time);
        }
        Some(Commands::Requires {
            file,
            literal,
            max,
            json,
        }) => {
            run_requires(Some(&file), &literal, max, json, cli.stdin, reference_time);
        }
        Some(Commands::Capabilities { json }) => {
            run_capabilities(json);
        }
        None => {
            if cli.stdin {
                run_reason(None, cli.scalable, cli.positive, cli.json, true, reference_time);
            } else if let Some(ref file) = cli.input {
                run_reason(Some(file), cli.scalable, cli.positive, cli.json, false, reference_time);
            } else {
                println!("Spindle v0.1.0 - Defeasible Logic Reasoning Engine");
                println!("Ported from SPINdle-Racket v1.7.0");
                println!();
                println!("Usage: spindle [OPTIONS] <FILE>");
                println!("       spindle reason <FILE>");
                println!("       spindle validate <FILE>");
                println!("       spindle stats <FILE>");
                println!("       spindle query <FILE> <LITERAL>");
                println!("       spindle explain <FILE> <LITERAL>");
                println!("       spindle why-not <FILE> <LITERAL>");
                println!("       spindle requires <FILE> <LITERAL>");
                println!("       spindle capabilities");
                println!();
                println!("Use --help for more information");
            }
        }
    }
}

fn load_theory_from_file(file: &PathBuf) -> spindle_core::Theory {
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {e}");
            std::process::exit(2);
        }
    };
    parse_theory_content(&content, Some(file))
}

fn load_theory_from_stdin() -> spindle_core::Theory {
    use std::io::{self, Read};
    
    let mut content = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut content) {
        eprintln!("Error reading from stdin: {e}");
        std::process::exit(2);
    }
    parse_theory_content(&content, None)
}

/// Resolve theory source with mutual exclusivity between file and --stdin.
/// Per contract §5.1: exactly one theory source per invocation.
///
/// For subcommands with two positional args (query, explain, why-not, requires),
/// clap requires the file positional even with --stdin. Users should pass "-"
/// as the file placeholder.
fn load_theory(file: Option<&PathBuf>, stdin: bool) -> spindle_core::Theory {
    match (file, stdin) {
        (Some(f), true) => {
            // For two-positional subcommands, file is always present due to clap.
            // Accept "-" as the explicit stdin placeholder.
            if f.as_os_str() != "-" {
                eprintln!(
                    "Error: cannot specify both file '{}' and --stdin",
                    f.display()
                );
                std::process::exit(2);
            }
            load_theory_from_stdin()
        }
        (Some(f), false) => load_theory_from_file(f),
        (None, true) => load_theory_from_stdin(),
        (None, false) => {
            eprintln!("Error: must specify either a file or --stdin");
            std::process::exit(2);
        }
    }
}

fn parse_theory_content(content: &str, file: Option<&PathBuf>) -> spindle_core::Theory {
    // Auto-detect SPL vs DFL based on file extension or content
    let is_spl = file.map(|f| f.extension().is_some_and(|ext| ext == "spl")).unwrap_or(false)
        || content.trim().starts_with("#lang")
        || content.trim().starts_with('(')
        || content.trim().starts_with(';');

    if is_spl {
        match parse_spl(content) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("SPL parse error: {e}");
                std::process::exit(2);
            }
        }
    } else {
        match parse_dfl(content) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("DFL parse error: {e}");
                std::process::exit(2);
            }
        }
    }
}

fn parse_literal_arg(s: &str) -> Literal {
    // If it looks like an SPL expression (starts with paren), try to parse it as a dummy fact
    // to extract the literal.
    // e.g. (gap_instance "..." ...)
    if s.trim().starts_with('(') {
        // Wrap in (given ...) so the full parser can handle it
        let dummy_spl = format!("(given {s})");
        if let Ok(theory) = parse_spl_str(&dummy_spl)
            && let Some(fact) = theory.facts().next()
        {
            // Return the literal from the first fact
            if let Some(head) = fact.head.first() {
                return head.clone();
            }
        }
    }

    // Fallback to simple parsing logic if SPL parse fails or it's not parenthesized
    if s.starts_with("(not ") && s.ends_with(')') {
        let inner = &s[5..s.len() - 1];
        Literal::negated(inner)
    } else if let Some(stripped) = s.strip_prefix('~') {
        Literal::negated(stripped)
    } else {
        Literal::simple(s)
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
    json_output: bool,
    stdin: bool,
    reference_time: Option<TimePoint>,
) {
    let theory = load_theory(file, stdin);

    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };

    let pipeline_result = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(3);
        }
    };

    let conclusions = if scalable {
        let indexed = spindle_core::index::IndexedTheory::build(&pipeline_result.theory);
        let result = spindle_core::scalable::reason_scalable(&indexed);
        result.to_conclusions(&indexed)
    } else {
        match spindle_core::reason::reason(&pipeline_result.theory) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error during reasoning: {e}");
                std::process::exit(3);
            }
        }
    };

    if json_output {
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

        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Conclusions:");
        println!();

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

            println!("  {} {}", symbol, c.literal);
        }
    }
}

// =============================================================================
// VALIDATE COMMAND
// =============================================================================

fn run_validate(file: &PathBuf) {
    let _theory = load_theory_from_file(file);
    println!("Valid theory file");
}

// =============================================================================
// STATS COMMAND
// =============================================================================

fn run_stats(file: &PathBuf) {
    let theory = load_theory_from_file(file);

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

    println!("Theory Statistics:");
    println!("  Total rules: {}", theory.rule_count());
    println!("    Facts:      {facts}");
    println!("    Strict:     {strict}");
    println!("    Defeasible: {defeasible}");
    println!("    Defeaters:  {defeaters}");
    println!("  Superiorities: {}", theory.superiorities().len());
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
) {
    let theory = load_theory(file, stdin);

    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let prepared = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(3);
        }
    };

    let lit = parse_literal_arg(literal);
    let result = match query(&prepared.theory, &lit) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error querying literal: {e}");
            std::process::exit(3);
        }
    };

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
            trust: None, // Trust not implemented in v1 yet
            diagnostics: vec![],
        };

        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        match result.status {
            QueryStatus::Provable => {
                let ct = result.conclusion_type.unwrap();
                println!("{} {}", ct.symbol(), result.literal);
            }
            QueryStatus::Refuted => {
                println!("Refuted: {}", result.literal);
            }
            QueryStatus::Unknown => {
                println!("Unknown: {}", result.literal);
            }
        }
    }

    // Exit code 0 for all logical outcomes per contract §8.2
    std::process::exit(0);
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
) {
    let theory = load_theory(file, stdin);

    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let prepared = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(3);
        }
    };

    let lit = parse_literal_arg(literal);

    // First query to get the status
    let query_result = match query(&prepared.theory, &lit) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error querying literal: {e}");
            std::process::exit(3);
        }
    };

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
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{}", explanation.to_natural_language());
            }
        }
        Ok(None) => {
            if json {
                // Per contract §8.2: explain with no proof tree is exit code 0
                let mut diagnostics = vec![];
                diagnostics.push(Diagnostic::warning(
                    "NOT_PROVABLE",
                    format!("Literal {} is not provable", lit)
                ));
                
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
                println!("{}", serde_json::to_string_pretty(&output).unwrap());
            } else {
                println!("{lit} is not provable.");
                println!("Use 'spindle why-not' to see why.");
            }
            // Exit code 0 per contract §8.2
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Error explaining literal: {e}");
            std::process::exit(3);
        }
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
) {
    let theory = load_theory(file, stdin);

    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let prepared = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(3);
        }
    };

    let lit = parse_literal_arg(literal);
    
    // Query to get the actual status (provable|refuted|unknown)
    let query_result = match query(&prepared.theory, &lit) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error querying literal: {e}");
            std::process::exit(3);
        }
    };
    
    let status = match query_result.status {
        QueryStatus::Provable => "provable",
        QueryStatus::Refuted => "refuted",
        QueryStatus::Unknown => "unknown",
    };
    
    let result = match why_not(&prepared.theory, &lit) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error checking why-not: {e}");
            std::process::exit(3);
        }
    };

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
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("{result}");
    }

    // Exit code 0 for all logical outcomes
    std::process::exit(0);
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
    // Facts as strings (per schema, must be homogeneous - all strings or all struct)
    facts: Vec<String>,
    // Score (not confidence) per contract
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
) {
    // Validate max parameter - must be at least 1 to satisfy contract
    if max == 0 {
        if json {
            let output = serde_json::json!({
                "schema_version": "spindle.requires.v1",
                "error": {
                    "code": "INVALID_ARGUMENT",
                    "message": "--max must be at least 1"
                },
                "diagnostics": [{
                    "severity": "error",
                    "code": "INVALID_ARGUMENT",
                    "message": "--max must be at least 1",
                    "details": {}
                }]
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        } else {
            eprintln!("Error: --max must be at least 1");
        }
        std::process::exit(2);
    }

    let theory = load_theory(file, stdin);

    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let prepared = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(3);
        }
    };

    let lit = parse_literal_arg(literal);
    let probe_max = max.saturating_add(1);
    let result = match abduce(&prepared.theory, &lit, probe_max) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error finding requirements: {e}");
            std::process::exit(3);
        }
    };

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
                format!("Results limited to {} solutions", max)
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
                        // Use confidence as score (renamed per contract)
                        score: s.confidence,
                    }
                })
                .collect()
        };

        // Sort solutions by set size then lexical order (per spec §7)
        let mut solutions = solutions;
        solutions.sort_by(|a, b| {
            match a.facts.len().cmp(&b.facts.len()) {
                std::cmp::Ordering::Equal => a.facts.cmp(&b.facts),
                other => other,
            }
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

        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("{result}");
    }

    // Exit code 0 per contract §8.2
    std::process::exit(0);
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

fn run_capabilities(json: bool) {
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
                given_flags: false, // Not yet implemented
                trust_overlay_v1: false, // Not yet implemented
                trust_explain_v1: false, // Not yet implemented
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
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Spindle Capabilities:");
        println!();
        println!("Commands: reason, query, requires, explain, why-not");
        println!();
        println!("Features:");
        println!("  --stdin: yes");
        println!("  --at: yes");
        println!("  --json: yes");
        println!("  Trust overlay: no");
        println!("  Given flags: no");
        println!();
        println!("Schema versions:");
        println!("  reason: spindle.reason.v1");
        println!("  query: spindle.query.v1");
        println!("  requires: spindle.requires.v1");
        println!("  explain: spindle.explain.v1");
        println!("  why-not: spindle.why_not.v1");
    }
}
