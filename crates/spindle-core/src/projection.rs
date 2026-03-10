//! First-class projection tokens for temporal family reasoning.
//!
//! Instead of synthesizing bridge rules to connect temporal literals to their
//! atemporal families, the engine emits projection tokens that carry the
//! original rule label and type. This preserves superiority and trust semantics
//! by construction (SPEC-020, §4.2).

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
    pub fn new(
        functor: InternedLiteralName,
        args: Vec<Term>,
        mode: Mode,
        negated: bool,
    ) -> Self {
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
/// This replaces synthetic bridge defeater rules — the attack carries the
/// original rule's identity so that superiority resolution uses authored
/// labels directly (SPEC-020, ADR-004).
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
        let lit_a = Literal::new("parent", false, Mode::empty(), Temporal::empty(), vec!["alice".into()]);
        let lit_b = Literal::new("parent", false, Mode::empty(), Temporal::empty(), vec!["bob".into()]);

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
        let lit = Literal::new("parent", true, Mode::empty(), Temporal::empty(), vec!["alice".into(), "bob".into()]);
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
}
