//! Pure data types for the explanation system.
//!
//! This module contains the proof-tree model and related data structures.
//! These are pure data containers with builder/construction methods.
//! Rendering and formatting logic lives in `format.rs`.

use std::collections::HashMap;
use std::fmt;

use crate::conclusion::ConclusionType;
use crate::literal::Literal;
use crate::rule::RuleType;

// ---------------------------------------------------------------------------
// Annotations
// ---------------------------------------------------------------------------

/// Annotations container for rules, facts, and superiorities
#[derive(Debug, Clone, Default)]
pub struct Annotations {
    /// Optional @id URI
    pub id: Option<String>,
    /// Key-value entries
    pub entries: HashMap<String, String>,
}

impl Annotations {
    /// Create empty annotations
    pub fn new() -> Self {
        Self::default()
    }

    /// Create annotations with entries
    pub fn with_entries(entries: Vec<(&str, &str)>) -> Self {
        Self {
            id: None,
            entries: entries
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    /// Check if annotations are empty
    pub fn is_empty(&self) -> bool {
        self.id.is_none() && self.entries.is_empty()
    }

    /// Get annotation value by key
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.as_str())
    }

    /// Get annotation trying multiple keys
    pub fn get_any(&self, keys: &[&str]) -> Option<&str> {
        for key in keys {
            if let Some(v) = self.get(key) {
                return Some(v);
            }
        }
        None
    }

    /// Get description from annotations
    pub fn description(&self) -> Option<&str> {
        self.get_any(&["description", "dc:description", "rdfs:comment"])
    }

    /// Get source from annotations
    pub fn source(&self) -> Option<&str> {
        self.get_any(&["source", "dc:source", "prov:wasAttributedTo"])
    }

    /// Get confidence from annotations
    pub fn confidence(&self) -> Option<&str> {
        self.get_any(&["confidence", "spindle:confidence"])
    }
}

// ---------------------------------------------------------------------------
// Derivation / Block / Resolution enums
// ---------------------------------------------------------------------------

/// Derivation type for a proof
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivationType {
    /// Derived via strict rules/facts only
    Definite,
    /// Derived via defeasible rules
    Defeasible,
}

/// Reason why a proof was blocked
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockReason {
    /// Blocked by a superior rule
    Superiority,
    /// Blocked by a defeater
    Defeater,
    /// Blocked due to unresolved conflict
    Conflict,
    /// Blocked because body wasn't provable
    BodyUnprovable,
}

impl fmt::Display for BlockReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockReason::Superiority => write!(f, "superiority"),
            BlockReason::Defeater => write!(f, "defeater"),
            BlockReason::Conflict => write!(f, "conflict"),
            BlockReason::BodyUnprovable => write!(f, "body unprovable"),
        }
    }
}

/// Resolution type for conflicts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionType {
    /// Resolved by superiority relation
    Superiority,
    /// Resolved by definite priority (strict > defeasible)
    DefinitePriority,
    /// Resolved by team defeat
    TeamDefeat,
}

impl fmt::Display for ResolutionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolutionType::Superiority => write!(f, "superiority"),
            ResolutionType::DefinitePriority => write!(f, "definite priority"),
            ResolutionType::TeamDefeat => write!(f, "team defeat"),
        }
    }
}

// ---------------------------------------------------------------------------
// Core data structures
// ---------------------------------------------------------------------------

/// A proof step represents one application of a rule
#[derive(Debug, Clone)]
pub struct ProofStep {
    /// Which rule was applied
    pub rule_label: String,
    /// Type of rule applied
    pub rule_type: RuleType,
    /// String representation of the rule
    pub rule_text: String,
    /// Proofs for each body literal
    pub body_proofs: Vec<ProofNode>,
    /// Rule annotations
    pub annotations: Annotations,
}

impl ProofStep {
    /// Create a new proof step
    pub fn new(
        rule_label: impl Into<String>,
        rule_type: RuleType,
        rule_text: impl Into<String>,
    ) -> Self {
        Self {
            rule_label: rule_label.into(),
            rule_type,
            rule_text: rule_text.into(),
            body_proofs: Vec::new(),
            annotations: Annotations::new(),
        }
    }

    /// Add body proofs
    pub fn with_body_proofs(mut self, proofs: Vec<ProofNode>) -> Self {
        self.body_proofs = proofs;
        self
    }

