//! First-class projection tokens for temporal family reasoning.
//!
//! Rather than the old approach of synthesizing bridge rules to connect
//! temporal literals to their atemporal families, the engine emits
//! projection tokens that carry the original rule label and type. This
//! preserves superiority and trust semantics by construction (SPEC-020,
//! §4.2).

use crate::index::LitId;
use crate::literal::{InternedLiteralName, Literal};
use crate::mode::Mode;
use crate::rule::{RuleLabel, RuleType};
use crate::term::Term;

/// Exact literal identity — a newtype over [`LitId`] that explicitly includes
/// temporal bounds in its semantics.
///
/// Two literals that differ only in temporal window produce different
/// `ExactLitId` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ExactLitId(LitId);

impl ExactLitId {
    /// Wrap a [`LitId`] as an exact literal identity.
    pub fn new(lit: LitId) -> Self {
        Self(lit)
    }

    /// Return the underlying [`LitId`].
    pub fn lit_id(self) -> LitId {
        self.0
    }
}

/// Base family identity — groups all temporal variants of a literal under a
/// single atemporal identity.
///
/// A `FamilyId` captures the functor name, predicate arguments, modal operator,
/// and polarity (negation) of a literal while **excluding** temporal bounds.
/// This means `p[1,10]` and `p[20,30]` share the same `FamilyId`, while
/// `~p[1,10]` belongs to a separate negative family.
///
/// # Examples
///
/// ```rust
/// use spindle_core::prelude::*;
/// use spindle_core::projection::FamilyId;
///
/// let lit = Literal::simple("bird");
/// let family = FamilyId::from(&lit);
/// assert_eq!(family.functor().resolve(), "bird");
/// assert!(!family.negated());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FamilyId {
    /// The interned functor name.
    functor: InternedLiteralName,
    /// Predicate arguments (e.g., `parent(alice, bob)` → `[alice, bob]`).
    args: Vec<Term>,
    /// Modal operator, if any.
    mode: Mode,
    /// Whether the literal is negated (`~p` vs `p`).
    negated: bool,
}

impl FamilyId {
    /// Create a `FamilyId` from its constituent parts.
    pub fn new(functor: InternedLiteralName, args: Vec<Term>, mode: Mode, negated: bool) -> Self {
        Self {
            functor,
            args,
            mode,
            negated,
        }
    }

    /// The interned functor name.
    #[inline]
    pub fn functor(&self) -> InternedLiteralName {
        self.functor
    }

    /// The predicate arguments.
    #[inline]
    pub fn args(&self) -> &[Term] {
        &self.args
    }

    /// The modal operator.
    #[inline]
    pub fn mode(&self) -> &Mode {
        &self.mode
    }

    /// Whether this family is negated.
    #[inline]
    pub fn negated(&self) -> bool {
        self.negated
    }

    /// Return the complementary family (negation flipped, everything else the same).
    pub fn complement(&self) -> Self {
        Self {
            functor: self.functor,
            args: self.args.clone(),
            mode: self.mode.clone(),
            negated: !self.negated,
        }
    }
}

impl From<&Literal> for FamilyId {
    /// Extract a `FamilyId` from a literal, dropping temporal information.
    fn from(lit: &Literal) -> Self {
        Self {
            functor: lit.interned_name(),
            args: lit.predicate_args().to_vec(),
            mode: lit.mode.clone(),
            negated: lit.negation,
        }
    }
}

impl std::fmt::Display for FamilyId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if !self.mode.is_empty() {
            write!(f, "{}", self.mode)?;
        }
        if self.negated {
            write!(f, "~")?;
        }
        write!(f, "{}", self.functor.resolve())?;
        if !self.args.is_empty() {
            let args: Vec<_> = self.args.iter().map(|t| t.to_string()).collect();
            write!(f, "({})", args.join(", "))?;
        }
        Ok(())
    }
}

