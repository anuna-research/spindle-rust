//! First-class projection tokens for temporal family reasoning.
//!
//! Rather than the old approach of synthesizing bridge rules to connect
//! temporal literals to their atemporal families, the engine emits
//! projection tokens that carry the original rule label and type. This
//! preserves superiority and trust semantics by construction (SPEC-020,
//! §4.2).

use crate::intern::resolve;
use crate::literal::{InternedLiteralName, Literal};
use crate::mode::Mode;
use crate::rule::{RuleLabel, RuleType};
use crate::temporal::TimePoint;
use crate::term::Term;
use crate::{error::Result, index::IndexedTheory, rule::Rule};
use smallvec::SmallVec;

/// Exact literal identity for the projection index.
///
/// Two literals that differ only in temporal window produce different
/// `ExactLitId` values.
///
/// This is intentionally **not** a [`LitId`](crate::index::LitId): exact-literal
/// IDs live in a separate projection-local ID space and must not be mixed with
/// the main theory index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct ExactLitId(u32);

impl ExactLitId {
    pub(crate) const NEGATION_BIT: u32 = 1 << 31;
    const ATOM_MASK: u32 = !Self::NEGATION_BIT;

    /// Create an exact literal ID from an exact-atom slot and negation flag.
    ///
    /// # Panics (debug only)
    /// Panics if `atom_index` exceeds the 31-bit address space (`2^31 - 1`),
    /// which would silently corrupt the negation bit.
    pub fn new(atom_index: u32, negated: bool) -> Self {
        debug_assert!(
            atom_index <= Self::ATOM_MASK,
            "ExactLitId::new: atom_index {atom_index} exceeds 31-bit capacity"
        );
        if negated {
            Self(atom_index | Self::NEGATION_BIT)
        } else {
            Self(atom_index & Self::ATOM_MASK)
        }
    }

    /// Return the projection-local exact-atom index.
    pub fn atom_index(self) -> u32 {
        self.0 & Self::ATOM_MASK
    }

    /// Return whether this exact literal is negated.
    pub fn is_negated(self) -> bool {
        (self.0 & Self::NEGATION_BIT) != 0
    }

    /// Return the underlying raw projection-local value.
    pub fn as_raw(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for ExactLitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_negated() {
            write!(f, "exact~{}", self.atom_index())
        } else {
            write!(f, "exact{}", self.atom_index())
        }
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
    args: SmallVec<[Term; 2]>,
    /// Modal operator, if any.
    mode: Mode,
    /// Whether the literal is negated (`~p` vs `p`).
    negated: bool,
}

impl FamilyId {
    /// Create a `FamilyId` from its constituent parts.
    pub fn new(
        functor: InternedLiteralName,
        args: impl Into<SmallVec<[Term; 2]>>,
        mode: Mode,
        negated: bool,
    ) -> Self {
        Self {
            functor,
            args: args.into(),
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
            args: lit.predicate_args().iter().cloned().collect(),
            mode: lit.mode.clone(),
            negated: lit.negation,
        }
    }
}

// ---------------------------------------------------------------------------
// Canonical semantic keys (process-independent, injective)
// ---------------------------------------------------------------------------

/// Append `s` with every structural key character escaped by `\`.
///
/// Structural characters — `\ ( ) , [ ] : #` — delimit fields in canonical
/// keys. Escaping them inside content makes the encoding injective: no two
/// distinct field decompositions can produce the same string.
pub(crate) fn push_key_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        if matches!(c, '\\' | '(' | ')' | ',' | '[' | ']' | ':' | '#') {
            out.push('\\');
        }
        out.push(c);
    }
}

