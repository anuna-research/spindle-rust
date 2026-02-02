//! Theory - a collection of rules and superiority relations
//!
//! A theory represents a complete defeasible logic program that
//! can be reasoned about.

use std::collections::HashMap;

use crate::conclusion::Conclusion;
use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel, RuleType};
use crate::superiority::Superiority;

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

/// A defeasible logic theory
#[derive(Debug, Clone, Default)]
pub struct Theory {
    /// Rules indexed by label
    rules: HashMap<RuleLabel, Rule>,
    /// Superiority relations
    superiorities: Vec<Superiority>,
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
        let rule = Rule::fact(&label, Literal::simple(name));
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
        self.rules.values().filter(move |r| r.rule_type == rule_type)
    }

    /// Get all superiority relations
    pub fn superiorities(&self) -> &[Superiority] {
        &self.superiorities
    }

    /// Get the number of rules
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Perform reasoning and return conclusions
    ///
    /// This is a stub - actual implementation in reason module
    pub fn reason(&self) -> Vec<Conclusion> {
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
}
