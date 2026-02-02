//! Trust Module for Source-Identity Aware Reasoning
//!
//! This module provides trust-weighted defeasible reasoning capabilities:
//!
//! - Source attribution for rules and facts
//! - Trust weighting for conclusions
//! - Trust-aware explanations
//! - Diminisher support (partial defeat)

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::literal::Literal;
use crate::conclusion::ConclusionType;

/// Trust value in range [0.0, 1.0]
pub type TrustValue = f64;

/// A source identifier for rules and facts
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Source {
    /// Source identifier (URI or name)
    pub id: String,
    /// Human-readable label
    pub label: Option<String>,
}

impl Source {
    /// Create a new source
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: None,
        }
    }

    /// Create with label
    pub fn with_label(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: Some(label.into()),
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref label) = self.label {
            write!(f, "{} ({})", label, self.id)
        } else {
            write!(f, "{}", self.id)
        }
    }
}

/// A conclusion with source attribution
#[derive(Debug, Clone)]
pub struct SourcedConclusion {
    /// The derived literal
    pub literal: Literal,
    /// Type of conclusion
    pub conclusion_type: ConclusionType,
    /// All contributing sources
    pub sources: HashSet<Source>,
    /// Rule labels that derived this
    pub derivation: Vec<String>,
}

impl SourcedConclusion {
    /// Create a new sourced conclusion
    pub fn new(literal: Literal, conclusion_type: ConclusionType) -> Self {
        Self {
            literal,
            conclusion_type,
            sources: HashSet::new(),
            derivation: Vec::new(),
        }
    }

    /// Add a source
    pub fn with_source(mut self, source: Source) -> Self {
        self.sources.insert(source);
        self
    }

    /// Add derivation rule
    pub fn with_derivation(mut self, rule_label: impl Into<String>) -> Self {
        self.derivation.push(rule_label.into());
        self
    }
}

/// A conclusion with trust-weighted degree
#[derive(Debug, Clone)]
pub struct WeightedConclusion {
    /// The derived literal
    pub literal: Literal,
    /// Type of conclusion
    pub conclusion_type: ConclusionType,
    /// Trust-weighted degree (weakest link in derivation)
    pub degree: TrustValue,
    /// All contributing sources
    pub sources: HashSet<Source>,
    /// Per-threshold evaluation results
    pub above_threshold: HashMap<String, bool>,
    /// Diminishers applied (partial defeats)
    pub diminished_by: Vec<DiminisherInfo>,
}

impl WeightedConclusion {
    /// Create a new weighted conclusion
    pub fn new(literal: Literal, conclusion_type: ConclusionType, degree: TrustValue) -> Self {
        Self {
            literal,
            conclusion_type,
            degree,
            sources: HashSet::new(),
            above_threshold: HashMap::new(),
            diminished_by: Vec::new(),
        }
    }

    /// Check if above a named threshold
    pub fn is_above_threshold(&self, name: &str) -> Option<bool> {
        self.above_threshold.get(name).copied()
    }

    /// Check if conclusion was diminished
    pub fn was_diminished(&self) -> bool {
        !self.diminished_by.is_empty()
    }
}

/// Information about a diminisher that reduced (but didn't fully defeat) a conclusion
#[derive(Debug, Clone)]
pub struct DiminisherInfo {
    /// The defeating rule label
    pub defeater_label: String,
    /// Trust degree of the defeater
    pub defeater_degree: TrustValue,
    /// Original degree before diminishment
    pub target_degree: TrustValue,
    /// Amount of reduction
    pub diminishment: TrustValue,
    /// True if fully defeated (not just diminished)
    pub full_defeat: bool,
}

impl DiminisherInfo {
    /// Create a new diminisher info
    pub fn new(
        defeater_label: impl Into<String>,
        defeater_degree: TrustValue,
        target_degree: TrustValue,
    ) -> Self {
        let diminishment = (defeater_degree * target_degree).min(target_degree);
        Self {
            defeater_label: defeater_label.into(),
            defeater_degree,
            target_degree,
            diminishment,
            full_defeat: false,
        }
    }

    /// Mark as full defeat
    pub fn as_full_defeat(mut self) -> Self {
        self.full_defeat = true;
        self
    }