/// Append the canonical TYPED encoding of a predicate argument.
///
/// Symbols are escaped verbatim (they can never begin with an unescaped
/// `#`), and numerics carry a `#`-prefixed type tag so the symbol `1`, the
/// integer `1`, and the float `1` cannot collide. Decimals render their
/// NORMALIZED value: `1.0` and `1.00` are one `Decimal` under `Eq`/`Hash`
/// (hence one interned identity), so the key must not depend on which
/// scale happened to be interned first.
pub(crate) fn push_canonical_term(out: &mut String, term: &Term) {
    match term {
        Term::Symbol(id) => push_key_escaped(out, resolve(*id)),
        Term::Integer(n) => {
            out.push_str("#i");
            out.push_str(&n.to_string());
        }
        Term::Decimal(d) => {
            out.push_str("#d");
            if d.is_zero() {
                out.push('0');
            } else {
                out.push_str(&d.normalize().to_string());
            }
        }
        Term::Float(f) => {
            out.push_str("#f");
            out.push_str(&f.value().to_string());
        }
    }
}

/// Append the canonical encoding of a time point: `-inf`, `+inf`, `m<i64>`.
pub(crate) fn push_canonical_timepoint(out: &mut String, tp: &TimePoint) {
    match tp {
        TimePoint::NegInf => out.push_str("-inf"),
        TimePoint::PosInf => out.push_str("+inf"),
        TimePoint::Moment(v) => {
            out.push('m');
            out.push_str(&v.to_string());
        }
    }
}

impl FamilyId {
    /// Canonical, process-independent, INJECTIVE key for this family.
    ///
    /// `Display` is for humans and is neither canonical nor injective:
    /// `p("a, b")` and `p(a, b)` display identically, and decimal
    /// arguments display with whatever scale they were built with even
    /// though `1.0` and `1.00` are the same value under `Eq`. This key
    /// escapes structural characters, tags argument types, and normalizes
    /// decimals, so it is constant on `FamilyId` equality classes and
    /// distinct across them. Snapshot serialization must use this, never
    /// `Display`.
    pub fn canonical_key(&self) -> String {
        let mut out = String::new();
        self.push_canonical_key(&mut out);
        out
    }

    pub(crate) fn push_canonical_key(&self, out: &mut String) {
        // Fixed-position header: literal polarity, mode polarity, mode-name
        // presence tag ('!' = named, '.' = none) — Some("") and None must
        // not collapse.
        out.push(if self.negated { '-' } else { '+' });
        out.push(if self.mode.negation { '-' } else { '+' });
        match &self.mode.name {
            Some(name) => {
                out.push('!');
                push_key_escaped(out, name);
            }
            None => out.push('.'),
        }
        out.push(':');
        push_key_escaped(out, self.functor.resolve());
        out.push('(');
        for (i, t) in self.args.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            push_canonical_term(out, t);
        }
        out.push(')');
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

/// A token recording that a non-defeater rule produced exact support for a
/// specific literal identity.
///
/// Emitted when a non-defeater rule fires and its head matches an exact
/// literal identity.
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
    /// The exact temporal literal from which the attack originates.
    pub source_exact_lit: ExactLitId,
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
    exact_supports: usize,
    family_supports: usize,
    family_attacks: usize,
}

impl ProjectionEngine {
    /// Create a new, empty projection engine.
    pub fn new() -> Self {
        Self {
            tokens: Vec::new(),
            exact_supports: 0,
            family_supports: 0,
            family_attacks: 0,
        }
    }

