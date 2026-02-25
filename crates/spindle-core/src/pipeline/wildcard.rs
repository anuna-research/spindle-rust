//! Wildcard rewrite pipeline stage.
//!
//! Rewrites anonymous wildcards (`_`) in rule bodies and heads to unique
//! fresh variables (`?_wN`) so that each wildcard position is treated as a
//! distinct variable during grounding.

use super::{PipelineContext, PipelineStage};
use crate::error::Result;
use crate::intern::{intern, resolve};
use crate::literal::Literal;
use crate::term::Term;
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
        let mut new_body: Vec<crate::body::BodyLiteral> = Vec::new();
        for bl in &rule.body {
            match bl {
                crate::body::BodyLiteral::Logic(logic_lit) => {
                    new_body.push(crate::body::BodyLiteral::Logic(
                        rewrite_body_logic_wildcards(logic_lit, &mut counter),
                    ));
                }
                crate::body::BodyLiteral::Arithmetic(c) => {
                    new_body.push(crate::body::BodyLiteral::Arithmetic(c.clone()));
                }
            }
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

/// Rewrite wildcards in a [`BodyLogicLiteral`], preserving `BodyArg::Arith` arguments.
///
/// Unlike `rewrite_literal_wildcards` which works on `Literal` (dropping arith args
/// via `to_literal()`), this function directly rewrites `BodyArg::Term` entries while
/// passing `BodyArg::Arith` entries through unchanged.
fn rewrite_body_logic_wildcards(
    lit: &crate::body::BodyLogicLiteral,
    counter: &mut usize,
) -> crate::body::BodyLogicLiteral {
    use crate::body::{BodyArg, BodyLogicLiteral};

    let name_id = if lit.name() == "_" {
        *counter += 1;
        intern(&format!("?_w{counter}"))
    } else {
        lit.name_id()
    };

    let args: Vec<BodyArg> = lit
        .predicate_args()
        .iter()
        .map(|a| match a {
            BodyArg::Term(t) => {
                if let Term::Symbol(id) = t
                    && resolve(*id) == "_"
                {
                    *counter += 1;
                    return BodyArg::Term(Term::Symbol(intern(&format!("?_w{counter}"))));
                }
                a.clone()
            }
            BodyArg::Arith(_) => a.clone(),
        })
        .collect();

    let mut result = BodyLogicLiteral::from_ids(
        name_id,
        lit.negation,
        lit.mode.clone(),
        lit.temporal.clone(),
        args,
    );
    result.temporal_expr = lit.temporal_expr.clone();
    result.interval_var = lit.interval_var;
    result
}

fn rewrite_literal_wildcards(lit: &Literal, counter: &mut usize) -> Literal {
    let name_id = if lit.name() == "_" {
        *counter += 1;
        intern(&format!("?_w{counter}"))
    } else {
        lit.name_id()
    };

    let args: Vec<Term> = lit
        .predicate_args()
        .iter()
        .map(|t| {
            if let Term::Symbol(id) = t
                && resolve(*id) == "_"
            {
                *counter += 1;
                return Term::Symbol(intern(&format!("?_w{counter}")));
            }
            t.clone()
        })
        .collect();

    let mut result = Literal::from_ids(
        name_id,
        lit.negation,
        lit.mode.clone(),
        lit.temporal.clone(),
        args,
    );
    // Propagate pre-grounding temporal expression (temporal variables)
    result.temporal_expr = lit.temporal_expr.clone();
    // Preserve single-variable interval binding from `(during ... ?T)`.
    result.interval_var = lit.interval_var;
    result
}
