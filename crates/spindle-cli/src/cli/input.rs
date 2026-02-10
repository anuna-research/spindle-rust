//! Theory source resolution and loading
//!
//! Handles input from files and stdin, auto-detecting SPL vs DFL format.

use std::path::PathBuf;

use spindle_core::literal::Literal;
use spindle_parser::spl::parse_spl as parse_spl_str;
use spindle_parser::{parse_dfl, parse_spl};

use super::error::CliError;

/// Theory source enum - exactly one source per invocation (per contract §5.1)
#[derive(Debug)]
pub(crate) enum TheorySource {
    File(PathBuf),
    Stdin,
}

/// Resolve theory source with mutual exclusivity validation.
/// Returns error if both file and stdin provided, or neither provided.
pub(crate) fn resolve_theory_source(
    file: Option<&PathBuf>,
    stdin: bool,
) -> Result<TheorySource, CliError> {
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
pub(crate) fn load_theory_source(
    source: &TheorySource,
) -> Result<spindle_core::Theory, CliError> {
    match source {
        TheorySource::File(path) => load_theory_from_file(path),
        TheorySource::Stdin => load_theory_from_stdin(),
    }
}

fn load_theory_from_file(file: &PathBuf) -> Result<spindle_core::Theory, CliError> {
    let content = std::fs::read_to_string(file).map_err(|e| {
        CliError::validation(
            "FILE_READ_ERROR",
            format!("Error reading file '{}': {}", file.display(), e),
        )
    })?;
    parse_theory_content(&content, Some(file))
}

fn load_theory_from_stdin() -> Result<spindle_core::Theory, CliError> {
    use std::io::Read;

    let mut content = String::new();
    std::io::stdin().read_to_string(&mut content).map_err(|e| {
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

pub(crate) fn parse_literal_arg(s: &str) -> Result<Literal, CliError> {
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
