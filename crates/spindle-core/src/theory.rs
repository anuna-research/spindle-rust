//! Theory - a collection of rules and superiority relations
//!
//! A theory represents a complete defeasible logic program that
//! can be reasoned about.

use std::collections::HashMap;

use crate::conclusion::ConclusionType;
use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel, RuleType};
use crate::superiority::{Superiority, SuperiorityIndex};
use crate::trust::TrustPolicy;
use crate::vocabulary::declaration::{MetaTarget, PredicateDeclaration};
use crate::vocabulary::symbol::PredicateSymbol;

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
    /// Metadata indexed by rule label (the historical store; its accessor shape
    /// is part of the public API and stays label-keyed).
    metadata: HashMap<String, Meta>,
    /// Metadata indexed by predicate symbol, kept distinct from rule-label
    /// metadata (SPEC-024 CON-008).
    predicate_metadata: HashMap<PredicateSymbol, Meta>,
    /// First-class predicate declarations in source/insertion order.
    ///
    /// Raw declarations are stored in order so conflict diagnostics retain every
    /// origin (SPEC-024 CON-008).
    predicate_declarations: Vec<PredicateDeclaration>,
    /// Auto-generated label counter
    label_counter: usize,
    /// Trust policy parsed from trust directives
    trust_policy: TrustPolicy,
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
        let body_lits: Vec<Literal> = body.iter().map(|s| parse_literal_str(s)).collect();
        let rule = Rule::strict(&label, body_lits, parse_literal_str(head));
        self.add_rule(rule);
        label
    }

    /// Add a defeasible rule to the theory
    pub fn add_defeasible_rule(&mut self, body: &[&str], head: &str) -> RuleLabel {
        let label = self.next_label("r");
        let body_lits: Vec<Literal> = body.iter().map(|s| parse_literal_str(s)).collect();
        let rule = Rule::defeasible(&label, body_lits, parse_literal_str(head));
        self.add_rule(rule);
        label
    }

    /// Add a defeater to the theory
    pub fn add_defeater(&mut self, body: &[&str], head: &str) -> RuleLabel {
        let label = self.next_label("d");
        let body_lits: Vec<Literal> = body.iter().map(|s| parse_literal_str(s)).collect();
        let rule = Rule::defeater(&label, body_lits, parse_literal_str(head));
        self.add_rule(rule);
        label
    }

    /// Add a superiority relation
    ///
    /// Logs a warning to stderr if adding this creates a circular superiority
    /// (i.e., prefer(a,b) and prefer(b,a) both exist).
    pub fn add_superiority(&mut self, superior: &str, inferior: &str) {
        // Check for circular superiority before adding
        if self.sup_index.is_superior(inferior, superior) {
            eprintln!(
                "Warning: circular superiority detected: prefer({superior},{inferior}) conflicts with existing prefer({inferior},{superior})"
            );
        }
        self.superiorities
            .push(Superiority::new(superior, inferior));
        self.sup_index.add(superior.to_owned(), inferior.to_owned());
    }

    /// Get a rule by label
    pub fn get_rule(&self, label: &str) -> Option<&Rule> {
        self.rules.get(label)
    }

    /// Get a mutable reference to a rule by label
    pub fn get_rule_mut(&mut self, label: &str) -> Option<&mut Rule> {
        self.rules.get_mut(label)
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

    /// Add metadata for a label.
    pub fn add_meta(&mut self, label: &str, key: &str, value: MetaValue) {
        let meta = self.metadata.entry(label.to_string()).or_default();
        meta.properties.insert(key.to_string(), value);
    }

    /// Add metadata for a structured target (SPEC-024 CON-008).
    ///
    /// Existing metadata property merge and overwrite semantics apply to
    /// predicate targets exactly as they do to label targets.
    pub fn add_meta_target(&mut self, target: MetaTarget, key: &str, value: MetaValue) {
        let meta = match target {
            MetaTarget::Label(label) => self.metadata.entry(label).or_default(),
            MetaTarget::Predicate(symbol) => self.predicate_metadata.entry(symbol).or_default(),
        };
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

    /// Get metadata for a label.
    pub fn get_meta(&self, label: &str) -> Option<&Meta> {
        self.metadata.get(label)
    }

    /// Get metadata for a structured target (SPEC-024 CON-008).
    pub fn get_meta_target(&self, target: &MetaTarget) -> Option<&Meta> {
        match target {
            MetaTarget::Label(label) => self.metadata.get(label),
            MetaTarget::Predicate(symbol) => self.predicate_metadata.get(symbol),
        }
    }

    /// Get all label metadata, keyed by rule label.
    pub fn metadata(&self) -> &HashMap<String, Meta> {
        &self.metadata
    }

    /// Get all predicate metadata, keyed by predicate symbol (SPEC-024 CON-008).
    pub fn predicate_metadata(&self) -> &HashMap<PredicateSymbol, Meta> {
        &self.predicate_metadata
    }

    /// Add a predicate declaration, preserving source/insertion order (SPEC-024 CON-008).
    pub fn add_predicate_declaration(&mut self, declaration: PredicateDeclaration) {
        self.predicate_declarations.push(declaration);
    }

    /// Get all predicate declarations in source/insertion order.
    pub fn predicate_declarations(&self) -> &[PredicateDeclaration] {
        &self.predicate_declarations
    }

    /// Copy metadata (label and predicate) from another theory
    pub fn copy_metadata_from(&mut self, other: &Theory) {
        self.metadata = other.metadata.clone();
        self.predicate_metadata = other.predicate_metadata.clone();
    }

    /// Copy all non-rule state — label and predicate metadata, predicate
    /// declarations, and the trust policy — from another theory.
    ///
    /// Pipeline stages that rebuild a theory rule-by-rule (grounding, wildcard
    /// rewriting, temporal filtering) use this so parsed declarations survive
    /// preparation instead of silently degrading declared symbols to
    /// `UndeclaredPredicate` (SPEC-024 CON-008).
    pub fn copy_declarative_state_from(&mut self, other: &Theory) {
        self.copy_metadata_from(other);
        self.predicate_declarations = other.predicate_declarations.clone();
        self.trust_policy = other.trust_policy.clone();
    }

    /// Get the trust policy
    pub fn trust_policy(&self) -> &TrustPolicy {
        &self.trust_policy
    }

    /// Get mutable reference to the trust policy
    pub fn trust_policy_mut(&mut self) -> &mut TrustPolicy {
        &mut self.trust_policy
    }

    /// Set the trust policy
    pub fn set_trust_policy(&mut self, policy: TrustPolicy) {
        self.trust_policy = policy;
    }

    /// Get the number of rules
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Check if any rule in the theory has non-empty temporal bounds on its literals.
    pub fn has_temporal_literals(&self) -> bool {
        self.rules.values().any(Rule::has_temporal_literals)
    }

    /// Perform defeasible reasoning on this theory and return conclusions.
    ///
    /// This is a convenience method that forwards to [`crate::reason::reason()`].
    /// Prefer the free function for new code.
    pub fn reason(&self) -> crate::error::Result<Vec<crate::conclusion::Conclusion>> {
        crate::reason::reason(self)
    }

    /// Check if the theory has any circular superiority relations
    /// (where prefer(a,b) and prefer(b,a) both exist).
    pub fn has_circular_superiority(&self) -> bool {
        !self.sup_index.find_circular_pairs().is_empty()
    }

    /// Check theory consistency after reasoning.
    ///
    /// Returns a list of warning messages for:
    /// - Circular superiority relations
    /// - Contradictory definite conclusions (both +D p and +D ~p)
    pub fn check_consistency(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        // Check circular superiorities
        for (a, b) in self.sup_index.find_circular_pairs() {
            warnings.push(format!(
                "Circular superiority: prefer({a},{b}) and prefer({b},{a})"
            ));
        }

        // Check contradictory strict conclusions
        if let Ok(conclusions) = self.reason() {
            let definite: std::collections::HashSet<_> = conclusions
                .iter()
                .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
                .map(|c| &c.literal)
                .collect();

            for lit in &definite {
                let comp = lit.complement();
                if definite.contains(&comp) {
                    warnings.push(format!(
                        "Contradictory definite conclusions: both +D {lit} and +D {comp}"
                    ));
                }
            }
        }

        warnings
    }

    /// Iterate over all rules as `(&RuleLabel, &Rule)` pairs.
    ///
    /// This provides access to both the label and the rule in a single
    /// iteration, which is useful for building indexes or performing
    /// label-based lookups during iteration.
    pub fn rules_with_labels(&self) -> impl Iterator<Item = (&RuleLabel, &Rule)> {
        self.rules.iter()
    }

    /// Generate next auto-label with prefix
    fn next_label(&mut self, prefix: &str) -> String {
        self.label_counter += 1;
        format!("{}{}", prefix, self.label_counter)
    }
}

/// Iterate over a borrowed `Theory`, yielding `(&RuleLabel, &Rule)` pairs.
///
/// This allows using `for (label, rule) in &theory { ... }` syntax.
impl<'a> IntoIterator for &'a Theory {
    type Item = (&'a RuleLabel, &'a Rule);
    type IntoIter = std::collections::hash_map::Iter<'a, RuleLabel, Rule>;

    fn into_iter(self) -> Self::IntoIter {
        self.rules.iter()
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
    fn test_predicate_metadata_kept_distinct_from_labels() {
        use crate::vocabulary::PredicateSymbol;

        let mut theory = Theory::new();
        let symbol = PredicateSymbol::try_new("p".into(), 1).unwrap();
        theory.add_meta_string("p/1", "description", "a label");
        theory.add_meta_target(
            MetaTarget::Predicate(symbol),
            "description",
            MetaValue::String("a predicate".to_string()),
        );

        assert_eq!(
            theory
                .get_meta("p/1")
                .unwrap()
                .properties
                .get("description"),
            Some(&MetaValue::String("a label".to_string()))
        );
        assert_eq!(
            theory
                .get_meta_target(&MetaTarget::Predicate(symbol))
                .unwrap()
                .properties
                .get("description"),
            Some(&MetaValue::String("a predicate".to_string()))
        );
        assert_eq!(theory.metadata().len(), 1);
        assert_eq!(theory.predicate_metadata().len(), 1);
    }

    #[test]
    fn test_copy_declarative_state_preserves_declarations() {
        use crate::vocabulary::{
            ArgumentDecl, DeclarationOrigin, PredicateDeclaration, PredicateSignature,
            PredicateSymbol, PrimitiveSort,
        };

        let mut source = Theory::new();
        let symbol = PredicateSymbol::try_new("p".into(), 1).unwrap();
        let sig = PredicateSignature::try_new(
            symbol,
            vec![ArgumentDecl::new("x", PrimitiveSort::Symbol)],
        )
        .unwrap();
        source.add_predicate_declaration(PredicateDeclaration::new(
            sig,
            DeclarationOrigin::Programmatic,
        ));
        source.add_meta_string("r1", "key", "value");

        let mut rebuilt = Theory::new();
        rebuilt.copy_declarative_state_from(&source);
        assert_eq!(rebuilt.predicate_declarations().len(), 1);
        assert!(rebuilt.get_meta("r1").is_some());
    }

    #[test]
    fn test_reason() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");
        let conclusions = theory.reason().unwrap();
        assert!(!conclusions.is_empty());
    }

    // =========================================================================
    // REGRESSION TESTS - Bug Hunt Fixes
    // =========================================================================

    #[test]
    fn test_circular_superiority_detected() {
        let mut theory = Theory::new();
        theory.add_superiority("r1", "r2");
        theory.add_superiority("r2", "r1"); // circular!

        assert!(theory.has_circular_superiority());

        let warnings = theory.check_consistency();
        assert!(
            warnings.iter().any(|w| w.contains("Circular")),
            "Should detect circular superiority"
        );
    }

    #[test]
    fn test_no_circular_superiority() {
        let mut theory = Theory::new();
        theory.add_superiority("r1", "r2");
        theory.add_superiority("r2", "r3");

        assert!(!theory.has_circular_superiority());
    }

    #[test]
    fn test_contradictory_strict_rules_detected() {
        let mut theory = Theory::new();
        theory.add_fact("trigger");
        theory.add_strict_rule(&["trigger"], "p");
        theory.add_strict_rule(&["trigger"], "~p"); // contradiction!

        let warnings = theory.check_consistency();
        assert!(
            warnings.iter().any(|w| w.contains("Contradictory")),
            "Should detect contradictory definite conclusions, got: {warnings:?}"
        );
    }

    #[test]
    fn test_no_contradictory_conclusions() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_strict_rule(&["bird"], "animal");

        let warnings = theory.check_consistency();
        assert!(
            !warnings.iter().any(|w| w.contains("Contradictory")),
            "No contradictions expected"
        );
    }

    // =========================================================================
    // IntoIterator and rules_with_labels tests
    // =========================================================================

    #[test]
    fn test_into_iterator_for_theory_ref() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let mut count = 0;
        for (label, rule) in &theory {
            assert_eq!(&rule.label, label);
            count += 1;
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn test_rules_with_labels() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let pairs: Vec<_> = theory.rules_with_labels().collect();
        assert_eq!(pairs.len(), 2);

        for (label, rule) in pairs {
            assert_eq!(&rule.label, label);
        }
    }

    #[test]
    fn test_into_iterator_empty_theory() {
        let theory = Theory::new();
        let count = (&theory).into_iter().count();
        assert_eq!(count, 0);
    }
}
