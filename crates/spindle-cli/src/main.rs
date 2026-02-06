//! Spindle CLI - Command-line interface for defeasible logic reasoning

#[cfg(test)]
mod tests;

use std::fs;
use std::path::PathBuf;

use chrono::DateTime;
use clap::{Parser, Subcommand};
use serde::Serialize;
use spindle_core::conclusion::{Conclusion, ConclusionType};
use spindle_core::explanation::explain;
use spindle_core::literal::Literal;
use spindle_core::pipeline::{prepare, PrepareOptions};
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
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Show reasoning conclusions from a theory file
    Reason {
        /// Input file
        file: PathBuf,
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
        /// Input file
        file: PathBuf,
        /// Literal to query
        literal: String,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Explain why a conclusion holds
    Explain {
        /// Input file
        file: PathBuf,
        /// Literal to explain
        literal: String,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Explain why a conclusion does NOT hold
    WhyNot {
        /// Input file
        file: PathBuf,
        /// Literal to check
        literal: String,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
    /// Find facts needed to derive a literal
    Requires {
        /// Input file
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
}

fn main() {
    let cli = Cli::parse();

    // Parse reference time if provided
    let reference_time = if let Some(ref s) = cli.at {
        match DateTime::parse_from_rfc3339(s) {
            Ok(dt) => Some(TimePoint::from_millis(dt.timestamp_millis())),
            Err(e) => {
                eprintln!("Error parsing time '{s}': {e}");
                std::process::exit(1);
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
            run_reason(&file, scalable, positive, json, reference_time);
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
            run_query(&file, &literal, json, reference_time);
        }
        Some(Commands::Explain {
            file,
            literal,
            json,
        }) => {
            run_explain(&file, &literal, json, reference_time);
        }
        Some(Commands::WhyNot {
            file,
            literal,
            json,
        }) => {
            run_why_not(&file, &literal, json, reference_time);
        }
        Some(Commands::Requires {
            file,
            literal,
            max,
            json,
        }) => {
            run_requires(&file, &literal, max, json, reference_time);
        }
        None => {
            if let Some(file) = cli.input {
                run_reason(&file, cli.scalable, cli.positive, cli.json, reference_time);
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
                println!();
                println!("Use --help for more information");
            }
        }
    }
}

fn load_theory(file: &PathBuf) -> spindle_core::Theory {
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {e}");
            std::process::exit(1);
        }
    };

    // Auto-detect SPL vs DFL based on file extension or content
    let is_spl = file.extension().is_some_and(|ext| ext == "spl")
        || content.trim().starts_with("#lang")
        || content.trim().starts_with('(')
        || content.trim().starts_with(';');

    if is_spl {
        match parse_spl(&content) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("SPL parse error: {e}");
                std::process::exit(1);
            }
        }
    } else {
        match parse_dfl(&content) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("DFL parse error: {e}");
                std::process::exit(1);
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

#[derive(serde::Serialize)]
struct ReasonOutput {
    schema_version: String,
    evaluated_at: Option<String>,
    grounding: GroundingStats,
    conclusions: Vec<ConclusionStruct>,
    stats: TheoryStats,
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
    literal_struct: Literal,
    positive: bool,
}

#[derive(serde::Serialize)]
struct TheoryStats {
    rule_count: usize,
    fact_count: usize,
}

fn run_reason(
    file: &PathBuf,
    scalable: bool,
    positive_only: bool,
    json_output: bool,
    reference_time: Option<TimePoint>,
) {
    let theory = load_theory(file);

    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };

    // Run the pipeline (validation, temporal filtering, grounding)
    // This enforces range restriction and other checks
    let pipeline_result = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(1);
        }
    };

    let conclusions = if scalable {
        let indexed = spindle_core::index::IndexedTheory::build(&pipeline_result.theory);
        let result = spindle_core::scalable::reason_scalable(&indexed);
        result.to_conclusions(&indexed)
    } else {
        // Pass the prepared theory. reason() will call prepare() again,
        // but since it's already grounded, it will be a fast no-op for grounding.
        match spindle_core::reason::reason(&pipeline_result.theory) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error during reasoning: {e}");
                std::process::exit(1);
            }
        }
    };

    if json_output {
        let output_conclusions: Vec<ConclusionStruct> = conclusions
            .into_iter()
            .filter(|c| !positive_only || c.is_positive())
            .map(|c| ConclusionStruct {
                conclusion_type: c.conclusion_type.symbol().to_string(),
                literal_spl: c.literal.to_string(), // Display impl is SPL format
                literal_struct: c.literal.clone(),
                positive: c.is_positive(),
            })
            .collect();

        let output = ReasonOutput {
            schema_version: "spindle.reason.v1".to_string(),
            evaluated_at: pipeline_result
                .evaluated_at
                .map(|t: TimePoint| t.to_string()), // Use actual evaluated_at from pipeline
            grounding: GroundingStats {
                performed: pipeline_result.grounding_report.performed,
                had_variables: pipeline_result.grounding_report.had_variables,
                instances: pipeline_result.grounding_report.instances,
                limit_hit: pipeline_result.grounding_report.limit_hit,
            },
            conclusions: output_conclusions,
            stats: TheoryStats {
                rule_count: pipeline_result.theory.rule_count(),
                fact_count: pipeline_result.theory.facts().count(),
            },
        };

        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("Conclusions:");
        println!();

        for c in &conclusions {
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

fn run_validate(file: &PathBuf) {
    let _theory = load_theory(file);
    // If load_theory returns, parsing succeeded
    println!("Valid theory file");
}

fn run_stats(file: &PathBuf) {
    let theory = load_theory(file);

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

fn run_query(file: &PathBuf, literal: &str, json: bool, reference_time: Option<TimePoint>) {
    let theory = load_theory(file);
    
    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let prepared = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(1);
        }
    };

    let lit = parse_literal_arg(literal);
    let result = match query(&prepared.theory, &lit) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error querying literal: {e}");
            std::process::exit(1);
        }
    };

    if json {
        use serde_json::json;
        let output = json!({
            "literal": result.literal.to_string(),
            "status": result.status.to_string(),
            "conclusion_type": result.conclusion_type.map(|ct| ct.symbol()),
        });
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
}

