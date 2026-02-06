//! Theory - a collection of rules and superiority relations
//!
//! A theory represents a complete defeasible logic program that
//! can be reasoned about.

use std::collections::HashMap;

use crate::conclusion::Conclusion;
use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel, RuleType};
use crate::superiority::{Superiority, SuperiorityIndex};

/// Parse a string literal, handling negation prefix
fn parse_literal_str(s: &str) -> Literal {
    if let Some(name) = s.strip_prefix('~') {
        Literal::negated(name)
    } else if let Some(name) = s.strip_prefix('-') {
        Literal::negated(name)
    } else {
        Literal::simple(s)
    }
}

/// Metadata entry (key-value pair with optional list values)
#[derive(Debug, Clone, PartialEq)]
pub enum MetaValue {
    /// Single string value
    String(String),
    /// List of strings
    List(Vec<String>),
}

/// Metadata for a label (e.g., task description, priority)
#[derive(Debug, Clone, Default)]
pub struct Meta {
    /// Properties for this label
    pub properties: HashMap<String, MetaValue>,
}

/// A defeasible logic theory
#[derive(Debug, Clone, Default)]
pub struct Theory {
    /// Rules indexed by label
    rules: HashMap<RuleLabel, Rule>,
    /// Superiority relations
    superiorities: Vec<Superiority>,
    /// Indexed superiority for O(1) lookup
    sup_index: SuperiorityIndex,
    /// Metadata indexed by label
    metadata: HashMap<String, Meta>,
    /// Auto-generated label counter
    label_counter: usize,
}

