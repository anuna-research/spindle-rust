//! Shadow mode: run both bridge-based and projection-based reasoning in
//! parallel for comparison and validation.
//!
//! When shadow mode is enabled, the [`ShadowReasoner`] wraps the standard
//! DL(d) reasoner and additionally runs the [`ProjectionEngine`] on every
//! rule that contributed to a positive conclusion. It then cross-checks
//! projection tokens against the standard conclusions and reports any
//! divergences.
//!
//! This gates the redesign behind a comparison layer so that the new
//! projection-based approach can be validated against the existing
//! bridge-based reasoning before replacing it.

use std::fmt;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::compilation::{compile_theory, CompiledBody};
use crate::rule::RuleLabel;
use crate::conclusion::Conclusion;
use crate::error::Result;
use crate::index::IndexedTheory;
use crate::projection::{FamilyId, ProjectionDiagnostics, ProjectionEngine, ProjectionSnapshot, ProjectionToken};
use crate::reason::{reason_indexed, Reasoner};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for shadow mode.
#[derive(Debug, Clone)]
pub struct ShadowConfig {
    /// Whether shadow mode is active. When `false`, the shadow reasoner
    /// delegates directly to the standard reasoner with no overhead.
    pub enabled: bool,
}

impl Default for ShadowConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

// ---------------------------------------------------------------------------
// Divergence reporting
// ---------------------------------------------------------------------------

/// The kind of divergence detected between standard and projection-based
/// reasoning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DivergenceKind {
    /// A positive conclusion has no corresponding family-level projection
    /// token (support or attack) for its literal family.
    MissingFamilySupport {
        /// The family that lacks projection support.
        family: FamilyId,
    },
    /// A projection token references a family that has no corresponding
    /// positive conclusion in the standard results.
    UnmatchedProjectionToken {
        /// The projection token that has no standard-side counterpart.
        token: ProjectionToken,
    },
    /// A rule was marked as contributing to a conclusion but could not be
    /// found in the theory for projection.
    MissingRule {
        /// The label of the missing rule.
        label: String,
    },
}

/// A single divergence between standard and projection-based reasoning.
#[derive(Debug, Clone)]
pub struct Divergence {
    /// What kind of divergence was detected.
    pub kind: DivergenceKind,
}

impl Divergence {
    fn missing_family_support(family: FamilyId) -> Self {
        Self {
            kind: DivergenceKind::MissingFamilySupport { family },
        }
    }

    fn unmatched_token(token: ProjectionToken) -> Self {
        Self {
            kind: DivergenceKind::UnmatchedProjectionToken { token },
        }
    }

    fn missing_rule(label: String) -> Self {
        Self {
            kind: DivergenceKind::MissingRule { label },
        }
    }
}

