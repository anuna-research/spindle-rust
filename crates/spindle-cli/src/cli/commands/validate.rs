//! Validate command implementation

use std::path::PathBuf;

use crate::cli::error::{CliError, Diagnostic};
use crate::cli::input::{load_theory_source, resolve_theory_source};
use crate::cli::output::CommandOutput;

#[derive(serde::Serialize)]
struct ValidateOutput {
    valid: bool,
    diagnostics: Vec<Diagnostic>,
}

pub(crate) fn run_validate(
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