/// A token recording that a rule produced exact support for a specific
/// temporal literal.
///
/// Emitted when a rule fires and its head matches an exact temporal literal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExactSupport {
    /// The exact literal that received support.
    pub exact_lit: ExactLitId,
    /// Label of the original rule that produced this support.
    pub rule_label: RuleLabel,
    /// Type/strength of the original rule.
    pub rule_type: RuleType,
}

/// A token recording that a rule produced family-level support for a base
/// literal via temporal projection.
///
/// Emitted when projection policy allows a temporal proof to provide
/// evidence for the atemporal family.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FamilySupport {
    /// The base family that received projected support.
    pub family_id: FamilyId,
    /// The exact temporal literal from which support was projected.
    pub source_exact_lit: ExactLitId,
    /// Label of the original rule that produced this support.
    pub rule_label: RuleLabel,
    /// Type/strength of the original rule.
    pub rule_type: RuleType,
}

/// A token recording that a temporal defeater attacks a base family using
/// its original body and label.
///
/// The attack carries the original rule's identity so that superiority
/// resolution uses authored labels directly (SPEC-020, ADR-004).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FamilyAttack {
    /// The base family being attacked.
    pub family_id: FamilyId,
    /// The exact temporal literal from which the attack originates, if any.
    pub source_exact_lit: Option<ExactLitId>,
    /// Label of the original defeater rule.
    pub rule_label: RuleLabel,
    /// Type/strength of the original defeater rule.
    pub rule_type: RuleType,
}

/// A projection token emitted during reasoning.
///
/// Unifies the three token kinds so they can be collected and inspected
/// uniformly (e.g. for observability counters, OBS-001).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProjectionToken {
    /// Exact support for a specific temporal literal.
    Exact(ExactSupport),
    /// Projected family-level support.
    Family(FamilySupport),
    /// Projected family-level attack.
    Attack(FamilyAttack),
}

// ---------------------------------------------------------------------------
// ProjectionEngine — CON-002
// ---------------------------------------------------------------------------

use crate::index::IndexedTheory;
use crate::rule::Rule;

/// The projection engine emits support/attack tokens when a rule fires.
///
/// Produces first-class [`ProjectionToken`]s that carry the original
/// rule label and type, preserving superiority and trust semantics by
/// construction (SPEC-020, CON-002).
///
/// # Usage
///
/// ```rust,ignore
/// let mut engine = ProjectionEngine::new();
/// engine.project_rule(&rule, &mut index);
/// let tokens = engine.tokens();
/// ```
#[derive(Debug, Clone)]
pub struct ProjectionEngine {
    tokens: Vec<ProjectionToken>,
}

impl ProjectionEngine {
    /// Create a new, empty projection engine.
    pub fn new() -> Self {
        Self { tokens: Vec::new() }
    }

