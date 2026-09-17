//! CLI application scaffolding
//!
//! Clap-derived argument parser and command enum.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Spindle - Defeasible Logic Reasoning Engine
#[derive(Parser, Debug)]
#[command(name = "spindle")]
#[command(version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,

    /// Output in JSON format
    #[arg(long, global = true)]
    pub(crate) json: bool,

    /// Reference time for "as-of" reasoning (ISO 8601)
    #[arg(long, global = true)]
    pub(crate) at: Option<String>,

    /// Read theory from stdin
    #[arg(long, global = true)]
    pub(crate) stdin: bool,

    /// Load portable lookup functions and named aggregators from JSON
    #[arg(long, global = true)]
    pub(crate) extensions: Option<PathBuf>,

    /// Show full error details (source chain, unredacted paths)
    #[arg(long, global = true)]
    pub(crate) debug_errors: bool,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Commands {
    /// Explain a stable error code
    ExplainCode {
        /// The error code to explain (e.g. RULE_NOT_FOUND)
        code: String,
    },
    /// Show reasoning conclusions from a theory file
    Reason {
        /// Input file (mutually exclusive with --stdin)
        file: Option<PathBuf>,
        /// Only show positive conclusions
        #[arg(long)]
        positive: bool,
        /// Output in JSON format
        #[arg(long)]
        json: bool,
        /// Show trust weights for conclusions
        #[arg(long)]
        trust: bool,
        /// Use v2 JSON schema with typed term arguments
        #[arg(long)]
        v2: bool,
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
    /// Inspect predicate vocabulary, declarations, shapes and provenance
    Vocabulary { file: Option<PathBuf> },
    /// Reason with hypothetical facts without changing the input theory
    WhatIf {
        literal: String,
        file: Option<PathBuf>,
        /// A hypothetical literal; repeat for multiple facts
        #[arg(long = "given", required = true)]
        given: Vec<String>,
    },
    /// Generate raw (unverified) abduction candidates
    Abduce {
        literal: String,
        file: Option<PathBuf>,
        #[arg(long, default_value = "10")]
        max: usize,
    },
    /// Show spindle capabilities
    Capabilities {
        /// Output in JSON format
        #[arg(long)]
        json: bool,
    },
}
