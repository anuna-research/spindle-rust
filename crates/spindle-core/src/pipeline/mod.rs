//! Pipeline - The central processing pipeline for preparing theories
//!
//! This module implements the `prepare()` function which standardizes the
//! processing of theories before reasoning, including:
//! 1. Validation (range restriction, wildcards)
//! 2. Temporal filtering (Phase T1 "as-of" semantics)
//! 3. Grounding (variable instantiation)
//! 4. Indexing (AtomKey/LitId generation)
//!
//! # Composable pipeline stages
//!
//! The pipeline is built from composable [`PipelineStage`] implementations
//! assembled via a [`PipelineBuilder`]. The default pipeline (matching the
//! legacy `prepare()` semantics) is available through [`Pipeline::default_pipeline()`].
//!
//! ```rust,no_run
//! use spindle_core::pipeline::{Pipeline, Validate, WildcardRewrite, Ground};
//!
//! let pipeline = Pipeline::builder()
//!     .stage(Validate::default())
//!     .stage(WildcardRewrite)
//!     .stage(Ground::default())
//!     .build();
//! ```

pub mod ground;
pub mod temporal;
pub mod validate;
pub mod wildcard;
pub use ground::Ground;
pub use temporal::{TemporalFilter, TemporalVarValidation};
pub use validate::Validate;
pub use wildcard::WildcardRewrite;

use crate::conclusion::{Conclusion, ConclusionType};
use crate::error::Result;
use crate::literal::Literal;
use crate::temporal::TimePoint;
use crate::theory::{MetaValue, Theory};
use crate::trust::{
    DiminisherInfo, Source, TrustDerivationNode, TrustPolicy, TrustValue, WeightedConclusion,
};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// PipelineStage trait and supporting types
// ---------------------------------------------------------------------------

/// Severity level for pipeline diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Informational message (e.g., "validation passed").
    Info,
    /// Non-fatal warning (e.g., "grounding limit approaching").
    Warning,
    /// Error collected for deferred reporting (stage may still return `Ok`).
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARN"),
            Self::Error => write!(f, "ERROR"),
        }
    }
}

/// A diagnostic message emitted by a pipeline stage.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// Severity of this diagnostic.
    pub severity: Severity,
    /// Name of the stage that produced it.
    pub stage: &'static str,
    /// Human-readable message.
    pub message: String,
}

/// A loosely-typed metadata value that stages can attach to the context.
#[derive(Debug, Clone)]
pub enum MetadataVal {
    /// Boolean metadata.
    Bool(bool),
    /// Unsigned integer metadata.
    Usize(usize),
    /// String metadata.
    String(String),
    /// TimePoint metadata.
    TimePoint(TimePoint),
}

/// Metadata and diagnostics accumulator threaded through the pipeline.
///
/// Stages can push [`Diagnostic`]s and read/write arbitrary key-value
/// metadata via [`PipelineContext::metadata`].
#[derive(Debug, Default)]
pub struct PipelineContext {
    /// Diagnostics collected by stages (warnings, info, timing).
    pub diagnostics: Vec<Diagnostic>,
    /// Arbitrary key-value metadata that stages can read/write.
    pub metadata: HashMap<String, MetadataVal>,
}

/// A single, self-contained transformation over a [`Theory`].
///
/// Stages are applied left-to-right. Each stage receives the theory
/// produced by the previous stage and a shared [`PipelineContext`] for
/// diagnostics and inter-stage communication.
pub trait PipelineStage: std::fmt::Debug {
    /// Human-readable name used in diagnostics and tracing.
    fn name(&self) -> &'static str;

    /// Apply this stage, returning a (possibly transformed) theory.
    ///
    /// Returning `Err` aborts the pipeline. To report a non-fatal
    /// problem, push a [`Diagnostic`] onto `ctx` and return `Ok`.
    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory>;
}

// ---------------------------------------------------------------------------
// Pipeline builder
// ---------------------------------------------------------------------------

/// A configured, ready-to-run pipeline of [`PipelineStage`]s.
#[derive(Debug)]
pub struct Pipeline {
    stages: Vec<Box<dyn PipelineStage>>,
}

impl Pipeline {
    /// Start building a pipeline with no stages.
    pub fn builder() -> PipelineBuilder {
        PipelineBuilder { stages: Vec::new() }
    }

