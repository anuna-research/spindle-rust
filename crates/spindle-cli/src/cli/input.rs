//! Theory source resolution and loading
//!
//! Handles input from files and stdin, parsing SPL format.

use std::path::PathBuf;

use spindle_core::literal::Literal;
use spindle_parser::parse_spl;
use spindle_parser::spl::parse_spl as parse_spl_str;

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
pub(crate) fn load_theory_source(source: &TheorySource) -> Result<spindle_core::Theory, CliError> {
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
        .with_source_name(file.display().to_string())
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
    let source_name = file
        .map(|f| f.display().to_string())
        .unwrap_or_else(|| "stdin".to_string());

    let enrich = |e: &spindle_parser::ParseError, cli_err: CliError| -> CliError {
        let mut err = cli_err.with_source_name(&source_name);
        if let Some(line) = e.line() {
            err = err.with_location(line, None);
            err = err.with_source_context(spindle_contract::error::SourceContext::from_source(
                content, line, 1,
            ));
        }
        err
    };

    let is_dfl_extension = file
        .and_then(|f| f.extension())
        .is_some_and(|ext| ext.to_string_lossy().eq_ignore_ascii_case("dfl"));

    if is_dfl_extension || has_dfl_shape(content) {
        return Err(CliError::validation(
            "UNSUPPORTED_INPUT_FORMAT",
            "DFL format is no longer supported; use SPL input.",
        )
        .with_source_name(&source_name)
        .with_details(serde_json::json!({
            "input_format": "dfl",
            "supported_formats": ["spl"],
            "hint": "Convert DFL rules to SPL, e.g. 'f1: >> bird' -> '(given bird)'."
        })));
    }

    parse_spl(content).map_err(|e| enrich(&e, CliError::parse(e.code(), e.to_string())))
}

fn has_dfl_shape(content: &str) -> bool {
    content.lines().any(is_dfl_line)
}

fn is_dfl_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return false;
    }

    if let Some((label, rest)) = trimmed.split_once(':') {
        let label = label.trim();
        if is_dfl_identifier(label)
            && (rest.contains(">>")
                || rest.contains("->")
                || rest.contains("=>")
                || rest.contains("~>"))
        {
            return true;
        }
    }

    if let Some((superior, inferior)) = trimmed.split_once('>') {
        let superior = superior.trim();
        let inferior = inferior.trim();
        if is_dfl_identifier(superior) && is_dfl_identifier(inferior) {
            return true;
        }
    }

    false
}

fn is_dfl_identifier(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
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

#[cfg(test)]
mod tests {
    use super::has_dfl_shape;

    #[test]
    fn detects_dfl_rule_shape() {
        assert!(has_dfl_shape("f1: >> bird\nr1: bird => flies\n"));
    }

    #[test]
    fn detects_dfl_superiority_shape() {
        assert!(has_dfl_shape("r2 > r1\n"));
    }

    #[test]
    fn does_not_flag_spl_input() {
        assert!(!has_dfl_shape("(given bird)\n(normally r1 bird flies)\n"));
    }
}