fn run_explain(file: &PathBuf, literal: &str, json: bool, reference_time: Option<TimePoint>) {
    let theory = load_theory(file);
    
    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let prepared = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(1);
        }
    };

    let lit = parse_literal_arg(literal);

    match explain(&prepared.theory, &lit) {
        Ok(Some(explanation)) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&explanation.to_json()).unwrap()
                );
            } else {
                println!("{}", explanation.to_natural_language());
            }
        }
        Ok(None) => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "error": "Literal is not provable",
                        "literal": lit.to_string()
                    })
                );
            } else {
                println!("{lit} is not provable.");
                println!("Use 'spindle why-not' to see why.");
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Error explaining literal: {e}");
            std::process::exit(1);
        }
    }
}

fn run_why_not(file: &PathBuf, literal: &str, json: bool, reference_time: Option<TimePoint>) {
    let theory = load_theory(file);
    
    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let prepared = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(1);
        }
    };

    let lit = parse_literal_arg(literal);
    let result = match why_not(&prepared.theory, &lit) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error checking why-not: {e}");
            std::process::exit(1);
        }
    };

    if json {
        use serde_json::json;
        let blockers: Vec<_> = result
            .blocked_by
            .iter()
            .map(|b| {
                json!({
                    "type": b.blocking_type.to_string(),
                    "rule": b.rule_label,
                    "explanation": b.explanation
                })
            })
            .collect();

        let output = json!({
            "literal": result.literal.to_string(),
            "would_derive": result.would_derive,
            "blocked_by": blockers
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("{result}");
    }
}

fn run_requires(file: &PathBuf, literal: &str, max: usize, json: bool, reference_time: Option<TimePoint>) {
    let theory = load_theory(file);
    
    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let prepared = match prepare(&theory, opts) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Error during preparation: {e}");
            std::process::exit(1);
        }
    };

    let lit = parse_literal_arg(literal);
    let result = match abduce(&prepared.theory, &lit, max) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error finding requirements: {e}");
            std::process::exit(1);
        }
    };

    if json {
        use serde_json::json;
        let solutions: Vec<_> = result
            .solutions
            .iter()
            .map(|s| {
                let facts: Vec<_> = s.facts.iter().map(|l| l.to_string()).collect();
                json!({
                    "facts": facts,
                    "confidence": s.confidence
                })
            })
            .collect();

        let output = json!({
            "goal": result.goal.to_string(),
            "solutions": solutions
        });
        println!("{}", serde_json::to_string_pretty(&output).unwrap());
    } else {
        println!("{result}");
    }
}
