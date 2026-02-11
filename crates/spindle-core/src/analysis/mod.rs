//! Theory analysis sub-module
//!
//! Provides conflict detection, semantic validation, and superiority
//! suggestion for defeasible logic theories.  Each sub-module exposes
//! an iterator-based API so callers can feed raw `&[Rule]` slices
//! without constructing a full event log or Petri net.
//!
//! Shared types (`ValidationDiagnostic`, `Severity`, `ConflictReport`,
//! `ConflictKind`, `SuperioritySuggestion`) are defined here so every
//! analysis sub-module can return them without circular imports.

pub mod conflicts;
pub mod superiority;
pub mod validation;

use crate::rule::RuleLabel;
use std::fmt;

// ---------------------------------------------------------------------------
// Conflict report
// ---------------------------------------------------------------------------

/// Two rules whose heads are complementary, with the evidence trail.
#[derive(Debug, Clone)]
pub struct ConflictReport {
    /// Label of the first conflicting rule.
    pub rule_a: RuleLabel,
    /// Label of the second conflicting rule.
    pub rule_b: RuleLabel,
    /// Display form of the first rule's head.
    pub head_a: String,
    /// Display form of the second rule's head.
    pub head_b: String,
    /// What kind of conflict was detected.
    pub conflict_type: ConflictKind,
}

/// The kind of conflict between two rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// Classical negation: `p` vs `~p`
    Negation,
    /// Mutual exclusion inferred from traces.
    MutualExclusion,
    /// XOR-choice from Petri net structure.
    Choice,
}

// ---------------------------------------------------------------------------
// Validation diagnostic
// ---------------------------------------------------------------------------

/// A diagnostic produced by theory validation.
#[derive(Debug, Clone)]
pub struct ValidationDiagnostic {
    /// How serious the diagnostic is.
    pub severity: Severity,
    /// A short machine-readable code (e.g. `"W001"`).
    pub code: &'static str,
    /// Human-readable explanation.
    pub message: String,
    /// The rule label(s) involved, if applicable.
    pub rules: Vec<RuleLabel>,
}

/// Severity of a [`ValidationDiagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Non-fatal issue that may indicate a mistake.
    Warning,
    /// Semantic error that will cause incorrect reasoning.
    Error,
}

impl fmt::Display for ValidationDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}: {}", self.severity, self.code, self.message)
    }
}

// ---------------------------------------------------------------------------
// Superiority suggestion
// ---------------------------------------------------------------------------

/// A suggested superiority relation with justification.
#[derive(Debug, Clone)]
pub struct SuperioritySuggestion {
    /// The rule that should be superior.
    pub superior: RuleLabel,
    /// The rule that should be inferior.
    pub inferior: RuleLabel,
    /// Human-readable justification for why this superiority makes sense.
    pub reason: String,
    /// Confidence score in `[0.0, 1.0]` based on trace evidence.
    pub confidence: f64,
}

/// A conflicting pair with no declared superiority relation.
///
/// Returned by [`superiority::check_completeness`] to highlight gaps
/// in the theory's superiority declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnresolvedConflict {
    /// Label of the first rule in the unresolved conflict.
    pub rule_a: RuleLabel,
    /// Label of the second rule in the unresolved conflict.
    pub rule_b: RuleLabel,
}
