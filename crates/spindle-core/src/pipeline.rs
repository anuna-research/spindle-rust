//! Pipeline - The central processing pipeline for preparing theories
//!
//! This module implements the `prepare()` function which standardizes the
//! processing of theories before reasoning, including:
//! 1. Validation (range restriction, wildcards)
//! 2. Temporal filtering (Phase T1 "as-of" semantics)
//! 3. Grounding (variable instantiation)
//! 4. Indexing (AtomKey/LitId generation)

use crate::conclusion::Conclusion;
use crate::error::{Result, SpindleError};
use crate::grounding::{ground_theory_with_limit, has_variables, is_variable};
use crate::literal::Literal;
use crate::temporal::TimePoint;
use crate::theory::{MetaValue, Theory};
use crate::trust::{Source, TrustDerivationNode, TrustPolicy, TrustValue, WeightedConclusion};
use std::collections::{HashMap, HashSet};

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
    /// Optional trust policy for computing weighted conclusions.
    /// If None, the policy from the parsed theory is used.
    pub trust_policy: Option<TrustPolicy>,
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
    /// Trust-weighted conclusions (populated when a trust policy is available)
    pub weighted_conclusions: Vec<WeightedConclusion>,
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
        weighted_conclusions: Vec::new(),
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

    Literal::new(
        name,
        lit.negation,
        lit.mode.clone(),
        lit.temporal.clone(),
        predicates,
    )
}

/// Resolve a rule label to its trust value and optional source.
///
/// Handles grounded instances by falling back to the template label for metadata lookup.
fn resolve_rule_trust(
    rule_label: &str,
    theory: &Theory,
    policy: &TrustPolicy,
) -> (TrustValue, Option<Source>) {
    // Try the rule label directly, then fall back to template label
    let labels_to_try = if let Some(rule) = theory.get_rule(rule_label) {
        let tl = rule.template_label();
        if tl != rule_label {
            vec![rule_label, tl]
        } else {
            vec![rule_label]
        }
    } else {
        vec![rule_label]
    };

    for label in labels_to_try {
        if let Some(meta) = theory.get_meta(label)
            && let Some(MetaValue::String(source_id)) = meta.properties.get("source")
        {
            let trust = policy.get_trust(source_id);
            return (trust, Some(Source::new(source_id.clone())));
        }
    }
    (policy.default_trust, None)
}

/// Build a trust derivation tree for a literal by tracing back through deriving rules.
fn build_trust_tree(
    literal: &Literal,
    rule_label: Option<&str>,
    positive_conclusions: &HashMap<String, &Conclusion>,
    theory: &Theory,
    policy: &TrustPolicy,
    visited: &mut HashSet<String>,
) -> TrustDerivationNode {
    let canonical = literal.canonical_name();

    // Cycle detection
    if visited.contains(&canonical) {
        return TrustDerivationNode::new(literal.clone(), policy.default_trust);
    }
    visited.insert(canonical.clone());

    let (trust, source) = if let Some(label) = rule_label {
        resolve_rule_trust(label, theory, policy)
    } else {
        (policy.default_trust, None)
    };

    let mut node = TrustDerivationNode::new(literal.clone(), trust);
    if let Some(src) = source {
        node = node.with_source(src);
    }

    // Recurse into body literals of the deriving rule
    if let Some(label) = rule_label
        && let Some(rule) = theory.get_rule(label)
    {
        let mut children = Vec::new();
        for body_lit in &rule.body {
            let body_canonical = body_lit.canonical_name();
            if let Some(body_conclusion) = positive_conclusions.get(&body_canonical) {
                let child = build_trust_tree(
                    body_lit,
                    body_conclusion.rule_label.as_deref(),
                    positive_conclusions,
                    theory,
                    policy,
                    visited,
                );
                children.push(child);
            }
        }
        if !children.is_empty() {
            node = node.with_children(children);
        }
    }

    visited.remove(&canonical);
    node
}

/// Recursively collect all sources from a derivation tree.
fn collect_tree_sources(node: &TrustDerivationNode) -> HashSet<Source> {
    let mut sources = HashSet::new();
    if let Some(ref src) = node.source {
        sources.insert(src.clone());
    }
    for child in &node.children {
        sources.extend(collect_tree_sources(child));
    }
    sources
}