    /// Add annotations
    pub fn with_annotations(mut self, annotations: Annotations) -> Self {
        self.annotations = annotations;
        self
    }
}

/// A proof node represents the derivation of a single literal
#[derive(Debug, Clone)]
pub struct ProofNode {
    /// What was derived
    pub literal: Literal,
    /// How it was derived (definite or defeasible)
    pub derivation_type: DerivationType,
    /// The proof step (how it was derived)
    pub proof_step: Option<ProofStep>,
    /// Alternatives that were blocked
    pub blocked_alternatives: Vec<BlockedProof>,
    /// Conflicts that were resolved
    pub conflicts_resolved: Vec<ConflictResolution>,
}

impl ProofNode {
    /// Create a new proof node
    pub fn new(literal: Literal, derivation_type: DerivationType) -> Self {
        Self {
            literal,
            derivation_type,
            proof_step: None,
            blocked_alternatives: Vec::new(),
            conflicts_resolved: Vec::new(),
        }
    }

    /// Add proof step
    pub fn with_proof_step(mut self, step: ProofStep) -> Self {
        self.proof_step = Some(step);
        self
    }

    /// Add blocked alternatives
    pub fn with_blocked(mut self, blocked: Vec<BlockedProof>) -> Self {
        self.blocked_alternatives = blocked;
        self
    }

    /// Add conflict resolutions
    pub fn with_conflicts(mut self, conflicts: Vec<ConflictResolution>) -> Self {
        self.conflicts_resolved = conflicts;
        self
    }
}

/// Record of a blocked alternative derivation
#[derive(Debug, Clone)]
pub struct BlockedProof {
    /// What we tried to prove
    pub literal: Literal,
    /// Which rule we tried
    pub rule_label: String,
    /// String representation
    pub rule_text: String,
    /// Why it was blocked
    pub reason: BlockReason,
    /// Rule that blocked this one (if applicable)
    pub blocking_rule: Option<String>,
    /// Human-readable explanation
    pub explanation: String,
}

impl BlockedProof {
    /// Create a new blocked proof record
    pub fn new(
        literal: Literal,
        rule_label: impl Into<String>,
        reason: BlockReason,
        explanation: impl Into<String>,
    ) -> Self {
        Self {
            literal,
            rule_label: rule_label.into(),
            rule_text: String::new(),
            reason,
            blocking_rule: None,
            explanation: explanation.into(),
        }
    }

    /// Set the blocking rule
    pub fn with_blocking_rule(mut self, rule: impl Into<String>) -> Self {
        self.blocking_rule = Some(rule.into());
        self
    }
}

/// Record of conflict resolution
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    /// Rule that won
    pub winning_rule: String,
    /// Rule that lost
    pub losing_rule: String,
    /// How the conflict was resolved
    pub resolution_type: ResolutionType,
    /// Superiority relation label (if applicable)
    pub superiority_label: Option<String>,
}

impl ConflictResolution {
    /// Create a new conflict resolution record
    pub fn new(
        winning_rule: impl Into<String>,
        losing_rule: impl Into<String>,
        resolution_type: ResolutionType,
    ) -> Self {
        Self {
            winning_rule: winning_rule.into(),
            losing_rule: losing_rule.into(),
            resolution_type,
            superiority_label: None,
        }
    }

    /// Set superiority label
    pub fn with_superiority(mut self, label: impl Into<String>) -> Self {
        self.superiority_label = Some(label.into());
        self
    }
}

/// A complete explanation for a conclusion
#[derive(Debug, Clone)]
pub struct Explanation {
    /// The conclusion type being explained
    pub conclusion_type: ConclusionType,
    /// The literal being explained
    pub literal: Literal,
    /// The derivation tree (if provable)
    pub proof_tree: Option<ProofNode>,
    /// Why other derivations failed
    pub blocked_alternatives: Vec<BlockedProof>,
    /// How conflicts were resolved
    pub conflicts_resolved: Vec<ConflictResolution>,
}

impl Explanation {
    /// Create a new explanation
    pub fn new(conclusion_type: ConclusionType, literal: Literal) -> Self {
        Self {
            conclusion_type,
            literal,
            proof_tree: None,
            blocked_alternatives: Vec::new(),
            conflicts_resolved: Vec::new(),
        }
    }

    /// Add proof tree
    pub fn with_proof(mut self, proof: ProofNode) -> Self {
        self.proof_tree = Some(proof);
        self
    }

