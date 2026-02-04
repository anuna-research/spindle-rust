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
        .map(|s| format!(" [source: {s}]"))
        .unwrap_or_default();

    output.push_str(&format!(
        "{}{}. \"{}\" (trust: {:.2}){}\n",
        indent, num, node.literal, node.trust, source_str
    ));

    for (i, child) in node.children.iter().enumerate() {
        let sub_indent = format!("{indent}   ");
        output.push_str(&derivation_node_to_string(child, i + 1, &sub_indent));
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Source Tests (Basic)
    // =========================================================================

    #[test]
    fn test_source() {
        let source = Source::new("http://example.org/alice");
        assert_eq!(source.id, "http://example.org/alice");

        let labeled = Source::with_label("http://example.org/bob", "Bob");
        assert_eq!(labeled.label, Some("Bob".to_string()));
    }

    #[test]
    fn test_source_display() {
        let source = Source::new("agent:coder");
        assert_eq!(format!("{}", source), "agent:coder");

        let labeled = Source::with_label("agent:security", "Security Team");
        assert_eq!(format!("{}", labeled), "Security Team (agent:security)");
    }

    #[test]
    fn test_source_equality() {
        let s1 = Source::new("agent:alice");
        let s2 = Source::new("agent:alice");
        let s3 = Source::new("agent:bob");

        assert_eq!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_source_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(Source::new("agent:alice"));
        set.insert(Source::new("agent:bob"));
        set.insert(Source::new("agent:alice")); // duplicate

        assert_eq!(set.len(), 2);
    }

    // =========================================================================
    // Source Tracking Tests (TEST-005.4 from Racket)
    // =========================================================================

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

    #[test]
    fn test_direct_facts_have_single_source() {
        // A directly asserted fact should have only one source
        let sc = SourcedConclusion::new(
            Literal::simple("tests_pass"),
            ConclusionType::DefeasiblyProvable,
        )
        .with_source(Source::new("agent:coder"));

        assert_eq!(sc.sources.len(), 1);
        assert!(sc.sources.contains(&Source::new("agent:coder")));
    }

    #[test]
    fn test_derived_conclusions_inherit_sources() {
        // Conclusion inherits sources from premises and rules
        let mut sc = SourcedConclusion::new(
            Literal::simple("ready"),
            ConclusionType::DefeasiblyProvable,
        );

        // Add sources from premise and rule
        sc = sc
            .with_source(Source::new("agent:coder"))
            .with_derivation("r1")
            .with_derivation("r2");

        assert_eq!(sc.sources.len(), 1);
        assert_eq!(sc.derivation.len(), 2);
    }

    #[test]
    fn test_sourced_conclusion_multiple_sources() {
        // Derived conclusion can have multiple contributing sources
        let sc = SourcedConclusion::new(
            Literal::simple("approved"),
            ConclusionType::DefeasiblyProvable,
        )
        .with_source(Source::new("agent:coder"))
        .with_source(Source::new("agent:reviewer"))
        .with_source(Source::new("agent:security"));

        assert_eq!(sc.sources.len(), 3);
    }

    #[test]
    fn test_derivation_chain_tracking() {
        // Track full derivation chain
        let sc = SourcedConclusion::new(
            Literal::simple("final_result"),
            ConclusionType::DefeasiblyProvable,
        )
        .with_derivation("fact1")
        .with_derivation("rule_a")
        .with_derivation("rule_b")
        .with_derivation("rule_c");

        assert_eq!(sc.derivation, vec!["fact1", "rule_a", "rule_b", "rule_c"]);
    }

    // =========================================================================
    // Trust Policy Tests (TEST-005.3 from Racket)
    // =========================================================================

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
    fn test_trust_policy_default_trust() {
        let policy = TrustPolicy::new(0.5);
        assert_eq!(policy.get_trust("unknown_source"), 0.5);

        let policy_full = TrustPolicy::new(1.0);
        assert_eq!(policy_full.get_trust("any_source"), 1.0);

        let policy_zero = TrustPolicy::new(0.0);
        assert_eq!(policy_zero.get_trust("untrusted"), 0.0);
    }

    #[test]
    fn test_trust_policy_multiple_sources() {
        let policy = TrustPolicy::new(0.5)
            .with_trust("agent:coder", 0.9)
            .with_trust("agent:security", 0.95)
            .with_trust("system:policy", 1.0)
            .with_trust("external:api", 0.6);

        assert_eq!(policy.get_trust("agent:coder"), 0.9);
        assert_eq!(policy.get_trust("agent:security"), 0.95);
        assert_eq!(policy.get_trust("system:policy"), 1.0);
        assert_eq!(policy.get_trust("external:api"), 0.6);
    }

    #[test]
    fn test_trust_policy_threshold_extraction() {
        let policy = TrustPolicy::new(0.5)
            .with_threshold("action", 0.7)
            .with_threshold("warn", 0.5)
            .with_threshold("log", 0.3);

        // Check thresholds are stored correctly
        assert_eq!(policy.thresholds.get("action"), Some(&0.7));
        assert_eq!(policy.thresholds.get("warn"), Some(&0.5));
        assert_eq!(policy.thresholds.get("log"), Some(&0.3));
    }

    #[test]
    fn test_trust_policy_threshold_evaluation() {
        let policy = TrustPolicy::new(0.5)
            .with_threshold("action", 0.7)
            .with_threshold("warn", 0.5);

        // 0.9 is above both thresholds
        assert_eq!(policy.is_above_threshold(0.9, "action"), Some(true));
        assert_eq!(policy.is_above_threshold(0.9, "warn"), Some(true));

        // 0.6 is above warn but below action
        assert_eq!(policy.is_above_threshold(0.6, "action"), Some(false));
        assert_eq!(policy.is_above_threshold(0.6, "warn"), Some(true));

        // 0.4 is below both
        assert_eq!(policy.is_above_threshold(0.4, "action"), Some(false));
        assert_eq!(policy.is_above_threshold(0.4, "warn"), Some(false));

        // Unknown threshold returns None
        assert_eq!(policy.is_above_threshold(0.9, "unknown"), None);
    }

    #[test]
    fn test_trust_policy_boundary_values() {
        let policy = TrustPolicy::new(0.5)
            .with_threshold("exact", 0.7);

        // Exactly at threshold should be considered "above" (>=)
        assert_eq!(policy.is_above_threshold(0.7, "exact"), Some(true));
        assert_eq!(policy.is_above_threshold(0.69999, "exact"), Some(false));
        assert_eq!(policy.is_above_threshold(0.70001, "exact"), Some(true));
    }

    #[test]
    fn test_trust_policy_edge_values() {
        let policy = TrustPolicy::new(0.5)
            .with_trust("untrusted", 0.0)
            .with_trust("fully_trusted", 1.0)
            .with_threshold("zero", 0.0)
            .with_threshold("full", 1.0);

        assert_eq!(policy.get_trust("untrusted"), 0.0);
        assert_eq!(policy.get_trust("fully_trusted"), 1.0);

        // 0.0 threshold: everything is above
        assert_eq!(policy.is_above_threshold(0.0, "zero"), Some(true));

        // 1.0 threshold: only 1.0 is above
        assert_eq!(policy.is_above_threshold(1.0, "full"), Some(true));
        assert_eq!(policy.is_above_threshold(0.99, "full"), Some(false));
    }

    // =========================================================================
    // Weakest-Link Computation Tests (TEST-005.5 from Racket)
    // =========================================================================

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
    fn test_trust_derivation_node() {
        let child1 = TrustDerivationNode::new(Literal::simple("bird"), 0.9);
        let child2 = TrustDerivationNode::new(Literal::simple("healthy"), 0.7);

        let parent = TrustDerivationNode::new(Literal::simple("flies"), 0.8)
            .with_children(vec![child1, child2]);

        // Weakest link should be 0.7
        assert_eq!(parent.weakest_link_trust(), 0.7);
    }

    #[test]
    fn test_single_source_degree_equals_trust() {
        // Single source: degree equals source trust
        let node = TrustDerivationNode::new(Literal::simple("tests_pass"), 0.9)
            .with_source(Source::new("agent:coder"));

        assert_eq!(node.weakest_link_trust(), 0.9);
    }

    #[test]
    fn test_multiple_sources_weakest_link() {
        // Chain: a (0.9) -> b (0.7) -> c
        // Weakest link is 0.7
        let leaf_a = TrustDerivationNode::new(Literal::simple("a"), 0.9);
        let node_b = TrustDerivationNode::new(Literal::simple("b"), 0.7)
            .with_children(vec![leaf_a]);
        let root_c = TrustDerivationNode::new(Literal::simple("c"), 0.8)
            .with_children(vec![node_b]);

        assert_eq!(root_c.weakest_link_trust(), 0.7);
    }

    #[test]
    fn test_chain_of_inference_weakest_link_propagates() {
        // a (0.9) -> b (0.9) -> c (0.5)
        // Weakest link is 0.5
        let leaf_a = TrustDerivationNode::new(Literal::simple("a"), 0.9)
            .with_source(Source::new("agent:hightrust"));
        let node_b = TrustDerivationNode::new(Literal::simple("b"), 0.9)
            .with_children(vec![leaf_a]);
        let root_c = TrustDerivationNode::new(Literal::simple("c"), 0.5)
            .with_source(Source::new("agent:lowtrust"))
            .with_children(vec![node_b]);

        assert_eq!(root_c.weakest_link_trust(), 0.5);
    }

    #[test]
    fn test_weakest_link_with_multiple_branches() {
        // Multiple branches: min of all paths
        //       root (0.8)
        //      /    \
        // branch1(0.9)  branch2(0.6)
        //     |           |
        // leaf1(0.95)  leaf2(0.7)
        let leaf1 = TrustDerivationNode::new(Literal::simple("leaf1"), 0.95);
        let leaf2 = TrustDerivationNode::new(Literal::simple("leaf2"), 0.7);
        let branch1 = TrustDerivationNode::new(Literal::simple("branch1"), 0.9)
            .with_children(vec![leaf1]);
        let branch2 = TrustDerivationNode::new(Literal::simple("branch2"), 0.6)
            .with_children(vec![leaf2]);
        let root = TrustDerivationNode::new(Literal::simple("root"), 0.8)
            .with_children(vec![branch1, branch2]);

        // Minimum is 0.6 from branch2
        assert_eq!(root.weakest_link_trust(), 0.6);
    }

    #[test]
    fn test_weighted_conclusion_with_threshold_checks() {
        let mut wc = WeightedConclusion::new(
            Literal::simple("important_fact"),
            ConclusionType::DefeasiblyProvable,
            0.9,
        );

        // Add threshold evaluation results
        wc.above_threshold.insert("action".to_string(), true);
        wc.above_threshold.insert("warn".to_string(), true);
        wc.above_threshold.insert("critical".to_string(), false);

        assert_eq!(wc.is_above_threshold("action"), Some(true));
        assert_eq!(wc.is_above_threshold("warn"), Some(true));
        assert_eq!(wc.is_above_threshold("critical"), Some(false));
        assert_eq!(wc.is_above_threshold("unknown"), None);
    }

    // =========================================================================
    // Diminisher Tests (TEST-005.6 from Racket)
    // =========================================================================

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
    fn test_compute_diminishment_formula() {
        // diminishment = defeater_degree * target_degree (capped at target_degree)
        let dim = DiminisherInfo::new("defeater", 0.5, 0.9);
        // diminishment = min(0.5 * 0.9, 0.9) = 0.45
        assert!((dim.diminishment - 0.45).abs() < 0.001);
    }

    #[test]
    fn test_full_defeat_when_defeater_exceeds_target() {
        // When defeater >= target, should be full defeat
        let dim = DiminisherInfo::new("strong_defeater", 0.8, 0.7).as_full_defeat();

        assert!(dim.full_defeat);
        assert_eq!(dim.resulting_degree(), 0.0);
    }

    #[test]
    fn test_partial_diminishment_when_defeater_below_target() {
        // When defeater < target, partial diminishment
        let dim = DiminisherInfo::new("weak_defeater", 0.5, 0.9);

        assert!(!dim.full_defeat);
        assert!(dim.resulting_degree() > 0.0, "Should still have positive degree");
        assert!(dim.resulting_degree() < 0.9, "Degree should be reduced");
    }

    #[test]
    fn test_resulting_degree_never_negative() {
        // Even with large diminishment, result should be >= 0
        let dim = DiminisherInfo::new("strong", 1.0, 0.5);
        assert!(dim.resulting_degree() >= 0.0);
    }

    #[test]
    fn test_diminisher_info_fields() {
        let dim = DiminisherInfo::new("rule_d1", 0.6, 0.8);

        assert_eq!(dim.defeater_label, "rule_d1");
        assert_eq!(dim.defeater_degree, 0.6);
        assert_eq!(dim.target_degree, 0.8);
    }

    #[test]
    fn test_weighted_conclusion_with_diminishers() {
        let mut wc = WeightedConclusion::new(
            Literal::simple("partially_defeated"),
            ConclusionType::DefeasiblyProvable,
            0.9,
        );

        assert!(!wc.was_diminished());

        wc.diminished_by.push(DiminisherInfo::new("d1", 0.4, 0.9));
        assert!(wc.was_diminished());
        assert_eq!(wc.diminished_by.len(), 1);
    }

    #[test]
    fn test_multiple_diminishers_cumulative() {
        let mut wc = WeightedConclusion::new(
            Literal::simple("multi_diminished"),
            ConclusionType::DefeasiblyProvable,
            0.9,
        );

        wc.diminished_by.push(DiminisherInfo::new("d1", 0.3, 0.9));
        wc.diminished_by.push(DiminisherInfo::new("d2", 0.4, 0.9));
        wc.diminished_by.push(DiminisherInfo::new("d3", 0.2, 0.9));

        assert_eq!(wc.diminished_by.len(), 3);
    }

    // =========================================================================
    // Trust Perspectives Tests (TEST-005.7 from Racket)
    // =========================================================================

    #[test]
    fn test_different_perspectives_same_source() {
        // Same source, different trust policies yield different trust values
        let policy_security = TrustPolicy::new(0.5)
            .with_trust("agent:security", 0.95)
            .with_trust("agent:coder", 0.6);

        let policy_coder = TrustPolicy::new(0.5)
            .with_trust("agent:security", 0.5)
            .with_trust("agent:coder", 0.9);

        // Security perspective trusts security team more
        assert!(policy_security.get_trust("agent:security") > policy_security.get_trust("agent:coder"));

        // Coder perspective trusts coders more
        assert!(policy_coder.get_trust("agent:coder") > policy_coder.get_trust("agent:security"));
    }

    #[test]
    fn test_perspectives_affect_threshold_evaluation() {
        // Same conclusion degree, different threshold policies
        let high_threshold = TrustPolicy::new(0.5)
            .with_threshold("action", 0.8);

        let low_threshold = TrustPolicy::new(0.5)
            .with_threshold("action", 0.6);

        let conclusion_degree = 0.7;

        // High threshold: below action
        assert_eq!(high_threshold.is_above_threshold(conclusion_degree, "action"), Some(false));

        // Low threshold: above action
        assert_eq!(low_threshold.is_above_threshold(conclusion_degree, "action"), Some(true));
    }

    #[test]
    fn test_conservative_vs_permissive_perspectives() {
        // Conservative perspective: high thresholds, low default trust
        let conservative = TrustPolicy::new(0.3)
            .with_threshold("action", 0.9)
            .with_threshold("warn", 0.7);

        // Permissive perspective: low thresholds, high default trust
        let permissive = TrustPolicy::new(0.8)
            .with_threshold("action", 0.5)
            .with_threshold("warn", 0.3);

        let degree = 0.75;

        // Conservative: above warn, below action
        assert_eq!(conservative.is_above_threshold(degree, "action"), Some(false));
        assert_eq!(conservative.is_above_threshold(degree, "warn"), Some(true));

        // Permissive: above both
        assert_eq!(permissive.is_above_threshold(degree, "action"), Some(true));
        assert_eq!(permissive.is_above_threshold(degree, "warn"), Some(true));
    }

    #[test]
    fn test_perspective_inheritance_in_derivation() {
        // Different perspectives on same derivation tree
        let policy_a = TrustPolicy::new(0.5)
            .with_trust("source_x", 0.9)
            .with_trust("source_y", 0.3);

        let policy_b = TrustPolicy::new(0.5)
            .with_trust("source_x", 0.3)
            .with_trust("source_y", 0.9);

        // With policy_a, source_x is highly trusted
        let trust_x_in_a = policy_a.get_trust("source_x");
        let trust_y_in_a = policy_a.get_trust("source_y");

        // With policy_b, source_y is highly trusted
        let trust_x_in_b = policy_b.get_trust("source_x");
        let trust_y_in_b = policy_b.get_trust("source_y");

        assert!(trust_x_in_a > trust_y_in_a);
        assert!(trust_y_in_b > trust_x_in_b);
    }

    // =========================================================================
    // Trust Explanation Tests (TEST-005.7B from Racket)
    // =========================================================================

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
    fn test_trust_explanation_includes_derivation_tree() {
        let child = TrustDerivationNode::new(Literal::simple("a"), 0.9)
            .with_source(Source::new("agent:coder"));
        let root = TrustDerivationNode::new(Literal::simple("b"), 0.8)
            .with_children(vec![child]);

        let explanation = TrustExplanation::new(Literal::simple("b"), 0.8)
            .with_tree(root);

        assert!(explanation.derivation_tree.is_some());
    }

    #[test]
    fn test_trust_explanation_with_diminishers() {
        let dim1 = DiminisherInfo::new("d1", 0.4, 0.9);
        let dim2 = DiminisherInfo::new("d2", 0.3, 0.9).as_full_defeat();

        let explanation = TrustExplanation::new(Literal::simple("goal"), 0.0)
            .with_diminishers(vec![dim1, dim2]);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("d1"));
        assert!(nl.contains("d2"));
        assert!(nl.contains("Diminished"));
        assert!(nl.contains("Fully defeated"));
    }

    #[test]
    fn test_non_provable_literal_zero_degree() {
        // Non-provable literal should have zero degree in explanation
        let explanation = TrustExplanation::new(Literal::simple("not_provable"), 0.0);

        assert_eq!(explanation.final_degree, 0.0);
        assert!(explanation.derivation_tree.is_none());
    }

    #[test]
    fn test_trust_explanation_natural_language_format() {
        let leaf = TrustDerivationNode::new(Literal::simple("premise"), 0.9)
            .with_source(Source::with_label("src1", "Source One"));
        let root = TrustDerivationNode::new(Literal::simple("conclusion"), 0.85)
            .with_children(vec![leaf]);

        let explanation = TrustExplanation::new(Literal::simple("conclusion"), 0.85)
            .with_tree(root);

        let nl = explanation.to_natural_language();

        // Should contain goal
        assert!(nl.contains("conclusion"));
        // Should contain final degree
        assert!(nl.contains("0.85"));
        // Should contain derivation tree info
        assert!(nl.contains("Derivation tree"));
        // Should contain premise info
        assert!(nl.contains("premise"));
    }

    // =========================================================================
    // Edge Cases and Integration Tests
    // =========================================================================

    #[test]
    fn test_deep_derivation_tree_weakest_link() {
        // Build a deep tree: depth 5
        let mut current = TrustDerivationNode::new(Literal::simple("level5"), 0.95);
        for i in (1..5).rev() {
            let trust = 0.8 + (i as f64 * 0.02); // 0.82, 0.84, 0.86, 0.88
            current = TrustDerivationNode::new(
                Literal::simple(format!("level{}", i)),
                trust,
            )
            .with_children(vec![current]);
        }

        // Minimum should be approximately 0.82 (from level1)
        // Use epsilon comparison for floating-point
        assert!((current.weakest_link_trust() - 0.82).abs() < 1e-10);
    }

    #[test]
    fn test_sources_collected_across_derivation() {
        let mut sc = SourcedConclusion::new(
            Literal::simple("final"),
            ConclusionType::DefeasiblyProvable,
        );

        // Add multiple sources from different parts of derivation
        let sources = vec![
            Source::new("agent:alice"),
            Source::new("agent:bob"),
            Source::new("agent:charlie"),
            Source::new("system:policy"),
        ];

        for source in sources {
            sc = sc.with_source(source);
        }

        assert_eq!(sc.sources.len(), 4);
    }

    #[test]
    fn test_trust_policy_override() {
        // Override existing trust value
        let policy = TrustPolicy::new(0.5)
            .with_trust("agent:coder", 0.7)
            .with_trust("agent:coder", 0.9); // Override

        // Later value should win
        assert_eq!(policy.get_trust("agent:coder"), 0.9);
    }

    #[test]
    fn test_diminisher_resulting_degree_calculation() {
        // Test the formula: (target_degree - diminishment).max(0.0)
        let dim = DiminisherInfo::new("d", 0.4, 0.8);
        // diminishment = 0.4 * 0.8 = 0.32
        // resulting = 0.8 - 0.32 = 0.48
        let expected = 0.8 - (0.4 * 0.8);
        assert!((dim.resulting_degree() - expected).abs() < 0.001);
    }

    #[test]
    fn test_trust_derivation_node_empty_children() {
        let leaf = TrustDerivationNode::new(Literal::simple("fact"), 0.9);

        assert!(leaf.children.is_empty());
        assert_eq!(leaf.weakest_link_trust(), 0.9);
    }

    #[test]
    fn test_weighted_conclusion_sources() {
        let mut wc = WeightedConclusion::new(
            Literal::simple("derived"),
            ConclusionType::DefeasiblyProvable,
            0.8,
        );

        wc.sources.insert(Source::new("s1"));
        wc.sources.insert(Source::new("s2"));

        assert_eq!(wc.sources.len(), 2);
    }

    #[test]
    fn test_all_conclusion_types() {
        // Test with different conclusion types
        let defeasible = WeightedConclusion::new(
            Literal::simple("def"),
            ConclusionType::DefeasiblyProvable,
            0.8,
        );
        let definite = WeightedConclusion::new(
            Literal::simple("def"),
            ConclusionType::DefinitelyProvable,
            1.0,
        );
        let not_defeasible = WeightedConclusion::new(
            Literal::simple("not_def"),
            ConclusionType::DefeasiblyNotProvable,
            0.0,
        );

        assert_eq!(defeasible.conclusion_type, ConclusionType::DefeasiblyProvable);
        assert_eq!(definite.conclusion_type, ConclusionType::DefinitelyProvable);
        assert_eq!(not_defeasible.conclusion_type, ConclusionType::DefeasiblyNotProvable);
    }

    #[test]
    fn test_trust_value_precision() {
        // Ensure trust values maintain precision
        let policy = TrustPolicy::new(0.123456789)
            .with_trust("precise", 0.987654321);

        assert!((policy.default_trust - 0.123456789).abs() < 1e-10);
        assert!((policy.get_trust("precise") - 0.987654321).abs() < 1e-10);
    }
}