    /// Create a projection engine with pre-allocated capacity.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            tokens: Vec::with_capacity(cap),
            exact_supports: 0,
            family_supports: 0,
            family_attacks: 0,
        }
    }

    fn push_exact(&mut self, support: ExactSupport) {
        self.exact_supports += 1;
        self.tokens.push(ProjectionToken::Exact(support));
    }

    fn push_family(&mut self, support: FamilySupport) {
        self.family_supports += 1;
        self.tokens.push(ProjectionToken::Family(support));
    }

    fn push_attack(&mut self, attack: FamilyAttack) {
        self.family_attacks += 1;
        self.tokens.push(ProjectionToken::Attack(attack));
    }

    /// Project a fired rule into support/attack tokens.
    ///
    /// For each head literal in the rule:
    ///
    /// - **Any rule type, any head**: emits [`ExactSupport`] for the exact
    ///   literal identity of the head.
    /// - **Defeater head**: additionally emits [`FamilyAttack`] against the
    ///   head's base family, using the original rule label and type.
    /// - **Non-defeater head**: additionally emits [`FamilySupport`] for the
    ///   head's base family.
    ///
    /// # Pre-conditions (CON-002)
    ///
    /// - Rule applicability has been evaluated against body match keys.
    /// - Original rule label and type are available.
    ///
    /// # Post-conditions (CON-002)
    ///
    /// - Emits support/attack tokens using the rule label space returned by
    ///   reasoning results (grounded instances keep their grounded labels).
    /// - Does not materialize synthetic rules in `Theory`.
    pub fn try_project_rule(&mut self, rule: &Rule, index: &mut IndexedTheory<'_>) -> Result<()> {
        let rule_type = rule.rule_type;

        for head_lit in &rule.head {
            let exact = index.try_exact_lit_id(head_lit)?;
            let family = FamilyId::from(head_lit);

            self.push_exact(ExactSupport {
                exact_lit: exact,
                rule_label: rule.label.clone(),
                rule_type,
            });

            if rule_type.is_defeater() {
                // Defeaters attack the base family.
                self.push_attack(FamilyAttack {
                    family_id: family,
                    source_exact_lit: exact,
                    rule_label: rule.label.clone(),
                    rule_type,
                });
            } else {
                // Non-defeaters support both the exact literal and its family.
                self.push_family(FamilySupport {
                    family_id: family,
                    source_exact_lit: exact,
                    rule_label: rule.label.clone(),
                    rule_type,
                });
            }
        }

        Ok(())
    }

    /// Project a fired rule into support/attack tokens.
    ///
    /// Panics if exact-literal interning exhausts the available ID space.
    pub fn project_rule(&mut self, rule: &Rule, index: &mut IndexedTheory<'_>) {
        self.try_project_rule(rule, index)
            .expect("exact literal capacity exceeded while projecting rule")
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
        self.exact_supports = 0;
        self.family_supports = 0;
        self.family_attacks = 0;
    }

    /// Count of exact support tokens.
    pub fn exact_count(&self) -> usize {
        self.exact_supports
    }

    /// Count of family support tokens.
    pub fn family_count(&self) -> usize {
        self.family_supports
    }

    /// Count of family attack tokens.
    pub fn attack_count(&self) -> usize {
        self.family_attacks
    }

    /// Produce structured diagnostic counters (OBS-001).
    ///
    /// Returns a [`ProjectionDiagnostics`] snapshot summarizing the current
    /// projection state: token counts by type and the set of rule labels
    /// that contributed projected evidence.
    pub fn diagnostics(&self) -> ProjectionDiagnostics {
        debug_assert_counts(self);
        let mut contributing_labels = std::collections::BTreeSet::new();

        for token in &self.tokens {
            match token {
                ProjectionToken::Exact(s) => {
                    contributing_labels.insert(s.rule_label.clone());
                }
                ProjectionToken::Family(s) => {
                    contributing_labels.insert(s.rule_label.clone());
                }
                ProjectionToken::Attack(a) => {
                    contributing_labels.insert(a.rule_label.clone());
                }
            }
        }

        ProjectionDiagnostics {
            exact_supports: self.exact_supports,
            family_supports: self.family_supports,
            family_attacks: self.family_attacks,
            contributing_labels,
        }
    }

    /// Produce a deterministic debug snapshot for test-mode comparison
    /// (OBS-002).
    ///
    /// The snapshot contains tokens sorted by a stable key so that
    /// non-deterministic iteration order does not cause spurious test
    /// failures. Labels are sorted alphabetically within each category.
    ///
    /// Exact literals are rendered through `index.exact_lit_key` and
    /// families through `FamilyId::canonical_key` — canonical, injective
    /// semantic keys — never through `ExactLitId`'s slot-number display
    /// (slot numbers follow interning order, which follows rule iteration
    /// over the theory's randomized `HashMap`) and never through the
    /// human-readable `Display` forms (non-canonical for equal decimals of
    /// different scales, non-injective on argument boundaries). `index`
    /// must be the same index the tokens were projected against; an ID
    /// unknown to it falls back to the raw slot display.
    pub fn snapshot(&self, index: &IndexedTheory<'_>) -> ProjectionSnapshot {
        debug_assert_counts(self);
        let exact_key = |id: ExactLitId| {
            index.exact_lit_key(id).unwrap_or_else(|| id.to_string())
        };
        let mut exact: Vec<(String, String)> = Vec::with_capacity(self.exact_supports);
        let mut family: Vec<(String, String, String)> = Vec::with_capacity(self.family_supports);
        let mut attack: Vec<(String, String, String)> = Vec::with_capacity(self.family_attacks);

        for token in &self.tokens {
            match token {
                ProjectionToken::Exact(s) => {
                    exact.push((s.rule_label.clone(), exact_key(s.exact_lit)));
                }
                ProjectionToken::Family(s) => {
                    family.push((
                        s.rule_label.clone(),
                        s.family_id.canonical_key(),
                        exact_key(s.source_exact_lit),
                    ));
                }
                ProjectionToken::Attack(a) => {
                    attack.push((
                        a.rule_label.clone(),
                        a.family_id.canonical_key(),
                        exact_key(a.source_exact_lit),
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

fn debug_assert_counts(engine: &ProjectionEngine) {
    let counts = engine.tokens.iter().fold(
        (0usize, 0usize, 0usize),
        |(exact, family, attack), token| match token {
            ProjectionToken::Exact(_) => (exact + 1, family, attack),
            ProjectionToken::Family(_) => (exact, family + 1, attack),
            ProjectionToken::Attack(_) => (exact, family, attack + 1),
        },
    );
    debug_assert_eq!(
        counts,
        (
            engine.exact_supports,
            engine.family_supports,
            engine.family_attacks
        ),
        "projection token counters drifted from the token buffer"
    );
}

// ---------------------------------------------------------------------------
// ProjectionDiagnostics — OBS-001
// ---------------------------------------------------------------------------

/// Structured diagnostic counters for projection activity (OBS-001).
///
/// Summarizes the number of exact supports, family supports, family attacks,
/// and the set of rule labels that contributed projected evidence. This
/// validates that projection activity is explainable and bounded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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
/// order does not cause test regressions, and literals are rendered by their
/// CANONICAL semantic keys ([`FamilyId::canonical_key`] /
/// `IndexedTheory::exact_lit_key`) — constant on interning equality classes
/// (normalized decimals) and injective across them (escaped, typed) — never
/// by projection-local slot numbers, so snapshots of the same theory agree
/// across processes. Useful for golden-file or snapshot testing of projected
/// evidence ordering and tie-break label selection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProjectionSnapshot {
    /// Sorted `(rule_label, exact_canonical_key)` pairs for exact support
    /// tokens.
    pub exact: Vec<(String, String)>,
    /// Sorted `(rule_label, family_canonical_key, source_exact_canonical_key)`
    /// triples for family support tokens.
    pub family: Vec<(String, String, String)>,
    /// Sorted `(rule_label, family_canonical_key, source_exact_canonical_key)`
    /// triples for family attack tokens.
    pub attack: Vec<(String, String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mode::Mode;
    use crate::temporal::{Temporal, TimePoint};

    #[test]
    fn exact_lit_id_display_is_stable() {
        assert_eq!(ExactLitId::new(7, false).to_string(), "exact7");
        assert_eq!(ExactLitId::new(7, true).to_string(), "exact~7");
    }

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

    fn first_exact(engine: &ProjectionEngine) -> &ExactSupport {
        engine
            .tokens()
            .iter()
            .find_map(|token| match token {
                ProjectionToken::Exact(s) => Some(s),
                _ => None,
            })
            .expect("expected Exact token")
    }

    fn first_family(engine: &ProjectionEngine) -> &FamilySupport {
        engine
            .tokens()
            .iter()
            .find_map(|token| match token {
                ProjectionToken::Family(s) => Some(s),
                _ => None,
            })
            .expect("expected Family token")
    }

    fn first_attack(engine: &ProjectionEngine) -> &FamilyAttack {
        engine
            .tokens()
            .iter()
            .find_map(|token| match token {
                ProjectionToken::Attack(a) => Some(a),
                _ => None,
            })
            .expect("expected Attack token")
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
        let exact = first_exact(&engine);
        assert_eq!(exact.rule_label, "f1");
        assert_eq!(exact.rule_type, crate::rule::RuleType::Fact);

        // Family support projects to the atemporal family.
        let family = first_family(&engine);
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

        let attack = first_attack(&engine);
        assert_eq!(attack.rule_label, "d1");
        assert_eq!(attack.family_id, FamilyId::from(&Literal::negated("works")));
        assert_eq!(
            attack.source_exact_lit,
            idx.exact_lit_id(&Literal::negated("works"))
        );
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

        let attack = first_attack(&engine);

        // Attack targets the base family ~q (no temporal bounds).
        let base_family = FamilyId::from(&Literal::negated("q"));
        assert_eq!(attack.family_id, base_family);
        assert_eq!(attack.rule_label, "d_temp");
    }

    #[test]
    fn grounded_rule_label_is_preserved() {
        // Grounded instances should stay in the grounded label space so
        // projection tokens can be joined back to conclusions and bodies.
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
                ProjectionToken::Exact(s) => assert_eq!(s.rule_label, "r1__ground_0"),
                ProjectionToken::Family(s) => assert_eq!(s.rule_label, "r1__ground_0"),
                ProjectionToken::Attack(a) => assert_eq!(a.rule_label, "r1__ground_0"),
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

        // Expected canonical keys, derived through the same public API.
        let id_a = idx.exact_lit_id(&Literal::simple("a"));
        let id_b = idx.exact_lit_id(&Literal::simple("b"));
        let key_a = idx.exact_lit_key(id_a).unwrap();
        let key_b = idx.exact_lit_key(id_b).unwrap();

        let snap = engine.snapshot(&idx);
        // Exact entries should be sorted by label.
        assert_eq!(snap.exact[0].0, "f1");
        assert_eq!(snap.exact[1].0, "f2");
        // Exact literals are rendered by canonical key, not interning slot.
        assert_eq!(snap.exact[0].1, key_a);
        assert_eq!(snap.exact[1].1, key_b);
        // Family entries should be sorted by label.
        assert_eq!(snap.family[0].0, "f1");
        assert_eq!(snap.family[1].0, "f2");

        // Two calls produce identical snapshots.
        assert_eq!(snap, engine.snapshot(&idx));
    }

    #[test]
    fn snapshot_uses_semantic_identity_not_interning_order() {
        // ExactLitId slots follow interning order, which follows rule
        // iteration over the theory's randomized HashMap. Force the two
        // orders EXPLICITLY on fresh indexes (an index built from a
        // populated theory has already interned everything, in the same
        // order for the same map instance): the slot assignments must
        // actually differ, yet the snapshots must be identical because
        // they serialize canonical semantic keys, not slots.
        let theory = Theory::new();
        let rule_a = Rule::fact("f1", Literal::simple("a"));
        let rule_b = Rule::fact("f2", Literal::simple("b"));

        let mut idx_ab = IndexedTheory::build(&theory);
        let a_slot_first = idx_ab.exact_lit_id(&Literal::simple("a"));
        let b_slot_second = idx_ab.exact_lit_id(&Literal::simple("b"));

        let mut idx_ba = IndexedTheory::build(&theory);
        let b_slot_first = idx_ba.exact_lit_id(&Literal::simple("b"));
        let a_slot_second = idx_ba.exact_lit_id(&Literal::simple("a"));

        // Premise: the two interning orders really assign different slots.
        assert_ne!(a_slot_first, a_slot_second);
        assert_ne!(b_slot_second, b_slot_first);

        let snap_ab = {
            let mut engine = ProjectionEngine::new();
            engine.project_rule(&rule_a, &mut idx_ab);
            engine.project_rule(&rule_b, &mut idx_ab);
            engine.snapshot(&idx_ab)
        };
        let snap_ba = {
            let mut engine = ProjectionEngine::new();
            engine.project_rule(&rule_a, &mut idx_ba);
            engine.project_rule(&rule_b, &mut idx_ba);
            engine.snapshot(&idx_ba)
        };
        assert_eq!(snap_ab, snap_ba);
    }

    #[test]
    fn exact_key_is_canonical_for_equal_decimals() {
        // p(1.0) and p(1.00) are equal Decimals, so they intern to ONE
        // ExactLitId per index; the retained ExactAtomKey is whichever
        // variant arrived first. The canonical key must not leak that
        // race: both interning orders yield the same key.
        use rust_decimal::Decimal;
        let lit_scale1 = Literal::from_ids(
            crate::intern::intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Decimal(Decimal::new(10, 1))], // 1.0
        );
        let lit_scale2 = Literal::from_ids(
            crate::intern::intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Decimal(Decimal::new(100, 2))], // 1.00
        );

        let theory = Theory::new();
        let mut idx1 = IndexedTheory::build(&theory);
        let id1 = idx1.exact_lit_id(&lit_scale1);
        // Same equality class: the second variant maps to the same id.
        assert_eq!(id1, idx1.exact_lit_id(&lit_scale2));

        let mut idx2 = IndexedTheory::build(&theory);
        let id2 = idx2.exact_lit_id(&lit_scale2);
        assert_eq!(id2, idx2.exact_lit_id(&lit_scale1));

        assert_eq!(idx1.exact_lit_key(id1), idx2.exact_lit_key(id2));
    }

    #[test]
    fn canonical_key_is_injective_on_argument_boundaries() {
        // p("a, b") (one symbol argument containing ", ") and p(a, b)
        // (two arguments) display identically via Display; their canonical
        // keys must differ.
        let one_arg = FamilyId::new(
            crate::intern::intern("p").into(),
            vec![Term::Symbol(crate::intern::intern("a, b"))],
            Mode::empty(),
            false,
        );
        let two_args = FamilyId::new(
            crate::intern::intern("p").into(),
            vec![
                Term::Symbol(crate::intern::intern("a")),
                Term::Symbol(crate::intern::intern("b")),
            ],
            Mode::empty(),
            false,
        );
        assert_eq!(one_arg.to_string(), two_args.to_string());
        assert_ne!(one_arg.canonical_key(), two_args.canonical_key());
    }

    #[test]
    fn canonical_key_tags_numeric_types() {
        // The symbol `1`, the integer 1, and the decimal 1 are distinct
        // identities that all display as `p(1)`.
        let functor: InternedLiteralName = crate::intern::intern("p").into();
        let sym = FamilyId::new(
            functor,
            vec![Term::Symbol(crate::intern::intern("1"))],
            Mode::empty(),
            false,
        );
        let int = FamilyId::new(functor, vec![Term::Integer(1)], Mode::empty(), false);
        let dec = FamilyId::new(
            functor,
            vec![Term::Decimal(rust_decimal::Decimal::from(1))],
            Mode::empty(),
            false,
        );
        let keys = [sym.canonical_key(), int.canonical_key(), dec.canonical_key()];
        assert_ne!(keys[0], keys[1]);
        assert_ne!(keys[0], keys[2]);
        assert_ne!(keys[1], keys[2]);
    }

    #[test]
    fn snapshot_empty_engine() {
        let theory = Theory::new();
        let idx = make_index(&theory);
        let engine = ProjectionEngine::new();
        let snap = engine.snapshot(&idx);
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

        let family_tok = first_family(&engine);

        // Family should be the atemporal base q.
        let base = FamilyId::from(&Literal::simple("q"));
        assert_eq!(family_tok.family_id, base);
    }
}