    /// Get the resulting degree after diminishment
    pub fn resulting_degree(&self) -> TrustValue {
        if self.full_defeat {
            0.0
        } else {
            (self.target_degree - self.diminishment).max(0.0)
        }
    }
}

/// Trust policy configuration
#[derive(Debug, Clone, Default)]
pub struct TrustPolicy {
    /// Source -> trust value mapping
    pub trust_map: HashMap<String, TrustValue>,
    /// Named thresholds
    pub thresholds: HashMap<String, TrustValue>,
    /// Default trust for unknown sources
    pub default_trust: TrustValue,
}

impl TrustPolicy {
    /// Create a new trust policy with default trust
    pub fn new(default_trust: TrustValue) -> Self {
        Self {
            trust_map: HashMap::new(),
            thresholds: HashMap::new(),
            default_trust,
        }
    }

    /// Add a source trust mapping
    pub fn with_trust(mut self, source: impl Into<String>, trust: TrustValue) -> Self {
        self.trust_map.insert(source.into(), trust);
        self
    }

    /// Add a threshold
    pub fn with_threshold(mut self, name: impl Into<String>, value: TrustValue) -> Self {
        self.thresholds.insert(name.into(), value);
        self
    }

    /// Get trust for a source
    pub fn get_trust(&self, source: &str) -> TrustValue {
        self.trust_map.get(source).copied().unwrap_or(self.default_trust)
    }

    /// Check if a value is above a named threshold
    pub fn is_above_threshold(&self, value: TrustValue, threshold_name: &str) -> Option<bool> {
        self.thresholds.get(threshold_name).map(|&t| value >= t)
    }
}

/// A node in the trust-aware derivation tree
#[derive(Debug, Clone)]
pub struct TrustDerivationNode {
    /// The literal at this node
    pub literal: Literal,
    /// Source who asserted this
    pub source: Option<Source>,
    /// Trust value for this source
    pub trust: TrustValue,
    /// Child derivations
    pub children: Vec<TrustDerivationNode>,
}

impl TrustDerivationNode {
    /// Create a new derivation node
    pub fn new(literal: Literal, trust: TrustValue) -> Self {
        Self {
            literal,
            source: None,
            trust,
            children: Vec::new(),
        }
    }

    /// Set the source
    pub fn with_source(mut self, source: Source) -> Self {
        self.source = Some(source);
        self
    }

    /// Add children
    pub fn with_children(mut self, children: Vec<TrustDerivationNode>) -> Self {
        self.children = children;
        self
    }

    /// Compute the weakest link trust in this subtree
    pub fn weakest_link_trust(&self) -> TrustValue {
        let mut min_trust = self.trust;
        for child in &self.children {
            min_trust = min_trust.min(child.weakest_link_trust());
        }
        min_trust
    }
}

/// A trust-aware explanation
#[derive(Debug, Clone)]
pub struct TrustExplanation {
    /// The explained literal
    pub goal: Literal,
    /// Final trust-weighted degree
    pub final_degree: TrustValue,
    /// Derivation tree with trust at each node
    pub derivation_tree: Option<TrustDerivationNode>,
    /// Diminishers that affected this conclusion
    pub diminishers: Vec<DiminisherInfo>,
}

impl TrustExplanation {
    /// Create a new trust explanation
    pub fn new(goal: Literal, final_degree: TrustValue) -> Self {
        Self {
            goal,
            final_degree,
            derivation_tree: None,
            diminishers: Vec::new(),
        }
    }

    /// Set derivation tree
    pub fn with_tree(mut self, tree: TrustDerivationNode) -> Self {
        self.derivation_tree = Some(tree);
        self
    }

    /// Add diminishers
    pub fn with_diminishers(mut self, diminishers: Vec<DiminisherInfo>) -> Self {
        self.diminishers = diminishers;
        self
    }

