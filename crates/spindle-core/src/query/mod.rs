//! Query Operators Module
//!
//! This module provides interactive query operators for defeasible logic theories:
//!
//! - **What-If**: Hypothetical reasoning - "What if we assumed X?"
//! - **Why-Not**: Explanation of failures - "Why isn't X provable?"
//! - **Abduction**: Finding hypotheses - "What facts would make X provable?"
//!
//! # Architecture
//!
//! All operators share a common [`QueryOperator`] trait that decouples query
//! logic from the specific reasoning backend. The trait accepts a
//! `&dyn Reasoner` (from [`crate::reason::Reasoner`]) so that operators can
//! be tested against mock reasoners without running the full pipeline.
//!
//! Existing free functions ([`query`], [`what_if`], [`why_not`], [`abduce`],
//! [`requires`]) are preserved for backward compatibility. They internally
//! use the standard reasoning path via [`crate::reason::reason()`].

pub mod why_not;

pub use why_not::{BlockingCondition, BlockingType, WhyNotResult, why_not};

use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::conclusion::{Conclusion, ConclusionType};
use crate::error::Result;
use crate::index::IndexedTheory;
use crate::literal::{InternedLiteralName, Literal};
use crate::mode::Mode;
use crate::pipeline::{PrepareOptions, compute_weighted_conclusions, prepare};
use crate::reason::{Reasoner, reason_with_options};
use crate::temporal::{Temporal, TimePoint};
use crate::term::Term;
use crate::theory::MetaValue;
use crate::theory::Theory;
use crate::trust::{TrustPolicy, TrustValue};

pub mod abduce;
pub use abduce::{AbducedFacts, AbductionResult, AbductionSolution, abduce};

pub mod requires;
pub use requires::{
    DEFAULT_MAX_RAW_CANDIDATES, RequiresOptions, RequiresResult, RequiresSearchStatus,
    RequiresVerificationStats, requires, requires_with_options,
};

pub mod what_if;
pub use what_if::{HypotheticalClaim, WhatIfResult, what_if, what_if_provable};

// =============================================================================
// QUERY OPERATOR TRAIT AND SHARED TYPES
// =============================================================================

/// Arguments common to all query operators.
///
/// Bundles pipeline options and operator-specific limits into a single
/// value that can be threaded through [`QueryOperator::execute`].
#[derive(Debug, Clone)]
pub struct QueryArgs {
    /// Pipeline options for reasoning (temporal filtering, grounding, etc.)
    pub prepare_options: PrepareOptions,
    /// Maximum number of solutions (used by abduce/requires operators).
    pub max_solutions: usize,
}

impl Default for QueryArgs {
    fn default() -> Self {
        Self {
            prepare_options: PrepareOptions::default(),
            max_solutions: 10,
        }
    }
}

/// A query operator that can be executed against a theory.
///
/// Implementors encapsulate a specific query strategy (what-if, why-not,
/// abduction, etc.) and produce a typed result. The [`Reasoner`] parameter
/// decouples the operator from a specific reasoning algorithm, enabling:
///
/// - **Mock testing**: inject a stub reasoner that returns fixed conclusions.
/// - **Backend selection**: use standard DL(d) or scalable DL(d||) without
///   changing operator code.
///
/// # Example
///
/// ```rust,ignore
/// use spindle_core::query::{QueryOperator, QueryArgs};
/// use spindle_core::reason::{Reasoner, StandardReasoner};
/// use spindle_core::theory::Theory;
///
/// struct MyOperator { /* ... */ }
///
/// impl QueryOperator for MyOperator {
///     type Output = bool;
///
///     fn execute(
///         &self,
///         theory: &Theory,
///         reasoner: &dyn Reasoner,
///         args: &QueryArgs,
///     ) -> spindle_core::error::Result<bool> {
///         let prepared = spindle_core::pipeline::prepare(theory, args.prepare_options.clone())?;
///         let mut indexed = spindle_core::index::IndexedTheory::build(&prepared.theory);
///         let conclusions = reasoner.reason(&mut indexed)?;
///         Ok(conclusions.iter().any(|c| c.is_positive()))
///     }
/// }
/// ```
pub trait QueryOperator {
    /// The specific result type this operator produces.
    type Output;

    /// Execute the operator against a theory using the given reasoner.
    ///
    /// Implementations should call [`prepare`](crate::pipeline::prepare) to
    /// obtain a grounded/filtered theory, build an [`IndexedTheory`], and
    /// then invoke `reasoner.reason()` to obtain conclusions.
    fn execute(
        &self,
        theory: &Theory,
        reasoner: &dyn Reasoner,
        args: &QueryArgs,
    ) -> Result<Self::Output>;
}

/// Helper: run reasoning on a theory using a `dyn Reasoner` and `QueryArgs`.
///
/// Handles the prepare -> index -> reason pipeline that most operators need.
/// This avoids duplicating the boilerplate in every operator implementation.
#[allow(dead_code)] // Will be used when operators are extracted in follow-up tasks.
pub(crate) fn run_reasoning(
    theory: &Theory,
    reasoner: &dyn Reasoner,
    args: &QueryArgs,
) -> Result<Vec<Conclusion>> {
    let prepared = prepare(theory, args.prepare_options.clone())?;
    let mut indexed = IndexedTheory::build(&prepared.theory);
    reasoner.reason(&mut indexed)
}

// =============================================================================
// QUERY RESULT STRUCTURES
// =============================================================================

/// Status of a query result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryStatus {
    /// The literal is provable
    Provable,
    /// The literal is refuted (complement provable)
    Refuted,
    /// The status is unknown (neither provable nor refuted)
    Unknown,
}

impl fmt::Display for QueryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryStatus::Provable => write!(f, "provable"),
            QueryStatus::Refuted => write!(f, "refuted"),
            QueryStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// Result of querying a literal from a theory
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// The queried literal
    pub literal: Literal,
    /// Query status
    pub status: QueryStatus,
    /// Conclusion type (if provable)
    pub conclusion_type: Option<ConclusionType>,
}

impl QueryResult {
    /// Create a new query result
    pub fn new(literal: Literal, status: QueryStatus) -> Self {
        Self {
            literal,
            status,
            conclusion_type: None,
        }
    }

    /// Set conclusion type
    pub fn with_conclusion_type(mut self, ct: ConclusionType) -> Self {
        self.conclusion_type = Some(ct);
        self
    }

    /// Check if provable
    pub fn is_provable(&self) -> bool {
        self.status == QueryStatus::Provable
    }

    /// Check if definitely provable
    pub fn is_definitely_provable(&self) -> bool {
        self.conclusion_type == Some(ConclusionType::DefinitelyProvable)
    }

    /// Check if defeasibly provable
    pub fn is_defeasibly_provable(&self) -> bool {
        self.conclusion_type == Some(ConclusionType::DefeasiblyProvable)
    }
}