    /// Create a projection engine with pre-allocated capacity.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            tokens: Vec::with_capacity(cap),
        }
    }

    /// Project a fired rule into support/attack tokens.
    ///
    /// For each head literal in the rule:
    ///
    /// - **Any rule type, any head**: emits [`ExactSupport`] for the exact
    ///   literal identity of the head.
    /// - **Defeater head**: additionally emits [`FamilyAttack`] against the
    ///   head's base family, using the original rule label and type.
    /// - **Non-defeater head**: additionally emits [`FamilySupport`] for
    ///   the head's base family, projecting temporal evidence to the
    ///   atemporal family.
    ///
    /// # Pre-conditions (CON-002)
    ///
    /// - Rule applicability has been evaluated against body match keys.
    /// - Original rule label and type are available.
    ///
    /// # Post-conditions (CON-002)
    ///
    /// - Emits support/attack tokens using original labels.
    /// - Does not materialize synthetic rules in `Theory`.
    pub fn project_rule(&mut self, rule: &Rule, index: &mut IndexedTheory<'_>) {
        let label = rule.template_label.as_ref().unwrap_or(&rule.label).clone();
        let rule_type = rule.rule_type;

        for head_lit in &rule.head {
            let exact = index.exact_lit_id(head_lit);

            // Always emit exact support.
            self.tokens.push(ProjectionToken::Exact(ExactSupport {
                exact_lit: exact,
                rule_label: label.clone(),
                rule_type,
            }));

            let family = FamilyId::from(head_lit);

            if rule_type.is_defeater() {
                // Defeaters attack the base family.
                self.tokens.push(ProjectionToken::Attack(FamilyAttack {
                    family_id: family,
                    source_exact_lit: Some(exact),
                    rule_label: label.clone(),
                    rule_type,
                }));
            } else {
                // Non-defeaters project family-level support.
                self.tokens.push(ProjectionToken::Family(FamilySupport {
                    family_id: family,
                    source_exact_lit: exact,
                    rule_label: label.clone(),
                    rule_type,
                }));
            }
        }
    }

    /// Return all tokens emitted so far.
    pub fn tokens(&self) -> &[ProjectionToken] {
        &self.tokens
    }

    /// Consume the engine and return all emitted tokens.
    pub fn into_tokens(self) -> Vec<ProjectionToken> {
        self.tokens
    }

    /// Number of tokens emitted so far.
    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    /// Whether any tokens have been emitted.
    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Clear all emitted tokens.
    pub fn clear(&mut self) {
        self.tokens.clear();
    }

    /// Count of exact support tokens.
    pub fn exact_count(&self) -> usize {
        self.tokens
            .iter()
            .filter(|t| matches!(t, ProjectionToken::Exact(_)))
            .count()
    }

    /// Count of family support tokens.
    pub fn family_count(&self) -> usize {
        self.tokens
            .iter()
            .filter(|t| matches!(t, ProjectionToken::Family(_)))
            .count()
    }

    /// Count of family attack tokens.
    pub fn attack_count(&self) -> usize {
        self.tokens
            .iter()
            .filter(|t| matches!(t, ProjectionToken::Attack(_)))
            .count()
    }

    /// Produce structured diagnostic counters (OBS-001).
    ///
    /// Returns a [`ProjectionDiagnostics`] snapshot summarizing the current
    /// projection state: token counts by type and the set of rule labels
    /// that contributed projected evidence.
    pub fn diagnostics(&self) -> ProjectionDiagnostics {
        let mut contributing_labels = std::collections::BTreeSet::new();
        let mut exact_supports = 0usize;
        let mut family_supports = 0usize;
        let mut family_attacks = 0usize;

        for token in &self.tokens {
            match token {
                ProjectionToken::Exact(s) => {
                    exact_supports += 1;
                    contributing_labels.insert(s.rule_label.clone());
                }
                ProjectionToken::Family(s) => {
                    family_supports += 1;
                    contributing_labels.insert(s.rule_label.clone());
                }
                ProjectionToken::Attack(a) => {
                    family_attacks += 1;
                    contributing_labels.insert(a.rule_label.clone());
                }
            }
        }

        ProjectionDiagnostics {
            exact_supports,
            family_supports,
            family_attacks,
            contributing_labels,
        }
    }

    /// Produce a deterministic debug snapshot for test-mode comparison
    /// (OBS-002).
    ///
    /// The snapshot contains tokens sorted by a stable key so that
    /// non-deterministic iteration order does not cause spurious test
    /// failures. Labels are sorted alphabetically within each category.
    pub fn snapshot(&self) -> ProjectionSnapshot {
        let mut exact: Vec<(String, String)> = Vec::new();
        let mut family: Vec<(String, String, String)> = Vec::new();
        let mut attack: Vec<(String, String, String)> = Vec::new();

        for token in &self.tokens {
            match token {
                ProjectionToken::Exact(s) => {
                    exact.push((s.rule_label.clone(), format!("{:?}", s.exact_lit)));
                }
                ProjectionToken::Family(s) => {
                    family.push((
                        s.rule_label.clone(),
                        format!("{}", s.family_id),
                        format!("{:?}", s.source_exact_lit),
                    ));
                }
                ProjectionToken::Attack(a) => {
                    attack.push((
                        a.rule_label.clone(),
                        format!("{}", a.family_id),
                        format!("{:?}", a.source_exact_lit),
                    ));
                }
            }
        }

        // Sort for deterministic ordering.
        exact.sort();
        family.sort();
        attack.sort();

        ProjectionSnapshot {
            exact,
            family,
            attack,
        }
    }
}

