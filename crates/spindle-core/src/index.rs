//! Theory indexing for O(1) rule lookup
//!
//! Indexed theories provide fast lookup of rules by head or body literals.
//! Uses `LiteralId` for efficient O(1) hashing instead of String keys.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::intern::LiteralId;
use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel};
use crate::theory::Theory;

/// An indexed theory for efficient rule lookup.
///
/// Uses `LiteralId` (4 bytes, Copy) as keys for O(1) lookups instead
/// of `String` keys which require hashing variable-length data.
///
/// Holds a reference to the theory to avoid deep cloning during reasoning.
#[derive(Debug)]
pub struct IndexedTheory<'a> {
    /// Reference to the underlying theory (avoids deep clone)
    theory: &'a Theory,
    /// Rules indexed by head literal (using LiteralId for fast lookup)
    head_index: FxHashMap<LiteralId, Vec<RuleLabel>>,
    /// Rules indexed by body literal (trigger index for semi-naive evaluation)
    body_index: FxHashMap<LiteralId, Vec<RuleLabel>>,
    /// Set of all literal IDs in the theory
    literal_set: FxHashSet<LiteralId>,
}

impl<'a> IndexedTheory<'a> {
    /// Build an indexed theory from a theory reference.
    ///
    /// Creates indexes using `LiteralId` keys for O(1) lookups:
    /// - `head_index`: Maps head literals to rules that produce them
    /// - `body_index`: Maps body literals to rules they can trigger (trigger index)
    ///
    /// Takes a reference to avoid deep cloning the theory.
    pub fn build(theory: &'a Theory) -> Self {
        let mut head_index: FxHashMap<LiteralId, Vec<RuleLabel>> = FxHashMap::default();
        let mut body_index: FxHashMap<LiteralId, Vec<RuleLabel>> = FxHashMap::default();
        let mut literal_set: FxHashSet<LiteralId> = FxHashSet::default();

        for rule in theory.rules() {
            // Index by head literals (using LiteralId for O(1) hashing)
            for head_lit in &rule.head {
                let key = head_lit.literal_id();
                head_index.entry(key).or_default().push(rule.label.clone());
                literal_set.insert(key);
            }

            // Index by body literals (trigger index for semi-naive evaluation)
            for body_lit in &rule.body {
                let key = body_lit.literal_id();
                body_index.entry(key).or_default().push(rule.label.clone());
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
        self.theory
    }

    /// Get rules with the given literal in the head.
    pub fn rules_with_head(&self, lit: &Literal) -> Vec<&Rule> {
        self.rules_with_head_id(lit.literal_id())
    }

    /// Get rules with the given literal ID in the head (O(1) lookup).
    pub fn rules_with_head_id(&self, lit_id: LiteralId) -> Vec<&Rule> {
        self.head_index
            .get(&lit_id)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|l| self.theory.get_rule(l))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get rule labels with the given literal ID in the head (avoids Rule lookup).
    pub fn rule_labels_with_head_id(&self, lit_id: LiteralId) -> &[RuleLabel] {
        self.head_index
            .get(&lit_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get rules with the given literal in the body (trigger index).
    pub fn rules_with_body(&self, lit: &Literal) -> Vec<&Rule> {
        self.rules_with_body_id(lit.literal_id())
    }

    /// Get rules with the given literal ID in the body (O(1) lookup).
    ///
    /// This is the trigger index for semi-naive evaluation: when a literal
    /// is proven, these are the rules that might become newly satisfiable.
    pub fn rules_with_body_id(&self, lit_id: LiteralId) -> Vec<&Rule> {
        self.body_index
            .get(&lit_id)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|l| self.theory.get_rule(l))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get rule labels with the given literal ID in the body (avoids Rule lookup).
    ///
    /// This is the trigger index for semi-naive evaluation.
    pub fn rule_labels_with_body_id(&self, lit_id: LiteralId) -> &[RuleLabel] {
        self.body_index
            .get(&lit_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all literal IDs in the theory.
    pub fn all_literal_ids(&self) -> impl Iterator<Item = &LiteralId> {
        self.literal_set.iter()
    }

    /// Check if a literal ID exists in the theory.
    pub fn contains_literal_id(&self, lit_id: LiteralId) -> bool {
        self.literal_set.contains(&lit_id)
    }

    /// Check if a literal exists in the theory.
    pub fn contains_literal(&self, lit: &Literal) -> bool {
        self.literal_set.contains(&lit.literal_id())
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

        let indexed = IndexedTheory::build(&theory);

        let bird = Literal::simple("bird");
        let flies = Literal::simple("flies");

        // bird should be in head (from fact) and body (from rule)
        assert!(!indexed.rules_with_head(&bird).is_empty());
        assert!(!indexed.rules_with_body(&bird).is_empty());

        // flies should be in head only
        assert!(!indexed.rules_with_head(&flies).is_empty());
        assert!(indexed.rules_with_body(&flies).is_empty());
    }

    #[test]
    fn test_indexed_theory_literal_id() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let indexed = IndexedTheory::build(&theory);

        let bird = Literal::simple("bird");
        let flies = Literal::simple("flies");

        // Test ID-based lookups
        assert!(!indexed.rules_with_head_id(bird.literal_id()).is_empty());
        assert!(!indexed.rules_with_body_id(bird.literal_id()).is_empty());
        assert!(!indexed.rules_with_head_id(flies.literal_id()).is_empty());
        assert!(indexed.rules_with_body_id(flies.literal_id()).is_empty());

        // Test contains
        assert!(indexed.contains_literal_id(bird.literal_id()));
        assert!(indexed.contains_literal_id(flies.literal_id()));
        assert!(indexed.contains_literal(&bird));
    }

    #[test]
    fn test_trigger_index() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["a", "b"], "d");

        let indexed = IndexedTheory::build(&theory);

        let a = Literal::simple("a");
        let b = Literal::simple("b");

        // When 'a' is proven, which rules can fire?
        let triggered_by_a = indexed.rule_labels_with_body_id(a.literal_id());
        assert_eq!(triggered_by_a.len(), 2); // r1 (a => b) and r3 (a, b => d)

        // When 'b' is proven, which rules can fire?
        let triggered_by_b = indexed.rule_labels_with_body_id(b.literal_id());
        assert_eq!(triggered_by_b.len(), 2); // r2 (b => c) and r3 (a, b => d)
    }

    // =========================================================================
    // ADDITIONAL COVERAGE TESTS
    // =========================================================================

    #[test]
    fn test_rule_labels_with_head_id() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");

        let indexed = IndexedTheory::build(&theory);

        let a = Literal::simple("a");
        let b = Literal::simple("b");

        // 'a' is head of fact
        let head_labels_a = indexed.rule_labels_with_head_id(a.literal_id());
        assert!(!head_labels_a.is_empty());

        // 'b' is head of rule
        let head_labels_b = indexed.rule_labels_with_head_id(b.literal_id());
        assert!(!head_labels_b.is_empty());

        // nonexistent literal returns empty
        let nonexistent = Literal::simple("nonexistent");
        let head_labels_none = indexed.rule_labels_with_head_id(nonexistent.literal_id());
        assert!(head_labels_none.is_empty());
    }

    #[test]
    fn test_all_literal_ids() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let indexed = IndexedTheory::build(&theory);

        // Should have a, b, c
        let all_ids: Vec<_> = indexed.all_literal_ids().collect();
        assert_eq!(all_ids.len(), 3);
    }

    #[test]
    fn test_contains_literal_not_found() {
        let mut theory = Theory::new();
        theory.add_fact("a");

        let indexed = IndexedTheory::build(&theory);

        let nonexistent = Literal::simple("nonexistent");
        assert!(!indexed.contains_literal(&nonexistent));
        assert!(!indexed.contains_literal_id(nonexistent.literal_id()));
    }

    #[test]
    fn test_theory_accessor() {
        let mut theory = Theory::new();
        theory.add_fact("test");

        let indexed = IndexedTheory::build(&theory);
        assert_eq!(indexed.theory().rule_count(), 1);
    }

    #[test]
    fn test_empty_theory_index() {
        let theory = Theory::new();
        let indexed = IndexedTheory::build(&theory);

        let lit = Literal::simple("any");
        assert!(indexed.rules_with_head(&lit).is_empty());
        assert!(indexed.rules_with_body(&lit).is_empty());
        assert!(
            indexed
                .rule_labels_with_head_id(lit.literal_id())
                .is_empty()
        );
        assert!(
            indexed
                .rule_labels_with_body_id(lit.literal_id())
                .is_empty()
        );
    }
}
