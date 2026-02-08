//! Pipeline - The central processing pipeline for preparing theories
//!
//! This module implements the `prepare()` function which standardizes the
//! processing of theories before reasoning, including:
//! 1. Validation (range restriction, wildcards)
//! 2. Temporal filtering (Phase T1 "as-of" semantics)
//! 3. Grounding (variable instantiation)
//! 4. Indexing (AtomKey/LitId generation)

use crate::error::{Result, SpindleError};
use crate::grounding::{ground_theory_with_limit, has_variables, is_variable};
use crate::literal::Literal;
use crate::temporal::TimePoint;
use crate::theory::Theory;
use std::collections::HashSet;

/// Options for the prepare pipeline
#[derive(Debug, Clone, Default)]
pub struct PrepareOptions {
    /// Reference time for "as-of" reasoning.
    /// If Some, only facts/rules active at this time are included.
    pub reference_time: Option<TimePoint>,
    /// Grounding configuration
    pub grounding: GroundingOptions,
    /// Validation configuration
    pub validation: ValidationOptions,
}

/// Options for grounding
#[derive(Debug, Clone)]
pub struct GroundingOptions {
    /// Whether grounding is enabled.
    pub enabled: bool,
    /// Maximum grounding iterations.
    pub max_iterations: usize,
    /// Maximum generated instances before stopping.
    pub max_instances: usize,
}

impl Default for GroundingOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_iterations: 100,
            max_instances: 10000,
        }
    }
}

/// Options for validation
#[derive(Debug, Clone)]
pub struct ValidationOptions {
    /// Enforce range restriction for rules.
    pub enforce_range_restricted: bool,
    /// Reject wildcard '_' usage in rule heads.
    pub reject_wildcard_in_head: bool,
}

impl Default for ValidationOptions {
    fn default() -> Self {
        Self {
            enforce_range_restricted: true,
            reject_wildcard_in_head: true,
        }
    }
}

/// Report on the grounding process
#[derive(Debug, Clone, Default)]
pub struct GroundingReport {
    /// Whether grounding ran.
    pub performed: bool,
    /// Whether the input contained variables.
    pub had_variables: bool,
    /// Number of grounded instances produced.
    pub instances: usize,
    /// Whether grounding stopped due to limits.
    pub limit_hit: bool,
}

/// Result of the prepare pipeline
pub struct PipelineResult {
    /// The prepared (grounded/filtered) theory
    pub theory: Theory,
    /// The time at which the theory was evaluated (if any)
    pub evaluated_at: Option<TimePoint>,
    /// Report on grounding statistics
    pub grounding_report: GroundingReport,
}

/// Prepare a theory for reasoning
///
/// This is the main entry point for the reasoning pipeline. It handles:
/// - Validation
/// - Temporal filtering
/// - Grounding
pub fn prepare(theory: &Theory, opts: PrepareOptions) -> Result<PipelineResult> {
    // 1. Temporal Filtering (Phase T1)
    let filtered_theory = if let Some(t) = opts.reference_time {
        filter_temporal(theory, t)
    } else {
        theory.clone()
    };

    // 2. Validation
    if opts.validation.reject_wildcard_in_head {
        validate_wildcards(&filtered_theory)?;
    }
    if opts.validation.enforce_range_restricted {
        validate_range_restriction(&filtered_theory)?;
    }

    // 3. Grounding
    // Rewrite wildcards (_) to unique variables before grounding
    let theory_with_rewrites = rewrite_wildcards(&filtered_theory);

    let (final_theory, report) = if opts.grounding.enabled {
        let had_vars = theory_with_rewrites.rules().any(has_variables);
        if had_vars {
            let (grounded, limit_hit) = ground_theory_with_limit(
                &theory_with_rewrites,
                opts.grounding.max_iterations,
                opts.grounding.max_instances,
            );
            // Rough instance count estimation
            let instances = grounded.rule_count();
            (
                grounded,
                GroundingReport {
                    performed: true,
                    had_variables: true,
                    instances,
                    limit_hit,
                },
            )
        } else {
            (
                theory_with_rewrites,
                GroundingReport {
                    performed: true,
                    had_variables: false,
                    instances: 0,
                    limit_hit: false,
                },
            )
        }
    } else {
        (
            theory_with_rewrites,
            GroundingReport {
                performed: false,
                had_variables: false,
                instances: 0,
                limit_hit: false,
            },
        )
    };

    Ok(PipelineResult {
        theory: final_theory,
        evaluated_at: opts.reference_time,
        grounding_report: report,
    })
}

/// Filter theory to include only facts/rules active at the given timepoint
fn filter_temporal(theory: &Theory, t: TimePoint) -> Theory {
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

    // Copy metadata
    new_theory.copy_metadata_from(theory);

    new_theory
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

fn rewrite_wildcards(theory: &Theory) -> Theory {
    let mut new_theory = Theory::new();
    new_theory.copy_metadata_from(theory);

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
        format!("?_w{}", counter)
    } else {
        lit.name().to_string()
    };

    let predicates = lit
        .predicates()
        .iter()
        .map(|p| {
            if *p == "_" {
                *counter += 1;
                format!("?_w{}", counter)
            } else {
                p.to_string()
            }
        })
        .collect();

    Literal::new(
        name,
        lit.negation,
        lit.mode.clone(),
        lit.temporal.clone(),
        predicates,
    )
}