impl Default for ProjectionEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ProjectionDiagnostics — OBS-001
// ---------------------------------------------------------------------------

/// Structured diagnostic counters for projection activity (OBS-001).
///
/// Summarizes the number of exact supports, family supports, family attacks,
/// and the set of rule labels that contributed projected evidence. This
/// validates that projection activity is explainable and bounded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionDiagnostics {
    /// Number of [`ExactSupport`] tokens emitted.
    pub exact_supports: usize,
    /// Number of [`FamilySupport`] tokens emitted.
    pub family_supports: usize,
    /// Number of [`FamilyAttack`] tokens emitted.
    pub family_attacks: usize,
    /// Rule labels that contributed projected evidence (sorted for stability).
    pub contributing_labels: std::collections::BTreeSet<RuleLabel>,
}

impl ProjectionDiagnostics {
    /// Total number of projection tokens emitted.
    pub fn total_tokens(&self) -> usize {
        self.exact_supports + self.family_supports + self.family_attacks
    }
}

impl std::fmt::Display for ProjectionDiagnostics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "exact_supports={}, family_supports={}, family_attacks={}, labels=[{}]",
            self.exact_supports,
            self.family_supports,
            self.family_attacks,
            self.contributing_labels
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
        )
    }
}

// ---------------------------------------------------------------------------
// ProjectionSnapshot — OBS-002
// ---------------------------------------------------------------------------

