//! Validate pipeline stage -- checks wildcard placement and range restriction.

use super::{Diagnostic, PipelineContext, PipelineStage, Severity};
use crate::error::{Result, SpindleError};
use crate::grounding::is_variable;
use crate::theory::Theory;
use std::collections::HashSet;

/// Validates wildcard placement and range restriction.
#[derive(Debug, Clone)]
pub struct Validate {
    /// Whether to enforce range restriction (head variables must appear in body).
    pub enforce_range_restricted: bool,
    /// Whether to reject wildcard `_` in rule heads.
    pub reject_wildcard_in_head: bool,
}

impl Default for Validate {
    fn default() -> Self {
        Self {
            enforce_range_restricted: true,
            reject_wildcard_in_head: true,
        }
    }
}

impl PipelineStage for Validate {
    fn name(&self) -> &'static str {
        "validate"
    }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        if self.reject_wildcard_in_head {
            validate_wildcards(&theory)?;
        }
        if self.enforce_range_restricted {
            validate_range_restriction(&theory)?;
        }
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            stage: self.name(),
            message: "validation passed".into(),
        });
        Ok(theory)
    }
}

fn validate_wildcards(theory: &Theory) -> Result<()> {
    for rule in theory.rules() {
        for head in &rule.head {
            if head.name() == "_" || head.predicates().contains(&"_") {
                return Err(SpindleError::Validation {
                    message: format!("Wildcard '_' found in rule head: {}", rule.label),
                });
            }
        }
    }
    Ok(())
}

fn validate_range_restriction(theory: &Theory) -> Result<()> {
    for rule in theory.rules() {
        // Collect body variables
        let mut body_vars = HashSet::new();
        for lit in &rule.body {
            if is_variable(lit.name()) {
                body_vars.insert(lit.name().to_string());
            }
            for pred in lit.predicates() {
                if is_variable(pred) {
                    body_vars.insert(pred.to_string());
                }
            }
        }

        // Check head variables
        for lit in &rule.head {
            if is_variable(lit.name()) && !body_vars.contains(lit.name()) {
                return Err(SpindleError::Validation {
                    message: format!(
                        "Unsafe rule '{}': variable {} in head but not in body",
                        rule.label,
                        lit.name()
                    ),
                });
            }
            for pred in lit.predicates() {
                if is_variable(pred) && !body_vars.contains(pred) {
                    return Err(SpindleError::Validation {
                        message: format!(
                            "Unsafe rule '{}': variable {} in head but not in body",
                            rule.label, pred
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}
