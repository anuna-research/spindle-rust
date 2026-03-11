//! Reasoning engine for defeasible logic
//!
//! Implements the standard DL(d) forward chaining algorithm and provides
//! the [`Reasoner`] trait for abstracting over reasoning backends.
//!
//! # Architecture
//!
//! The reasoning state (worklist, proven sets, body counters, conclusions)
//! is consolidated into [`ReasoningState`](state::ReasoningState) in the
//! [`state`] submodule. This makes data-flow dependencies between phases
//! explicit and enables phase-isolated testing.
//!
//! # Trait-based dispatch
//!
//! The [`Reasoner`] trait abstracts over reasoning algorithms. One concrete
//! implementation is provided:
//!
//! - [`StandardReasoner`]: wraps the standard DL(d) forward-chaining
//!   algorithm (the default).
//!
//! Additional backends can be added by implementing the [`Reasoner`] trait.
//! Use [`select_reasoner`] to obtain a boxed trait object by name for
//! runtime backend selection.
//!
//! # Performance
//!
//! Uses `LitId` (4-byte Copy type) and BitSet for O(1) proven literal
//! checks, eliminating heap allocations and hash computations in the hot
//! reasoning loop.

pub(crate) mod defeasible;
pub(crate) mod definite;
pub(crate) mod facts;
pub(crate) mod state;

use rustc_hash::FxHashSet;

use crate::conclusion::Conclusion;
use crate::error::Result;
use crate::index::IndexedTheory;
use crate::pipeline::{PrepareOptions, prepare};
use crate::projection::{
    ProjectionDiagnostics, ProjectionEngine, ProjectionSnapshot, ProjectionToken,
};
use crate::rule::Rule;
use crate::theory::Theory;

use self::state::ReasoningState;

pub(crate) struct ReasoningTrace {
    pub conclusions: Vec<Conclusion>,
    pub projection_labels: FxHashSet<String>,
}

fn collect_projection_labels(state: &ReasoningState<'_>) -> FxHashSet<String> {
    let mut labels = state
        .conclusions
        .iter()
        .filter(|c| c.is_positive())
        .filter_map(|c| c.rule_label.clone())
        .collect::<FxHashSet<_>>();

    labels.extend(
        state
            .defeasible_body_remaining
            .iter()
            .filter(|(label, remaining)| {
                **remaining == 0 && !state.rule_discarded.get(**label).copied().unwrap_or(false)
            })
            .map(|(label, _)| (*label).to_string()),
    );
    labels.extend(state.projection_labels.iter().cloned());
    labels
}

pub(crate) fn should_project_rule(_rule: &Rule) -> bool {
    true
}

// ---------------------------------------------------------------------------
// Reasoner trait
// ---------------------------------------------------------------------------

/// A reasoning engine that computes conclusions from an indexed theory.
///
/// Implementors encapsulate a specific defeasible logic algorithm
/// (e.g., standard DL(d) forward chaining, scalable DL(d||) three-phase
/// closure, or a test stub).
///
/// # Thread safety
///
/// The `Send + Sync` bounds are required for sharing a `dyn Reasoner`
/// across threads in async CLI or WASM-worker contexts.
///
/// # Statelessness
///
/// Reasoners are stateless algorithm selectors; all mutable working state
/// lives in stack-local variables within [`reason`](Reasoner::reason).
pub trait Reasoner: Send + Sync {
    /// Compute all conclusions (positive and negative) for the given
    /// indexed theory.
    ///
    /// The theory must already be prepared (grounded, temporally filtered,
    /// validated) before being passed here. The indexed theory is passed
    /// as `&mut` because fact initialization may intern new literals into
    /// the index.
    fn reason<'t>(&self, theory: &mut IndexedTheory<'t>) -> Result<Vec<Conclusion>>;

    /// Human-readable name for diagnostics and logging.
    fn name(&self) -> &str;
}

// ---------------------------------------------------------------------------
// StandardReasoner
// ---------------------------------------------------------------------------

/// Standard DL(d) forward-chaining reasoner.
///
/// Wraps the existing algorithm from the `reason` module. This is the
/// default backend.
#[derive(Debug, Clone, Copy, Default)]
pub struct StandardReasoner;

impl Reasoner for StandardReasoner {
    fn reason<'t>(&self, indexed: &mut IndexedTheory<'t>) -> Result<Vec<Conclusion>> {
        reason_indexed(indexed)
    }

    fn name(&self) -> &str {
        "standard-dl-d"
    }
}

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

/// Select a reasoner implementation by name.
///
/// Returns a boxed trait object for runtime backend selection. Currently
/// only `"standard"` is recognized; all other names fall back to
/// [`StandardReasoner`].
pub fn select_reasoner(name: &str) -> Box<dyn Reasoner> {
    match name {
        "standard" => Box::new(StandardReasoner),
        "shadow" => Box::new(crate::shadow::ShadowReasoner::enabled()),
        _ => Box::new(StandardReasoner),
    }
}

// ---------------------------------------------------------------------------
// ReasonResult: enriched output with projection tokens
// ---------------------------------------------------------------------------

/// The result of a full reasoning pass, including both conclusions and
/// projection tokens.
///
/// The projection engine runs automatically on positive contributors and on
/// applicable blockers, emitting [`ProjectionToken`]s that carry family-level
/// support and attack information without synthetic bridge rules.
#[derive(Debug, Clone)]
pub struct ReasonResult {
    /// All conclusions (positive and negative) from the DL(d) reasoner.
    pub conclusions: Vec<Conclusion>,
    /// Projection tokens emitted for positive contributors and blocker rules.
    /// These provide family-level support/attack semantics for temporal
    /// literals even when a rule only prevented a proof.
    pub projection_tokens: Vec<ProjectionToken>,
    /// Structured diagnostic counters for projection activity.
    pub diagnostics: ProjectionDiagnostics,
    /// Deterministic debug snapshot of projection state.
    pub snapshot: ProjectionSnapshot,
}

