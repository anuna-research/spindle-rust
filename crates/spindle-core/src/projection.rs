//! First-class projection tokens for temporal family reasoning.
//!
//! Instead of synthesizing bridge rules to connect temporal literals to their
//! atemporal families, the engine emits projection tokens that carry the
//! original rule label and type. This preserves superiority and trust semantics
//! by construction (SPEC-020, §4.2).

use crate::index::LitId;
use crate::rule::{RuleLabel, RuleType};

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
/// `p[1,10]` and `p[20,30]` share the same `FamilyId`, while `~p[1,10]`
/// belongs to a separate negative family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct FamilyId(u32);

impl FamilyId {
    /// Create a `FamilyId` from a raw identifier.
    pub fn from_raw(value: u32) -> Self {
        Self(value)
    }

    /// Return the underlying raw identifier.
    pub fn as_raw(self) -> u32 {
        self.0
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