    /// The default pipeline that reproduces current `prepare()` semantics.
    ///
    /// Equivalent to:
    /// ```rust,no_run
    /// # use spindle_core::pipeline::*;
    /// Pipeline::builder()
    ///     .stage(Validate::default())
    ///     .stage(WildcardRewrite)
    ///     .stage(Ground::default())
    ///     .build();
    /// ```
    ///
    /// [`TemporalFilter`] is omitted by default because it requires a
    /// reference time that most callers do not provide.
    pub fn default_pipeline() -> Self {
        Self::builder()
            .stage(Validate::default())
            .stage(WildcardRewrite)
            .stage(Ground::default())
            .build()
    }

    /// Run all stages in order, returning the final theory and context.
    pub fn run(&self, theory: Theory) -> Result<(Theory, PipelineContext)> {
        let mut ctx = PipelineContext::default();
        let mut current = theory;

        for stage in &self.stages {
            current = stage.apply(current, &mut ctx)?;
        }

        Ok((current, ctx))
    }
}

/// Builder for constructing a [`Pipeline`] from individual stages.
#[derive(Debug)]
pub struct PipelineBuilder {
    stages: Vec<Box<dyn PipelineStage>>,
}

impl PipelineBuilder {
    /// Append a stage to the end of the pipeline.
    pub fn stage<S: PipelineStage + 'static>(mut self, s: S) -> Self {
        self.stages.push(Box::new(s));
        self
    }

    /// Insert a stage at a specific position (0-indexed).
    pub fn stage_at<S: PipelineStage + 'static>(mut self, index: usize, s: S) -> Self {
        self.stages.insert(index, Box::new(s));
        self
    }

    /// Consume the builder and produce an immutable [`Pipeline`].
    pub fn build(self) -> Pipeline {
        Pipeline {
            stages: self.stages,
        }
    }
}

// ---------------------------------------------------------------------------
// default_pipeline() free function
// ---------------------------------------------------------------------------

/// Convenience function returning the default pipeline.
///
/// This is equivalent to [`Pipeline::default_pipeline()`].
pub fn default_pipeline() -> Pipeline {
    Pipeline::default_pipeline()
}

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

/// Prepare a theory for reasoning.
///
/// This is the main entry point for the reasoning pipeline. It handles:
/// - Temporal filtering
/// - Validation
/// - Wildcard rewrite
/// - Grounding
///
/// Internally delegates to a [`Pipeline`] built from the standard stages,
/// configured according to `opts`. All existing callers continue to work
/// without changes.
pub fn prepare(theory: &Theory, opts: PrepareOptions) -> Result<PipelineResult> {
    let mut builder = Pipeline::builder();

    // 1. Temporal filter (optional)
    if let Some(t) = opts.reference_time {
        builder = builder.stage(TemporalFilter { reference_time: t });
    }

    // 2. Validation
    builder = builder.stage(Validate {
        enforce_range_restricted: opts.validation.enforce_range_restricted,
        reject_wildcard_in_head: opts.validation.reject_wildcard_in_head,
    });

    // 3. Wildcard rewrite
    builder = builder.stage(WildcardRewrite);

    // 4. Grounding (optional)
    if opts.grounding.enabled {
        builder = builder.stage(Ground {
            max_iterations: opts.grounding.max_iterations,
            max_instances: opts.grounding.max_instances,
        });
    }

    // 5. Temporal variable validation (rejects unresolved temporal vars after grounding)
    builder = builder.stage(TemporalVarValidation::default());

    // 6. Post-grounding temporal re-filter (grounding may have resolved temporal vars
    //    to concrete intervals that are now filterable by reference_time)
    if let Some(t) = opts.reference_time {
        builder = builder.stage(TemporalFilter { reference_time: t });
    }

    let pipeline = builder.build();
    let (mut theory, ctx) = pipeline.run(theory.clone())?;

    // Apply explicit trust policy from options, overriding the parsed one
    if let Some(tp) = opts.trust_policy {
        *theory.trust_policy_mut() = tp;
    }

    // Reconstruct GroundingReport from context metadata
    let grounding_performed = ctx
        .metadata
        .get("grounding_performed")
        .and_then(|v| {
            if let MetadataVal::Bool(b) = v {
                Some(*b)
            } else {
                None
            }
        })
        .unwrap_or(false);

    let grounding_had_variables = ctx
        .metadata
        .get("grounding_had_variables")
        .and_then(|v| {
            if let MetadataVal::Bool(b) = v {
                Some(*b)
            } else {
                None
            }
        })
        .unwrap_or(false);

    let grounding_instances = ctx
        .metadata
        .get("grounding_instances")
        .and_then(|v| {
            if let MetadataVal::Usize(n) = v {
                Some(*n)
            } else {
                None
            }
        })
        .unwrap_or(0);

    let grounding_limit_hit = ctx
        .metadata
        .get("grounding_limit_hit")
        .and_then(|v| {
            if let MetadataVal::Bool(b) = v {
                Some(*b)
            } else {
                None
            }
        })
        .unwrap_or(false);

    let grounding_report = GroundingReport {
        performed: grounding_performed,
        had_variables: grounding_had_variables,
        instances: grounding_instances,
        limit_hit: grounding_limit_hit,
    };

    Ok(PipelineResult {
        theory,
        evaluated_at: opts.reference_time,
        grounding_report,
        weighted_conclusions: Vec::new(),
    })
}