// ---------------------------------------------------------------------------
// Free-function convenience API (unchanged public surface)
// ---------------------------------------------------------------------------

/// Perform defeasible reasoning on a theory.
pub fn reason(theory: &Theory) -> Result<Vec<Conclusion>> {
    reason_with_options(theory, PrepareOptions::default())
}

/// Perform defeasible reasoning on a theory with custom options
///
/// This is the primary API for as-of reasoning. Use `reference_time` in options
/// to reason at a specific point in time:
///
/// ```rust
/// use spindle_core::prelude::*;
/// use spindle_core::reason::reason_with_options;
/// use spindle_core::pipeline::PrepareOptions;
/// use spindle_core::temporal::TimePoint;
///
/// let mut theory = Theory::new();
/// theory.add_fact("bird");
///
/// // Reason at a specific time (milliseconds since epoch)
/// let opts = PrepareOptions {
///     reference_time: Some(TimePoint::from_millis(1707220800000)),
///     ..Default::default()
/// };
/// let conclusions = reason_with_options(&theory, opts).unwrap();
/// ```
pub fn reason_with_options(theory: &Theory, opts: PrepareOptions) -> Result<Vec<Conclusion>> {
    // Phase 0: Pipeline (Filtering + Validation + Grounding)
    let prepared = prepare(theory, opts)?;

    reason_prepared(&prepared.theory)
}

/// Perform defeasible reasoning on an already-prepared theory.
///
/// Use this when you have already called [`prepare()`] and want to avoid
/// redundant pipeline work. The theory must have been prepared with the
/// desired options (grounding, temporal filtering, etc.) before calling
/// this function.
///
/// Internally builds an [`IndexedTheory`] and delegates to
/// [`reason_indexed`].
pub fn reason_prepared(theory: &Theory) -> Result<Vec<Conclusion>> {
    let mut indexed = IndexedTheory::try_build(theory)?;
    reason_indexed(&mut indexed)
}

/// Perform full reasoning with projection tokens on a theory.
///
/// Like [`reason`], but returns a [`ReasonResult`] that includes
/// projection tokens alongside conclusions. The projection engine runs on
/// positive contributors and applicable blockers, preserving both temporal
/// and authored atemporal contributors in the family-level audit stream.
pub fn reason_full(theory: &Theory) -> Result<ReasonResult> {
    reason_full_with_options(theory, PrepareOptions::default())
}

/// Perform full reasoning with projection tokens and custom options.
///
/// Like [`reason_with_options`], but returns a [`ReasonResult`] that
/// includes projection tokens alongside conclusions.
pub fn reason_full_with_options(theory: &Theory, opts: PrepareOptions) -> Result<ReasonResult> {
    let prepared = prepare(theory, opts)?;
    reason_full_prepared(&prepared.theory)
}

/// Perform full reasoning with projection tokens on a prepared theory.
///
/// Like [`reason_prepared`], but returns a [`ReasonResult`] that
/// includes projection tokens alongside conclusions.
pub fn reason_full_prepared(theory: &Theory) -> Result<ReasonResult> {
    let mut indexed = IndexedTheory::try_build(theory)?;
    reason_full_indexed(&mut indexed)
}

/// Core reasoning with projection, operating on an already-indexed theory.
///
/// Runs the standard DL(d) algorithm, then projects contributing rules
/// through the [`ProjectionEngine`] to produce family-level support and
/// attack tokens.
pub fn reason_full_indexed(indexed: &mut IndexedTheory<'_>) -> Result<ReasonResult> {
    let trace = reason_indexed_trace(indexed)?;
    let ReasoningTrace {
        conclusions,
        projection_labels: contributing_labels,
    } = trace;

    let mut engine = ProjectionEngine::with_capacity(conclusions.len() * 2);
    let theory = indexed.theory();
    for label in &contributing_labels {
        if let Some(rule) = theory.get_rule(label) {
            engine.try_project_rule(rule, indexed)?;
        }
    }

    let diagnostics = engine.diagnostics();
    let snapshot = engine.snapshot();

    Ok(ReasonResult {
        conclusions,
        projection_tokens: engine.into_tokens(),
        diagnostics,
        snapshot,
    })
}

/// Core reasoning loop operating on an already-indexed theory.
///
/// This is the function that [`StandardReasoner`] delegates to.
/// Orchestrates three phases:
///
/// 1. **Fact Initialization + Strict Forward Chaining** -- seed facts and
///    empty-body strict rules, then forward-chain strict rules only until
///    all definite (+D) conclusions are computed.
/// 2. **Defeasible Fixed-Point** -- compute +d and -d conclusions using
///    SDL ambiguity blocking semantics with a worklist-based fixed-point loop.
/// 3. **Negative Conclusions** -- emit `-D` and `-d` for all unproven
///    literals.
pub fn reason_indexed(indexed: &mut IndexedTheory<'_>) -> Result<Vec<Conclusion>> {
    Ok(reason_indexed_trace(indexed)?.conclusions)
}