    /// Generate natural language explanation
    pub fn to_natural_language(&self) -> String {
        let mut output = String::new();

        output.push_str(&format!(
            "Trust Explanation for \"{}\"\n",
            self.goal
        ));
        output.push_str(&format!(
            "Final trust degree: {:.2}\n\n",
            self.final_degree
        ));

        if let Some(ref tree) = self.derivation_tree {
            output.push_str("Derivation tree:\n");
            output.push_str(&derivation_node_to_string(tree, 1, "  "));
        }

        if !self.diminishers.is_empty() {
            output.push_str("\nDiminishers:\n");
            for (i, dim) in self.diminishers.iter().enumerate() {
                if dim.full_defeat {
                    output.push_str(&format!(
                        "  {}. Fully defeated by '{}' (degree {:.2})\n",
                        i + 1,
                        dim.defeater_label,
                        dim.defeater_degree
                    ));
                } else {
                    output.push_str(&format!(
                        "  {}. Diminished by '{}' (degree {:.2}): {:.2} -> {:.2}\n",
                        i + 1,
                        dim.defeater_label,
                        dim.defeater_degree,
                        dim.target_degree,
                        dim.resulting_degree()
                    ));
                }
            }
        }

        output
    }
}

/// Convert derivation node to string representation
fn derivation_node_to_string(node: &TrustDerivationNode, num: usize, indent: &str) -> String {
    let mut output = String::new();

    let source_str = node.source.as_ref()
        .map(|s| format!(" [source: {}]", s))
        .unwrap_or_default();

    output.push_str(&format!(
        "{}{}. \"{}\" (trust: {:.2}){}\n",
        indent, num, node.literal, node.trust, source_str
    ));

    for (i, child) in node.children.iter().enumerate() {
        let sub_indent = format!("{}   ", indent);
        output.push_str(&derivation_node_to_string(child, i + 1, &sub_indent));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source() {
        let source = Source::new("http://example.org/alice");
        assert_eq!(source.id, "http://example.org/alice");

        let labeled = Source::with_label("http://example.org/bob", "Bob");
        assert_eq!(labeled.label, Some("Bob".to_string()));
    }

    #[test]
    fn test_trust_policy() {
        let policy = TrustPolicy::new(0.5)
            .with_trust("alice", 0.9)
            .with_trust("bob", 0.7)
            .with_threshold("high", 0.8)
            .with_threshold("low", 0.3);

        assert_eq!(policy.get_trust("alice"), 0.9);
        assert_eq!(policy.get_trust("bob"), 0.7);
        assert_eq!(policy.get_trust("unknown"), 0.5);

        assert_eq!(policy.is_above_threshold(0.9, "high"), Some(true));
        assert_eq!(policy.is_above_threshold(0.7, "high"), Some(false));
    }

    #[test]
    fn test_weighted_conclusion() {
        let wc = WeightedConclusion::new(
            Literal::simple("flies"),
            ConclusionType::DefeasiblyProvable,
            0.8,
        );

        assert_eq!(wc.degree, 0.8);
        assert!(!wc.was_diminished());
    }

    #[test]
    fn test_diminisher_info() {
        let dim = DiminisherInfo::new("d1", 0.6, 0.8);
        assert!(!dim.full_defeat);
        assert!(dim.resulting_degree() < 0.8);

        let full = DiminisherInfo::new("d2", 0.9, 0.5).as_full_defeat();
        assert!(full.full_defeat);
        assert_eq!(full.resulting_degree(), 0.0);
    }

    #[test]
    fn test_trust_derivation_node() {
        let child1 = TrustDerivationNode::new(Literal::simple("bird"), 0.9);
        let child2 = TrustDerivationNode::new(Literal::simple("healthy"), 0.7);

        let parent = TrustDerivationNode::new(Literal::simple("flies"), 0.8)
            .with_children(vec![child1, child2]);

        // Weakest link should be 0.7
        assert_eq!(parent.weakest_link_trust(), 0.7);
    }

    #[test]
    fn test_trust_explanation() {
        let tree = TrustDerivationNode::new(Literal::simple("flies"), 0.8)
            .with_source(Source::new("expert"));

        let explanation = TrustExplanation::new(Literal::simple("flies"), 0.8)
            .with_tree(tree);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("flies"));
        assert!(nl.contains("0.80"));
        assert!(nl.contains("expert"));
    }

    #[test]
    fn test_sourced_conclusion() {
        let sc = SourcedConclusion::new(
            Literal::simple("flies"),
            ConclusionType::DefeasiblyProvable,
        )
        .with_source(Source::new("alice"))
        .with_derivation("r1");

        assert_eq!(sc.sources.len(), 1);
        assert_eq!(sc.derivation, vec!["r1"]);
    }
}