/// Resolve a rule label to its trust value and optional source.
///
/// Handles grounded instances by falling back to the template label for metadata lookup.
/// When `reference_time` is provided, applies decay based on the claim's `:at` timestamp.
fn resolve_rule_trust(
    rule_label: &str,
    theory: &Theory,
    policy: &TrustPolicy,
    reference_time: Option<TimePoint>,
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
            let trust = compute_source_trust(source_id, meta, policy, reference_time);
            return (trust, Some(Source::new(source_id.clone())));
        }
    }
    (policy.default_trust, None)
}

/// Compute trust for a source, applying decay if a timestamp and reference time are available.
fn compute_source_trust(
    source_id: &str,
    meta: &crate::theory::Meta,
    policy: &TrustPolicy,
    reference_time: Option<TimePoint>,
) -> TrustValue {
    if let Some(TimePoint::Moment(ref_millis)) = reference_time
        && let Some(MetaValue::String(ts)) = meta.properties.get("timestamp")
        && let Some(claim_millis) = parse_iso8601_millis(ts.as_str())
    {
        let age_secs = (ref_millis - claim_millis) as f64 / 1000.0;
        return policy.get_effective_trust(source_id, age_secs);
    }
    policy.get_trust(source_id)
}

/// Parse an ISO-8601 timestamp string to milliseconds since epoch.
fn parse_iso8601_millis(ts: &str) -> Option<i64> {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(ts)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

/// Select the best positive conclusion for trust chain traversal.
///
/// Prefers `+D` (definitely provable) over `+d` (defeasibly provable) since
/// definite proofs are stronger. Among same type, prefers conclusions that
/// have a `rule_label` set so the derivation chain can be traced.
fn best_conclusion<'a>(conclusions: &[&'a Conclusion]) -> Option<&'a Conclusion> {
    conclusions.iter().copied().max_by_key(|c| {
        let type_rank = if c.conclusion_type == ConclusionType::DefinitelyProvable {
            1
        } else {
            0
        };
        let label_rank = if c.rule_label.is_some() { 1 } else { 0 };
        (type_rank, label_rank)
    })
}