/// Compute trust-weighted conclusions using weakest-link propagation through derivation chains.
///
/// For each positive conclusion, traces the full derivation chain back through
/// body literals, building a trust tree. The conclusion's degree is the minimum
/// trust value across the entire chain (weakest link). Sources from all chain
/// nodes are collected.
///
/// Negative conclusions get the default trust value.
pub fn compute_weighted_conclusions(
    conclusions: &[Conclusion],
    theory: &Theory,
    policy: &TrustPolicy,
) -> Vec<WeightedConclusion> {
    // Build index of positive conclusions for fast body-literal lookup
    let positive_conclusions: HashMap<String, &Conclusion> = conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| (c.literal.canonical_name(), c))
        .collect();

    conclusions
        .iter()
        .map(|c| {
            let (degree, sources) = if c.is_positive() {
                let tree = build_trust_tree(
                    &c.literal,
                    c.rule_label.as_deref(),
                    &positive_conclusions,
                    theory,
                    policy,
                    &mut HashSet::new(),
                );
                let degree = tree.weakest_link_trust();
                let sources = collect_tree_sources(&tree);
                (degree, sources)
            } else {
                (policy.default_trust, HashSet::new())
            };

            let mut wc = WeightedConclusion::new(c.literal.clone(), c.conclusion_type, degree);
            wc.sources = sources;

            // Evaluate all named thresholds
            for (name, &threshold_val) in &policy.thresholds {
                wc.above_threshold
                    .insert(name.clone(), degree >= threshold_val);
            }

            wc
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::reason::reason_prepared;

    /// Helper: run full pipeline (reason + compute_weighted_conclusions) and
    /// return the WeightedConclusion for a specific literal+conclusion_type.
    fn weighted_for(
        theory: &Theory,
        policy: &TrustPolicy,
        literal_name: &str,
        ctype: ConclusionType,
    ) -> Option<WeightedConclusion> {
        let conclusions = reason_prepared(theory).unwrap();
        let weighted = compute_weighted_conclusions(&conclusions, theory, policy);
        weighted
            .into_iter()
            .find(|wc| wc.literal.canonical_name() == literal_name && wc.conclusion_type == ctype)
    }

    #[test]
    fn test_single_fact_degree_equals_source_trust() {
        let mut theory = Theory::new();
        let f1 = theory.add_fact("a");
        theory.add_meta_string(&f1, "source", "alice");

        let policy = TrustPolicy::new(0.5).with_trust("alice", 0.8);

        let wc = weighted_for(&theory, &policy, "a", ConclusionType::DefinitelyProvable).unwrap();
        assert!((wc.degree - 0.8).abs() < 1e-10);
        assert!(wc.sources.contains(&Source::new("alice")));
    }

    #[test]
    fn test_two_step_chain_weakest_link() {
        // fact a (source: alice, trust 0.6) => rule r1 (source: bob, trust 0.9) => b
        // weakest link = 0.6
        let mut theory = Theory::new();
        let f1 = theory.add_fact("a");
        theory.add_meta_string(&f1, "source", "alice");
        let r1 = theory.add_defeasible_rule(&["a"], "b");
        theory.add_meta_string(&r1, "source", "bob");

        let policy = TrustPolicy::new(0.5)
            .with_trust("alice", 0.6)
            .with_trust("bob", 0.9);

        let wc = weighted_for(&theory, &policy, "b", ConclusionType::DefeasiblyProvable).unwrap();
        assert!(
            (wc.degree - 0.6).abs() < 1e-10,
            "Expected 0.6 (weakest link), got {}",
            wc.degree
        );
        assert!(wc.sources.contains(&Source::new("alice")));
        assert!(wc.sources.contains(&Source::new("bob")));
    }

    #[test]
    fn test_three_step_chain_weakest_link() {
        // a(0.9) -> b(0.4) -> c(0.8) => min = 0.4
        let mut theory = Theory::new();
        let f1 = theory.add_fact("a");
        theory.add_meta_string(&f1, "source", "s1");
        let r1 = theory.add_defeasible_rule(&["a"], "b");
        theory.add_meta_string(&r1, "source", "s2");
        let r2 = theory.add_defeasible_rule(&["b"], "c");
        theory.add_meta_string(&r2, "source", "s3");

        let policy = TrustPolicy::new(0.5)
            .with_trust("s1", 0.9)
            .with_trust("s2", 0.4)
            .with_trust("s3", 0.8);

        let wc = weighted_for(&theory, &policy, "c", ConclusionType::DefeasiblyProvable).unwrap();
        assert!(
            (wc.degree - 0.4).abs() < 1e-10,
            "Expected 0.4, got {}",
            wc.degree
        );
        assert_eq!(wc.sources.len(), 3);
    }

    #[test]
    fn test_diamond_dependency() {
        // a(0.9) is used by two rules: r1(0.7) => b, r2(0.8) => c
        // Both b and c are used by r3(0.6) => d
        // Chain for d: min(0.6, min(0.7, 0.9), min(0.8, 0.9)) = min(0.6, 0.7, 0.8, 0.9) = 0.6
        let mut theory = Theory::new();
        let f1 = theory.add_fact("a");
        theory.add_meta_string(&f1, "source", "s_a");
        let r1 = theory.add_defeasible_rule(&["a"], "b");
        theory.add_meta_string(&r1, "source", "s_r1");
        let r2 = theory.add_defeasible_rule(&["a"], "c");
        theory.add_meta_string(&r2, "source", "s_r2");
        let r3 = theory.add_defeasible_rule(&["b", "c"], "d");
        theory.add_meta_string(&r3, "source", "s_r3");

        let policy = TrustPolicy::new(0.5)
            .with_trust("s_a", 0.9)
            .with_trust("s_r1", 0.7)
            .with_trust("s_r2", 0.8)
            .with_trust("s_r3", 0.6);

        let wc = weighted_for(&theory, &policy, "d", ConclusionType::DefeasiblyProvable).unwrap();
        assert!(
            (wc.degree - 0.6).abs() < 1e-10,
            "Expected 0.6, got {}",
            wc.degree
        );
    }

    #[test]
    fn test_missing_source_metadata_uses_default() {
        let mut theory = Theory::new();
        theory.add_fact("a"); // no source metadata
        theory.add_defeasible_rule(&["a"], "b"); // no source metadata

        let policy = TrustPolicy::new(0.5);

        let wc = weighted_for(&theory, &policy, "b", ConclusionType::DefeasiblyProvable).unwrap();
        assert!(
            (wc.degree - 0.5).abs() < 1e-10,
            "Expected default 0.5, got {}",
            wc.degree
        );
    }

    #[test]
    fn test_negative_conclusions_use_default_trust() {
        let mut theory = Theory::new();
        let f1 = theory.add_fact("a");
        theory.add_meta_string(&f1, "source", "alice");
        // Add a rule mentioning "b" so it appears in the literal set, but b is not provable
        theory.add_defeasible_rule(&["b"], "c");

        let policy = TrustPolicy::new(0.3).with_trust("alice", 0.9);

        let conclusions = reason_prepared(&theory).unwrap();
        let weighted = compute_weighted_conclusions(&conclusions, &theory, &policy);

        // "b" is not provable, so -D b and -d b should exist
        let neg = weighted
            .iter()
            .find(|wc| wc.literal.canonical_name() == "b" && !wc.conclusion_type.is_positive());
        assert!(neg.is_some());
        assert!((neg.unwrap().degree - 0.3).abs() < 1e-10);
    }

    #[test]
    fn test_multi_source_collection() {
        // Chain: a(alice) -> r1(bob) -> b -> r2(charlie) -> c
        // c should have all three sources
        let mut theory = Theory::new();
        let f1 = theory.add_fact("a");
        theory.add_meta_string(&f1, "source", "alice");
        let r1 = theory.add_defeasible_rule(&["a"], "b");
        theory.add_meta_string(&r1, "source", "bob");
        let r2 = theory.add_defeasible_rule(&["b"], "c");
        theory.add_meta_string(&r2, "source", "charlie");

        let policy = TrustPolicy::new(0.5)
            .with_trust("alice", 0.9)
            .with_trust("bob", 0.8)
            .with_trust("charlie", 0.7);

        let wc = weighted_for(&theory, &policy, "c", ConclusionType::DefeasiblyProvable).unwrap();
        assert_eq!(wc.sources.len(), 3);
        assert!(wc.sources.contains(&Source::new("alice")));
        assert!(wc.sources.contains(&Source::new("bob")));
        assert!(wc.sources.contains(&Source::new("charlie")));
    }

    #[test]
    fn test_threshold_evaluation_with_weakest_link() {
        let mut theory = Theory::new();
        let f1 = theory.add_fact("a");
        theory.add_meta_string(&f1, "source", "low_trust");
        let r1 = theory.add_defeasible_rule(&["a"], "b");
        theory.add_meta_string(&r1, "source", "high_trust");

        let policy = TrustPolicy::new(0.5)
            .with_trust("low_trust", 0.4)
            .with_trust("high_trust", 0.9)
            .with_threshold("action", 0.7)
            .with_threshold("warn", 0.3);

        let wc = weighted_for(&theory, &policy, "b", ConclusionType::DefeasiblyProvable).unwrap();
        // degree = 0.4 (weakest link)
        assert_eq!(wc.is_above_threshold("action"), Some(false));
        assert_eq!(wc.is_above_threshold("warn"), Some(true));
    }
}
