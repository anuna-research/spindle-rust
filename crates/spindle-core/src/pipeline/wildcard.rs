//! Wildcard rewrite pipeline stage.
//!
//! Rewrites anonymous wildcards (`_`) in rule bodies and heads to unique
//! fresh variables (`?_wN`) so that each wildcard position is treated as a
//! distinct variable during grounding.

use super::{PipelineContext, PipelineStage};
use crate::error::Result;
use crate::literal::Literal;
use crate::theory::Theory;

/// Rewrites anonymous wildcards (`_`) to unique variables (`?_wN`).
#[derive(Debug, Clone, Copy)]
pub struct WildcardRewrite;

impl PipelineStage for WildcardRewrite {
    fn name(&self) -> &'static str {
        "wildcard_rewrite"
    }

    fn apply(&self, theory: Theory, _ctx: &mut PipelineContext) -> Result<Theory> {
        Ok(rewrite_wildcards(&theory))
    }
}

fn rewrite_wildcards(theory: &Theory) -> Theory {
    let mut new_theory = Theory::new();
    new_theory.copy_metadata_from(theory);
    *new_theory.trust_policy_mut() = theory.trust_policy().clone();

    // Copy superiorities
    for sup in theory.superiorities() {
        new_theory.add_superiority(&sup.superior, &sup.inferior);
    }

    let mut counter = 0;

    for rule in theory.rules() {
        let mut new_rule = rule.clone();

        // Rewrite body literals
        let mut new_body = Vec::new();
        for lit in &rule.body {
            new_body.push(rewrite_literal_wildcards(lit, &mut counter));
        }
        new_rule.body = new_body.into();

        // Rewrite head literals (though discouraged, handling them keeps consistency)
        // Spec says they are rejected by validation, but if validation is off, this is safer.
        let mut new_head = Vec::new();
        for lit in &rule.head {
            new_head.push(rewrite_literal_wildcards(lit, &mut counter));
        }
        new_rule.head = new_head.into();

        new_theory.add_rule(new_rule);
    }
    new_theory
}

fn rewrite_literal_wildcards(lit: &Literal, counter: &mut usize) -> Literal {
    let name = if lit.name() == "_" {
        *counter += 1;
        format!("?_w{counter}")
    } else {
        lit.name().to_string()
    };

    let predicates = lit
        .predicates()
        .iter()
        .map(|p| {
            if *p == "_" {
                *counter += 1;
                format!("?_w{counter}")
            } else {
                p.to_string()
            }
        })
        .collect();

    let mut result = Literal::new(
        name,
        lit.negation,
        lit.mode.clone(),
        lit.temporal.clone(),
        predicates,
    );
    // Propagate pre-grounding temporal expression (temporal variables)
    result.temporal_expr = lit.temporal_expr.clone();
    // Preserve single-variable interval binding from `(during ... ?T)`.
    result.interval_var = lit.interval_var;
    result
}
