//! Spindle CLI - Command-line interface for defeasible logic reasoning

use std::fs;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use spindle_core::conclusion::ConclusionType;
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
                println!();
                println!("Use --help for more information");
            }
        }
    }
}

fn run_reason(file: &PathBuf, scalable: bool, positive_only: bool) {
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    // Auto-detect SPL vs DFL based on file extension or content
    let is_spl = file.extension().map_or(false, |ext| ext == "spl")
        || content.trim().starts_with("#lang")
        || content.trim().starts_with('(')
        || content.trim().starts_with(';');

    let theory = if is_spl {
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
    };

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
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    match parse_dfl(&content) {
        Ok(theory) => {
            println!("Valid DFL file");
            println!("  Rules: {}", theory.rule_count());
            println!("  Facts: {}", theory.facts().count());
            println!("  Superiorities: {}", theory.superiorities().len());
        }
        Err(e) => {
            eprintln!("Invalid: {}", e);
            std::process::exit(1);
        }
    }
}

fn run_stats(file: &PathBuf) {
    let content = match fs::read_to_string(file) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    let theory = match parse_dfl(&content) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Parse error: {}", e);
            std::process::exit(1);
        }
    };

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
