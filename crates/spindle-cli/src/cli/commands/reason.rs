//! Reason command implementation

use std::collections::HashMap;
use std::path::PathBuf;

use spindle_core::conclusion::ConclusionType;
use spindle_core::pipeline::{PrepareOptions, compute_weighted_conclusions, prepare};
use spindle_core::projection::FamilyId;

use crate::cli::error::CliError;
use crate::cli::input::{load_theory_source, resolve_theory_source};
use crate::cli::output::CommandOutput;

pub(crate) struct ReasonOutputOptions {
    pub positive_only: bool,
    pub detailed: bool,
    pub json: bool,
    pub trust: bool,
    pub v2: bool,
}

pub(crate) fn run_reason(
    file: Option<&PathBuf>,
    stdin: bool,
    opts: PrepareOptions,
    output: ReasonOutputOptions,
) -> Result<CommandOutput, CliError> {
    let ReasonOutputOptions {
        positive_only,
        detailed,
        json,
        trust,
        v2,
    } = output;
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
        let rows: Vec<usize> = if detailed {
            conclusions
                .iter()
                .enumerate()
                .filter(|(_, c)| !positive_only || c.is_positive())
                .map(|(i, _)| i)
                .collect()
        } else {
            // Facts carry both +D and +d. Show each exact semantic literal once,
            // using its definite proof (and trust) when one is available.
            let mut selected = HashMap::new();
            for (i, c) in conclusions
                .iter()
                .enumerate()
                .filter(|(_, c)| c.is_positive())
            {
                let key = (FamilyId::from(&c.literal), c.literal.temporal.clone());
                selected
                    .entry(key)
                    .and_modify(|previous: &mut usize| {
                        if c.conclusion_type == ConclusionType::DefinitelyProvable
                            && conclusions[*previous].conclusion_type
                                != ConclusionType::DefinitelyProvable
                        {
                            *previous = i;
                        }
                    })
                    .or_insert(i);
            }
            let mut rows: Vec<_> = selected.into_values().collect();
            rows.sort_by_key(|&i| (conclusions[i].literal.to_spl(), i));
            rows
        };
        if !detailed && rows.is_empty() {
            return Ok(CommandOutput::text("No positive conclusions proved.\n"));
        }
        let mut text = if detailed {
            "Conclusions:\n\n"
        } else {
            "Proved:\n\n"
        }
        .to_string();
        for i in rows {
            let c = &conclusions[i];
            let prefix = if detailed {
                format!("{} ", c.conclusion_type.symbol())
            } else {
                String::new()
            };
            text.push_str(&format!("  {prefix}{}", c.literal.to_spl()));
            if let Some(wc) = weighted.as_ref().and_then(|wcs| wcs.get(i)) {
                text.push_str(&format!(" (trust: {:.2})", wc.degree));
                if !wc.sources.is_empty() {
                    let mut sources: Vec<_> = wc.sources.iter().map(|s| s.id.as_str()).collect();
                    sources.sort();
                    text.push_str(&format!(" [{}]", sources.join(", ")));
                }
            }
            text.push('\n');
        }

        Ok(CommandOutput::text(text))
    }
}