/// Filter for trust-based query results
#[derive(Debug, Clone, Default)]
pub struct TrustFilter {
    /// Minimum trust degree for a conclusion to be included
    pub min_degree: Option<TrustValue>,
    /// Only include conclusions from this source pattern
    pub source_pattern: Option<String>,
    /// Trust policy to use for evaluating trust
    pub policy: Option<TrustPolicy>,
}

impl TrustFilter {
    /// Create a new empty trust filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum trust degree
    pub fn with_min_degree(mut self, degree: TrustValue) -> Self {
        self.min_degree = Some(degree);
        self
    }

    /// Set source pattern filter
    pub fn with_source(mut self, pattern: impl Into<String>) -> Self {
        self.source_pattern = Some(pattern.into());
        self
    }

    /// Set trust policy
    pub fn with_policy(mut self, policy: TrustPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Check if a conclusion passes this filter using derived (weakest-link) trust.
    ///
    /// Computes trust degrees via `compute_weighted_conclusions`, so a high-trust
    /// rule with low-trust premises gets the minimum trust across the chain.
    ///
    /// For the source pattern check, rule metadata is consulted directly (with
    /// template_label fallback for grounded instances).
    pub fn passes(
        &self,
        conclusion: &Conclusion,
        conclusions: &[Conclusion],
        theory: &Theory,
        reference_time: Option<TimePoint>,
    ) -> bool {
        let policy = match &self.policy {
            Some(p) => p,
            None => return true, // No policy means everything passes
        };

        // Check source pattern using rule metadata (unchanged — this is per-rule, not per-chain)
        if let Some(ref pattern) = self.source_pattern {
            let source_id = conclusion.rule_label.as_deref().and_then(|label| {
                let meta_source =
                    theory
                        .get_meta(label)
                        .and_then(|meta| match meta.properties.get("source") {
                            Some(MetaValue::String(s)) => Some(s.as_str()),
                            _ => None,
                        });
                if meta_source.is_some() {
                    return meta_source;
                }
                theory.get_rule(label).and_then(|rule| {
                    let tl = rule.template_label();
                    if tl != label {
                        theory
                            .get_meta(tl)
                            .and_then(|meta| match meta.properties.get("source") {
                                Some(MetaValue::String(s)) => Some(s.as_str()),
                                _ => None,
                            })
                    } else {
                        None
                    }
                })
            });

            match source_id {
                Some(src) => {
                    if !src.contains(pattern.as_str()) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        // Check minimum degree using derived (weakest-link) trust
        if let Some(min) = self.min_degree {
            let weighted =
                compute_weighted_conclusions(conclusions, theory, policy, reference_time);
            let wc = weighted.iter().find(|wc| {
                wc.literal.to_spl() == conclusion.literal.to_spl()
                    && wc.conclusion_type == conclusion.conclusion_type
            });
            let degree = wc.map(|w| w.degree).unwrap_or(policy.default_trust);
            if degree < min {
                return false;
            }
        }

        true
    }
}

/// Check whether a candidate temporal matches a query temporal.
///
/// - If the query temporal is empty (unbounded), it acts as a **wildcard** and
///   matches any candidate temporal.
/// - If the query temporal is non-empty, the candidate must **exactly** equal
///   the query temporal.
///
/// This is the standard matching semantics for temporal queries: an unadorned
/// query like `?- bird` should match `bird`, `bird[1,10]`, etc., while a
/// temporally-qualified query like `?- bird[1,10]` should only match
/// `bird[1,10]`.
pub fn matches_query_temporal(
    query_temporal: &crate::temporal::Temporal,
    candidate_temporal: &crate::temporal::Temporal,
) -> bool {
    if query_temporal.is_empty() {
        true
    } else {
        query_temporal == candidate_temporal
    }
}

/// Check whether `candidate` matches `query` including temporal bounds.
///
/// `Literal::PartialEq` intentionally excludes temporal (ADR-005), so code
/// that compares rule heads or proven-set members against a query literal
/// must use this function to avoid conflating different temporal windows.
pub fn matches_literal_temporal(query: &Literal, candidate: &Literal) -> bool {
    query.name_id() == candidate.name_id()
        && query.negation == candidate.negation
        && query.mode == candidate.mode
        && query.predicate_args() == candidate.predicate_args()
        && matches_query_temporal(&query.temporal, &candidate.temporal)
}

/// A literal key that includes temporal bounds in its equality and hash,
/// unlike `Literal` whose `PartialEq`/`Hash` intentionally exclude temporal
/// (ADR-005).
#[derive(Debug, Clone)]
struct TemporalLitKey {
    name_id: InternedLiteralName,
    negation: bool,
    mode: Mode,
    temporal: Temporal,
    args: Vec<Term>,
}

impl PartialEq for TemporalLitKey {
    fn eq(&self, other: &Self) -> bool {
        self.name_id == other.name_id
            && self.negation == other.negation
            && self.mode == other.mode
            && self.temporal == other.temporal
            && self.args == other.args
    }
}

impl Eq for TemporalLitKey {}

impl Hash for TemporalLitKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name_id.hash(state);
        self.negation.hash(state);
        self.mode.hash(state);
        self.temporal.hash(state);
        self.args.hash(state);
    }
}

impl TemporalLitKey {
    fn from_literal(lit: &Literal) -> Self {
        Self {
            name_id: InternedLiteralName(lit.name_id()),
            negation: lit.negation,
            mode: lit.mode.clone(),
            temporal: lit.temporal.clone(),
            args: lit.predicate_args().to_vec(),
        }
    }
}

/// O(1) lookup set for proven literals with temporal awareness.
///
/// Uses two internal sets:
/// - `base`: keyed by `Literal`'s default Eq/Hash (excludes temporal) for
///   actual atemporal conclusions only.
/// - `temporal`: keyed by `TemporalLitKey` (includes temporal) for exact
///   temporal matching.
pub struct ProvenSet {
    base: HashSet<Literal>,
    temporal: HashSet<TemporalLitKey>,
}

impl ProvenSet {
    /// Build from positive conclusions.
    pub fn from_conclusions(conclusions: &[Conclusion]) -> Self {
        let positive: Vec<_> = conclusions
            .iter()
            .filter(|c| c.conclusion_type.is_positive())
            .collect();
        let mut base = HashSet::with_capacity(positive.len());
        let mut temporal = HashSet::with_capacity(positive.len());
        for c in positive {
            if c.literal.temporal.is_empty() {
                base.insert(c.literal.clone());
            }
            temporal.insert(TemporalLitKey::from_literal(&c.literal));
        }
        Self { base, temporal }
    }

    /// Check whether `lit` is proven:
    /// - If `lit.temporal` is empty → require an actual atemporal/base conclusion (O(1) via `base`).
    /// - If `lit.temporal` is non-empty → exact match required (O(1) via `temporal`).
    pub fn contains(&self, lit: &Literal) -> bool {
        if lit.temporal.is_empty() {
            self.base.contains(lit)
        } else {
            self.temporal.contains(&TemporalLitKey::from_literal(lit))
        }
    }
}

/// Check whether any positive conclusion in `conclusions` matches `lit`
/// with temporal awareness.
///
/// **Prefer [`ProvenSet`]** when checking multiple literals against the same
/// conclusion set — it provides O(1) per lookup instead of O(n).
pub fn is_proven_temporal(conclusions: &[Conclusion], lit: &Literal) -> bool {
    conclusions
        .iter()
        .any(|c| c.conclusion_type.is_positive() && matches_literal_temporal(lit, &c.literal))
}

/// Query a literal against a theory
pub fn query(theory: &Theory, literal: &Literal) -> Result<QueryResult> {
    query_with_options(theory, literal, PrepareOptions::default())
}

/// Query a literal against a theory with custom options
///
/// This is the primary API for as-of queries. Use `reference_time` in options
/// to query at a specific point in time:
///
/// ```rust
/// use spindle_core::prelude::*;
/// use spindle_core::query::query_with_options;
/// use spindle_core::pipeline::PrepareOptions;
/// use spindle_core::temporal::TimePoint;
///
/// let mut theory = Theory::new();
/// theory.add_fact("bird");
///
/// // Query at a specific time (milliseconds since epoch)
/// let opts = PrepareOptions {
///     reference_time: Some(TimePoint::from_millis(1707220800000)),
///     ..Default::default()
/// };
/// let result = query_with_options(&theory, &Literal::simple("bird"), opts).unwrap();
/// ```
pub fn query_with_options(
    theory: &Theory,
    literal: &Literal,
    opts: PrepareOptions,
) -> Result<QueryResult> {
    let conclusions = reason_with_options(theory, opts)?;
    let complement = literal.complement();

    // Check if literal is provable
    for conc in &conclusions {
        if conc.literal == *literal
            && matches_query_temporal(&literal.temporal, &conc.literal.temporal)
            && conc.conclusion_type.is_positive()
        {
            return Ok(QueryResult::new(literal.clone(), QueryStatus::Provable)
                .with_conclusion_type(conc.conclusion_type));
        }
    }

    // Check if complement is provable (refuted)
    for conc in &conclusions {
        if conc.literal == complement
            && matches_query_temporal(&complement.temporal, &conc.literal.temporal)
            && conc.conclusion_type.is_positive()
        {
            return Ok(QueryResult::new(literal.clone(), QueryStatus::Refuted));
        }
    }

    // Unknown
    Ok(QueryResult::new(literal.clone(), QueryStatus::Unknown))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::reason::reason;
    use crate::rule::{Rule, RuleType};

    // ==========================================================================
    // HELPER FUNCTIONS - Theory Building
    // ==========================================================================

    /// Create a minimal theory with just a single fact
    fn make_fact_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("bird");
        th
    }

    /// Create a theory with a defeasible rule chain: bird => flies
    fn make_defeasible_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_defeasible_rule(&["bird"], "flies");
        th
    }