impl Theory {
    /// Create a new empty theory
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rule to the theory
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.insert(rule.label.clone(), rule);
    }

    /// Add a fact to the theory
    pub fn add_fact(&mut self, name: &str) -> RuleLabel {
        let label = self.next_label("f");
        let rule = Rule::fact(&label, parse_literal_str(name));
        self.add_rule(rule);
        label
    }

    /// Add a strict rule to the theory
    pub fn add_strict_rule(&mut self, body: &[&str], head: &str) -> RuleLabel {
        let label = self.next_label("s");
        let body_lits: Vec<_> = body.iter().map(|s| parse_literal_str(s)).collect();
        let rule = Rule::strict(&label, body_lits, parse_literal_str(head));
        self.add_rule(rule);
        label
    }

    /// Add a defeasible rule to the theory
    pub fn add_defeasible_rule(&mut self, body: &[&str], head: &str) -> RuleLabel {
        let label = self.next_label("r");
        let body_lits: Vec<_> = body.iter().map(|s| parse_literal_str(s)).collect();
        let rule = Rule::defeasible(&label, body_lits, parse_literal_str(head));
        self.add_rule(rule);
        label
    }

    /// Add a defeater to the theory
    pub fn add_defeater(&mut self, body: &[&str], head: &str) -> RuleLabel {
        let label = self.next_label("d");
        let body_lits: Vec<_> = body.iter().map(|s| parse_literal_str(s)).collect();
        let rule = Rule::defeater(&label, body_lits, parse_literal_str(head));
        self.add_rule(rule);
        label
    }

    /// Add a superiority relation
    pub fn add_superiority(&mut self, superior: &str, inferior: &str) {
        self.superiorities
            .push(Superiority::new(superior, inferior));
        self.sup_index.add(superior.to_owned(), inferior.to_owned());
    }

    /// Get a rule by label
    pub fn get_rule(&self, label: &str) -> Option<&Rule> {
        self.rules.get(label)
    }

    /// Get all rules
    pub fn rules(&self) -> impl Iterator<Item = &Rule> {
        self.rules.values()
    }

    /// Get all facts
    pub fn facts(&self) -> impl Iterator<Item = &Rule> {
        self.rules.values().filter(|r| r.is_fact())
    }

    /// Get rules by type
    pub fn rules_by_type(&self, rule_type: RuleType) -> impl Iterator<Item = &Rule> {
        self.rules
            .values()
            .filter(move |r| r.rule_type == rule_type)
    }

    /// Get all superiority relations
    pub fn superiorities(&self) -> &[Superiority] {
        &self.superiorities
    }

    /// Get the superiority index for O(1) lookups
    ///
    /// Use this for checking if one rule is superior to another:
    /// ```rust
    /// use spindle_core::prelude::*;
    ///
    /// let mut theory = Theory::new();
    /// theory.add_superiority("r2", "r1");
    ///
    /// assert!(theory.sup_index().is_superior("r2", "r1"));
    /// assert!(!theory.sup_index().is_superior("r1", "r2"));
    /// ```
    #[inline]
    pub fn sup_index(&self) -> &SuperiorityIndex {
        &self.sup_index
    }

    /// Check if `superior` rule is superior to `inferior` rule.
    ///
    /// This is a convenience method that uses the indexed lookup.
    /// Complexity: O(1) average case.
    #[inline]
    pub fn is_superior(&self, superior: &str, inferior: &str) -> bool {
        self.sup_index.is_superior(superior, inferior)
    }

    /// Add metadata for a label
    pub fn add_meta(&mut self, label: &str, key: &str, value: MetaValue) {
        let meta = self.metadata.entry(label.to_string()).or_default();
        meta.properties.insert(key.to_string(), value);
    }

    /// Add a string metadata value
    pub fn add_meta_string(&mut self, label: &str, key: &str, value: &str) {
        self.add_meta(label, key, MetaValue::String(value.to_string()));
    }

    /// Add a list metadata value
    pub fn add_meta_list(&mut self, label: &str, key: &str, values: Vec<String>) {
        self.add_meta(label, key, MetaValue::List(values));
    }

    /// Get metadata for a label
    pub fn get_meta(&self, label: &str) -> Option<&Meta> {
        self.metadata.get(label)
    }

    /// Get all metadata
    pub fn metadata(&self) -> &HashMap<String, Meta> {
        &self.metadata
    }

    /// Copy metadata from another theory
    pub fn copy_metadata_from(&mut self, other: &Theory) {
        self.metadata = other.metadata.clone();
    }

    /// Get the number of rules
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Perform reasoning and return conclusions
    ///
    /// This is a stub - actual implementation in reason module
    pub fn reason(&self) -> crate::error::Result<Vec<Conclusion>> {
        crate::reason::reason(self)
    }

    /// Generate next auto-label with prefix
    fn next_label(&mut self, prefix: &str) -> String {
        self.label_counter += 1;
        format!("{}{}", prefix, self.label_counter)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_theory() {
        let theory = Theory::new();
        assert_eq!(theory.rule_count(), 0);
    }

    #[test]
    fn test_add_fact() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        assert_eq!(theory.rule_count(), 1);
        assert_eq!(theory.facts().count(), 1);
    }

    #[test]
    fn test_add_fact_negated_with_dash() {
        let mut theory = Theory::new();
        theory.add_fact("-bird");
        let facts: Vec<_> = theory.facts().collect();
        assert_eq!(facts.len(), 1);
        assert!(facts[0].head_literal().is_negated());
    }

    #[test]
    fn test_add_rules() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");
        assert_eq!(theory.rule_count(), 2);
    }

    #[test]
    fn test_superiority() {
        let mut theory = Theory::new();
        theory.add_superiority("r2", "r1");
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_sup_index() {
        let mut theory = Theory::new();
        theory.add_superiority("r2", "r1");
        assert!(theory.sup_index().is_superior("r2", "r1"));
        assert!(!theory.sup_index().is_superior("r1", "r2"));
    }

    #[test]
    fn test_add_meta_string() {
        let mut theory = Theory::new();
        theory.add_meta_string("r1", "description", "test rule");
        let meta = theory.get_meta("r1").unwrap();
        assert_eq!(
            meta.properties.get("description"),
            Some(&MetaValue::String("test rule".to_string()))
        );
    }

    #[test]
    fn test_add_meta_list() {
        let mut theory = Theory::new();
        theory.add_meta_list("r1", "tags", vec!["a".to_string(), "b".to_string()]);
        let meta = theory.get_meta("r1").unwrap();
        assert_eq!(
            meta.properties.get("tags"),
            Some(&MetaValue::List(vec!["a".to_string(), "b".to_string()]))
        );
    }

    #[test]
    fn test_metadata() {
        let mut theory = Theory::new();
        theory.add_meta_string("r1", "key", "value");
        let all_meta = theory.metadata();
        assert!(all_meta.contains_key("r1"));
    }

    #[test]
    fn test_reason() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");
        let conclusions = theory.reason().unwrap();
        assert!(!conclusions.is_empty());
    }
}
