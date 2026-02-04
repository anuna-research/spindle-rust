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

    /// Convert to JSON-LD representation with semantic annotations
    ///
    /// Returns a JSON-LD document with:
    /// - @context: Defines vocabulary (spindle, prov, rdfs namespaces)
    /// - @type: "spindle:Explanation"
    /// - Semantic annotations for provenance tracking
    pub fn to_jsonld(&self) -> serde_json::Value {
        let context = serde_json::json!({
            "spindle": "https://spindle.dev/ontology#",
            "prov": "http://www.w3.org/ns/prov#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "xsd": "http://www.w3.org/2001/XMLSchema#",
            "conclusionType": "spindle:conclusionType",
            "literal": "spindle:literal",
            "proofTree": "spindle:proofTree",
            "derivationType": "spindle:derivationType",
            "ruleLabel": "spindle:ruleLabel",
            "ruleType": "spindle:ruleType",
            "blockedAlternatives": "spindle:blockedAlternatives",
            "conflictsResolved": "spindle:conflictsResolved",
            "winningRule": "spindle:winningRule",
            "losingRule": "spindle:losingRule",
            "resolutionType": "spindle:resolutionType",
            "blockReason": "spindle:blockReason",
            "wasGeneratedBy": "prov:wasGeneratedBy",
            "wasAttributedTo": "prov:wasAttributedTo"
        });

        let mut doc = serde_json::json!({
            "@context": context,
            "@type": "spindle:Explanation",
            "conclusionType": self.conclusion_type.symbol(),
            "literal": self.literal.to_string(),
        });

        // Add proof tree with semantic annotations
        if let Some(ref proof) = self.proof_tree {
            doc["proofTree"] = proof_node_to_jsonld(proof);
        }

        // Add blocked alternatives
        if !self.blocked_alternatives.is_empty() {
            doc["blockedAlternatives"] = self
                .blocked_alternatives
                .iter()
                .map(blocked_proof_to_jsonld)
                .collect::<Vec<_>>()
                .into();
        }

        // Add conflict resolutions
        if !self.conflicts_resolved.is_empty() {
            doc["conflictsResolved"] = self
                .conflicts_resolved
                .iter()
                .map(conflict_to_jsonld)
                .collect::<Vec<_>>()
                .into();
        }

        doc
    }

    /// Convert to DOT graph format for visualization
    ///
    /// Generates a DOT language representation that can be rendered
    /// with Graphviz or similar tools. The graph shows:
    /// - Proof nodes as boxes (blue for definite, green for defeasible)
    /// - Rule applications as edges
    /// - Blocked alternatives as red dashed nodes
    /// - Conflict resolutions as orange diamond nodes
    pub fn to_dot(&self) -> String {
        let mut output = String::new();
        let mut node_counter = 0;

        output.push_str("digraph Explanation {\n");
        output.push_str("  rankdir=BT;\n"); // Bottom-to-top layout
        output.push_str("  node [fontname=\"Helvetica\"];\n");
        output.push_str("  edge [fontname=\"Helvetica\"];\n\n");

        // Title node
        let escaped_literal = escape_dot_label(&self.literal.to_string());
        output.push_str(&format!(
            "  title [label=\"{} {}\" shape=plaintext fontsize=14 fontcolor=black];\n\n",
            self.conclusion_type, escaped_literal
        ));

        // Render proof tree
        if let Some(ref proof) = self.proof_tree {
            let root_id = render_proof_node_to_dot(proof, &mut output, &mut node_counter);
            output.push_str(&format!("  title -> n{root_id} [style=invis];\n"));
        }

        // Render blocked alternatives
        if !self.blocked_alternatives.is_empty() {
            output.push_str("\n  // Blocked alternatives\n");
            output.push_str("  subgraph cluster_blocked {\n");
            output.push_str("    label=\"Blocked Alternatives\";\n");
            output.push_str("    style=dashed;\n");
            output.push_str("    color=red;\n");

            for blocked in &self.blocked_alternatives {
                node_counter += 1;
                let escaped_lit = escape_dot_label(&blocked.literal.to_string());
                let escaped_rule = escape_dot_label(&blocked.rule_label);
                let escaped_reason = escape_dot_label(&blocked.reason.to_string());
                output.push_str(&format!(
                    "    b{node_counter} [label=\"{escaped_lit}\\n(rule: {escaped_rule})\\nblocked: {escaped_reason}\" shape=box style=\"dashed,filled\" fillcolor=\"#ffcccc\"];\n"
                ));
            }
            output.push_str("  }\n");
        }

        // Render conflict resolutions
        if !self.conflicts_resolved.is_empty() {
            output.push_str("\n  // Conflict resolutions\n");
            output.push_str("  subgraph cluster_conflicts {\n");
            output.push_str("    label=\"Conflict Resolutions\";\n");
            output.push_str("    style=dashed;\n");
            output.push_str("    color=orange;\n");

            for conflict in &self.conflicts_resolved {
                node_counter += 1;
                let escaped_winner = escape_dot_label(&conflict.winning_rule);
                let escaped_loser = escape_dot_label(&conflict.losing_rule);
                let escaped_resolution = escape_dot_label(&conflict.resolution_type.to_string());
                output.push_str(&format!(
                    "    c{node_counter} [label=\"{escaped_winner} > {escaped_loser}\\n({escaped_resolution})\" shape=diamond style=filled fillcolor=\"#ffe0b3\"];\n"
                ));
            }
            output.push_str("  }\n");
        }

        output.push_str("}\n");
        output
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

    if let Some(rule_label) = &conclusion.rule_label
        && let Some(rule) = theory.get_rule(rule_label) {
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
                    })
                        && let Some(body_expl) = explain(theory, &matching_conc.literal)
                        && let Some(body_tree) = body_expl.proof_tree {
                        body_proofs.push(body_tree);
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
            output.push_str(&format!("{indent}   Description: {desc}\n"));
        }
        if let Some(source) = step.annotations.source() {
            output.push_str(&format!("{indent}   Source: {source}\n"));
        }

        // Body proofs (prerequisites)
        if !step.body_proofs.is_empty() {
            output.push_str(&format!("{indent}   Prerequisites:\n"));
            for (i, bp) in step.body_proofs.iter().enumerate() {
                let sub_num = format!("{}.{}", num, i + 1);
                let sub_indent = format!("{indent}     ");
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

/// Convert proof node to JSON-LD with semantic annotations
fn proof_node_to_jsonld(node: &ProofNode) -> serde_json::Value {
    let mut json = serde_json::json!({
        "@type": "spindle:ProofNode",
        "literal": node.literal.to_string(),
        "derivationType": match node.derivation_type {
            DerivationType::Definite => "definite",
            DerivationType::Defeasible => "defeasible",
        },
    });

    if let Some(ref step) = node.proof_step {
        let mut step_json = serde_json::json!({
            "@type": "spindle:ProofStep",
            "ruleLabel": step.rule_label,
            "ruleType": match step.rule_type {
                RuleType::Fact => "fact",
                RuleType::Strict => "strict",
                RuleType::Defeasible => "defeasible",
                RuleType::Defeater => "defeater",
            },
            "ruleText": step.rule_text,
        });

        // Add annotations with provenance
        if !step.annotations.is_empty() {
            if let Some(source) = step.annotations.source() {
                step_json["wasAttributedTo"] = serde_json::json!(source);
            }
            if let Some(desc) = step.annotations.description() {
                step_json["rdfs:comment"] = serde_json::json!(desc);
            }
            if let Some(conf) = step.annotations.confidence() {
                step_json["spindle:confidence"] = serde_json::json!(conf);
            }
            if let Some(id) = &step.annotations.id {
                step_json["@id"] = serde_json::json!(id);
            }
        }

        // Recursively add body proofs
        if !step.body_proofs.is_empty() {
            step_json["bodyProofs"] = step
                .body_proofs
                .iter()
                .map(proof_node_to_jsonld)
                .collect::<Vec<_>>()
                .into();
        }

        json["proofStep"] = step_json;
    }

    json
}

/// Convert blocked proof to JSON-LD
fn blocked_proof_to_jsonld(blocked: &BlockedProof) -> serde_json::Value {
    let mut json = serde_json::json!({
        "@type": "spindle:BlockedProof",
        "literal": blocked.literal.to_string(),
        "ruleLabel": blocked.rule_label,
        "blockReason": blocked.reason.to_string(),
        "rdfs:comment": blocked.explanation,
    });

    if let Some(ref blocking) = blocked.blocking_rule {
        json["blockedBy"] = serde_json::json!(blocking);
    }

    json
}

/// Convert conflict resolution to JSON-LD
fn conflict_to_jsonld(conflict: &ConflictResolution) -> serde_json::Value {
    let mut json = serde_json::json!({
        "@type": "spindle:ConflictResolution",
        "winningRule": conflict.winning_rule,
        "losingRule": conflict.losing_rule,
        "resolutionType": conflict.resolution_type.to_string(),
    });

    if let Some(ref sup) = conflict.superiority_label {
        json["superiorityLabel"] = serde_json::json!(sup);
    }

    json
}

/// Escape special characters for DOT labels
fn escape_dot_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('<', "\\<")
        .replace('>', "\\>")
        .replace('{', "\\{")
        .replace('}', "\\}")
}

/// Render a proof node to DOT format, returning the node ID
fn render_proof_node_to_dot(
    node: &ProofNode,
    output: &mut String,
    counter: &mut usize,
) -> usize {
    *counter += 1;
    let node_id = *counter;

    let color = match node.derivation_type {
        DerivationType::Definite => "#cce5ff", // Light blue
        DerivationType::Defeasible => "#d4edda", // Light green
    };

    let escaped_literal = escape_dot_label(&node.literal.to_string());

    // Create node label
    let label = if let Some(ref step) = node.proof_step {
        let rule_type_str = match step.rule_type {
            RuleType::Fact => "fact",
            RuleType::Strict => "strict",
            RuleType::Defeasible => "defeasible",
            RuleType::Defeater => "defeater",
        };
        let escaped_label = escape_dot_label(&step.rule_label);
        format!(
            "{escaped_literal}\\n[{rule_type_str}: {escaped_label}]"
        )
    } else {
        escaped_literal
    };

    output.push_str(&format!(
        "  n{node_id} [label=\"{label}\" shape=box style=filled fillcolor=\"{color}\"];\n"
    ));

    // Render body proofs and add edges
    if let Some(ref step) = node.proof_step {
        for body_proof in &step.body_proofs {
            let child_id = render_proof_node_to_dot(body_proof, output, counter);
            let escaped_rule = escape_dot_label(&step.rule_label);
            output.push_str(&format!(
                "  n{child_id} -> n{node_id} [label=\"{escaped_rule}\"];\n"
            ));
        }
    }

    node_id
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Basic struct tests
    // ==========================================================================

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
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies")
            .with_annotations(annots);

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
        assert_eq!(ResolutionType::DefinitePriority.to_string(), "definite priority");
        assert_eq!(ResolutionType::TeamDefeat.to_string(), "team defeat");
    }

    // ==========================================================================
    // Natural language output tests
    // ==========================================================================

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
    fn test_natural_language_defeasible_derivation() {
        let body_step = ProofStep::new("f1", RuleType::Fact, ">> bird");
        let body_proof = ProofNode::new(Literal::simple("bird"), DerivationType::Definite)
            .with_proof_step(body_step);

        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies")
            .with_body_proofs(vec![body_proof]);
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("flies"));
        assert!(nl.contains("+d")); // Defeasibly provable symbol
        assert!(nl.contains("derived defeasibly"));
        assert!(nl.contains("defeasible rule"));
        assert!(nl.contains("Prerequisites"));
        assert!(nl.contains("bird"));
    }

    #[test]
    fn test_natural_language_with_annotations() {
        let annots = Annotations::with_entries(vec![
            ("description", "Birds typically fly"),
            ("source", "ornithology-textbook"),
        ]);
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies")
            .with_annotations(annots);
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("Description: Birds typically fly"));
        assert!(nl.contains("Source: ornithology-textbook"));
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
    fn test_natural_language_with_blocked_alternatives() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Rule r2 is superior and concludes ~flies",
        )
        .with_blocking_rule("r2");

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_blocked(vec![blocked]);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("Blocked Alternatives"));
        assert!(nl.contains("r1"));
        assert!(nl.contains("superiority"));
        assert!(nl.contains("Rule r2 is superior"));
    }

    #[test]
    fn test_natural_language_no_derivation() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyNotProvable, Literal::simple("flies"));

        let nl = explanation.to_natural_language();
        assert!(nl.contains("-d")); // Not provable symbol
        assert!(nl.contains("could not be proven defeasibly"));
    }

    #[test]
    fn test_natural_language_conclusion_type_explanations() {
        // Test all conclusion type explanations
        let exp_dp = Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("p"));
        assert!(exp_dp.to_natural_language().contains("strict rules and facts"));

        let exp_dnp = Explanation::new(ConclusionType::DefinitelyNotProvable, Literal::simple("p"));
        assert!(exp_dnp.to_natural_language().contains("cannot be proven using strict rules"));

        let exp_dfp = Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("p"));
        assert!(exp_dfp.to_natural_language().contains("defeasible rules"));

        let exp_dfnp = Explanation::new(ConclusionType::DefeasiblyNotProvable, Literal::simple("p"));
        assert!(exp_dfnp.to_natural_language().contains("could not be proven defeasibly"));
    }

    // ==========================================================================
    // JSON output tests
    // ==========================================================================

    #[test]
    fn test_explanation_json() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let json = explanation.to_json();
        assert_eq!(json["conclusion_type"], "+d");
        assert_eq!(json["literal"], "flies");
    }

    #[test]
    fn test_json_with_proof_tree() {
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let json = explanation.to_json();
        assert!(json["proof_tree"].is_object());
        assert_eq!(json["proof_tree"]["literal"], "flies");
        assert_eq!(json["proof_tree"]["derivation_type"], "defeasible");
        assert_eq!(json["proof_tree"]["proof_step"]["rule_label"], "r1");
        assert_eq!(json["proof_tree"]["proof_step"]["rule_type"], "defeasible");
    }

    #[test]
    fn test_json_with_blocked_alternatives() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Blocked by r2",
        )
        .with_blocking_rule("r2");

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_blocked(vec![blocked]);

        let json = explanation.to_json();
        assert!(json["blocked_alternatives"].is_array());
        assert_eq!(json["blocked_alternatives"][0]["rule_label"], "r1");
        assert_eq!(json["blocked_alternatives"][0]["reason"], "superiority");
        assert_eq!(json["blocked_alternatives"][0]["blocking_rule"], "r2");
    }

    #[test]
    fn test_json_with_conflicts() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority)
            .with_superiority("sup1");

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_conflicts(vec![conflict]);

        let json = explanation.to_json();
        assert!(json["conflicts_resolved"].is_array());
        assert_eq!(json["conflicts_resolved"][0]["winning_rule"], "r2");
        assert_eq!(json["conflicts_resolved"][0]["losing_rule"], "r1");
        assert_eq!(json["conflicts_resolved"][0]["resolution_type"], "superiority");
        assert_eq!(json["conflicts_resolved"][0]["superiority_label"], "sup1");
    }

    // ==========================================================================
    // JSON-LD output tests
    // ==========================================================================

    #[test]
    fn test_jsonld_context() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let jsonld = explanation.to_jsonld();

        // Verify @context exists and has required namespaces
        assert!(jsonld["@context"].is_object());
        assert_eq!(jsonld["@context"]["spindle"], "https://spindle.dev/ontology#");
        assert_eq!(jsonld["@context"]["prov"], "http://www.w3.org/ns/prov#");
        assert_eq!(jsonld["@context"]["rdfs"], "http://www.w3.org/2000/01/rdf-schema#");
        assert_eq!(jsonld["@context"]["xsd"], "http://www.w3.org/2001/XMLSchema#");
    }

    #[test]
    fn test_jsonld_type() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let jsonld = explanation.to_jsonld();
        assert_eq!(jsonld["@type"], "spindle:Explanation");
    }

    #[test]
    fn test_jsonld_basic_fields() {
        let explanation =
            Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("bird"));

        let jsonld = explanation.to_jsonld();
        assert_eq!(jsonld["conclusionType"], "+D");
        assert_eq!(jsonld["literal"], "bird");
    }

    #[test]
    fn test_jsonld_with_proof_tree() {
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let jsonld = explanation.to_jsonld();

        assert!(jsonld["proofTree"].is_object());
        assert_eq!(jsonld["proofTree"]["@type"], "spindle:ProofNode");
        assert_eq!(jsonld["proofTree"]["literal"], "flies");
        assert_eq!(jsonld["proofTree"]["derivationType"], "defeasible");

        // Verify proof step
        assert!(jsonld["proofTree"]["proofStep"].is_object());
        assert_eq!(jsonld["proofTree"]["proofStep"]["@type"], "spindle:ProofStep");
        assert_eq!(jsonld["proofTree"]["proofStep"]["ruleLabel"], "r1");
        assert_eq!(jsonld["proofTree"]["proofStep"]["ruleType"], "defeasible");
    }

    #[test]
    fn test_jsonld_with_annotations_provenance() {
        let mut annots = Annotations::with_entries(vec![
            ("source", "expert-dr-smith"),
            ("description", "Birds typically fly"),
            ("confidence", "0.9"),
        ]);
        annots.id = Some("https://example.org/rules/r1".to_string());

        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies")
            .with_annotations(annots);
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let jsonld = explanation.to_jsonld();
        let proof_step = &jsonld["proofTree"]["proofStep"];

        // Verify provenance annotations
        assert_eq!(proof_step["wasAttributedTo"], "expert-dr-smith");
        assert_eq!(proof_step["rdfs:comment"], "Birds typically fly");
        assert_eq!(proof_step["spindle:confidence"], "0.9");
        assert_eq!(proof_step["@id"], "https://example.org/rules/r1");
    }

    #[test]
    fn test_jsonld_with_nested_body_proofs() {
        // Create a two-level proof: flies <- bird <- penguin
        let penguin_step = ProofStep::new("f1", RuleType::Fact, ">> penguin");
        let penguin_proof = ProofNode::new(Literal::simple("penguin"), DerivationType::Definite)
            .with_proof_step(penguin_step);

        let bird_step = ProofStep::new("s1", RuleType::Strict, "penguin -> bird")
            .with_body_proofs(vec![penguin_proof]);
        let bird_proof = ProofNode::new(Literal::simple("bird"), DerivationType::Definite)
            .with_proof_step(bird_step);

        let flies_step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies")
            .with_body_proofs(vec![bird_proof]);
        let flies_proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(flies_step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(flies_proof);

        let jsonld = explanation.to_jsonld();

        // Navigate to nested proofs
        let body_proofs = &jsonld["proofTree"]["proofStep"]["bodyProofs"];
        assert!(body_proofs.is_array());
        assert_eq!(body_proofs[0]["literal"], "bird");
        assert_eq!(body_proofs[0]["@type"], "spindle:ProofNode");

        // Even deeper nesting
        let nested_body = &body_proofs[0]["proofStep"]["bodyProofs"];
        assert!(nested_body.is_array());
        assert_eq!(nested_body[0]["literal"], "penguin");
    }

    #[test]
    fn test_jsonld_with_blocked_alternatives() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Blocked by superior rule r2",
        )
        .with_blocking_rule("r2");

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_blocked(vec![blocked]);

        let jsonld = explanation.to_jsonld();

        assert!(jsonld["blockedAlternatives"].is_array());
        let blocked_alt = &jsonld["blockedAlternatives"][0];
        assert_eq!(blocked_alt["@type"], "spindle:BlockedProof");
        assert_eq!(blocked_alt["literal"], "flies");
        assert_eq!(blocked_alt["ruleLabel"], "r1");
        assert_eq!(blocked_alt["blockReason"], "superiority");
        assert_eq!(blocked_alt["rdfs:comment"], "Blocked by superior rule r2");
        assert_eq!(blocked_alt["blockedBy"], "r2");
    }

    #[test]
    fn test_jsonld_with_conflict_resolutions() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority)
            .with_superiority("sup1");

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_conflicts(vec![conflict]);

        let jsonld = explanation.to_jsonld();

        assert!(jsonld["conflictsResolved"].is_array());
        let resolved = &jsonld["conflictsResolved"][0];
        assert_eq!(resolved["@type"], "spindle:ConflictResolution");
        assert_eq!(resolved["winningRule"], "r2");
        assert_eq!(resolved["losingRule"], "r1");
        assert_eq!(resolved["resolutionType"], "superiority");
        assert_eq!(resolved["superiorityLabel"], "sup1");
    }

    #[test]
    fn test_jsonld_all_rule_types() {
        // Test that all rule types are properly serialized
        let fact_step = ProofStep::new("f1", RuleType::Fact, ">> p");
        let fact_proof = ProofNode::new(Literal::simple("p"), DerivationType::Definite)
            .with_proof_step(fact_step);
        let exp1 = Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("p"))
            .with_proof(fact_proof);
        assert_eq!(exp1.to_jsonld()["proofTree"]["proofStep"]["ruleType"], "fact");

        let strict_step = ProofStep::new("s1", RuleType::Strict, "p -> q");
        let strict_proof = ProofNode::new(Literal::simple("q"), DerivationType::Definite)
            .with_proof_step(strict_step);
        let exp2 = Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("q"))
            .with_proof(strict_proof);
        assert_eq!(exp2.to_jsonld()["proofTree"]["proofStep"]["ruleType"], "strict");

        let def_step = ProofStep::new("r1", RuleType::Defeasible, "p => r");
        let def_proof = ProofNode::new(Literal::simple("r"), DerivationType::Defeasible)
            .with_proof_step(def_step);
        let exp3 = Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("r"))
            .with_proof(def_proof);
        assert_eq!(exp3.to_jsonld()["proofTree"]["proofStep"]["ruleType"], "defeasible");

        let defeater_step = ProofStep::new("d1", RuleType::Defeater, "p ~> s");
        let defeater_proof = ProofNode::new(Literal::simple("s"), DerivationType::Defeasible)
            .with_proof_step(defeater_step);
        let exp4 = Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("s"))
            .with_proof(defeater_proof);
        assert_eq!(exp4.to_jsonld()["proofTree"]["proofStep"]["ruleType"], "defeater");
    }

    // ==========================================================================
    // DOT graph output tests
    // ==========================================================================

    #[test]
    fn test_dot_basic_structure() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let dot = explanation.to_dot();

        // Check basic DOT structure
        assert!(dot.starts_with("digraph Explanation {"));
        assert!(dot.ends_with("}\n"));
        assert!(dot.contains("rankdir=BT")); // Bottom-to-top layout
        assert!(dot.contains("node [fontname=\"Helvetica\"]"));
    }

    #[test]
    fn test_dot_title_node() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let dot = explanation.to_dot();

        // Title should contain conclusion type and literal
        assert!(dot.contains("title [label=\"+d flies\""));
        assert!(dot.contains("shape=plaintext"));
    }

    #[test]
    fn test_dot_with_proof_node() {
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let dot = explanation.to_dot();

        // Should have a node for the literal
        assert!(dot.contains("flies"));
        assert!(dot.contains("[defeasible: r1]"));
        assert!(dot.contains("shape=box"));
        assert!(dot.contains("style=filled"));
        // Green color for defeasible
        assert!(dot.contains("#d4edda"));
    }

    #[test]
    fn test_dot_definite_derivation_color() {
        let step = ProofStep::new("f1", RuleType::Fact, ">> bird");
        let proof = ProofNode::new(Literal::simple("bird"), DerivationType::Definite)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("bird"))
                .with_proof(proof);

        let dot = explanation.to_dot();

        // Blue color for definite
        assert!(dot.contains("#cce5ff"));
    }

    #[test]
    fn test_dot_with_body_proofs() {
        let body_step = ProofStep::new("f1", RuleType::Fact, ">> bird");
        let body_proof = ProofNode::new(Literal::simple("bird"), DerivationType::Definite)
            .with_proof_step(body_step);

        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies")
            .with_body_proofs(vec![body_proof]);
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let dot = explanation.to_dot();

        // Should have edge between nodes
        assert!(dot.contains("->")); // Edge notation
        assert!(dot.contains("[label=\"r1\"]")); // Edge label
        assert!(dot.contains("bird"));
        assert!(dot.contains("flies"));
    }

    #[test]
    fn test_dot_with_blocked_alternatives() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Blocked by r2",
        );

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_blocked(vec![blocked]);

        let dot = explanation.to_dot();

        // Check blocked alternatives cluster
        assert!(dot.contains("subgraph cluster_blocked"));
        assert!(dot.contains("label=\"Blocked Alternatives\""));
        assert!(dot.contains("style=dashed"));
        assert!(dot.contains("color=red"));
        assert!(dot.contains("#ffcccc")); // Light red fill
        assert!(dot.contains("blocked: superiority"));
        assert!(dot.contains("(rule: r1)"));
    }

    #[test]
    fn test_dot_with_conflict_resolutions() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_conflicts(vec![conflict]);

        let dot = explanation.to_dot();

        // Check conflicts cluster
        assert!(dot.contains("subgraph cluster_conflicts"));
        assert!(dot.contains("label=\"Conflict Resolutions\""));
        assert!(dot.contains("color=orange"));
        assert!(dot.contains("shape=diamond")); // Conflict nodes are diamonds
        assert!(dot.contains("#ffe0b3")); // Orange fill
        assert!(dot.contains("r2 > r1"));
        assert!(dot.contains("(superiority)"));
    }

    #[test]
    fn test_dot_escaping_special_characters() {
        // Test that special characters in labels are properly escaped
        let step = ProofStep::new("r<1>", RuleType::Defeasible, "bird \"tweety\" => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let dot = explanation.to_dot();

        // Special characters should be escaped
        assert!(dot.contains("r\\<1\\>")); // Escaped < and >
    }

    #[test]
    fn test_dot_with_negated_literal() {
        let step = ProofStep::new("r2", RuleType::Defeasible, "penguin => ~flies");
        let proof = ProofNode::new(Literal::negated("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_proof(proof);

        let dot = explanation.to_dot();

        assert!(dot.contains("~flies"));
    }

    #[test]
    fn test_dot_complex_proof_tree() {
        // Create a more complex proof tree with multiple levels
        let penguin_step = ProofStep::new("f1", RuleType::Fact, ">> penguin");
        let penguin_proof = ProofNode::new(Literal::simple("penguin"), DerivationType::Definite)
            .with_proof_step(penguin_step);

        let bird_step = ProofStep::new("s1", RuleType::Strict, "penguin -> bird")
            .with_body_proofs(vec![penguin_proof]);
        let bird_proof = ProofNode::new(Literal::simple("bird"), DerivationType::Definite)
            .with_proof_step(bird_step);

        let not_flies_step = ProofStep::new("r2", RuleType::Defeasible, "penguin => ~flies")
            .with_body_proofs(vec![bird_proof.clone()]);
        let not_flies_proof =
            ProofNode::new(Literal::negated("flies"), DerivationType::Defeasible)
                .with_proof_step(not_flies_step);

        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "r2 > r1 via superiority",
        );

        let conflict =
            ConflictResolution::new("r2", "r1", ResolutionType::Superiority).with_superiority("s1");

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_proof(not_flies_proof)
                .with_blocked(vec![blocked])
                .with_conflicts(vec![conflict]);

        let dot = explanation.to_dot();

        // Verify all components are present
        assert!(dot.contains("penguin"));
        assert!(dot.contains("bird"));
        assert!(dot.contains("~flies"));
        assert!(dot.contains("cluster_blocked"));
        assert!(dot.contains("cluster_conflicts"));

        // Verify edges exist (at least some -> notation)
        let edge_count = dot.matches("->").count();
        assert!(edge_count >= 2, "Expected at least 2 edges, found {}", edge_count);
    }

    #[test]
    fn test_dot_multiple_blocked_alternatives() {
        let blocked1 = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Blocked by r2",
        );
        let blocked2 = BlockedProof::new(
            Literal::simple("flies"),
            "r3",
            BlockReason::Defeater,
            "Blocked by defeater d1",
        );

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_blocked(vec![blocked1, blocked2]);

        let dot = explanation.to_dot();

        // Both blocked alternatives should be present
        assert!(dot.contains("(rule: r1)"));
        assert!(dot.contains("(rule: r3)"));
        assert!(dot.contains("blocked: superiority"));
        assert!(dot.contains("blocked: defeater"));
    }

    #[test]
    fn test_dot_multiple_conflict_resolutions() {
        let conflict1 = ConflictResolution::new("r2", "r1", ResolutionType::Superiority);
        let conflict2 = ConflictResolution::new("r4", "r3", ResolutionType::TeamDefeat);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_conflicts(vec![conflict1, conflict2]);

        let dot = explanation.to_dot();

        assert!(dot.contains("r2 > r1"));
        assert!(dot.contains("r4 > r3"));
        assert!(dot.contains("(superiority)"));
        assert!(dot.contains("(team defeat)"));
    }

    // ==========================================================================
    // Annotation preservation tests
    // ==========================================================================

    #[test]
    fn test_annotation_preservation_rule_labels() {
        let step = ProofStep::new("bird_flies_rule", RuleType::Defeasible, "bird => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        // Check JSON preserves rule label
        let json = explanation.to_json();
        assert_eq!(json["proof_tree"]["proof_step"]["rule_label"], "bird_flies_rule");

        // Check JSON-LD preserves rule label
        let jsonld = explanation.to_jsonld();
        assert_eq!(jsonld["proofTree"]["proofStep"]["ruleLabel"], "bird_flies_rule");

        // Check natural language mentions rule label
        let nl = explanation.to_natural_language();
        assert!(nl.contains("bird_flies_rule"));

        // Check DOT preserves rule label
        let dot = explanation.to_dot();
        assert!(dot.contains("bird_flies_rule"));
    }

    #[test]
    fn test_annotation_preservation_source_tracking() {
        let annots = Annotations::with_entries(vec![
            ("source", "legal-statute-42-usc-1983"),
            ("dc:source", "Civil Rights Act"),
        ]);
        let step = ProofStep::new("r1", RuleType::Defeasible, "violation => liable")
            .with_annotations(annots);
        let proof = ProofNode::new(Literal::simple("liable"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("liable"))
                .with_proof(proof);

        // Natural language should show source
        let nl = explanation.to_natural_language();
        assert!(nl.contains("Source: legal-statute-42-usc-1983"));

        // JSON-LD should have provenance annotation
        let jsonld = explanation.to_jsonld();
        assert_eq!(
            jsonld["proofTree"]["proofStep"]["wasAttributedTo"],
            "legal-statute-42-usc-1983"
        );
    }

    #[test]
    fn test_annotation_preservation_multiple_annotations() {
        let mut annots = Annotations::with_entries(vec![
            ("source", "expert-opinion"),
            ("description", "Standard medical practice guideline"),
            ("confidence", "0.85"),
        ]);
        annots.id = Some("urn:guideline:med-123".to_string());

        let step = ProofStep::new("med_rule", RuleType::Defeasible, "symptom => diagnosis")
            .with_annotations(annots);
        let proof = ProofNode::new(Literal::simple("diagnosis"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("diagnosis"))
                .with_proof(proof);

        // Verify all annotations in JSON-LD
        let jsonld = explanation.to_jsonld();
        let step_jsonld = &jsonld["proofTree"]["proofStep"];

        assert_eq!(step_jsonld["@id"], "urn:guideline:med-123");
        assert_eq!(step_jsonld["wasAttributedTo"], "expert-opinion");
        assert_eq!(step_jsonld["rdfs:comment"], "Standard medical practice guideline");
        assert_eq!(step_jsonld["spindle:confidence"], "0.85");

        // Verify in natural language
        let nl = explanation.to_natural_language();
        assert!(nl.contains("Source: expert-opinion"));
        assert!(nl.contains("Description: Standard medical practice guideline"));
    }

    #[test]
    fn test_annotation_preservation_blocked_proof_info() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "bird_flies_rule",
            BlockReason::Superiority,
            "The penguin exception rule (penguin_no_fly) is superior",
        )
        .with_blocking_rule("penguin_no_fly");

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_blocked(vec![blocked]);

        // JSON preserves all info
        let json = explanation.to_json();
        assert_eq!(json["blocked_alternatives"][0]["rule_label"], "bird_flies_rule");
        assert_eq!(json["blocked_alternatives"][0]["blocking_rule"], "penguin_no_fly");
        assert!(json["blocked_alternatives"][0]["explanation"]
            .as_str()
            .unwrap()
            .contains("penguin exception"));

        // JSON-LD preserves all info
        let jsonld = explanation.to_jsonld();
        assert_eq!(jsonld["blockedAlternatives"][0]["ruleLabel"], "bird_flies_rule");
        assert_eq!(jsonld["blockedAlternatives"][0]["blockedBy"], "penguin_no_fly");

        // Natural language preserves all info
        let nl = explanation.to_natural_language();
        assert!(nl.contains("bird_flies_rule"));
        assert!(nl.contains("penguin exception"));
    }

    #[test]
    fn test_annotation_preservation_conflict_info() {
        let conflict = ConflictResolution::new(
            "specific_penguin_rule",
            "general_bird_rule",
            ResolutionType::Superiority,
        )
        .with_superiority("penguin_beats_bird_sup");

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::negated("flies"))
                .with_conflicts(vec![conflict]);

        // JSON preserves all info
        let json = explanation.to_json();
        assert_eq!(json["conflicts_resolved"][0]["winning_rule"], "specific_penguin_rule");
        assert_eq!(json["conflicts_resolved"][0]["losing_rule"], "general_bird_rule");
        assert_eq!(
            json["conflicts_resolved"][0]["superiority_label"],
            "penguin_beats_bird_sup"
        );

        // JSON-LD preserves all info
        let jsonld = explanation.to_jsonld();
        assert_eq!(jsonld["conflictsResolved"][0]["winningRule"], "specific_penguin_rule");
        assert_eq!(jsonld["conflictsResolved"][0]["losingRule"], "general_bird_rule");
        assert_eq!(
            jsonld["conflictsResolved"][0]["superiorityLabel"],
            "penguin_beats_bird_sup"
        );

        // Natural language preserves all info
        let nl = explanation.to_natural_language();
        assert!(nl.contains("specific_penguin_rule"));
        assert!(nl.contains("general_bird_rule"));
    }

    #[test]
    fn test_escape_dot_label() {
        assert_eq!(escape_dot_label("simple"), "simple");
        assert_eq!(escape_dot_label("with\"quotes"), "with\\\"quotes");
        assert_eq!(escape_dot_label("with<angle>brackets"), "with\\<angle\\>brackets");
        assert_eq!(escape_dot_label("with{curly}braces"), "with\\{curly\\}braces");
        assert_eq!(escape_dot_label("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_dot_label("with\nnewline"), "with\\nnewline");
    }

    // ==========================================================================
    // explain() function tests
    // ==========================================================================

    #[test]
    fn test_explain_simple_fact() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("bird");

        let result = explain(&theory, &Literal::simple("bird"));
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert_eq!(explanation.conclusion_type, ConclusionType::DefinitelyProvable);
        assert_eq!(explanation.literal.name(), "bird");
    }

    #[test]
    fn test_explain_defeasible_chain() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let result = explain(&theory, &Literal::simple("flies"));
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert_eq!(explanation.conclusion_type, ConclusionType::DefeasiblyProvable);
        assert_eq!(explanation.literal.name(), "flies");
        // Should have proof tree
        assert!(explanation.proof_tree.is_some());
    }

    #[test]
    fn test_explain_not_provable() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("bird");

        // Try to explain something that isn't provable
        let result = explain(&theory, &Literal::simple("nonexistent"));
        assert!(result.is_none());
    }

    #[test]
    fn test_explain_strict_rule_chain() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");
        theory.add_strict_rule(&["q"], "r");

        let result = explain(&theory, &Literal::simple("r"));
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert_eq!(explanation.conclusion_type, ConclusionType::DefinitelyProvable);
    }

    #[test]
    fn test_explain_negated_literal() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("~guilty");

        let result = explain(&theory, &Literal::negated("guilty"));
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert!(explanation.literal.is_negated());
    }

    #[test]
    fn test_explain_with_body_proofs() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_defeasible_rule(&["a", "b"], "c");

        let result = explain(&theory, &Literal::simple("c"));
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert!(explanation.proof_tree.is_some());
        let tree = explanation.proof_tree.unwrap();
        // Should have proof step with body proofs
        if let Some(step) = &tree.proof_step {
            // Body proofs should have a and b
            assert!(!step.body_proofs.is_empty());
        }
    }

    #[test]
    fn test_explanation_with_blocked_builder() {
        let blocked = BlockedProof::new(
            Literal::simple("x"),
            "r1",
            BlockReason::Conflict,
            "test",
        );
        let explanation = Explanation::new(ConclusionType::DefeasiblyNotProvable, Literal::simple("x"))
            .with_blocked(vec![blocked]);
        assert_eq!(explanation.blocked_alternatives.len(), 1);
    }

    #[test]
    fn test_explanation_with_conflicts_builder() {
        let conflict = ConflictResolution::new("r1", "r2", ResolutionType::TeamDefeat);
        let explanation = Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("x"))
            .with_conflicts(vec![conflict]);
        assert_eq!(explanation.conflicts_resolved.len(), 1);
    }

    #[test]
    fn test_dot_output_with_strict_rule() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");

        let result = explain(&theory, &Literal::simple("b"));
        assert!(result.is_some());
        let explanation = result.unwrap();

        // Convert to DOT format
        let dot = explanation.to_dot();
        assert!(dot.contains("strict"));
        assert!(dot.contains("digraph"));
    }

    #[test]
    fn test_dot_output_with_body_proofs() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("x");
        theory.add_fact("y");
        theory.add_defeasible_rule(&["x", "y"], "z");

        let result = explain(&theory, &Literal::simple("z"));
        assert!(result.is_some());
        let explanation = result.unwrap();

        // Should have body proofs shown as edges in DOT
        let dot = explanation.to_dot();
        assert!(dot.contains("->"));
    }

    #[test]
    fn test_json_output_with_strict_rule() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");

        let result = explain(&theory, &Literal::simple("q"));
        assert!(result.is_some());
        let explanation = result.unwrap();

        let json = explanation.to_json();
        // Proof tree should exist
        assert!(json.get("proof_tree").is_some());
    }

    #[test]
    fn test_blocked_proof_with_blocking_rule() {
        let blocked = BlockedProof::new(
            Literal::simple("x"),
            "r1",
            BlockReason::Superiority,
            "blocked by superior rule",
        )
        .with_blocking_rule("r2");

        assert_eq!(blocked.blocking_rule, Some("r2".to_string()));

        // Test JSON-LD output includes blocking rule
        let jsonld = blocked_proof_to_jsonld(&blocked);
        assert_eq!(jsonld["blockedBy"], "r2");
    }

    #[test]
    fn test_rule_type_name_all_types() {
        assert_eq!(rule_type_name(RuleType::Fact), "fact");
        assert_eq!(rule_type_name(RuleType::Strict), "strict rule");
        assert_eq!(rule_type_name(RuleType::Defeasible), "defeasible rule");
        assert_eq!(rule_type_name(RuleType::Defeater), "defeater");
    }

    #[test]
    fn test_proof_node_to_json_with_defeater() {
        let step = ProofStep {
            rule_label: "d1".to_string(),
            rule_type: RuleType::Defeater,
            rule_text: "d1: p ~> q".to_string(),
            body_proofs: vec![],
            annotations: Default::default(),
        };

        let node = ProofNode::new(Literal::simple("q"), DerivationType::Defeasible)
            .with_proof_step(step);

        let json = proof_node_to_json(&node);
        assert_eq!(json["proof_step"]["rule_type"], "defeater");
    }
}
