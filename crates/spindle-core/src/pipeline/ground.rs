//! Ground pipeline stage -- bottom-up Datalog grounding of rules containing variables.
//!
//! This stage instantiates (grounds) rules that contain variables by iterating
//! a bottom-up fixpoint over the facts in the theory, up to configurable limits
//! on iterations and generated instances.

use super::{Diagnostic, MetadataVal, PipelineContext, PipelineStage, Severity};
use crate::error::Result;
use crate::function_registry::EvalContext;
use crate::grounding::{ground_theory_with_limit, has_variables};
use crate::theory::Theory;

/// Bottom-up Datalog grounding of rules containing variables.
#[derive(Debug, Clone)]
pub struct Ground {
    /// Maximum grounding iterations.
    pub max_iterations: usize,
    /// Maximum generated instances before stopping.
    pub max_instances: usize,
}

impl Default for Ground {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            max_instances: 10_000,
        }
    }
}

impl PipelineStage for Ground {
    fn name(&self) -> &'static str {
        "ground"
    }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        let had_vars = theory.rules().any(has_variables);
        if !had_vars {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Info,
                stage: self.name(),
                message: "no variables found; grounding skipped".into(),
            });
            ctx.metadata
                .insert("grounding_performed".into(), MetadataVal::Bool(false));
            ctx.metadata
                .insert("grounding_had_variables".into(), MetadataVal::Bool(false));
            ctx.metadata
                .insert("grounding_instances".into(), MetadataVal::Usize(0));
            ctx.metadata
                .insert("grounding_limit_hit".into(), MetadataVal::Bool(false));
            return Ok(theory);
        }

        let eval_ctx = match ctx.function_registry.as_ref() {
            Some(reg) => EvalContext::with_registry(reg),
            None => EvalContext::empty(),
        };
        let (grounded, limit_hit) =
            ground_theory_with_limit(&theory, self.max_iterations, self.max_instances, &eval_ctx);

        let instances = grounded.rule_count();
        ctx.metadata
            .insert("grounding_performed".into(), MetadataVal::Bool(true));
        ctx.metadata
            .insert("grounding_had_variables".into(), MetadataVal::Bool(true));
        ctx.metadata
            .insert("grounding_instances".into(), MetadataVal::Usize(instances));
        ctx.metadata
            .insert("grounding_limit_hit".into(), MetadataVal::Bool(limit_hit));

        if limit_hit {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                stage: self.name(),
                message: format!(
                    "grounding limit reached ({} instances, max {})",
                    instances, self.max_instances
                ),
            });
        }

        Ok(grounded)
    }
}
