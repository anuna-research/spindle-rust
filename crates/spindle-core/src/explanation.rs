//! Explanation System for Spindle
//!
//! This module provides explanation generation capabilities for the
//! Spindle defeasible logic reasoner:
//!
//! - Proof/derivation tree structures
//! - Explanation output generation (natural language, JSON)
//! - Blocked alternatives tracking
//! - Conflict resolution explanations

use std::collections::HashMap;
use std::fmt;

use crate::conclusion::ConclusionType;
use crate::grounding::{apply_substitution_to_literal, match_literal};
use crate::literal::Literal;
use crate::rule::RuleType;

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

/// Derivation type for a proof
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DerivationType {
    /// Derived via strict rules/facts only
    Definite,
    /// Derived via defeasible rules
    Defeasible,
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

    /// Generate natural language explanation
    pub fn to_natural_language(&self) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!(
            "Explanation for {} {}\n",
            self.conclusion_type, self.literal
        ));
        output.push_str(&format!("{}\n\n", self.conclusion_type_explanation()));

        // Proof tree
        if let Some(ref proof) = self.proof_tree {
            output.push_str("Derivation:\n");
            output.push_str(&proof_node_to_natural_language(proof, 1, "  "));
        } else if self.conclusion_type.is_positive() {
            output.push_str("No derivation found.\n");
        }

        // Blocked alternatives
        if !self.blocked_alternatives.is_empty() {
            output.push_str("\nBlocked Alternatives:\n");
            for (i, blocked) in self.blocked_alternatives.iter().enumerate() {
                output.push_str(&format!(
                    "  {}. Rule '{}' was blocked due to {}: {}\n",
                    i + 1,
                    blocked.rule_label,
                    blocked.reason,
                    blocked.explanation
                ));
            }
        }

        // Conflict resolutions
        if !self.conflicts_resolved.is_empty() {
            output.push_str("\nConflict Resolutions:\n");
            for (i, conflict) in self.conflicts_resolved.iter().enumerate() {
                output.push_str(&format!(
                    "  {}. '{}' defeated '{}' via {}\n",
                    i + 1,
                    conflict.winning_rule,
                    conflict.losing_rule,
                    conflict.resolution_type
                ));
            }
        }

        output
    }

    /// Get explanation text for conclusion type
    fn conclusion_type_explanation(&self) -> &'static str {
        match self.conclusion_type {
            ConclusionType::DefinitelyProvable => {
                "This was proven using only strict rules and facts (cannot be defeated)."
            }
            ConclusionType::DefinitelyNotProvable => {
                "This cannot be proven using strict rules alone."
            }
            ConclusionType::DefeasiblyProvable => {
                "This was proven using defeasible rules and was not defeated by any conflicting rule."
            }
            ConclusionType::DefeasiblyNotProvable => {
                "This could not be proven defeasibly, either because no applicable rule exists or it was defeated."
            }
        }
    }

    /// Convert to JSON representation
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "conclusion_type": self.conclusion_type.symbol(),
            "literal": self.literal.to_string(),
            "proof_tree": self.proof_tree.as_ref().map(proof_node_to_json),
            "blocked_alternatives": self.blocked_alternatives.iter()
                .map(blocked_proof_to_json)
                .collect::<Vec<_>>(),
            "conflicts_resolved": self.conflicts_resolved.iter()
                .map(conflict_to_json)
                .collect::<Vec<_>>(),
        })
    }
}

/// Explain why a conclusion holds (returns Proof Tree)
pub fn explain(theory: &crate::theory::Theory, literal: &Literal) -> Option<Explanation> {
    use crate::reason::reason;
    let conclusions = reason(theory);

    // Find the conclusion for the target literal
    let conclusion = conclusions
        .iter()
        .find(|c| c.literal == *literal && c.conclusion_type.is_positive())?;

    let mut explanation = Explanation::new(conclusion.conclusion_type, conclusion.literal.clone());

    if let Some(rule_label) = &conclusion.rule_label {
        if let Some(rule) = theory.get_rule(rule_label) {
            let mut step = ProofStep::new(
                rule.label.clone(),
                rule.rule_type,
                rule.to_string(), // Rule struct doesn't have source text, approximation
            );

            // Determine substitution by matching rule head against conclusion literal
            // If rule has multiple heads, find the one that matches
            let head_pattern = rule
                .head
                .iter()
                .find(|h| match_literal(h, literal).is_some())
                .unwrap_or(&rule.head[0]); // Fallback, shouldn't happen if logic correct

            let subst = match_literal(head_pattern, literal).unwrap_or_default();

            // Recursively build proofs for body, applying substitution
            let mut body_proofs = Vec::new();
            for body_lit in &rule.body {
                let ground_body_lit = apply_substitution_to_literal(body_lit, &subst);

                if let Some(body_expl) = explain(theory, &ground_body_lit) {
                    if let Some(body_tree) = body_expl.proof_tree {
                        body_proofs.push(body_tree);
                    }
                } else {
                    // If exact ground literal not found, try to find a matching one
                    // (Handle existential cases like matter_seen where body var isn't in head)
                    if let Some(matching_conc) = conclusions.iter().find(|c| {
                        c.conclusion_type.is_positive()
                            && match_literal(body_lit, &c.literal).is_some()
                    }) {
                        if let Some(body_expl) = explain(theory, &matching_conc.literal) {
                            if let Some(body_tree) = body_expl.proof_tree {
                                body_proofs.push(body_tree);
                            }
                        }
                    }
                }
            }
            step.body_proofs = body_proofs;

            // Annotations
            // (Assuming rule has annotations or we mock them)
            // step.annotations = ...

            let proof_node = ProofNode::new(
                literal.clone(),
                match conclusion.conclusion_type {
                    ConclusionType::DefinitelyProvable => DerivationType::Definite,
                    _ => DerivationType::Defeasible,
                },
            )
            .with_proof_step(step);

            explanation.proof_tree = Some(proof_node);
        }
    }

    Some(explanation)
}

