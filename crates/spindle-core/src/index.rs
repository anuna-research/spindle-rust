//! Theory indexing for O(1) rule lookup
//!
//! Indexed theories provide fast lookup of rules by head or body literals.

use std::collections::{HashMap, HashSet};

use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel};
use crate::theory::Theory;

/// An indexed theory for efficient rule lookup
#[derive(Debug, Clone)]
pub struct IndexedTheory {
    /// The underlying theory
    theory: Theory,
    /// Rules indexed by head literal
    head_index: HashMap<String, Vec<RuleLabel>>,
    /// Rules indexed by body literal
    body_index: HashMap<String, Vec<RuleLabel>>,
    /// Set of all literals in the theory
    literal_set: HashSet<String>,
}

impl IndexedTheory {
    /// Build an indexed theory from a theory
    pub fn build(theory: Theory) -> Self {
        let mut head_index: HashMap<String, Vec<RuleLabel>> = HashMap::new();
        let mut body_index: HashMap<String, Vec<RuleLabel>> = HashMap::new();
        let mut literal_set: HashSet<String> = HashSet::new();

        for rule in theory.rules() {
            // Index by head literals
            for head_lit in &rule.head {
                let key = head_lit.canonical_name();
                head_index
                    .entry(key.clone())
                    .or_default()
                    .push(rule.label.clone());
                literal_set.insert(key);
            }

            // Index by body literals
            for body_lit in &rule.body {
                let key = body_lit.canonical_name();
                body_index
                    .entry(key.clone())
                    .or_default()
                    .push(rule.label.clone());
                literal_set.insert(key);
            }
        }

        Self {
            theory,
            head_index,
            body_index,
            literal_set,
        }
    }

    /// Get the underlying theory
    pub fn theory(&self) -> &Theory {
        &self.theory
    }

    /// Get rules with the given literal in the head
    pub fn rules_with_head(&self, lit: &Literal) -> Vec<&Rule> {
        let key = lit.canonical_name();
        self.head_index
            .get(&key)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|l| self.theory.get_rule(l))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get rules with the given literal in the body
    pub fn rules_with_body(&self, lit: &Literal) -> Vec<&Rule> {
        let key = lit.canonical_name();
        self.body_index
            .get(&key)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|l| self.theory.get_rule(l))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all literals in the theory
    pub fn all_literals(&self) -> impl Iterator<Item = &String> {
        self.literal_set.iter()
    }

    /// Check if a literal exists in the theory
    pub fn contains_literal(&self, name: &str) -> bool {
        self.literal_set.contains(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexed_theory() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let indexed = IndexedTheory::build(theory);

        let bird = Literal::simple("bird");
        let flies = Literal::simple("flies");

        // bird should be in head (from fact) and body (from rule)
        assert!(!indexed.rules_with_head(&bird).is_empty());
        assert!(!indexed.rules_with_body(&bird).is_empty());

        // flies should be in head only
        assert!(!indexed.rules_with_head(&flies).is_empty());
        assert!(indexed.rules_with_body(&flies).is_empty());
    }
}