pub(crate) fn reason_indexed_trace(indexed: &mut IndexedTheory<'_>) -> Result<ReasoningTrace> {
    let theory = indexed.theory();

    // Pre-allocate state sized for the theory.
    let atom_count = indexed.atom_count();
    let rule_count = theory.rule_count();
    let estimated_size = rule_count * 2 + indexed.all_literal_ids().count() * 2;

    let mut state = ReasoningState::new(atom_count, rule_count, estimated_size);

    // Phase 1: Initialize facts and body_remaining counters
    facts::initialize_facts(theory, indexed, &mut state);

    // Phase 1 continued: Forward chaining with strict rules only
    definite::forward_chain_strict(theory, indexed, &mut state);

    // Phase 2: Defeasible fixed-point loop + Phase 3: Negative conclusions
    defeasible::resolve_defeasible(theory, indexed, &mut state);

    let projection_labels = collect_projection_labels(&state);

    Ok(ReasoningTrace {
        conclusions: state.conclusions,
        projection_labels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::index::LitId;
    use crate::literal::Literal;
    use crate::projection::ProjectionToken;
    use crate::rule::{Rule, RuleType};
    use crate::temporal::Temporal;

    use super::state::LiteralBitSet;

    // ==========================================================================
    // BASIC FACTS AND REASONING
    // ==========================================================================

    #[test]
    fn test_simple_fact() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "bird")
        );
    }

    #[test]
    fn test_negated_fact() {
        let mut theory = Theory::new();
        theory.add_fact("~guilty");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "guilty"
                    && c.literal.negation)
        );
    }

    #[test]
    fn test_strict_chain() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_strict_rule(&["bird"], "animal");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "animal")
        );
    }

    #[test]
    fn test_defeasible_rule() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies")
        );
    }

    #[test]
    fn test_multiple_body_literals() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("q");
        theory.add_fact("r");
        theory.add_defeasible_rule(&["p", "q", "r"], "s");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "s"),
            "s should be defeasibly provable when all antecedents are satisfied"
        );
    }

    #[test]
    fn test_unsatisfied_rule_body() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p", "q"], "r");

        let conclusions = reason(&theory).unwrap();

        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "r"),
            "r should not be provable without q"
        );
    }

    // ==========================================================================
    // CONFLICT AND SUPERIORITY TESTS
    // ==========================================================================

    #[test]
    fn test_penguin_doesnt_fly() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");

        theory.add_superiority(&r2, &r1);

        let conclusions = reason(&theory).unwrap();

        // ~flies should be defeasibly provable (penguins don't fly)
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && c.literal.negation)
        );
    }

    #[test]
    fn test_superiority_resolves_conflict() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["bird"], "~flies");

        theory.add_superiority(&r1, &r2);

        let conclusions = reason(&theory).unwrap();

        // Superior rule r1 should win
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && !c.literal.negation),
            "flies should be defeasibly provable via superior rule"
        );
    }

    #[test]
    fn test_strict_beats_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_strict_rule(&["a", "b"], "c");
        theory.add_defeasible_rule(&["a", "b"], "~c");

        let conclusions = reason(&theory).unwrap();

        // Strict rule should definitely prove c
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "c"
                    && !c.literal.negation),
            "c should be definitely provable via strict rule"
        );

        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "c"
                    && c.literal.negation
            }),
            "~c should not be defeasibly provable against a strict attacker"
        );
    }

    #[test]
    fn test_forward_chaining() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "d");

        let conclusions = reason(&theory).unwrap();

        for lit_name in &["b", "c", "d"] {
            assert!(
                conclusions
                    .iter()
                    .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == *lit_name),
                "{lit_name} should be defeasibly provable via chain"
            );
        }
    }

    // ==========================================================================
    // NEGATIVE CONCLUSIONS (-d, -D)
    // ==========================================================================

    #[test]
    fn test_negative_definite_conclusion() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let conclusions = reason(&theory).unwrap();

        // q has no strict path, so -D q
        assert!(
            conclusions.iter().any(
                |c| c.conclusion_type == ConclusionType::DefinitelyNotProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            ),
            "q should be definitely NOT provable (no strict rule)"
        );
    }

    #[test]
    fn test_negative_defeasible_unsatisfied() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q"); // p not proven

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(
                |c| c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                    && c.literal.name() == "q"
            ),
            "q should be defeasibly NOT provable when body unsatisfied"
        );
    }

    #[test]
    fn test_superiority_yields_negative_for_inferior() {
        let mut theory = Theory::new();
        theory.add_fact("p");

        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");

        theory.add_superiority(&r1, &r2);

        let conclusions = reason(&theory).unwrap();

        // r1 > r2, so +d q, but ~q should be blocked
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation),
            "q should be defeasibly provable via superior rule"
        );
    }

    // ==========================================================================
    // DEFEATER TESTS
    // ==========================================================================

    #[test]
    fn test_defeater_blocks_conclusion() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeater(&["p"], "~q");

        let conclusions = reason(&theory).unwrap();

        // Defeater should block q from being proven
        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        let has_not_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && c.literal.negation
        });

        // Defeater blocks q from being proven
        assert!(
            !has_q,
            "q should NOT be defeasibly provable when blocked by defeater"
        );

        // Defeater doesn't prove ~q (it only blocks)
        assert!(
            !has_not_q,
            "~q should NOT be defeasibly provable by defeater alone"
        );
    }

    #[test]
    fn test_defeater_doesnt_prove() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeater(&["p"], "q");

        let conclusions = reason(&theory).unwrap();

        // Defeaters don't prove anything
        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"),
            "Defeater alone should not prove q"
        );
    }

    // ==========================================================================
    // EDGE CASES
    // ==========================================================================

    #[test]
    fn test_empty_theory() {
        let theory = Theory::new();
        let conclusions = reason(&theory).unwrap();

        // Empty theory should produce no positive conclusions
        assert!(
            !conclusions.iter().any(|c| c.conclusion_type.is_positive()),
            "Empty theory should produce no positive conclusions"
        );
    }

    #[test]
    fn test_theory_with_only_rules() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["q"], "r");

        let conclusions = reason(&theory).unwrap();

        // No facts, so no positive conclusions
        assert!(
            !conclusions.iter().any(|c| c.conclusion_type.is_positive()),
            "Theory with no facts should produce no positive conclusions"
        );
    }

    #[test]
    fn test_self_referential_rule() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "p");

        let conclusions = reason(&theory).unwrap();

        // Should not crash and p should not be proven
        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"),
            "Self-referential rule without initial fact should not prove p"
        );
    }

    #[test]
    fn test_circular_dependencies() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["q"], "r");
        theory.add_defeasible_rule(&["r"], "p");

        let conclusions = reason(&theory).unwrap();

        // Should terminate without infinite loop - the fact that we reach this assertion
        // means the function terminated successfully
        let _ = conclusions;
    }

    #[test]
    fn test_conflicting_facts() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("~p");

        let conclusions = reason(&theory).unwrap();

        // Both should be definitely provable (inconsistent theory)
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation),
            "p should be definitely provable"
        );
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && c.literal.negation),
            "~p should be definitely provable"
        );
    }

    // ==========================================================================
    // STRESS TESTS
    // ==========================================================================

    #[test]
    fn test_long_chain() {
        let mut theory = Theory::new();
        theory.add_fact("l0");

        for i in 0..50 {
            theory.add_defeasible_rule(&[&format!("l{i}")], &format!("l{}", i + 1));
        }

        let conclusions = reason(&theory).unwrap();

        // All literals in chain should be provable
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "l50"),
            "l50 should be defeasibly provable through long chain"
        );
    }

    #[test]
    fn test_wide_theory() {
        let mut theory = Theory::new();

        // 100 independent facts and derived conclusions
        for i in 0..100 {
            theory.add_fact(&format!("fact{i}"));
            theory.add_defeasible_rule(&[&format!("fact{i}")], &format!("derived{i}"));
        }

        let conclusions = reason(&theory).unwrap();

        // Check a sample of derived conclusions
        for i in [0, 25, 50, 75, 99].iter() {
            assert!(
                conclusions
                    .iter()
                    .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == format!("derived{i}")),
                "derived{i} should be defeasibly provable"
            );
        }
    }

    #[test]
    fn test_many_facts() {
        let mut theory = Theory::new();

        for i in 0..100 {
            theory.add_fact(&format!("p{i}"));
        }

        let conclusions = reason(&theory).unwrap();

        let definite_count = conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
            .count();

        assert!(
            definite_count >= 100,
            "Should definitely prove all {} facts, got {}",
            100,
            definite_count
        );
    }

    // ==========================================================================
    // ADDITIONAL COVERAGE TESTS
    // ==========================================================================

    #[test]
    fn test_rule_superior_to_defeater() {
        // Test that a rule can override a defeater with explicit superiority
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let d1 = theory.add_defeater(&["p"], "~q");
        theory.add_superiority(&r1, &d1); // r1 is superior to defeater

        let conclusions = reason(&theory).unwrap();

        // q should be provable because r1 > d1
        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(
            has_q,
            "q should be defeasibly provable when rule is superior to defeater"
        );
    }

    #[test]
    fn test_ambiguity_blocking_no_superiority() {
        // SDL ambiguity blocking: conflicting defeasible rules with no superiority
        // relation should block BOTH conclusions (neither is +d).
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");
        // No superiority relation

        let conclusions = reason(&theory).unwrap();

        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });
        let has_not_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && c.literal.negation
        });

        assert!(
            !has_q,
            "q should NOT be defeasibly provable (ambiguity blocking)"
        );
        assert!(
            !has_not_q,
            "~q should NOT be defeasibly provable (ambiguity blocking)"
        );

        // Both should be -d
        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "-d q expected"
        );
        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                    && c.literal.name() == "q"
                    && c.literal.negation
            }),
            "-d ~q expected"
        );
    }

    #[test]
    fn test_attacker_with_unsatisfied_body() {
        // Attacker's body is not satisfied, so it can't block
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["x"], "~q"); // x is not a fact

        let conclusions = reason(&theory).unwrap();

        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(
            has_q,
            "q should be provable when attacker's body is unsatisfied"
        );
    }

    #[test]
    fn test_attacker_superior_blocks() {
        // Attacker is superior, so it blocks the defender
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");
        theory.add_superiority(&r2, &r1); // r2 > r1

        let conclusions = reason(&theory).unwrap();

        // q should NOT be provable (blocked by superior r2)
        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        // ~q should be provable
        let has_not_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && c.literal.negation
        });

        assert!(!has_q, "q should be blocked by superior attacker");
        assert!(has_not_q, "~q should be provable via superior rule");
    }

    #[test]
    fn test_empty_body_strict_rule() {
        // Empty-body strict rules should fire and prove their head
        use crate::rule::Rule;

        let mut theory = Theory::new();
        let rule = Rule::new(
            "axiom",
            RuleType::Strict,
            vec![],
            vec![Literal::simple("truth")],
        );
        theory.add_rule(rule);

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "truth"),
            "Empty-body strict rule should prove its head"
        );
    }

    #[test]
    fn test_empty_body_defeasible_rule() {
        use crate::rule::Rule;

        let mut theory = Theory::new();
        let rule = Rule::new(
            "axiom",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("maybe")],
        );
        theory.add_rule(rule);

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "maybe"),
            "Empty-body defeasible rule should prove its head"
        );
    }

    #[test]
    fn test_empty_body_rule_chains() {
        // Empty-body rule should seed forward chaining
        use crate::rule::Rule;

        let mut theory = Theory::new();
        let axiom = Rule::new(
            "axiom",
            RuleType::Strict,
            vec![],
            vec![Literal::simple("base")],
        );
        theory.add_rule(axiom);
        theory.add_defeasible_rule(&["base"], "derived");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "derived"),
            "Forward chaining should work from empty-body rule heads"
        );
    }

    #[test]
    fn test_empty_body_strict_not_lost_when_defeasible_same_head_exists() {
        // Regression: if an empty-body defeasible rule for `p` is seen before an
        // empty-body strict rule for `p`, strict derivation must still be emitted.
        // We iterate labels to avoid depending on HashMap iteration order.
        use crate::rule::Rule;

        for i in 0..128 {
            let mut theory = Theory::new();
            theory.add_rule(Rule::new(
                format!("d{i}"),
                RuleType::Defeasible,
                vec![],
                vec![Literal::simple("p")],
            ));
            theory.add_rule(Rule::new(
                format!("s{i}"),
                RuleType::Strict,
                vec![],
                vec![Literal::simple("p")],
            ));

            let conclusions = reason(&theory).unwrap();
            let has_definite_p = conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            });

            assert!(has_definite_p, "missing +D p for label pair d{i} / s{i}");
        }
    }

    #[test]
    fn test_fact_plus_empty_body_strict_no_duplicate_conclusions() {
        // A fact already proves +D p. An empty-body strict rule for p should not
        // emit a second +D p conclusion.
        use crate::rule::Rule;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_rule(Rule::new(
            "axiom",
            RuleType::Strict,
            vec![],
            vec![Literal::simple("p")],
        ));

        let conclusions = reason(&theory).unwrap();
        let definite_p_count = conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            })
            .count();

        assert_eq!(definite_p_count, 1, "Should have exactly one +D p");
    }

    #[test]
    fn test_fact_plus_empty_body_defeasible_no_duplicate_and_chains() {
        // A fact already proves p. An empty-body defeasible rule for p should
        // not produce a duplicate, and forward chaining from p should still work.
        use crate::rule::Rule;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_rule(Rule::new(
            "d_axiom",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));
        theory.add_defeasible_rule(&["p"], "q");

        let conclusions = reason(&theory).unwrap();

        let defeasible_p_count = conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            })
            .count();
        assert_eq!(defeasible_p_count, 1, "Should have exactly one +d p");

        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });
        assert!(has_q, "Forward chaining from p to q should still work");
    }

    #[test]
    fn test_two_empty_body_defeasible_same_head_only_one_conclusion() {
        // Two empty-body defeasible rules for the same head should produce
        // exactly one +d conclusion, not two.
        use crate::rule::Rule;

        let mut theory = Theory::new();
        theory.add_rule(Rule::new(
            "d1",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));
        theory.add_rule(Rule::new(
            "d2",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));

        let conclusions = reason(&theory).unwrap();
        let defeasible_p_count = conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            })
            .count();

        assert_eq!(defeasible_p_count, 1, "Should have exactly one +d p");
    }

    // ==========================================================================
    // REGRESSION TESTS: Duplicate fact deduplication
    // ==========================================================================

    #[test]
    fn test_duplicate_facts_no_double_conclusions() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("bird"); // duplicate

        let conclusions = reason(&theory).unwrap();

        let bird_definite_count = conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "bird"
            })
            .count();

        assert_eq!(
            bird_definite_count, 1,
            "Duplicate facts should produce exactly one +D conclusion, got {bird_definite_count}"
        );
    }

    #[test]
    fn test_duplicate_facts_no_premature_rule_firing() {
        // Critical regression test: duplicate facts must not cause
        // multi-body rules to fire with unsatisfied body literals.
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("p"); // duplicate
        theory.add_defeasible_rule(&["p", "q"], "r");

        let conclusions = reason(&theory).unwrap();

        let has_r = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "r"
        });

        assert!(
            !has_r,
            "Rule with body [p, q] must NOT fire when only p is proven (even with duplicate p facts)"
        );
    }

    // ==========================================================================
    // REGRESSION TESTS: Defeasible conflict handling
    // ==========================================================================

    #[test]
    fn test_conflict_resolution_with_superiority() {
        // When superiority resolves the conflict, the superior rule should win
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");
        theory.add_superiority(&r1, &r2);

        let conclusions = reason(&theory).unwrap();

        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(has_q, "Superior rule should prove q");
    }

    #[test]
    fn test_conflict_resolution_no_conflict_when_attacker_unsatisfied() {
        // If attacker's body is not satisfied, no blocking should occur
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["unproven"], "~q");

        let conclusions = reason(&theory).unwrap();

        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(
            has_q,
            "q should be provable when attacker body is unsatisfied"
        );
    }

    // ==========================================================================
    // ASSERTION MESSAGE COVERAGE TESTS
    // These tests verify assertion messages by intentionally triggering failures
    // ==========================================================================

    #[test]
    #[should_panic(expected = "s should be defeasibly provable when all antecedents are satisfied")]
    fn test_assert_msg_multiple_body_literals() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        // Intentionally missing q and r facts
        theory.add_defeasible_rule(&["p", "q", "r"], "s");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "s"),
            "s should be defeasibly provable when all antecedents are satisfied"
        );
    }

    #[test]
    #[should_panic(expected = "r should not be provable without q")]
    fn test_assert_msg_unsatisfied_rule_body() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("q"); // Adding q so r IS provable
        theory.add_defeasible_rule(&["p", "q"], "r");

        let conclusions = reason(&theory).unwrap();

        // This will fail because r IS provable (we added q)
        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "r"),
            "r should not be provable without q"
        );
    }

    #[test]
    #[should_panic(expected = "flies should be defeasibly provable via superior rule")]
    fn test_assert_msg_superiority_resolves_conflict() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["bird"], "~flies");

        // Wrong direction - r2 beats r1, so ~flies wins instead of flies
        theory.add_superiority(&r2, &r1);

        let conclusions = reason(&theory).unwrap();

        // This fails because r2 > r1 means ~flies wins, not flies
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && !c.literal.negation),
            "flies should be defeasibly provable via superior rule"
        );
    }

    #[test]
    #[should_panic(expected = "c should be definitely provable via strict rule")]
    fn test_assert_msg_strict_beats_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        // Missing fact "b"
        theory.add_strict_rule(&["a", "b"], "c");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "c"
                    && !c.literal.negation),
            "c should be definitely provable via strict rule"
        );
    }

    #[test]
    #[should_panic(expected = "should be defeasibly provable via chain")]
    fn test_assert_msg_forward_chaining() {
        let mut theory = Theory::new();
        // No facts - chain won't fire
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let conclusions = reason(&theory).unwrap();

        for lit_name in &["b", "c"] {
            assert!(
                conclusions
                    .iter()
                    .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == *lit_name),
                "{lit_name} should be defeasibly provable via chain"
            );
        }
    }

    #[test]
    #[should_panic(expected = "q should be definitely NOT provable")]
    fn test_assert_msg_negative_definite() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q"); // Adding strict rule so q IS provable

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(
                |c| c.conclusion_type == ConclusionType::DefinitelyNotProvable
                    && c.literal.name() == "q"
            ),
            "q should be definitely NOT provable (no strict rule)"
        );
    }

    #[test]
    #[should_panic(expected = "q should be defeasibly NOT provable when body unsatisfied")]
    fn test_assert_msg_negative_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q"); // Adding rule so q IS provable

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(
                |c| c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                    && c.literal.name() == "q"
            ),
            "q should be defeasibly NOT provable when body unsatisfied"
        );
    }

    #[test]
    #[should_panic(expected = "Empty theory should produce no positive conclusions")]
    fn test_assert_msg_empty_theory() {
        let mut theory = Theory::new();
        theory.add_fact("x"); // Not empty

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().all(|c| !c.conclusion_type.is_positive()),
            "Empty theory should produce no positive conclusions"
        );
    }

    #[test]
    #[should_panic(expected = "Self-referential rule without initial fact should not prove p")]
    fn test_assert_msg_self_reference() {
        let mut theory = Theory::new();
        theory.add_fact("p"); // Adding initial fact so it IS provable
        theory.add_defeasible_rule(&["p"], "p");

        let conclusions = reason(&theory).unwrap();

        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"),
            "Self-referential rule without initial fact should not prove p"
        );
    }

    // ==========================================================================
    // reason_with_options API TESTS (spec 3.2, Milestone 5)
    // ==========================================================================

    #[test]
    fn test_reason_with_options_default_matches_reason() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let conclusions_default = reason(&theory).unwrap();
        let conclusions_with_opts =
            reason_with_options(&theory, PrepareOptions::default()).unwrap();

        // Both should produce the same positive conclusions
        let pos_default: Vec<_> = conclusions_default
            .iter()
            .filter(|c| c.is_positive())
            .map(|c| c.literal.name())
            .collect();

        let pos_with_opts: Vec<_> = conclusions_with_opts
            .iter()
            .filter(|c| c.is_positive())
            .map(|c| c.literal.name())
            .collect();

        assert_eq!(pos_default, pos_with_opts);
    }

    #[test]
    fn test_reason_with_options_accepts_reference_time() {
        use crate::temporal::{Temporal, TimePoint};

        let mut theory = Theory::new();

        // Add a fact with a specific temporal window
        let bird_lit = Literal::new(
            "bird",
            false,
            crate::mode::Mode::default(),
            Temporal::from_bounds(1000, 2000), // active from 1000 to 2000
            vec![],
        );
        theory.add_rule(crate::rule::Rule::fact("f1", bird_lit));

        // Reason at time 1500 (inside the window)
        let opts_inside = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(1500)),
            ..Default::default()
        };
        let conclusions_inside = reason_with_options(&theory, opts_inside).unwrap();
        let has_bird_inside = conclusions_inside
            .iter()
            .any(|c| c.literal.name() == "bird" && c.is_positive());
        assert!(has_bird_inside, "bird should be provable at time 1500");

        // Reason at time 3000 (outside the window)
        let opts_outside = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(3000)),
            ..Default::default()
        };
        let conclusions_outside = reason_with_options(&theory, opts_outside).unwrap();
        let has_bird_outside = conclusions_outside
            .iter()
            .any(|c| c.literal.name() == "bird" && c.is_positive());
        assert!(
            !has_bird_outside,
            "bird should NOT be provable at time 3000 (outside temporal window)"
        );
    }

    #[test]
    fn test_reason_keeps_disjoint_temporal_windows_distinct() {
        let early = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::from_bounds(1, 10),
            vec![],
        );
        let late = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::from_bounds(20, 30),
            vec![],
        );
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", early));
        theory.add_rule(Rule::defeasible("r1", vec![late], Literal::simple("q")));

        let conclusions = reason(&theory).unwrap();

        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "a fact for p[1,10] must not satisfy a rule body requiring p[20,30]"
        );
    }

    #[test]
    fn test_reason_full_defeater_projects_attack_without_exact_support() {
        let temporal_head = Literal::new(
            "q",
            true,
            Default::default(),
            Temporal::from_bounds(1, 10),
            vec![],
        );
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_rule(Rule::defeater(
            "d1",
            vec![Literal::simple("a")],
            temporal_head,
        ));

        let result = reason_full(&theory).unwrap();

        assert!(result.projection_tokens.iter().any(|token| matches!(
            token,
            ProjectionToken::Attack(attack) if attack.rule_label == "d1"
        )));
        assert!(!result.projection_tokens.iter().any(|token| matches!(
            token,
            ProjectionToken::Exact(exact) if exact.rule_label == "d1"
        )));
    }

    #[test]
    fn test_reason_full_projects_mixed_temporal_and_atemporal_contributors() {
        let temporal_head = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::from_bounds(1, 10),
            vec![],
        );
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_rule(Rule::defeasible(
            "r_temporal",
            vec![Literal::simple("a")],
            temporal_head,
        ));
        theory.add_rule(Rule::defeasible(
            "r_atemporal",
            vec![Literal::simple("a")],
            Literal::negated("p"),
        ));

        let result = reason_full(&theory).unwrap();

        assert!(result.projection_tokens.iter().any(|token| matches!(
            token,
            ProjectionToken::Family(family) if family.rule_label == "r_temporal"
        )));
        assert!(result.projection_tokens.iter().any(|token| matches!(
            token,
            ProjectionToken::Family(family) if family.rule_label == "r_atemporal"
        )));
    }

    #[test]
    fn test_reason_with_options_grounding_can_be_disabled() {
        use crate::pipeline::GroundingOptions;

        let mut theory = Theory::new();
        theory.add_fact("p");

        // With grounding disabled, variables won't be instantiated
        let opts = PrepareOptions {
            grounding: GroundingOptions {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };

        // Should still work for ground theories
        let conclusions = reason_with_options(&theory, opts).unwrap();
        assert!(
            conclusions.iter().any(|c| c.is_positive()),
            "should have positive conclusions"
        );
    }

    // ==========================================================================
    // REGRESSION TESTS: LiteralBitSet auto-grow
    // (LiteralBitSet is now in state.rs; these tests remain here
    // to verify integration)
    // ==========================================================================

    #[test]
    fn test_bitset_grows_on_insert_beyond_capacity() {
        use crate::index::AtomId;

        // Create a small bitset (capacity for 2 atoms = 4 bits)
        let mut bitset = LiteralBitSet::new(2);

        // Insert at atom 10, well beyond initial capacity
        let lit_id = LitId::new(AtomId::from_raw(10), false);
        bitset.insert(lit_id);

        assert!(
            bitset.contains(lit_id),
            "Bitset should contain the inserted literal after growing"
        );
    }

    // ==========================================================================
    // SDL SPEC COMPLIANCE TESTS (specs/DEFEASIBLE-LOGIC-SEMANTICS.md)
    // ==========================================================================

    #[test]
    fn test_spec_worked_example_conflicting_facts() {
        // Spec worked example (lines 114-134):
        //   f1: >> p
        //   f2: >> -p
        // Both are facts. No superiority. Result: -d p, -d -p.
        //
        // Note: our implementation treats facts as +D (definitively provable).
        // With +D p and +D -p, condition (2) of +d fails for both:
        //   +d p requires -D -p, but +D -p, so blocked.
        //   +d -p requires -D p, but +D p, so blocked.
        // So neither is defeasibly provable even though both are definite.
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("~p");

        let conclusions = reason(&theory).unwrap();

        // Both should be +D (they are axioms)
        assert!(conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.name() == "p"
                && !c.literal.negation
        }));
        assert!(conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.name() == "p"
                && c.literal.negation
        }));
    }

    #[test]
    fn test_spec_ambiguity_blocking_localized() {
        // Spec section "Ambiguity Blocking" (lines 143-148):
        // When p and ~p are blocked, downstream rules that don't depend on p
        // proceed normally.
        //
        //   r1: => p
        //   r2: => -p
        //   r3: => q          (independent support for q)
        //   r4: p => q         (depends on ambiguous p)
        //
        // Result: -d p, -d -p, but +d q (via r3, uncontested)
        use crate::rule::Rule;

        let mut theory = Theory::new();
        theory.add_rule(Rule::new(
            "r1",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));
        theory.add_rule(Rule::new(
            "r2",
            RuleType::Defeasible,
            vec![],
            vec![Literal::new(
                "p",
                true,
                Default::default(),
                Default::default(),
                vec![],
            )],
        ));
        theory.add_rule(Rule::new(
            "r3",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("q")],
        ));
        theory.add_rule(Rule::defeasible(
            "r4",
            vec![Literal::simple("p")],
            Literal::simple("q"),
        ));

        let conclusions = reason(&theory).unwrap();

        // p and ~p are ambiguous → -d
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            }),
            "p should NOT be +d (ambiguity blocked)"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && c.literal.negation
            }),
            "~p should NOT be +d (ambiguity blocked)"
        );

        // q should be +d via r3 (uncontested, independent of ambiguous p)
        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "q should be +d via independent rule r3 (ambiguity blocking is localized)"
        );
    }

    #[test]
    fn test_spec_superiority_defeats_attacker() {
        // Spec condition (3): an applicable attacker s is defeated if
        // ∃t ∈ Rsd[q]: t applicable AND t > s.
        //
        //   >> bird, >> penguin
        //   r1: bird => flies
        //   r2: penguin => ~flies
        //   r2 > r1
        //
        // Result: +d ~flies (r2 wins), -d flies (r1 is defeated)
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r2, &r1);

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && c.literal.negation
            }),
            "+d ~flies expected (superior rule)"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && !c.literal.negation
            }),
            "flies should NOT be +d (defeated by superior r2)"
        );
    }

    #[test]
    fn test_spec_defeater_uniform_blocking() {
        // Defeaters and defeasible attackers use the same blocking condition.
        //   >> p
        //   r1: p => q
        //   d1: p ~> ~q     (defeater)
        //
        // d1 is an applicable attacker. Need t ∈ Rsd[q] with t > d1.
        // r1 exists but no superiority → blocked.
        // Result: -d q AND -d ~q (defeater blocks but can't prove)
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeater(&["p"], "~q");

        let conclusions = reason(&theory).unwrap();

        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "q should NOT be +d (blocked by defeater)"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && c.literal.negation
            }),
            "~q should NOT be +d (defeater can't prove)"
        );
    }

    #[test]
    fn test_spec_cross_rule_superiority() {
        // Issue 5: is_blocked_by_superior must check ALL applicable Rsd[q] rules.
        // Rule t (different from the triggering rule) may be the one that defeats
        // the attacker.
        //
        //   >> a, >> b
        //   r1: a => q
        //   r2: b => ~q
        //   r3: b => q         (r3 > r2)
        //
        // r1 fires first. Attacker r2 is applicable. r1 is NOT superior to r2,
        // but r3 IS superior to r2. So the attacker is defeated, +d q.
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");

        theory.add_defeasible_rule(&["a"], "q");
        let r2 = theory.add_defeasible_rule(&["b"], "~q");
        let r3 = theory.add_defeasible_rule(&["b"], "q");
        theory.add_superiority(&r3, &r2);

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "+d q expected (r3 > r2 defeats the attacker, even though r1 triggered first)"
        );
    }

    #[test]
    fn test_spec_strict_always_blocks_defeasible() {
        // Strict attackers always block defeasible conclusions, regardless of
        // superiority. (+D ~q means condition (2) of +d q fails.)
        //
        //   >> a
        //   r1: a => q
        //   r2: a -> ~q     (strict)
        //
        // Result: +D ~q, -d q
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "q");
        theory.add_strict_rule(&["a"], "~q");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "q"
                    && c.literal.negation
            }),
            "+D ~q expected"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "q should NOT be +d (strict rule proves ~q)"
        );
    }

    #[test]
    fn test_spec_cascading_chain_uncontested() {
        // Uncontested defeasible chain should propagate.
        //   >> a
        //   r1: a => b
        //   r2: b => c
        //   r3: c => d
        //
        // Result: +d a, +d b, +d c, +d d
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "d");

        let conclusions = reason(&theory).unwrap();

        for lit in &["b", "c", "d"] {
            assert!(
                conclusions.iter().any(|c| {
                    c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == *lit
                        && !c.literal.negation
                }),
                "+d {lit} expected in uncontested chain"
            );
        }
    }

    #[test]
    fn test_two_phase_strict_chain_blocks_defeasible() {
        // Regression test for Issue 3 (two-phase reasoning).
        // A multi-hop strict chain to ~q must complete in phase 1 before
        // phase 2 checks definite_proven for the complement.
        //
        //   >> a, >> b
        //   r1: a -> c       (strict)
        //   r2: c -> ~p      (strict chain: a → c → ~p)
        //   r3: b => p       (defeasible)
        //
        // Phase 1 must produce +D ~p. Then phase 2: condition (2) of +d p
        // requires -D ~p, which is false → -d p.
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_strict_rule(&["a"], "c");
        theory.add_strict_rule(&["c"], "~p");
        theory.add_defeasible_rule(&["b"], "p");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && c.literal.negation
            }),
            "+D ~p expected from strict chain"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            }),
            "p should NOT be +d (blocked by +D ~p from strict chain)"
        );
    }

    #[test]
    fn test_bitset_preserves_existing_on_grow() {
        use crate::index::AtomId;

        let mut bitset = LiteralBitSet::new(2);

        // Insert at atom 0
        let lit_0 = LitId::new(AtomId::from_raw(0), false);
        bitset.insert(lit_0);
        assert!(bitset.contains(lit_0));

        // Grow by inserting at atom 10
        let lit_10 = LitId::new(AtomId::from_raw(10), true);
        bitset.insert(lit_10);

        // Atom 0 should still be present
        assert!(
            bitset.contains(lit_0),
            "Existing bit at atom 0 should be preserved after grow"
        );
        assert!(
            bitset.contains(lit_10),
            "New bit at atom 10 should be present after grow"
        );
    }

    // ==========================================================================
    // REASONER TRAIT TESTS
    // ==========================================================================

    #[test]
    fn test_standard_reasoner_name() {
        let r = StandardReasoner;
        assert_eq!(r.name(), "standard-dl-d");
    }

    #[test]
    fn test_select_reasoner_standard() {
        let r = select_reasoner("standard");
        assert_eq!(r.name(), "standard-dl-d");
    }

    #[test]
    fn test_select_reasoner_unknown_falls_back_to_standard() {
        let r = select_reasoner("nonexistent");
        assert_eq!(r.name(), "standard-dl-d");
    }

    #[test]
    fn test_standard_reasoner_matches_free_function() {
        // Build a non-trivial theory with facts, rules, superiority, and
        // defeaters so that any divergence between the trait path and the
        // free-function path would surface.
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r2, &r1);

        theory.add_defeasible_rule(&["bird"], "has_feathers");
        theory.add_strict_rule(&["bird"], "animal");

        // Free-function path
        let free_fn_conclusions = reason(&theory).unwrap();

        // Trait path: manually prepare + index + call trait method
        let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
        let mut indexed = IndexedTheory::build(&prepared.theory);
        let trait_conclusions = StandardReasoner.reason(&mut indexed).unwrap();

        // Compare: sort both by display representation for deterministic comparison
        let mut free_sorted: Vec<String> =
            free_fn_conclusions.iter().map(|c| format!("{c}")).collect();
        let mut trait_sorted: Vec<String> =
            trait_conclusions.iter().map(|c| format!("{c}")).collect();
        free_sorted.sort();
        trait_sorted.sort();

        assert_eq!(
            free_sorted, trait_sorted,
            "StandardReasoner trait path should produce identical conclusions to the free function"
        );
    }

    #[test]
    fn test_standard_reasoner_simple_fact() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
        let mut indexed = IndexedTheory::build(&prepared.theory);
        let conclusions = StandardReasoner.reason(&mut indexed).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "bird"),
            "StandardReasoner should prove +D bird"
        );
    }

    #[test]
    fn test_standard_reasoner_defeasible_chain() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
        let mut indexed = IndexedTheory::build(&prepared.theory);
        let conclusions = StandardReasoner.reason(&mut indexed).unwrap();

        for name in &["b", "c"] {
            assert!(
                conclusions
                    .iter()
                    .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == *name),
                "{name} should be defeasibly provable via StandardReasoner"
            );
        }
    }

    #[test]
    fn test_standard_reasoner_via_dyn_dispatch() {
        // Verify that dynamic dispatch through Box<dyn Reasoner> works.
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let reasoner: Box<dyn Reasoner> = select_reasoner("standard");

        let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
        let mut indexed = IndexedTheory::build(&prepared.theory);
        let conclusions = reasoner.reason(&mut indexed).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"),
            "q should be defeasibly provable via dyn Reasoner dispatch"
        );
    }

    /// A mock reasoner that returns a fixed set of conclusions.
    /// Demonstrates that the trait enables test doubles.
    struct MockReasoner {
        conclusions: Vec<Conclusion>,
    }

    impl Reasoner for MockReasoner {
        fn reason<'t>(&self, _theory: &mut IndexedTheory<'t>) -> Result<Vec<Conclusion>> {
            Ok(self.conclusions.clone())
        }

        fn name(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_mock_reasoner() {
        let mock = MockReasoner {
            conclusions: vec![
                Conclusion::definitely_provable(Literal::simple("bird")),
                Conclusion::defeasibly_provable(Literal::simple("flies")),
            ],
        };

        assert_eq!(mock.name(), "mock");

        let theory = Theory::new();
        let mut indexed = IndexedTheory::build(&theory);
        let conclusions = mock.reason(&mut indexed).unwrap();

        assert_eq!(conclusions.len(), 2);
        assert!(conclusions.iter().any(|c| c.literal.name() == "bird"
            && c.conclusion_type == ConclusionType::DefinitelyProvable));
        assert!(conclusions.iter().any(|c| c.literal.name() == "flies"
            && c.conclusion_type == ConclusionType::DefeasiblyProvable));
    }
}