/// Convert proof node to natural language (recursive)
fn proof_node_to_natural_language(node: &ProofNode, num: usize, indent: &str) -> String {
    let mut output = String::new();

    if let Some(ref step) = node.proof_step {
        let derivation_str = match node.derivation_type {
            DerivationType::Definite => {
                if step.rule_type == RuleType::Fact {
                    "established as a fact"
                } else {
                    "derived strictly"
                }
            }
            DerivationType::Defeasible => "derived defeasibly",
        };

        output.push_str(&format!(
            "{}{}. \"{}\" was {}\n",
            indent, num, node.literal, derivation_str
        ));
        output.push_str(&format!(
            "{}   Using {}: {}\n",
            indent,
            rule_type_name(step.rule_type),
            step.rule_label
        ));

        // Annotations
        if let Some(desc) = step.annotations.description() {
            output.push_str(&format!("{}   Description: {}\n", indent, desc));
        }
        if let Some(source) = step.annotations.source() {
            output.push_str(&format!("{}   Source: {}\n", indent, source));
        }

        // Body proofs (prerequisites)
        if !step.body_proofs.is_empty() {
            output.push_str(&format!("{}   Prerequisites:\n", indent));
            for (i, bp) in step.body_proofs.iter().enumerate() {
                let sub_num = format!("{}.{}", num, i + 1);
                let sub_indent = format!("{}     ", indent);
                output.push_str(&proof_node_to_natural_language(
                    bp,
                    sub_num.parse().unwrap_or(1),
                    &sub_indent,
                ));
            }
        }
    }

    output
}

/// Get human-readable rule type name
fn rule_type_name(rt: RuleType) -> &'static str {
    match rt {
        RuleType::Fact => "fact",
        RuleType::Strict => "strict rule",
        RuleType::Defeasible => "defeasible rule",
        RuleType::Defeater => "defeater",
    }
}

/// Convert proof node to JSON
fn proof_node_to_json(node: &ProofNode) -> serde_json::Value {
    serde_json::json!({
        "literal": node.literal.to_string(),
        "derivation_type": match node.derivation_type {
            DerivationType::Definite => "definite",
            DerivationType::Defeasible => "defeasible",
        },
        "proof_step": node.proof_step.as_ref().map(|step| serde_json::json!({
            "rule_label": step.rule_label,
            "rule_type": match step.rule_type {
                RuleType::Fact => "fact",
                RuleType::Strict => "strict",
                RuleType::Defeasible => "defeasible",
                RuleType::Defeater => "defeater",
            },
            "rule_text": step.rule_text,
            "body_proofs": step.body_proofs.iter()
                .map(proof_node_to_json)
                .collect::<Vec<_>>(),
        })),
    })
}

/// Convert blocked proof to JSON
fn blocked_proof_to_json(blocked: &BlockedProof) -> serde_json::Value {
    serde_json::json!({
        "literal": blocked.literal.to_string(),
        "rule_label": blocked.rule_label,
        "reason": blocked.reason.to_string(),
        "blocking_rule": blocked.blocking_rule,
        "explanation": blocked.explanation,
    })
}

/// Convert conflict resolution to JSON
fn conflict_to_json(conflict: &ConflictResolution) -> serde_json::Value {
    serde_json::json!({
        "winning_rule": conflict.winning_rule,
        "losing_rule": conflict.losing_rule,
        "resolution_type": conflict.resolution_type.to_string(),
        "superiority_label": conflict.superiority_label,
    })
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
    fn test_proof_step() {
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies");
        assert_eq!(step.rule_label, "r1");
        assert_eq!(step.rule_type, RuleType::Defeasible);
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
    fn test_conflict_resolution() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority)
            .with_superiority("sup1");

        assert_eq!(conflict.winning_rule, "r2");
        assert_eq!(conflict.losing_rule, "r1");
        assert_eq!(conflict.superiority_label, Some("sup1".to_string()));
    }

    #[test]
    fn test_explanation_natural_language() {
        let step = ProofStep::new("f1", RuleType::Fact, ">> bird");
        let proof =
            ProofNode::new(Literal::simple("bird"), DerivationType::Definite).with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("bird"))
                .with_proof(proof);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("bird"));
        assert!(nl.contains("+D")); // Uses symbol
        assert!(nl.contains("fact"));
    }

    #[test]
    fn test_explanation_with_conflict() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority);

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_conflicts(vec![conflict]);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("Conflict Resolutions"));
        assert!(nl.contains("r2"));
        assert!(nl.contains("r1"));
    }

    #[test]
    fn test_explanation_json() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let json = explanation.to_json();
        assert_eq!(json["conclusion_type"], "+d");
        assert_eq!(json["literal"], "flies");
    }
}
