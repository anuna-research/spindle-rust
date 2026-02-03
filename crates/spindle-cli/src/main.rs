//! Spindle CLI - Command-line interface for defeasible logic reasoning

use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use spindle_core::conclusion::ConclusionType;
use spindle_core::explanation::explain;
use spindle_core::literal::Literal;
use spindle_core::query::{abduce, query, why_not, QueryStatus};
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

    match cli.command {
        Some(Commands::Reason {
            file,
            scalable,
            positive,
        }) => {
            run_reason(&file, scalable, positive);
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
            run_query(&file, &literal, json);
        }
        Some(Commands::Explain {
            file,
            literal,
            json,
        }) => {
            run_explain(&file, &literal, json);
        }
        Some(Commands::WhyNot {
            file,
            literal,
            json,
        }) => {
            run_why_not(&file, &literal, json);
        }
        Some(Commands::Requires {
            file,
            literal,
            max,
            json,
        }) => {
            run_requires(&file, &literal, max, json);
        }
        None => {
            if let Some(file) = cli.input {
                run_reason(&file, cli.scalable, cli.positive);
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
            eprintln!("Error reading file: {}", e);
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
                eprintln!("SPL parse error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        match parse_dfl(&content) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("DFL parse error: {}", e);
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
        let dummy_spl = format!("(given {})", s);
        if let Ok(theory) = parse_spl_str(&dummy_spl)
            && let Some(fact) = theory.facts().next() {
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

fn run_reason(file: &PathBuf, scalable: bool, positive_only: bool) {
    let theory = load_theory(file);

    let conclusions = if scalable {
        let result = spindle_core::scalable::reason_scalable(&theory);
        let indexed = spindle_core::index::IndexedTheory::build(theory.clone());
        result.to_conclusions(&indexed)
    } else {
        theory.reason()
    };

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
    println!("    Facts:      {}", facts);
    println!("    Strict:     {}", strict);
    println!("    Defeasible: {}", defeasible);
    println!("    Defeaters:  {}", defeaters);
    println!("  Superiorities: {}", theory.superiorities().len());
}

fn run_query(file: &PathBuf, literal: &str, json: bool) {
    let theory = load_theory(file);
    let lit = parse_literal_arg(literal);
    let result = query(&theory, &lit);

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

fn run_explain(file: &PathBuf, literal: &str, json: bool) {
    let theory = load_theory(file);
    let lit = parse_literal_arg(literal);

    match explain(&theory, &lit) {
        Some(explanation) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&explanation.to_json()).unwrap()
                );
            } else {
                println!("{}", explanation.to_natural_language());
            }
        }
        None => {
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "error": "Literal is not provable",
                        "literal": lit.to_string()
                    })
                );
            } else {
                println!("{} is not provable.", lit);
                println!("Use 'spindle why-not' to see why.");
            }
            std::process::exit(1);
        }
    }
}

fn run_why_not(file: &PathBuf, literal: &str, json: bool) {
    let theory = load_theory(file);
    let lit = parse_literal_arg(literal);
    let result = why_not(&theory, &lit);

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
        println!("{}", result);
    }
}

fn run_requires(file: &PathBuf, literal: &str, max: usize, json: bool) {
    let theory = load_theory(file);
    let lit = parse_literal_arg(literal);
    let result = abduce(&theory, &lit, max);

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
        println!("{}", result);
    }
}