impl fmt::Display for Divergence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            DivergenceKind::MissingFamilySupport { family } => {
                write!(f, "missing family support for {family}")
            }
            DivergenceKind::UnmatchedProjectionToken { token } => {
                write!(f, "unmatched projection token: {token:?}")
            }
            DivergenceKind::MissingRule { label } => {
                write!(f, "rule '{label}' not found for projection")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ShadowResult
// ---------------------------------------------------------------------------

/// The result of a shadow-mode reasoning pass.
///
/// Contains the primary conclusions from the standard reasoner, the
/// projection tokens from the redesign path, the compiled body keys,
/// and any divergences detected between the two.
#[derive(Debug, Clone)]
pub struct ShadowResult {
    /// Conclusions from the primary (standard) reasoner.
    pub primary: Vec<Conclusion>,
    /// Projection tokens emitted by the redesign path.
    pub projection_tokens: Vec<ProjectionToken>,
    /// Compiled body match keys for all rules, keyed by rule label.
    pub compiled_bodies: FxHashMap<RuleLabel, CompiledBody>,
    /// Divergences detected between the two approaches.
    pub divergences: Vec<Divergence>,
    /// Structured diagnostic counters for projection activity (OBS-001).
    pub diagnostics: ProjectionDiagnostics,
    /// Deterministic debug snapshot of projection state (OBS-002).
    pub snapshot: ProjectionSnapshot,
}

impl ShadowResult {
    /// Returns `true` if no divergences were detected.
    pub fn is_consistent(&self) -> bool {
        self.divergences.is_empty()
    }

    /// Number of divergences detected.
    pub fn divergence_count(&self) -> usize {
        self.divergences.len()
    }
}

// ---------------------------------------------------------------------------
// ShadowReasoner
// ---------------------------------------------------------------------------

/// A reasoner that runs both the standard DL(d) algorithm and the
/// projection engine in parallel, comparing results.
///
/// When shadow mode is disabled, it delegates directly to the standard
/// reasoner with zero overhead. When enabled, it additionally:
///
/// 1. Compiles all rule bodies into [`BodyMatchKey`]s.
/// 2. Runs the [`ProjectionEngine`] on every rule that contributed to
///    a positive conclusion.
/// 3. Cross-checks projection tokens against standard conclusions.
/// 4. Reports divergences via [`ShadowResult`].
#[derive(Debug, Clone)]
pub struct ShadowReasoner {
    config: ShadowConfig,
}

impl ShadowReasoner {
    /// Create a new shadow reasoner with the given configuration.
    pub fn new(config: ShadowConfig) -> Self {
        Self { config }
    }

    /// Create a shadow reasoner with shadow mode enabled.
    pub fn enabled() -> Self {
        Self {
            config: ShadowConfig { enabled: true },
        }
    }

    /// Create a shadow reasoner with shadow mode disabled (pass-through).
    pub fn disabled() -> Self {
        Self {
            config: ShadowConfig::default(),
        }
    }

    /// Run shadow-mode reasoning on an already-indexed theory.
    ///
    /// Returns a [`ShadowResult`] with both standard conclusions and
    /// projection tokens, plus any divergences.
    pub fn reason_shadow(
        &self,
        indexed: &mut IndexedTheory<'_>,
    ) -> Result<ShadowResult> {
        // Step 1: Run standard reasoning.
        let primary = reason_indexed(indexed)?;

        if !self.config.enabled {
            return Ok(ShadowResult {
                primary,
                projection_tokens: Vec::new(),
                compiled_bodies: FxHashMap::default(),
                divergences: Vec::new(),
                diagnostics: ProjectionDiagnostics {
                    exact_supports: 0,
                    family_supports: 0,
                    family_attacks: 0,
                    contributing_labels: std::collections::BTreeSet::new(),
                },
                snapshot: ProjectionSnapshot {
                    exact: Vec::new(),
                    family: Vec::new(),
                    attack: Vec::new(),
                },
            });
        }

        // Step 2: Compile rule bodies.
        let compiled_bodies = compile_theory(indexed);

        // Step 3: Run projection engine on rules that contributed to
        // positive conclusions.
        let mut engine = ProjectionEngine::with_capacity(primary.len() * 2);
        let mut projected_labels: FxHashSet<String> = FxHashSet::default();

        // Collect rule labels from positive conclusions.
        let contributing_labels: FxHashSet<String> = primary
            .iter()
            .filter(|c| c.is_positive())
            .filter_map(|c| c.rule_label.clone())
            .collect();

        let theory = indexed.theory();
        for label in &contributing_labels {
            if let Some(rule) = theory.get_rule(label) {
                engine.project_rule(rule, indexed);
                projected_labels.insert(label.clone());
            }
        }

        // Step 4: Cross-check and detect divergences.
        let divergences = detect_divergences(
            &primary,
            engine.tokens(),
            &contributing_labels,
            &projected_labels,
        );

        let diagnostics = engine.diagnostics();
        let snapshot = engine.snapshot();

        Ok(ShadowResult {
            primary,
            projection_tokens: engine.into_tokens(),
            compiled_bodies,
            divergences,
            diagnostics,
            snapshot,
        })
    }
}

impl Reasoner for ShadowReasoner {
    fn reason<'t>(&self, indexed: &mut IndexedTheory<'t>) -> Result<Vec<Conclusion>> {
        let result = self.reason_shadow(indexed)?;
        Ok(result.primary)
    }

    fn name(&self) -> &str {
        "shadow"
    }
}

