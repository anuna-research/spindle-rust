//! Temporal pipeline stages.
//!
//! Provides:
//! - [`TemporalFilter`] — removes rules/facts not active at a reference [`TimePoint`]
//!   ("as-of" semantics).
//! - [`TemporalVarValidation`] — rejects theories with unresolved temporal variables
//!   after grounding.

use super::{Diagnostic, MetadataVal, PipelineContext, PipelineStage, Severity};
use crate::error::Result;
use crate::error::SpindleError;
use crate::temporal::TimePoint;
use crate::theory::Theory;

/// Filters the theory to include only rules/facts active at a reference time.
#[derive(Debug, Clone)]
pub struct TemporalFilter {
    /// The reference time for "as-of" filtering.
    pub reference_time: TimePoint,
}

impl PipelineStage for TemporalFilter {
    fn name(&self) -> &'static str {
        "temporal_filter"
    }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        let filtered = filter_temporal(&theory, self.reference_time);
        let removed = theory.rule_count() - filtered.rule_count();
        if removed > 0 {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Info,
                stage: self.name(),
                message: format!("removed {removed} temporally inactive rules"),
            });
        }
        ctx.metadata.insert(
            "evaluated_at".into(),
            MetadataVal::TimePoint(self.reference_time),
        );
        Ok(filtered)
    }
}

/// Filter theory to include only facts/rules active at the given timepoint.
pub(crate) fn filter_temporal(theory: &Theory, t: TimePoint) -> Theory {
    let mut new_theory = Theory::new();

    // Filter rules
    for rule in theory.rules() {
        // Check head — skip literals with unresolved temporal_expr (can't filter those)
        let head_active = rule.head.iter().all(|lit| {
            lit.temporal_expr.is_some() || lit.temporal.is_empty() || lit.temporal.active_at(t)
        });

        // Check body (only logic body literals have temporal)
        let body_active = rule.body.iter().all(|bl| match bl.as_logic() {
            Some(lit) => {
                lit.temporal_expr.is_some() || lit.temporal.is_empty() || lit.temporal.active_at(t)
            }
            None => true, // arithmetic constraints have no temporal
        });

        let rule_active = rule.temporal.is_empty() || rule.temporal.active_at(t);

        if rule_active && head_active && body_active {
            new_theory.add_rule(rule.clone());
        }
    }

    // Copy superiorities for kept rules
    for sup in theory.superiorities() {
        if new_theory.get_rule(&sup.superior).is_some()
            && new_theory.get_rule(&sup.inferior).is_some()
        {
            new_theory.add_superiority(&sup.superior, &sup.inferior);
        }
    }

    // Copy metadata and trust policy
    new_theory.copy_metadata_from(theory);
    *new_theory.trust_policy_mut() = theory.trust_policy().clone();

    new_theory
}

// ---------------------------------------------------------------------------
// TemporalVarValidation stage
// ---------------------------------------------------------------------------

/// Rejects theories that still contain unresolved temporal variables after grounding.
///
/// This stage should be placed **after** the [`Ground`](super::Ground) stage.
/// If any rule body or head literal still has a `temporal_expr` (meaning its
/// temporal variables were not fully bound during grounding), the stage emits
/// an error diagnostic. By default it returns the theory with a warning; set
/// `strict` to true to return `Err`.
#[derive(Debug, Clone)]
pub struct TemporalVarValidation {
    /// If true, unresolved temporal variables cause a hard error.
    /// If false (default), they produce a warning diagnostic.
    pub strict: bool,
}

impl Default for TemporalVarValidation {
    fn default() -> Self {
        Self { strict: true }
    }
}

impl PipelineStage for TemporalVarValidation {
    fn name(&self) -> &'static str {
        "temporal_var_validation"
    }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        let mut unresolved = Vec::new();

        for rule in theory.rules() {
            // Check body literals
            for bl in &rule.body {
                if bl.has_temporal_variables() {
                    unresolved.push(format!(
                        "rule '{}': literal '{}' has unresolved temporal variables",
                        rule.label,
                        bl.to_spl(),
                    ));
                }
            }
            // Check head literals
            for lit in &rule.head {
                if lit.has_temporal_variables() {
                    unresolved.push(format!(
                        "rule '{}': literal '{}' has unresolved temporal variables",
                        rule.label,
                        lit.to_spl(),
                    ));
                }
            }
        }

        if !unresolved.is_empty() {
            let message = format!(
                "{} unresolved temporal variable(s) after grounding:\n  {}",
                unresolved.len(),
                unresolved.join("\n  ")
            );

            if self.strict {
                return Err(SpindleError::Validation { message });
            }

            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                stage: self.name(),
                message,
            });
        }

        Ok(theory)
    }
}