    /// Add blocked alternatives
    pub fn with_blocked(mut self, blocked: Vec<BlockedProof>) -> Self {
        self.blocked_alternatives = blocked;
        self
    }

    /// Add conflict resolutions
    pub fn with_conflicts(mut self, conflicts: Vec<ConflictResolution>) -> Self {
        self.conflicts_resolved = conflicts;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_annotations() {
        let mut annots = Annotations::new();
        assert!(annots.is_empty());

        annots
            .entries
            .insert("description".to_string(), "A test".to_string());
        assert!(!annots.is_empty());
        assert_eq!(annots.description(), Some("A test"));
    }

    #[test]
    fn test_annotations_with_entries() {
        let annots = Annotations::with_entries(vec![
            ("source", "legal-code-section-1"),
            ("confidence", "0.95"),
            ("dc:description", "Test description"),
        ]);

        assert!(!annots.is_empty());
        assert_eq!(annots.source(), Some("legal-code-section-1"));
        assert_eq!(annots.confidence(), Some("0.95"));
        // dc:description should be found via get_any fallback
        assert_eq!(annots.description(), Some("Test description"));
    }

    #[test]
    fn test_annotations_id() {
        let mut annots = Annotations::new();
        annots.id = Some("https://example.org/rule/r1".to_string());
        assert!(!annots.is_empty());
        assert_eq!(annots.id, Some("https://example.org/rule/r1".to_string()));
    }

    #[test]
    fn test_proof_step() {
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies");
        assert_eq!(step.rule_label, "r1");
        assert_eq!(step.rule_type, RuleType::Defeasible);
    }

    #[test]
    fn test_proof_step_with_body_proofs() {
        let body_node = ProofNode::new(Literal::simple("bird"), DerivationType::Definite);
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies")
            .with_body_proofs(vec![body_node]);

        assert_eq!(step.body_proofs.len(), 1);
        assert_eq!(step.body_proofs[0].literal.name(), "bird");
    }

    #[test]
    fn test_proof_step_with_annotations() {
        let annots = Annotations::with_entries(vec![("source", "expert-knowledge")]);
        let step =
            ProofStep::new("r1", RuleType::Defeasible, "bird => flies").with_annotations(annots);

        assert_eq!(step.annotations.source(), Some("expert-knowledge"));
    }

    #[test]
    fn test_proof_node() {
        let node = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible);
        assert_eq!(node.literal.name(), "flies");
        assert_eq!(node.derivation_type, DerivationType::Defeasible);
    }

    #[test]
    fn test_blocked_proof() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "r2 > r1",
        )
        .with_blocking_rule("r2");

        assert_eq!(blocked.reason, BlockReason::Superiority);
        assert_eq!(blocked.blocking_rule, Some("r2".to_string()));
    }

    #[test]
    fn test_block_reason_display() {
        assert_eq!(BlockReason::Superiority.to_string(), "superiority");
        assert_eq!(BlockReason::Defeater.to_string(), "defeater");
        assert_eq!(BlockReason::Conflict.to_string(), "conflict");
        assert_eq!(BlockReason::BodyUnprovable.to_string(), "body unprovable");
    }

    #[test]
    fn test_conflict_resolution() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority)
            .with_superiority("sup1");

        assert_eq!(conflict.winning_rule, "r2");
        assert_eq!(conflict.losing_rule, "r1");
        assert_eq!(conflict.superiority_label, Some("sup1".to_string()));
    }

    #[test]
    fn test_resolution_type_display() {
        assert_eq!(ResolutionType::Superiority.to_string(), "superiority");
        assert_eq!(
            ResolutionType::DefinitePriority.to_string(),
            "definite priority"
        );
        assert_eq!(ResolutionType::TeamDefeat.to_string(), "team defeat");
    }

    #[test]
    fn test_explanation_with_blocked_builder() {
        let blocked = BlockedProof::new(Literal::simple("x"), "r1", BlockReason::Conflict, "test");
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyNotProvable, Literal::simple("x"))
                .with_blocked(vec![blocked]);
        assert_eq!(explanation.blocked_alternatives.len(), 1);
    }

    #[test]
    fn test_explanation_with_conflicts_builder() {
        let conflict = ConflictResolution::new("r1", "r2", ResolutionType::TeamDefeat);
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("x"))
                .with_conflicts(vec![conflict]);
        assert_eq!(explanation.conflicts_resolved.len(), 1);
    }
}
