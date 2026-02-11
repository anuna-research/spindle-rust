//! Temporal filtering pipeline stage.
//!
//! Provides the [`TemporalFilter`] stage which removes rules and facts that
//! are not active at a given reference [`TimePoint`] ("as-of" semantics).

use super::{Diagnostic, MetadataVal, PipelineContext, PipelineStage, Severity};
use crate::error::Result;
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
        // A rule is active if ALL its body literals and its head literals are active
        // Wait, spec says:
        // - Rule firing at time t requires all body literals be active at t.
        // - A rule can only derive a head literal that is active at t.
        // So we filter the RULES themselves based on their literals.
        // Actually, we should probably keep the rule if it COULD be active,
        // but strict filtering removes it if ANY literal is definitely inactive (disjoint).
        // Since we don't have interval sets yet, we just check if the literal's temporal
        // includes t.

        // Check head
        let head_active = rule
            .head
            .iter()
            .all(|lit| lit.temporal.is_empty() || lit.temporal.active_at(t));

        // Check body
        let body_active = rule
            .body
            .iter()
            .all(|lit| lit.temporal.is_empty() || lit.temporal.active_at(t));

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