/// A deterministic debug snapshot of projection state (OBS-002).
///
/// All entries are sorted by a stable key so that non-deterministic iteration
/// order does not cause test regressions. Useful for golden-file or snapshot
/// testing of projected evidence ordering and tie-break label selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionSnapshot {
    /// Sorted `(rule_label, exact_lit_debug)` pairs for exact support tokens.
    pub exact: Vec<(String, String)>,
    /// Sorted `(rule_label, family_display, source_exact_debug)` triples for
    /// family support tokens.
    pub family: Vec<(String, String, String)>,
    /// Sorted `(rule_label, family_display, source_exact_debug)` triples for
    /// family attack tokens.
    pub attack: Vec<(String, String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::Mode;
    use crate::temporal::{Temporal, TimePoint};

    #[test]
    fn temporal_variants_share_family() {
        let lit_a = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let lit_b = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30)),
            vec![],
        );

        assert_eq!(FamilyId::from(&lit_a), FamilyId::from(&lit_b));
    }

    #[test]
    fn negation_separates_families() {
        let pos = Literal::simple("p");
        let neg = Literal::negated("p");

        assert_ne!(FamilyId::from(&pos), FamilyId::from(&neg));
    }

    #[test]
    fn different_args_separate_families() {
        let lit_a = Literal::new(
            "parent",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["alice".into()],
        );
        let lit_b = Literal::new(
            "parent",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["bob".into()],
        );

        assert_ne!(FamilyId::from(&lit_a), FamilyId::from(&lit_b));
    }

    #[test]
    fn mode_separates_families() {
        let plain = Literal::new("pay", false, Mode::empty(), Temporal::empty(), vec![]);
        let obligatory = Literal::new("pay", false, Mode::obligation(), Temporal::empty(), vec![]);

        assert_ne!(FamilyId::from(&plain), FamilyId::from(&obligatory));
    }

    #[test]
    fn complement_flips_negation() {
        let family = FamilyId::from(&Literal::simple("p"));
        let comp = family.complement();

        assert!(comp.negated());
        assert_eq!(comp.functor(), family.functor());
        assert_eq!(comp.complement(), family);
    }

    #[test]
    fn literal_family_id_method() {
        let lit = Literal::simple("bird");
        assert_eq!(lit.family_id(), FamilyId::from(&lit));
    }

    #[test]
    fn display_simple() {
        let family = FamilyId::from(&Literal::simple("bird"));
        assert_eq!(format!("{family}"), "bird");
    }

    #[test]
    fn display_negated_with_args() {
        let lit = Literal::new(
            "parent",
            true,
            Mode::empty(),
            Temporal::empty(),
            vec!["alice".into(), "bob".into()],
        );
        let family = FamilyId::from(&lit);
        assert_eq!(format!("{family}"), "~parent(alice, bob)");
    }

    #[test]
    fn display_modal() {
        let lit = Literal::new("pay", false, Mode::obligation(), Temporal::empty(), vec![]);
        let family = FamilyId::from(&lit);
        assert_eq!(format!("{family}"), "[O]pay");
    }

    #[test]
    fn usable_as_hashmap_key() {
        use std::collections::HashMap;

        let lit_a = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let lit_b = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30)),
            vec![],
        );

        let mut map: HashMap<FamilyId, u32> = HashMap::new();
        map.insert(FamilyId::from(&lit_a), 1);

        // lit_b should map to the same family
        assert_eq!(map.get(&FamilyId::from(&lit_b)), Some(&1));
    }

    // -- ProjectionEngine tests --

    use crate::index::IndexedTheory;
    use crate::rule::Rule;
    use crate::theory::Theory;

    fn make_index(theory: &Theory) -> IndexedTheory<'_> {
        IndexedTheory::build(theory)
    }

    #[test]
    fn fact_emits_exact_and_family_support() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("bird")));

        let mut idx = make_index(&theory);
        let rule = theory.get_rule("f1").unwrap();

        let mut engine = ProjectionEngine::new();
        engine.project_rule(rule, &mut idx);

        assert_eq!(engine.len(), 2);
        assert_eq!(engine.exact_count(), 1);
        assert_eq!(engine.family_count(), 1);
        assert_eq!(engine.attack_count(), 0);

        // Exact support preserves rule label and type.
        let exact = match &engine.tokens()[0] {
            ProjectionToken::Exact(s) => s,
            other => panic!("expected Exact, got {other:?}"),
        };
        assert_eq!(exact.rule_label, "f1");
        assert_eq!(exact.rule_type, crate::rule::RuleType::Fact);

        // Family support projects to the atemporal family.
        let family = match &engine.tokens()[1] {
            ProjectionToken::Family(s) => s,
            other => panic!("expected Family, got {other:?}"),
        };
        assert_eq!(family.rule_label, "f1");
        assert_eq!(family.family_id, FamilyId::from(&Literal::simple("bird")));
    }

    #[test]
    fn defeasible_rule_emits_exact_and_family_support() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        ));

        let mut idx = make_index(&theory);
        let rule = theory.get_rule("r1").unwrap();

        let mut engine = ProjectionEngine::new();
        engine.project_rule(rule, &mut idx);

        assert_eq!(engine.exact_count(), 1);
        assert_eq!(engine.family_count(), 1);
        assert_eq!(engine.attack_count(), 0);
    }

    #[test]
    fn defeater_emits_exact_support_and_family_attack() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::defeater(
            "d1",
            vec![Literal::simple("broken")],
            Literal::negated("works"),
        ));

        let mut idx = make_index(&theory);
        let rule = theory.get_rule("d1").unwrap();

        let mut engine = ProjectionEngine::new();
        engine.project_rule(rule, &mut idx);

        assert_eq!(engine.exact_count(), 1);
        assert_eq!(engine.family_count(), 0);
        assert_eq!(engine.attack_count(), 1);

        let attack = match &engine.tokens()[1] {
            ProjectionToken::Attack(a) => a,
            other => panic!("expected Attack, got {other:?}"),
        };
        assert_eq!(attack.rule_label, "d1");
        assert_eq!(attack.family_id, FamilyId::from(&Literal::negated("works")));
        assert!(attack.source_exact_lit.is_some());
    }

    #[test]
    fn temporal_defeater_attacks_base_family() {
        // b ~> ~q[1,10] should attack base family ~q
        let head = Literal::new(
            "q",
            true,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let mut theory = Theory::new();
        theory.add_rule(Rule::defeater(
            "d_temp",
            vec![Literal::simple("b")],
            head.clone(),
        ));

        let mut idx = make_index(&theory);
        let rule = theory.get_rule("d_temp").unwrap();

        let mut engine = ProjectionEngine::new();
        engine.project_rule(rule, &mut idx);

        let attack = match &engine.tokens()[1] {
            ProjectionToken::Attack(a) => a,
            other => panic!("expected Attack, got {other:?}"),
        };

        // Attack targets the base family ~q (no temporal bounds).
        let base_family = FamilyId::from(&Literal::negated("q"));
        assert_eq!(attack.family_id, base_family);
        assert_eq!(attack.rule_label, "d_temp");
    }

    #[test]
    fn template_label_is_preferred() {
        // Grounded instances carry template_label; projection should use it.
        let mut rule = Rule::defeasible(
            "r1__ground_0",
            vec![Literal::simple("a")],
            Literal::simple("b"),
        );
        rule.template_label = Some("r1".to_string());

        let mut theory = Theory::new();
        theory.add_rule(rule);

        let mut idx = make_index(&theory);
        let rule = theory.get_rule("r1__ground_0").unwrap();

        let mut engine = ProjectionEngine::new();
        engine.project_rule(rule, &mut idx);

        for token in engine.tokens() {
            match token {
                ProjectionToken::Exact(s) => assert_eq!(s.rule_label, "r1"),
                ProjectionToken::Family(s) => assert_eq!(s.rule_label, "r1"),
                ProjectionToken::Attack(a) => assert_eq!(a.rule_label, "r1"),
            }
        }
    }

    #[test]
    fn distinct_defeaters_remain_distinct() {
        // Two defeaters with same shape but different labels → separate attacks.
        let mut theory = Theory::new();
        theory.add_rule(Rule::defeater(
            "d1",
            vec![Literal::simple("b")],
            Literal::negated("q"),
        ));
        theory.add_rule(Rule::defeater(
            "d2",
            vec![Literal::simple("b")],
            Literal::negated("q"),
        ));

        let mut idx = make_index(&theory);
        let mut engine = ProjectionEngine::new();

        let r1 = theory.get_rule("d1").unwrap();
        let r2 = theory.get_rule("d2").unwrap();
        engine.project_rule(r1, &mut idx);
        engine.project_rule(r2, &mut idx);

        assert_eq!(engine.attack_count(), 2);

        let attacks: Vec<_> = engine
            .tokens()
            .iter()
            .filter_map(|t| match t {
                ProjectionToken::Attack(a) => Some(a),
                _ => None,
            })
            .collect();

        assert_eq!(attacks[0].rule_label, "d1");
        assert_eq!(attacks[1].rule_label, "d2");
    }

    #[test]
    fn clear_resets_engine() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("a")));

        let mut idx = make_index(&theory);
        let mut engine = ProjectionEngine::new();
        engine.project_rule(theory.get_rule("f1").unwrap(), &mut idx);

        assert!(!engine.is_empty());
        engine.clear();
        assert!(engine.is_empty());
        assert_eq!(engine.len(), 0);
    }

    #[test]
    fn into_tokens_consumes_engine() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("a")));

        let mut idx = make_index(&theory);
        let mut engine = ProjectionEngine::new();
        engine.project_rule(theory.get_rule("f1").unwrap(), &mut idx);

        let tokens = engine.into_tokens();
        assert_eq!(tokens.len(), 2);
    }

    #[test]
    fn strict_rule_emits_support_not_attack() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::strict(
            "s1",
            vec![Literal::simple("a")],
            Literal::simple("b"),
        ));

        let mut idx = make_index(&theory);
        let mut engine = ProjectionEngine::new();
        engine.project_rule(theory.get_rule("s1").unwrap(), &mut idx);

        assert_eq!(engine.exact_count(), 1);
        assert_eq!(engine.family_count(), 1);
        assert_eq!(engine.attack_count(), 0);
    }

    // -- OBS-001 diagnostics tests --

    #[test]
    fn diagnostics_counts_tokens_correctly() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("bird")));
        theory.add_rule(Rule::defeater(
            "d1",
            vec![Literal::simple("broken")],
            Literal::negated("flies"),
        ));

        let mut idx = make_index(&theory);
        let mut engine = ProjectionEngine::new();
        engine.project_rule(theory.get_rule("f1").unwrap(), &mut idx);
        engine.project_rule(theory.get_rule("d1").unwrap(), &mut idx);

        let diag = engine.diagnostics();
        assert_eq!(diag.exact_supports, 2);
        assert_eq!(diag.family_supports, 1);
        assert_eq!(diag.family_attacks, 1);
        assert_eq!(diag.total_tokens(), 4);
        assert!(diag.contributing_labels.contains("f1"));
        assert!(diag.contributing_labels.contains("d1"));
        assert_eq!(diag.contributing_labels.len(), 2);
    }

    #[test]
    fn diagnostics_display_format() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("a")));

        let mut idx = make_index(&theory);
        let mut engine = ProjectionEngine::new();
        engine.project_rule(theory.get_rule("f1").unwrap(), &mut idx);

        let diag = engine.diagnostics();
        let s = format!("{diag}");
        assert!(s.contains("exact_supports=1"));
        assert!(s.contains("family_supports=1"));
        assert!(s.contains("family_attacks=0"));
        assert!(s.contains("f1"));
    }

    #[test]
    fn empty_engine_diagnostics() {
        let engine = ProjectionEngine::new();
        let diag = engine.diagnostics();
        assert_eq!(diag.exact_supports, 0);
        assert_eq!(diag.family_supports, 0);
        assert_eq!(diag.family_attacks, 0);
        assert_eq!(diag.total_tokens(), 0);
        assert!(diag.contributing_labels.is_empty());
    }

    // -- OBS-002 snapshot tests --

    #[test]
    fn snapshot_is_deterministic() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f2", Literal::simple("b")));
        theory.add_rule(Rule::fact("f1", Literal::simple("a")));

        let mut idx = make_index(&theory);
        let mut engine = ProjectionEngine::new();
        // Project in reverse label order to verify sorting.
        engine.project_rule(theory.get_rule("f2").unwrap(), &mut idx);
        engine.project_rule(theory.get_rule("f1").unwrap(), &mut idx);

        let snap = engine.snapshot();
        // Exact entries should be sorted by label.
        assert_eq!(snap.exact[0].0, "f1");
        assert_eq!(snap.exact[1].0, "f2");
        // Family entries should be sorted by label.
        assert_eq!(snap.family[0].0, "f1");
        assert_eq!(snap.family[1].0, "f2");

        // Two calls produce identical snapshots.
        assert_eq!(snap, engine.snapshot());
    }

    #[test]
    fn snapshot_empty_engine() {
        let engine = ProjectionEngine::new();
        let snap = engine.snapshot();
        assert!(snap.exact.is_empty());
        assert!(snap.family.is_empty());
        assert!(snap.attack.is_empty());
    }

    #[test]
    fn temporal_head_projects_to_atemporal_family() {
        // A defeasible rule with temporal head q[1,10] should project
        // family support to base family q.
        let head = Literal::new(
            "q",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let mut theory = Theory::new();
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("a")],
            head.clone(),
        ));

        let mut idx = make_index(&theory);
        let mut engine = ProjectionEngine::new();
        engine.project_rule(theory.get_rule("r1").unwrap(), &mut idx);

        let family_tok = match &engine.tokens()[1] {
            ProjectionToken::Family(f) => f,
            other => panic!("expected Family, got {other:?}"),
        };

        // Family should be the atemporal base q.
        let base = FamilyId::from(&Literal::simple("q"));
        assert_eq!(family_tok.family_id, base);
    }
}
