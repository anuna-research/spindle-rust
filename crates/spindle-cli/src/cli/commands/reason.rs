//! Reason command implementation

use std::path::PathBuf;

use spindle_core::conclusion::ConclusionType;
use spindle_core::pipeline::{PrepareOptions, compute_weighted_conclusions, prepare};

use crate::cli::error::CliError;
use crate::cli::input::{load_theory_source, resolve_theory_source};
use crate::cli::output::CommandOutput;

pub(crate) fn run_reason(
    file: Option<&PathBuf>,
    positive_only: bool,
    json: bool,
    stdin: bool,
    opts: PrepareOptions,
    trust: bool,
    v2: bool,
) -> Result<CommandOutput, CliError> {
    let source = resolve_theory_source(file, stdin)?;
    let theory = load_theory_source(&source)?;

    let pipeline_result = prepare(&theory, opts).map_err(|e| {
        CliError::execution(
            "PREPARATION_ERROR",
            format!("Error during preparation: {e}"),
        )
    })?;

    let conclusions =
        spindle_core::reason::reason_prepared(&pipeline_result.theory).map_err(|e| {
            CliError::execution("REASONING_ERROR", format!("Error during reasoning: {e}"))
        })?;

    // Compute trust-weighted conclusions if requested
    let weighted = if trust {
        let policy = pipeline_result.theory.trust_policy();
        Some(compute_weighted_conclusions(
            &conclusions,
            &pipeline_result.theory,
            policy,
            pipeline_result.evaluated_at,
        ))
    } else {
        None
    };

    if json {
        CommandOutput::json(spindle_contract::reason::reason_output(
            &pipeline_result,
            &conclusions,
            weighted.as_deref(),
            positive_only,
            v2,
        ))
    } else {
        let mut text = String::new();
        text.push_str("Conclusions:\n\n");

        for (i, c) in conclusions.iter().enumerate() {
            if positive_only && !c.is_positive() {
                continue;
            }

            let symbol = match c.conclusion_type {
                ConclusionType::DefinitelyProvable => "+D",
                ConclusionType::DefinitelyNotProvable => "-D",
                ConclusionType::DefeasiblyProvable => "+d",
                ConclusionType::DefeasiblyNotProvable => "-d",
            };

            if let Some(ref wcs) = weighted {
                if let Some(wc) = wcs.get(i) {
                    let sources_str = if wc.sources.is_empty() {
                        String::new()
                    } else {
                        let mut src_list: Vec<String> =
                            wc.sources.iter().map(|s| s.id.clone()).collect();
                        src_list.sort();
                        format!(" [{}]", src_list.join(", "))
                    };
                    text.push_str(&format!(
                        "  {} {} (trust: {:.2}){}\n",
                        symbol, c.literal, wc.degree, sources_str
                    ));
                } else {
                    text.push_str(&format!("  {} {}\n", symbol, c.literal));
                }
            } else {
                text.push_str(&format!("  {} {}\n", symbol, c.literal));
            }
        }

        Ok(CommandOutput::text(text))
    }
}