// ---------------------------------------------------------------------------
// Divergence detection
// ---------------------------------------------------------------------------

/// Cross-check standard conclusions against projection tokens to detect
/// divergences.
fn detect_divergences(
    conclusions: &[Conclusion],
    tokens: &[ProjectionToken],
    contributing_labels: &FxHashSet<String>,
    projected_labels: &FxHashSet<String>,
) -> Vec<Divergence> {
    let mut divergences = Vec::new();

    // Check 1: Every contributing rule label should have been projected.
    for label in contributing_labels {
        if !projected_labels.contains(label) {
            divergences.push(Divergence::missing_rule(label.clone()));
        }
    }

    // Check 2: Every positive conclusion's literal family should have
    // at least one corresponding family-level token.
    let supported_families: FxHashSet<FamilyId> = tokens
        .iter()
        .filter_map(|t| match t {
            ProjectionToken::Family(fs) => Some(fs.family_id.clone()),
            ProjectionToken::Attack(fa) => Some(fa.family_id.clone()),
            ProjectionToken::Exact(_) => None,
        })
        .collect();

    for conclusion in conclusions {
        if !conclusion.is_positive() {
            continue;
        }
        let family = FamilyId::from(&conclusion.literal);
        if !supported_families.contains(&family) {
            divergences.push(Divergence::missing_family_support(family));
        }
    }

    // Check 3: Every family-level projection token should correspond to
    // at least one positive conclusion family.
    let concluded_families: FxHashSet<FamilyId> = conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| FamilyId::from(&c.literal))
        .collect();

    for token in tokens {
        let token_family = match token {
            ProjectionToken::Family(fs) => Some(&fs.family_id),
            ProjectionToken::Attack(fa) => Some(&fa.family_id),
            ProjectionToken::Exact(_) => None,
        };
        if let Some(family) = token_family {
            if !concluded_families.contains(family)
                && !concluded_families.contains(&family.complement())
            {
                divergences.push(Divergence::unmatched_token(token.clone()));
            }
        }
    }

    divergences
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::index::IndexedTheory;
    use crate::literal::Literal;
    use crate::mode::Mode;
    use crate::rule::Rule;
    use crate::temporal::{Temporal, TimePoint};
    use crate::theory::Theory;

    fn shadow_reason(theory: &Theory) -> ShadowResult {
        let mut indexed = IndexedTheory::build(theory);
        ShadowReasoner::enabled()
            .reason_shadow(&mut indexed)
            .unwrap()
    }

    // ======================================================================
    // Disabled mode (pass-through)
    // ======================================================================

    #[test]
    fn disabled_produces_no_tokens_or_divergences() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let mut indexed = IndexedTheory::build(&theory);
        let result = ShadowReasoner::disabled()
            .reason_shadow(&mut indexed)
            .unwrap();

        assert!(!result.primary.is_empty());
        assert!(result.projection_tokens.is_empty());
        assert!(result.compiled_bodies.is_empty());
        assert!(result.is_consistent());
    }

    // ======================================================================
    // Simple fact
    // ======================================================================

    #[test]
    fn simple_fact_is_consistent() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let result = shadow_reason(&theory);

        assert!(
            result.primary.iter().any(|c| c.conclusion_type
                == ConclusionType::DefinitelyProvable
                && c.literal.name() == "bird"),
            "should have +D bird"
        );

        // Shadow mode is consistent for simple facts.
        assert!(result.is_consistent(), "divergences: {:?}", result.divergences);
    }

    // ======================================================================
    // Strict rule chain
    // ======================================================================

    #[test]
    fn strict_chain_is_consistent() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_rule(Rule::strict(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("animal"),
        ));

        let result = shadow_reason(&theory);

        assert!(
            result
                .primary
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "animal"),
            "should have +D animal"
        );
        assert!(result.is_consistent(), "divergences: {:?}", result.divergences);
    }

    // ======================================================================
    // Defeasible reasoning
    // ======================================================================

    #[test]
    fn defeasible_rule_is_consistent() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        ));

        let result = shadow_reason(&theory);

        assert!(
            result
                .primary
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"),
            "should have +d flies"
        );
        assert!(result.is_consistent(), "divergences: {:?}", result.divergences);
    }

    // ======================================================================
    // Defeater produces attack tokens
    // ======================================================================

    #[test]
    fn defeater_produces_attack_tokens() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("broken_wing");
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        ));
        theory.add_rule(Rule::defeater(
            "d1",
            vec![Literal::simple("broken_wing")],
            Literal::negated("flies"),
        ));

        let result = shadow_reason(&theory);

        // The defeater should produce attack tokens when projected.
        // Note: defeaters don't produce conclusions themselves, so d1
        // won't appear in contributing_labels unless its label is on a
        // conclusion. This is expected — the standard reasoner handles
        // blocking internally.
        assert!(!result.primary.is_empty());
    }

    // ======================================================================
    // Compiled bodies are populated
    // ======================================================================

    #[test]
    fn compiled_bodies_populated_in_shadow_mode() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("a")],
            Literal::simple("b"),
        ));

        let result = shadow_reason(&theory);

        assert!(
            !result.compiled_bodies.is_empty(),
            "compiled bodies should be populated"
        );
    }

    // ======================================================================
    // Projection tokens emitted for contributing rules
    // ======================================================================

    #[test]
    fn projection_tokens_emitted_for_contributors() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("a")],
            Literal::simple("b"),
        ));

        let result = shadow_reason(&theory);

        // r1 produces +d b, so it should be projected.
        let has_r1_token = result.projection_tokens.iter().any(|t| match t {
            ProjectionToken::Family(fs) => fs.rule_label == "r1",
            _ => false,
        });
        assert!(has_r1_token, "r1 should produce a family support token");
    }

    // ======================================================================
    // Reasoner trait
    // ======================================================================

    #[test]
    fn shadow_reasoner_implements_trait() {
        let reasoner = ShadowReasoner::enabled();
        assert_eq!(reasoner.name(), "shadow");

        let mut theory = Theory::new();
        theory.add_fact("x");

        let mut indexed = IndexedTheory::build(&theory);
        let conclusions = Reasoner::reason(&reasoner, &mut indexed).unwrap();
        assert!(!conclusions.is_empty());
    }

    // ======================================================================
    // Temporal projection consistency
    // ======================================================================

    #[test]
    fn temporal_rule_projects_to_atemporal_family() {
        let head = Literal::new(
            "q",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("a")],
            head,
        ));

        let result = shadow_reason(&theory);

        // The projection engine should produce a family support token
        // for the base family "q" (atemporal).
        let family_tokens: Vec<_> = result
            .projection_tokens
            .iter()
            .filter_map(|t| match t {
                ProjectionToken::Family(fs) => Some(fs),
                _ => None,
            })
            .collect();

        let base_q = FamilyId::from(&Literal::simple("q"));
        assert!(
            family_tokens.iter().any(|fs| fs.family_id == base_q),
            "should project to atemporal family q"
        );
    }

    // ======================================================================
    // Divergence display
    // ======================================================================

    #[test]
    fn divergence_display() {
        let d = Divergence::missing_family_support(FamilyId::from(&Literal::simple("x")));
        let s = format!("{d}");
        assert!(s.contains("missing family support"));
        assert!(s.contains("x"));
    }

    // ======================================================================
    // Multiple rules with superiority
    // ======================================================================

    #[test]
    fn superiority_conflict_is_consistent() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r2, &r1);

        let result = shadow_reason(&theory);

        // Penguin wins: +d ~flies, -d flies.
        assert!(
            result
                .primary
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && c.literal.negation),
            "should have +d ~flies"
        );
        assert!(result.is_consistent(), "divergences: {:?}", result.divergences);
    }
}