    /// Create a theory with strict rules: human -> mortal
    fn make_strict_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("human");
        th.add_strict_rule(&["human"], "mortal");
        th
    }

    /// Create a theory with conflicting rules and superiority
    /// bird => flies, penguin => ~flies, penguin > bird
    fn make_conflict_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("penguin");
        let r1 = th.add_defeasible_rule(&["bird"], "flies");
        let r2 = th.add_defeasible_rule(&["penguin"], "~flies");
        th.add_superiority(&r2, &r1);
        th
    }

    /// Create a theory with a defeater
    /// bird => flies, broken_wing =X> ~flies (defeater)
    fn make_defeater_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("broken_wing");
        th.add_defeasible_rule(&["bird"], "flies");
        th.add_defeater(&["broken_wing"], "~flies");
        th
    }

    /// Create a theory with missing premises
    /// tests_pass + code_complete => ready_review (but tests_pass is missing)
    fn make_missing_premise_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("code_complete");
        th.add_defeasible_rule(&["tests_pass", "code_complete"], "ready_review");
        th
    }

    /// Create a multi-step chain theory: a => b => c => d
    fn make_chain_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("a");
        th.add_defeasible_rule(&["a"], "b");
        th.add_defeasible_rule(&["b"], "c");
        th.add_defeasible_rule(&["c"], "d");
        th
    }

    // ==========================================================================
    // QUERY OPERATOR TESTS - Basic Query Status
    // ==========================================================================

    #[test]
    fn test_query_provable_fact() {
        let th = make_fact_theory();
        let result = query(&th, &Literal::simple("bird")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
        assert!(result.is_provable());
    }

    #[test]
    fn test_query_unknown_for_nonexistent() {
        let th = make_fact_theory();
        let result = query(&th, &Literal::simple("unknown")).unwrap();
        assert_eq!(result.status, QueryStatus::Unknown);
        assert!(!result.is_provable());
    }

    #[test]
    fn test_query_refuted_when_complement_proven() {
        let mut theory = Theory::new();
        theory.add_fact("~bird");

        let result = query(&theory, &Literal::simple("bird")).unwrap();
        assert_eq!(result.status, QueryStatus::Refuted);
    }

    // ==========================================================================
    // QUERY OPERATOR TESTS - Definite vs Defeasible Conclusions
    // ==========================================================================

    #[test]
    fn test_query_detects_definite_conclusions() {
        let th = make_fact_theory();
        let result = query(&th, &Literal::simple("bird")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
        assert!(result.is_definitely_provable());
    }

    #[test]
    fn test_query_detects_defeasible_conclusions() {
        let th = make_defeasible_theory();
        let result = query(&th, &Literal::simple("flies")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
        assert!(result.is_defeasibly_provable());
    }

    #[test]
    fn test_query_strict_rule_produces_definite() {
        let th = make_strict_theory();
        let result = query(&th, &Literal::simple("mortal")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
        assert!(result.is_definitely_provable());
    }

    #[test]
    fn test_query_chain_derivation() {
        let th = make_chain_theory();

        // All should be provable through the chain
        assert_eq!(
            query(&th, &Literal::simple("a")).unwrap().status,
            QueryStatus::Provable
        );
        assert_eq!(
            query(&th, &Literal::simple("b")).unwrap().status,
            QueryStatus::Provable
        );
        assert_eq!(
            query(&th, &Literal::simple("c")).unwrap().status,
            QueryStatus::Provable
        );
        assert_eq!(
            query(&th, &Literal::simple("d")).unwrap().status,
            QueryStatus::Provable
        );
    }

    #[test]
    fn test_query_conflict_with_superiority() {
        let th = make_conflict_theory();
        // flies should be refuted because penguin > bird and penguin => ~flies
        let result = query(&th, &Literal::simple("flies")).unwrap();
        assert_eq!(result.status, QueryStatus::Refuted);
    }

    #[test]
    fn test_query_defeater_blocks() {
        let th = make_defeater_theory();
        // With broken_wing, flies should be unknown (defeated but not refuted)
        let result = query(&th, &Literal::simple("flies")).unwrap();
        // Defeaters prevent positive conclusion but don't prove negative
        assert!(result.status == QueryStatus::Unknown || result.status == QueryStatus::Refuted);
    }

    #[test]
    fn test_query_negated_literal() {
        let mut theory = Theory::new();
        theory.add_fact("~happy");
        theory.add_defeasible_rule(&["~happy"], "sad");

        let result = query(&theory, &Literal::negated("happy")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
    }

    #[test]
    fn test_query_result_literal_preserved() {
        let th = make_fact_theory();
        let lit = Literal::simple("bird");
        let result = query(&th, &lit).unwrap();
        assert_eq!(result.literal.name(), "bird");
    }

    #[test]
    fn test_query_status_display() {
        assert_eq!(format!("{}", QueryStatus::Provable), "provable");
        assert_eq!(format!("{}", QueryStatus::Refuted), "refuted");
        assert_eq!(format!("{}", QueryStatus::Unknown), "unknown");
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - Missing Premise Detection
    // ==========================================================================

    #[test]
    fn test_why_not_returns_result() {
        let th = make_missing_premise_theory();
        let result = why_not(&th, &Literal::simple("ready_review")).unwrap();
        assert!(result.has_blockers());
    }

    #[test]
    fn test_why_not_preserves_literal() {
        let th = make_missing_premise_theory();
        let lit = Literal::simple("ready_review");
        let result = why_not(&th, &lit).unwrap();
        assert_eq!(result.literal.name(), "ready_review");
    }

    #[test]
    fn test_why_not_shows_missing_premise() {
        let th = make_missing_premise_theory();
        let result = why_not(&th, &Literal::simple("ready_review")).unwrap();

        let missing = result.get_missing_premises();
        assert!(!missing.is_empty());
        assert!(missing.iter().any(|l| l.name() == "tests_pass"));
    }

    #[test]
    fn test_why_not_multiple_missing_premises() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a", "b", "c"], "goal");
        // All three premises are missing

        let result = why_not(&theory, &Literal::simple("goal")).unwrap();
        let missing = result.get_missing_premises();
        assert_eq!(missing.len(), 3);
    }

    #[test]
    fn test_why_not_partial_premises() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_defeasible_rule(&["a", "b", "c"], "goal");

        let result = why_not(&theory, &Literal::simple("goal")).unwrap();
        let missing = result.get_missing_premises();
        assert_eq!(missing.len(), 1);
        assert!(missing.iter().any(|l| l.name() == "c"));
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - No Rules Case
    // ==========================================================================

    #[test]
    fn test_why_not_no_rules_for_literal() {
        let th = Theory::new();
        let result = why_not(&th, &Literal::simple("unknown")).unwrap();
        assert!(result.would_derive.is_none());
    }

    #[test]
    fn test_why_not_no_applicable_rules() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["x"], "y"); // Rule exists but for different literal

        let result = why_not(&theory, &Literal::simple("z")).unwrap();
        assert!(result.would_derive.is_none());
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - Contradicted Detection
    // ==========================================================================

    #[test]
    fn test_why_not_contradicted() {
        let mut theory = Theory::new();
        theory.add_fact("~flies"); // Complement is proven

        let result = why_not(&theory, &Literal::simple("flies")).unwrap();
        assert!(
            result
                .blocked_by
                .iter()
                .any(|b| b.blocking_type == BlockingType::Contradicted)
        );
    }

    #[test]
    fn test_why_not_contradicted_explanation() {
        let mut theory = Theory::new();
        theory.add_fact("~happy");

        let result = why_not(&theory, &Literal::simple("happy")).unwrap();
        let contradicted = result
            .blocked_by
            .iter()
            .find(|b| b.blocking_type == BlockingType::Contradicted);

        assert!(contradicted.is_some());
        assert!(contradicted.unwrap().explanation.contains("~happy"));
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - For Provable Literals
    // ==========================================================================

    #[test]
    fn test_why_not_for_provable_literal() {
        let th = make_fact_theory();
        let result = why_not(&th, &Literal::simple("bird")).unwrap();
        // For provable literals, why-not should return no blockers
        assert!(!result.has_blockers());
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - Display and Formatting
    // ==========================================================================

    #[test]
    fn test_why_not_display_format() {
        let th = make_missing_premise_theory();
        let result = why_not(&th, &Literal::simple("ready_review")).unwrap();
        let s = result.to_string();
        assert!(s.contains("not provable"));
        assert!(s.contains("ready_review"));
    }

    #[test]
    fn test_why_not_display_no_rules() {
        let th = Theory::new();
        let result = why_not(&th, &Literal::simple("unknown")).unwrap();
        let s = result.to_string();
        assert!(s.contains("no rules"));
    }

    #[test]
    fn test_blocking_condition_missing_premise() {
        let missing = vec![Literal::simple("a"), Literal::simple("b")];
        let bc = BlockingCondition::missing_premise("r1", missing);
        assert_eq!(bc.blocking_type, BlockingType::MissingPremise);
        assert_eq!(bc.rule_label, "r1");
        assert_eq!(bc.missing_literals.len(), 2);
    }

    #[test]
    fn test_blocking_condition_defeated() {
        let bc = BlockingCondition::defeated("r1", "d1");
        assert_eq!(bc.blocking_type, BlockingType::Defeated);
        assert_eq!(bc.blocking_rule, Some("d1".to_string()));
    }

    #[test]
    fn test_blocking_condition_contradicted() {
        let bc = BlockingCondition::contradicted("r1", "r2");
        assert_eq!(bc.blocking_type, BlockingType::Contradicted);
        assert!(bc.explanation.contains("Contradicted"));
    }

    #[test]
    fn test_blocking_type_display() {
        assert_eq!(
            format!("{}", BlockingType::MissingPremise),
            "missing premise"
        );
        assert_eq!(format!("{}", BlockingType::Defeated), "defeated");
        assert_eq!(format!("{}", BlockingType::Contradicted), "contradicted");
    }

    // ==========================================================================
    // WHAT-IF OPERATOR TESTS - Basic Hypothetical Reasoning
    // ==========================================================================

    #[test]
    fn test_what_if_returns_result() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_what_if_enables_conclusion() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert!(result.is_provable());
        assert!(
            result
                .new_conclusions
                .iter()
                .any(|l| l.name() == "ready_review")
        );
    }

    #[test]
    fn test_what_if_does_not_modify_original_theory() {
        let th = make_missing_premise_theory();
        let original_count = th.rule_count();

        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
        let _ = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();

        // Original theory unchanged
        assert_eq!(th.rule_count(), original_count);
    }

    #[test]
    fn test_what_if_provable_convenience() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        assert!(what_if_provable(&th, hypotheticals, &Literal::simple("ready_review")).unwrap());
    }

    #[test]
    fn test_what_if_provable_false_when_insufficient() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("irrelevant"))];

        assert!(!what_if_provable(&th, hypotheticals, &Literal::simple("ready_review")).unwrap());
    }

    // ==========================================================================
    // WHAT-IF OPERATOR TESTS - Source Attribution
    // ==========================================================================

    #[test]
    fn test_what_if_with_source() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::with_source(
            Literal::simple("tests_pass"),
            "ci_server",
        )];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert!(result.is_provable());
        assert_eq!(
            result.hypotheticals[0].source,
            Some("ci_server".to_string())
        );
    }
    #[test]
    fn test_hypothetical_claim_anonymous() {
        let claim = HypotheticalClaim::new(Literal::simple("fact"));
        assert!(claim.source.is_none());
        assert_eq!(claim.literal.name(), "fact");
    }

    #[test]
    fn test_hypothetical_claim_with_source() {
        let claim = HypotheticalClaim::with_source(Literal::simple("verified"), "qa_team");
        assert_eq!(claim.source, Some("qa_team".to_string()));
    }

    // ==========================================================================
    // WHAT-IF OPERATOR TESTS - Multiple Hypotheticals
    // ==========================================================================

    #[test]
    fn test_what_if_multiple_claims() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a", "b"], "c");

        let hypotheticals = vec![
            HypotheticalClaim::new(Literal::simple("a")),
            HypotheticalClaim::new(Literal::simple("b")),
        ];

        assert!(what_if_provable(&theory, hypotheticals, &Literal::simple("c")).unwrap());
    }

    #[test]
    fn test_what_if_hypotheticals_preserved() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![
            HypotheticalClaim::new(Literal::simple("tests_pass")),
            HypotheticalClaim::new(Literal::simple("extra")),
        ];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert_eq!(result.hypotheticals.len(), 2);
    }

    // ==========================================================================
    // WHAT-IF OPERATOR TESTS - Counterfactual Queries
    // ==========================================================================

    #[test]
    fn test_what_if_chain_derivation() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("a"))];

        let result = what_if(&theory, hypotheticals, &Literal::simple("c")).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_what_if_negated_hypothetical() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["~broken"], "works");

        let hypotheticals = vec![HypotheticalClaim::new(Literal::negated("broken"))];

        let result = what_if(&theory, hypotheticals, &Literal::simple("works")).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_what_if_tracks_new_conclusions() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();

        // new_conclusions should contain ready_review and tests_pass
        assert!(!result.new_conclusions.is_empty());
    }

    #[test]
    fn test_what_if_newly_provable() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();

        let newly = result.newly_provable();
        assert!(newly.iter().any(|l| l.name() == "ready_review"));
    }

    // ==========================================================================
    // WHAT-IF OPERATOR TESTS - Already Provable
    // ==========================================================================

    #[test]
    fn test_what_if_already_provable() {
        let th = make_fact_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("extra"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("bird")).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_what_if_goal_already_satisfied() {
        let th = make_defeasible_theory();
        let hypotheticals = vec![]; // No hypotheticals needed

        let result = what_if(&th, hypotheticals, &Literal::simple("flies")).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_what_if_temporal_goal_requires_exact_window() {
        use crate::mode::Mode;
        use crate::temporal::{Temporal, TimePoint};

        let mut theory = Theory::new();
        theory.add_rule(Rule::fact(
            "f1",
            Literal::new(
                "p",
                false,
                Mode::empty(),
                Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
                vec![],
            ),
        ));

        let goal = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30)),
            vec![],
        );
        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::simple("irrelevant"))],
            &goal,
        )
        .unwrap();

        assert!(
            !result.is_provable(),
            "p[20,30] should not be treated as provable from baseline p[1,10]"
        );
    }

    #[test]
    fn test_what_if_new_conclusions_distinguish_temporal_windows() {
        use crate::mode::Mode;
        use crate::temporal::{Temporal, TimePoint};

        let mut theory = Theory::new();
        theory.add_rule(Rule::fact(
            "f1",
            Literal::new(
                "p",
                false,
                Mode::empty(),
                Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
                vec![],
            ),
        ));

        let new_window = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30)),
            vec![],
        );
        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(new_window.clone())],
            &new_window,
        )
        .unwrap();

        assert!(result.is_provable());
        assert!(
            result.new_conclusions.iter().any(|lit| lit == &new_window),
            "new_conclusions should include the newly introduced temporal window"
        );
    }

    // ==========================================================================
    // INTEGRATION TESTS - Cross-Operator Consistency
    // ==========================================================================

    #[test]
    fn test_query_and_why_not_consistency_for_provable() {
        let th = make_defeasible_theory();
        let flies = Literal::simple("flies");

        // Query says provable
        let q_result = query(&th, &flies).unwrap();
        assert!(q_result.is_provable());

        // Why-not should have no blockers
        let wn_result = why_not(&th, &flies).unwrap();
        assert!(!wn_result.has_blockers());
    }

    #[test]
    fn test_query_and_why_not_consistency_for_non_provable() {
        let th = make_missing_premise_theory();
        let ready = Literal::simple("ready_review");

        // Query says not provable
        let q_result = query(&th, &ready).unwrap();
        assert!(!q_result.is_provable());

        // Why-not should explain why
        let wn_result = why_not(&th, &ready).unwrap();
        assert!(wn_result.has_blockers());
    }

    #[test]
    fn test_why_not_and_requires_consistency() {
        let th = make_missing_premise_theory();
        let ready = Literal::simple("ready_review");

        // Why-not shows blocking
        let wn = why_not(&th, &ready).unwrap();
        let missing = wn.get_missing_premises();
        assert!(!missing.is_empty());

        // Requires finds what to add
        let needed = requires(&th, &ready).unwrap();
        assert!(!needed.is_empty());

        // Both should identify tests_pass
        assert!(missing.iter().any(|l| l.name() == "tests_pass"));
        assert!(needed.iter().any(|l| l.name() == "tests_pass"));
    }

    #[test]
    fn test_requires_and_what_if_consistency() {
        let th = make_missing_premise_theory();
        let ready = Literal::simple("ready_review");

        // Get what's needed via abduction
        let needed = requires(&th, &ready).unwrap();

        // Use those as hypotheticals
        let hypotheticals: Vec<_> = needed.into_iter().map(HypotheticalClaim::new).collect();

        // What-if should now prove the goal
        let result = what_if(&th, hypotheticals, &ready).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_full_workflow_non_provable() {
        let th = make_missing_premise_theory();
        let ready = Literal::simple("ready_review");

        // 1. Query shows not provable
        let q = query(&th, &ready).unwrap();
        assert!(!q.is_provable());

        // 2. Why-not explains the blocker
        let wn = why_not(&th, &ready).unwrap();
        assert!(wn.has_blockers());
        assert!(!wn.get_missing_premises().is_empty());

        // 3. Abduction finds solution
        let ab = abduce(&th, &ready, 10).unwrap();
        assert!(ab.has_solutions());
        let sol = ab.smallest_solution().unwrap();
        assert!(sol.facts.iter().any(|l| l.name() == "tests_pass"));

        // 4. What-if verifies solution works
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
        let wi = what_if(&th, hypotheticals, &ready).unwrap();
        assert!(wi.is_provable());
    }

    #[test]
    fn test_full_workflow_provable() {
        let th = make_defeasible_theory();
        let flies = Literal::simple("flies");

        // 1. Query shows provable
        let q = query(&th, &flies).unwrap();
        assert!(q.is_provable());

        // 2. Why-not has no blockers
        let wn = why_not(&th, &flies).unwrap();
        assert!(!wn.has_blockers());

        // 3. Abduction shows already provable
        let ab = abduce(&th, &flies, 10).unwrap();
        assert!(ab.is_already_provable());
    }

    // ==========================================================================
    // query_with_options API TESTS (spec §3.2, Milestone 5)
    // ==========================================================================

    #[test]
    fn test_query_with_options_default_matches_query() {
        let th = make_defeasible_theory();
        let bird = Literal::simple("bird");

        let result_default = query(&th, &bird).unwrap();
        let result_with_opts = query_with_options(&th, &bird, PrepareOptions::default()).unwrap();

        assert_eq!(result_default.status, result_with_opts.status);
        assert_eq!(result_default.is_provable(), result_with_opts.is_provable());
    }

    #[test]
    fn test_query_with_options_accepts_reference_time() {
        use crate::mode::Mode;
        use crate::rule::Rule;
        use crate::temporal::{Temporal, TimePoint};

        let mut theory = Theory::new();

        // Add a fact with a specific temporal window
        let bird_lit = Literal::new(
            "bird",
            false,
            Mode::default(),
            Temporal::from_bounds(1000, 2000), // active from 1000 to 2000
            vec![],
        );
        theory.add_rule(Rule::fact("f1", bird_lit.clone()));

        let bird_query = Literal::simple("bird");

        // Query at time 1500 (inside the window)
        let opts_inside = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(1500)),
            ..Default::default()
        };
        let result_inside = query_with_options(&theory, &bird_query, opts_inside).unwrap();
        assert!(
            result_inside.is_provable(),
            "bird should be provable at time 1500"
        );

        // Query at time 3000 (outside the window)
        let opts_outside = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(3000)),
            ..Default::default()
        };
        let result_outside = query_with_options(&theory, &bird_query, opts_outside).unwrap();
        assert!(
            !result_outside.is_provable(),
            "bird should NOT be provable at time 3000 (outside temporal window)"
        );
    }

    #[test]
    fn test_query_with_options_temporal_disjoint_no_conflict() {
        use crate::mode::Mode;
        use crate::rule::Rule;
        use crate::temporal::{Temporal, TimePoint};

        let mut theory = Theory::new();

        // Add bird fact active from 1000 to 2000
        let bird_lit = Literal::new(
            "bird",
            false,
            Mode::default(),
            Temporal::from_bounds(1000, 2000),
            vec![],
        );
        theory.add_rule(Rule::fact("f1", bird_lit));

        // Add ~bird fact active from 3000 to 4000 (disjoint!)
        let not_bird_lit = Literal::new(
            "bird",
            true, // negated
            Mode::default(),
            Temporal::from_bounds(3000, 4000),
            vec![],
        );
        theory.add_rule(Rule::fact("f2", not_bird_lit));

        // At time 1500, bird should be provable (no conflict because ~bird is disjoint)
        let opts = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(1500)),
            ..Default::default()
        };
        let result = query_with_options(&theory, &Literal::simple("bird"), opts).unwrap();
        assert!(
            result.is_provable(),
            "bird should be provable at time 1500 (disjoint temporals don't conflict)"
        );

        // At time 3500, ~bird should be provable
        let opts = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(3500)),
            ..Default::default()
        };
        let result = query_with_options(&theory, &Literal::negated("bird"), opts).unwrap();
        assert!(
            result.is_provable(),
            "~bird should be provable at time 3500"
        );
    }

    // ==========================================================================
    // REGRESSION TESTS - Bug Hunt Fixes
    // ==========================================================================

    #[test]
    fn test_why_not_detects_defeater_blocking() {
        // Regression: why_not should detect when a defeater blocks a conclusion
        let th = make_defeater_theory();
        // bird => flies, broken_wing ~> ~flies (defeater)
        let result = why_not(&th, &Literal::simple("flies")).unwrap();

        // Should detect the defeater blocking
        let has_defeated = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_type == BlockingType::Defeated);
        let has_contradicted = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_type == BlockingType::Contradicted);

        assert!(
            has_defeated || has_contradicted,
            "why_not should detect defeater or contradiction blocking, got: {:?}",
            result.blocked_by
        );
    }

    #[test]
    fn test_what_if_no_triple_reasoning() {
        // Regression: what_if should not call reason() 3 times.
        // We verify correctness (the performance fix is structural).
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert!(result.is_provable());
        assert!(!result.new_conclusions.is_empty());
    }

    #[test]
    fn test_what_if_unique_labels_no_collision() {
        // Regression: hypothetical labels should not collide with user labels
        let mut theory = Theory::new();
        // User has a rule labeled "hyp1" (which old code would collide with)
        theory.add_rule(Rule::defeasible(
            "hyp1",
            vec![Literal::simple("a")],
            Literal::simple("b"),
        ));
        theory.add_fact("x");
        theory.add_defeasible_rule(&["x"], "goal");

        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("a"))];
        let result = what_if(&theory, hypotheticals, &Literal::simple("goal")).unwrap();

        // Should still work correctly despite user having "hyp1" label
        assert!(result.is_provable());
    }

    #[test]
    fn test_next_hyp_label_skips_existing_rule_labels() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("__hyp_42_1", Literal::simple("a")));
        theory.add_rule(Rule::fact("__hyp_42_2", Literal::simple("b")));

        let label = what_if::next_hyp_label(&theory, 42, 1);
        assert_eq!(label, "__hyp_42_3");
    }

    #[test]
    fn test_what_if_changed_conclusions_no_false_positive_when_status_unchanged() {
        // Regression: baseline and modified can both contain +D and +d for the same literal.
        // This should not be reported as a status change.
        let mut theory = Theory::new();
        theory.add_fact("p");

        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::simple("irrelevant"))],
            &Literal::simple("p"),
        )
        .unwrap();

        assert!(
            result.changed_conclusions.is_empty(),
            "No status change expected for p, got {:?}",
            result.changed_conclusions
        );
    }

    #[test]
    fn test_what_if_changed_conclusions_includes_becomes_unprovable() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        // Baseline: +d q. Hypothetical ~q fact should block q.
        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::negated("q"))],
            &Literal::simple("q"),
        )
        .unwrap();

        let has_transition = result.changed_conclusions.iter().any(|(lit, old, new)| {
            lit == &Literal::simple("q")
                && *old == ConclusionType::DefeasiblyProvable
                && !new.is_positive()
        });
        assert!(
            has_transition,
            "Expected q to appear in changed_conclusions when it becomes unprovable, got {:?}",
            result.changed_conclusions
        );
    }

    #[test]
    fn test_what_if_changed_conclusions_detects_real_upgrade() {
        // When a hypothetical adds a strict rule that upgrades a literal
        // from +d to +D, changed_conclusions should report it.
        use crate::rule::Rule;

        let mut theory = Theory::new();
        // p is only defeasibly provable in baseline
        theory.add_rule(Rule::new(
            "d1",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));
        // q depends on a premise we'll supply
        theory.add_defeasible_rule(&["trigger"], "q");

        // Use a strict rule body: trigger => p (strict)
        theory.add_rule(Rule::strict(
            "s1",
            vec![Literal::simple("trigger")],
            Literal::simple("p"),
        ));

        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::simple("trigger"))],
            &Literal::simple("q"),
        )
        .unwrap();

        // p should go from +d (baseline) to +D (modified) — a real change
        let p_change = result
            .changed_conclusions
            .iter()
            .find(|(lit, _, _)| lit.name() == "p" && !lit.negation);
        assert!(
            p_change.is_some(),
            "Should detect p upgrading from +d to +D, got: {:?}",
            result.changed_conclusions
        );
        let (_, old, new) = p_change.unwrap();
        assert_eq!(*old, ConclusionType::DefeasiblyProvable);
        assert_eq!(*new, ConclusionType::DefinitelyProvable);
    }

    #[test]
    fn test_what_if_new_conclusion_also_in_changed() {
        // A literal that becomes provable under hypotheticals should appear
        // in new_conclusions AND in changed_conclusions (negative → positive).
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["trigger"], "q");

        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::simple("trigger"))],
            &Literal::simple("q"),
        )
        .unwrap();

        // q is new (not provable in baseline)
        assert!(
            result.new_conclusions.iter().any(|l| l.name() == "q"),
            "q should appear in new_conclusions"
        );
        // q should also appear in changed_conclusions (it went from -d to +d)
        let q_in_changed = result.changed_conclusions.iter().any(|(lit, old, new)| {
            lit.name() == "q" && !lit.negation && !old.is_positive() && new.is_positive()
        });
        assert!(
            q_in_changed,
            "q should appear in changed_conclusions as a negative→positive transition, got {:?}",
            result.changed_conclusions
        );
    }

    #[test]
    fn test_why_not_provable_literal_no_false_blockers() {
        // Regression: why_not on a provable literal should return no blockers
        // and should identify the deriving rule
        let th = make_defeasible_theory();
        let result = why_not(&th, &Literal::simple("flies")).unwrap();

        assert!(!result.has_blockers());
        assert!(
            result.would_derive.is_some(),
            "Should identify deriving rule"
        );
    }

    #[test]
    fn test_why_not_respects_superiority() {
        // r1 > r2: why_not for r1's head should NOT list r2 as blocker
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r1, &r2);

        // flies should be provable since r1 > r2
        let q = query(&theory, &Literal::simple("flies")).unwrap();
        assert!(q.is_provable(), "flies should be provable with r1 > r2");

        // why_not should have no blockers (since it IS provable)
        let result = why_not(&theory, &Literal::simple("flies")).unwrap();
        assert!(
            !result.has_blockers(),
            "why_not should not report blockers for provable literal with superior rule"
        );
    }

    #[test]
    fn test_why_not_superiority_inferior_not_blocker() {
        // When r1 > r2 and flies is NOT provable for other reasons,
        // r2 should still not be listed as a blocker for r1
        let mut theory = Theory::new();
        theory.add_fact("penguin");
        // bird is NOT a fact, so r1's body is unsatisfied
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r1, &r2);

        let result = why_not(&theory, &Literal::simple("flies")).unwrap();
        assert!(result.has_blockers());

        // The blocker should be missing premise (bird), not r2 as contradicted
        let has_r2_blocker = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_rule.as_deref() == Some(&r2));
        assert!(
            !has_r2_blocker,
            "Inferior rule r2 should not be listed as a blocker when r1 > r2"
        );
    }

    #[test]
    fn test_why_not_defeater_not_blocker_when_superior() {
        // When rule is superior to a defeater, the defeater should not block
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("broken_wing");
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let d1 = theory.add_defeater(&["broken_wing"], "~flies");
        theory.add_superiority(&r1, &d1);

        let result = why_not(&theory, &Literal::simple("flies")).unwrap();
        // flies should be provable since r1 > d1
        assert!(
            !result.has_blockers(),
            "Defeater should not block when rule is superior"
        );
    }

    // =========================================================================
    // Trust Filter Tests
    // =========================================================================

    #[test]
    fn test_trust_filter_default_passes_all() {
        let filter = TrustFilter::new();
        let theory = Theory::new();
        let conclusion = Conclusion::defeasibly_provable(Literal::simple("a")).with_rule("r1");
        assert!(filter.passes(&conclusion, &[], &theory, None));
    }

    #[test]
    fn test_trust_filter_min_degree() {
        let policy = TrustPolicy::new(0.5)
            .with_trust("agent:trusted", 0.9)
            .with_trust("agent:untrusted", 0.3);

        let filter = TrustFilter::new().with_min_degree(0.7).with_policy(policy);

        let mut theory = Theory::new();
        let label_a = theory.add_fact("a");
        let label_b = theory.add_fact("b");
        theory.add_meta_string(&label_a, "source", "agent:trusted");
        theory.add_meta_string(&label_b, "source", "agent:untrusted");

        let conclusions = reason(&theory).unwrap();
        let c_a = conclusions
            .iter()
            .find(|c| c.literal.name() == "a" && c.is_positive())
            .unwrap();
        let c_b = conclusions
            .iter()
            .find(|c| c.literal.name() == "b" && c.is_positive())
            .unwrap();

        // 0.9 >= 0.7 → passes; 0.3 < 0.7 → fails
        assert!(filter.passes(c_a, &conclusions, &theory, None));
        assert!(!filter.passes(c_b, &conclusions, &theory, None));
    }

    #[test]
    fn test_trust_filter_source_pattern() {
        let policy = TrustPolicy::new(0.5);
        let filter = TrustFilter::new().with_source("agent:").with_policy(policy);

        let mut theory = Theory::new();
        let label_a = theory.add_fact("a");
        let label_b = theory.add_fact("b");
        theory.add_meta_string(&label_a, "source", "agent:coder");
        theory.add_meta_string(&label_b, "source", "system:policy");

        let conclusions = reason(&theory).unwrap();
        let c_a = conclusions
            .iter()
            .find(|c| c.literal.name() == "a" && c.is_positive())
            .unwrap();
        let c_b = conclusions
            .iter()
            .find(|c| c.literal.name() == "b" && c.is_positive())
            .unwrap();

        assert!(filter.passes(c_a, &conclusions, &theory, None)); // matches "agent:"
        assert!(!filter.passes(c_b, &conclusions, &theory, None)); // doesn't match
    }

    #[test]
    fn test_trust_filter_no_policy_passes_all() {
        let filter = TrustFilter::new()
            .with_min_degree(0.9)
            .with_source("agent:");
        // No policy set, so filter should pass everything
        let theory = Theory::new();
        let conclusion = Conclusion::defeasibly_provable(Literal::simple("a"));
        assert!(filter.passes(&conclusion, &[], &theory, None));
    }

    #[test]
    fn test_trust_filter_combined() {
        let policy = TrustPolicy::new(0.5)
            .with_trust("agent:trusted", 0.9)
            .with_trust("agent:low", 0.3);

        let filter = TrustFilter::new()
            .with_min_degree(0.5)
            .with_source("agent:")
            .with_policy(policy);

        let mut theory = Theory::new();
        let label_a = theory.add_fact("a");
        let label_b = theory.add_fact("b");
        let label_c = theory.add_fact("c");
        theory.add_meta_string(&label_a, "source", "agent:trusted");
        theory.add_meta_string(&label_b, "source", "agent:low");
        theory.add_meta_string(&label_c, "source", "system:policy");

        let conclusions = reason(&theory).unwrap();
        let c_a = conclusions
            .iter()
            .find(|c| c.literal.name() == "a" && c.is_positive())
            .unwrap();
        let c_b = conclusions
            .iter()
            .find(|c| c.literal.name() == "b" && c.is_positive())
            .unwrap();
        let c_c = conclusions
            .iter()
            .find(|c| c.literal.name() == "c" && c.is_positive())
            .unwrap();

        assert!(filter.passes(c_a, &conclusions, &theory, None)); // agent: + 0.9 >= 0.5
        assert!(!filter.passes(c_b, &conclusions, &theory, None)); // agent: + 0.3 < 0.5
        assert!(!filter.passes(c_c, &conclusions, &theory, None)); // system: no match
    }

    #[test]
    fn test_trust_filter_weakest_link_chain() {
        // Chain: low-trust fact → high-trust rule → derived conclusion
        // TrustFilter should see the weakest-link degree (0.3), not the rule's 0.9
        let policy = TrustPolicy::new(0.5)
            .with_trust("agent:low", 0.3)
            .with_trust("agent:high", 0.9);

        let filter = TrustFilter::new().with_min_degree(0.5).with_policy(policy);

        let mut theory = Theory::new();
        theory.add_fact("premise");
        let fact_label: String = theory.rules().next().unwrap().label.clone();
        theory.add_meta_string(&fact_label, "source", "agent:low");
        let rule_label = theory.add_defeasible_rule(&["premise"], "derived");
        theory.add_meta_string(&rule_label, "source", "agent:high");

        let conclusions = reason(&theory).unwrap();
        let c_derived = conclusions
            .iter()
            .find(|c| c.literal.name() == "derived" && c.is_positive())
            .unwrap();

        // Derived conclusion should have weakest-link degree 0.3 < 0.5 → fails
        assert!(
            !filter.passes(c_derived, &conclusions, &theory, None),
            "Derived conclusion should fail filter because weakest-link degree (0.3) < min (0.5)"
        );
    }

    // ==========================================================================
    // matches_query_temporal tests
    // ==========================================================================

    #[test]
    fn test_matches_query_temporal_empty_query_matches_anything() {
        use crate::temporal::{Temporal, TimePoint};

        let empty = Temporal::empty();
        let bounded = Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10));
        let another = Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30));

        assert!(matches_query_temporal(&empty, &empty));
        assert!(matches_query_temporal(&empty, &bounded));
        assert!(matches_query_temporal(&empty, &another));
    }

    #[test]
    fn test_matches_query_temporal_nonempty_requires_exact() {
        use crate::temporal::{Temporal, TimePoint};

        let t1 = Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10));
        let t2 = Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30));
        let t1_dup = Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10));

        assert!(matches_query_temporal(&t1, &t1_dup));
        assert!(!matches_query_temporal(&t1, &t2));
    }

    #[test]
    fn test_matches_query_temporal_nonempty_vs_empty_candidate() {
        use crate::temporal::{Temporal, TimePoint};

        let bounded = Temporal::new(TimePoint::Moment(5), TimePoint::Moment(15));
        let empty = Temporal::empty();

        // Non-empty query should NOT match empty candidate (they differ)
        assert!(!matches_query_temporal(&bounded, &empty));
    }

    // ==========================================================================
    // TEST-014: Query wildcard matches all temporal variants
    // Trace: REQ-007
    // ==========================================================================

    /// An undecorated query (empty temporal) should act as a wildcard,
    /// matching the base literal and all temporal variants.
    #[test]
    fn test_014_query_wildcard_matches_all_temporal_variants() {
        use crate::literal::Literal;
        use crate::mode::Mode;
        use crate::temporal::{Temporal, TimePoint};

        // Query p (no temporal) should match p, p[1,10], p[20,30]
        let query = Literal::simple("p");
        let c_base = Literal::simple("p");
        let c_t1 = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let c_t2 = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30)),
            vec![],
        );

        assert!(matches_query_temporal(&query.temporal, &c_base.temporal));
        assert!(matches_query_temporal(&query.temporal, &c_t1.temporal));
        assert!(matches_query_temporal(&query.temporal, &c_t2.temporal));
    }

    // ==========================================================================
    // TEST-015: Query with specific temporal matches only exact
    // Trace: REQ-007
    // ==========================================================================

    /// A query with specific temporal bounds should only match a candidate
    /// with identical temporal bounds — not different windows or the base.
    #[test]
    fn test_015_query_with_specific_temporal_matches_only_exact() {
        use crate::literal::Literal;
        use crate::mode::Mode;
        use crate::temporal::{Temporal, TimePoint};

        let query = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let c_match = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let c_different = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30)),
            vec![],
        );
        let c_base = Literal::simple("p");

        assert!(matches_query_temporal(&query.temporal, &c_match.temporal));
        assert!(!matches_query_temporal(
            &query.temporal,
            &c_different.temporal
        ));
        assert!(!matches_query_temporal(&query.temporal, &c_base.temporal));
    }
}