/// Build a trust derivation tree for a literal by tracing back through deriving rules.
fn build_trust_tree(
    literal: &Literal,
    rule_label: Option<&str>,
    positive_conclusions: &HashMap<String, Vec<&Conclusion>>,
    theory: &Theory,
    policy: &TrustPolicy,
    reference_time: Option<TimePoint>,
    visited: &mut HashSet<String>,
) -> TrustDerivationNode {
    let lit_key = literal.to_spl();

    // Cycle detection
    if visited.contains(&lit_key) {
        return TrustDerivationNode::new(literal.clone(), policy.default_trust);
    }
    visited.insert(lit_key.clone());

    let (trust, source) = if let Some(label) = rule_label {
        resolve_rule_trust(label, theory, policy, reference_time)
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
        for body_bl in &rule.body {
            let body_key = body_bl.to_spl();
            // Only logic body literals participate in trust derivation
            let body_lit = match body_bl.as_logic() {
                Some(lit) => lit.to_literal(),
                None => continue,
            };
            if let Some(body_conclusions) = positive_conclusions.get(&body_key)
                && let Some(best) = best_conclusion(body_conclusions)
            {
                let child = build_trust_tree(
                    &body_lit,
                    best.rule_label.as_deref(),
                    positive_conclusions,
                    theory,
                    policy,
                    reference_time,
                    visited,
                );
                children.push(child);
            }
        }
        if !children.is_empty() {
            node = node.with_children(children);
        }
    }

    visited.remove(&lit_key);
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

/// Find defeaters that fired against a defeasible conclusion and were overruled.
///
/// A defeater diminishes a `+d` conclusion when its head is the complement of
/// the conclusion's literal and its body is defeasibly provable (it *fired*).
/// Because the conclusion is nonetheless positive, the proof theory guarantees
/// the defeater was beaten by the superiority relation (*overruled*). A
/// defeater whose antecedent is not provable mounts no challenge and does not
/// diminish. The defeater's credibility is the weakest link of its own
/// derivation, mirroring `build_trust_tree` on the conclusion side.
fn defeater_diminishers(
    conclusion_literal: &Literal,
    positive_conclusions: &HashMap<String, Vec<&Conclusion>>,
    theory: &Theory,
    policy: &TrustPolicy,
    reference_time: Option<TimePoint>,
) -> Vec<(String, TrustValue)> {
    let complement_key = conclusion_literal.complement().to_spl();
    let mut out = Vec::new();

    for rule in theory.rules() {
        if !rule.rule_type.is_defeater() {
            continue;
        }
        if rule.head.is_empty() || rule.head[0].to_spl() != complement_key {
            continue;
        }
        // Fired: every logic body literal is positively provable.
        let fired = rule.body.iter().all(|bl| match bl.as_logic() {
            Some(lit) => positive_conclusions.contains_key(&lit.to_literal().to_spl()),
            None => true,
        });
        if !fired {
            continue;
        }
        let tree = build_trust_tree(
            &rule.head[0],
            Some(rule.label.as_str()),
            positive_conclusions,
            theory,
            policy,
            reference_time,
            &mut HashSet::new(),
        );
        out.push((rule.label.clone(), tree.weakest_link_trust()));
    }
    out
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
    reference_time: Option<TimePoint>,
) -> Vec<WeightedConclusion> {
    // Build index of positive conclusions for fast body-literal lookup.
    // Key by to_spl() so that literals with different arguments (e.g. p(a) vs p(b))
    // remain distinct. Store all conclusions per key so we can pick the best one
    // (e.g. +D over +d) rather than silently overwriting.
    let mut positive_conclusions: HashMap<String, Vec<&Conclusion>> = HashMap::new();
    for c in conclusions.iter().filter(|c| c.is_positive()) {
        positive_conclusions
            .entry(c.literal.to_spl())
            .or_default()
            .push(c);
    }

    conclusions
        .iter()
        .map(|c| {
            let (mut degree, sources) = if c.is_positive() {
                let tree = build_trust_tree(
                    &c.literal,
                    c.rule_label.as_deref(),
                    &positive_conclusions,
                    theory,
                    policy,
                    reference_time,
                    &mut HashSet::new(),
                );
                let degree = tree.weakest_link_trust();
                let sources = collect_tree_sources(&tree);
                (degree, sources)
            } else {
                (policy.default_trust, HashSet::new())
            };

            // Layer-2 diminishment: each defeater that fired and was
            // overruled reduces the degree to degree * (1 - defeater_degree),
            // folded in rule order. Applies to +d only; +D cannot be
            // challenged by a defeater.
            let mut diminishers = Vec::new();
            if c.conclusion_type == ConclusionType::DefeasiblyProvable {
                for (label, defeater_trust) in defeater_diminishers(
                    &c.literal,
                    &positive_conclusions,
                    theory,
                    policy,
                    reference_time,
                ) {
                    let dim = DiminisherInfo::new(label, defeater_trust, degree);
                    degree = dim.resulting_degree();
                    diminishers.push(dim);
                }
            }

            let mut wc = WeightedConclusion::new(c.literal.clone(), c.conclusion_type, degree);
            wc.sources = sources;
            wc.diminished_by = diminishers;

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
        let weighted = compute_weighted_conclusions(&conclusions, theory, policy, None);
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
        let weighted = compute_weighted_conclusions(&conclusions, &theory, &policy, None);

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
