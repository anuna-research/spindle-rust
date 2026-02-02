//! Scalable DL(d||) Reasoning Algorithm
//!
//! This module implements the three-phase closure computation for scalable
//! defeasible logic reasoning (DL(d||)):
//!
//! **Phase 1: Delta Closure (P_Δ)**
//!   Computes definite conclusions via forward chaining of strict rules.
//!   Complexity: O(|facts| + |body_literals|)
//!
//! **Phase 2: Lambda Closure (P_λ)**
//!   Over-approximation of defeasibly provable literals. Extends delta with
//!   defeasible rules, checking that complements aren't strictly proven.
//!
//! **Phase 3: Partial Closure (P_∂||)**
//!   Actual defeasible conclusions with conflict resolution via superiority.
//!   Uses the key insight: "NOT in lambda" replaces computing "-d" proofs.
//!
//! References:
//! - SPINdle-Racket v1.7.0 scalable/closures.rkt
//! - Allen, J.F. "Maintaining Knowledge about Temporal Intervals" (1983)

use std::collections::{HashMap, HashSet, VecDeque};

use crate::conclusion::{Conclusion, ConclusionType};
use crate::index::IndexedTheory;
use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel, RuleType};
use crate::theory::Theory;

/// Result of scalable reasoning: three closure sets
#[derive(Debug, Clone)]
pub struct ScalableResult {
    /// Delta closure: definitely provable (+Δ)
    pub delta: HashSet<String>,
    /// Lambda closure: potentially provable (over-approximation)
    pub lambda: HashSet<String>,
    /// Partial closure: defeasibly provable (+∂||)
    pub partial: HashSet<String>,
}

impl ScalableResult {
    /// Convert to conclusions list
    pub fn to_conclusions(&self, indexed: &IndexedTheory) -> Vec<Conclusion> {
        let mut conclusions = Vec::new();

        for lit_key in &self.delta {
            let lit = key_to_literal(lit_key);
            conclusions.push(Conclusion::definitely_provable(lit.clone()));
        }

        for lit_key in &self.partial {
            if !self.delta.contains(lit_key) {
                let lit = key_to_literal(lit_key);
                conclusions.push(Conclusion::defeasibly_provable(lit));
            }
        }

        // Add defeasibly provable for delta items too
        for lit_key in &self.delta {
            let lit = key_to_literal(lit_key);
            conclusions.push(Conclusion::defeasibly_provable(lit));
        }

        // Negative conclusions
        for lit_key in indexed.all_literals() {
            if !self.delta.contains(lit_key) {
                let lit = key_to_literal(lit_key);
                conclusions.push(Conclusion::new(
                    ConclusionType::DefinitelyNotProvable,
                    lit,
                ));
            }
            if !self.partial.contains(lit_key) {
                let lit = key_to_literal(lit_key);
                conclusions.push(Conclusion::new(
                    ConclusionType::DefeasiblyNotProvable,
                    lit,
                ));
            }
        }

        conclusions
    }
}

/// Convert a literal to a canonical key string
fn literal_to_key(lit: &Literal) -> String {
    lit.canonical_name()
}

/// Convert a key string back to a literal
fn key_to_literal(key: &str) -> Literal {
    if let Some(name) = key.strip_prefix('~') {
        Literal::negated(name)
    } else {
        Literal::simple(key)
    }
}

/// Get the complement key of a literal key
fn complement_key(key: &str) -> String {
    if let Some(name) = key.strip_prefix('~') {
        name.to_string()
    } else {
        format!("~{}", key)
    }
}

/// Rule state for tracking body satisfaction
#[derive(Debug, Clone)]
struct RuleState {
    /// Number of unsatisfied body literals
    remaining: usize,
    /// Whether the rule has been activated
    activated: bool,
}

/// Perform scalable DL(d||) reasoning on a theory
pub fn reason_scalable(theory: &Theory) -> ScalableResult {
    let indexed = IndexedTheory::build(theory.clone());

    // Initialize rule states
    let mut states: HashMap<RuleLabel, RuleState> = HashMap::new();
    for rule in theory.rules() {
        states.insert(
            rule.label.clone(),
            RuleState {
                remaining: rule.body.len(),
                activated: false,
            },
        );
    }

    // Phase 1: Delta Closure
    let delta = compute_delta_closure(&indexed, &mut states);

    // Phase 2: Lambda Closure
    let lambda = compute_lambda_closure(&indexed, &delta);

    // Phase 3: Partial Closure
    let partial = compute_partial_closure(&indexed, theory, &delta, &lambda);

    ScalableResult {
        delta,
        lambda,
        partial,
    }
}

/// Phase 1: Compute delta closure (definite conclusions)
///
/// Uses injection-based forward chaining:
/// 1. Inject all facts as initial conclusions
/// 2. For each proven literal, decrement counters for rules containing it
/// 3. When a strict rule's counter reaches 0, inject its head
/// 4. Continue until fixpoint
fn compute_delta_closure(
    indexed: &IndexedTheory,
    states: &mut HashMap<RuleLabel, RuleState>,
) -> HashSet<String> {
    let mut delta: HashSet<String> = HashSet::new();
    let mut worklist: VecDeque<String> = VecDeque::new();

    // Initialize with facts and empty-body strict rules
    for rule in indexed.theory().rules() {
        if rule.rule_type == RuleType::Fact
            || (rule.rule_type == RuleType::Strict && rule.body.is_empty())
        {
            for head_lit in &rule.head {
                let key = literal_to_key(head_lit);
                if !delta.contains(&key) {
                    delta.insert(key.clone());
                    worklist.push_back(key);
                }
            }
        }
    }

    // Forward chaining loop
    while let Some(lit_key) = worklist.pop_front() {
        let lit = key_to_literal(&lit_key);

        // Find rules containing this literal in body
        for rule in indexed.rules_with_body(&lit) {
            let state = states.get_mut(&rule.label).unwrap();

            if !state.activated && state.remaining > 0 {
                state.remaining -= 1;

                // If body fully satisfied and rule is strict, fire it
                if state.remaining == 0 && rule.rule_type == RuleType::Strict {
                    state.activated = true;

                    for head_lit in &rule.head {
                        let head_key = literal_to_key(head_lit);
                        if !delta.contains(&head_key) {
                            delta.insert(head_key.clone());
                            worklist.push_back(head_key);
                        }
                    }
                }
            }
        }
    }

    delta
}

/// Phase 2: Compute lambda closure (potential conclusions - over-approximation)
///
/// Lambda extends delta with defeasible conclusions. A literal can be in lambda if:
/// (1) It's in delta, OR
/// (2) There's a strict/defeasible rule with:
///     (a) All body literals in lambda
///     (b) Complement NOT in delta
fn compute_lambda_closure(indexed: &IndexedTheory, delta: &HashSet<String>) -> HashSet<String> {
    let mut lambda: HashSet<String> = delta.clone();
    let mut worklist: VecDeque<String> = delta.iter().cloned().collect();

    // Track remaining counts for lambda computation
    let mut lambda_remaining: HashMap<RuleLabel, usize> = HashMap::new();
    let mut fired: HashSet<RuleLabel> = HashSet::new();

    for rule in indexed.theory().rules() {
        lambda_remaining.insert(rule.label.clone(), rule.body.len());
    }

    // Handle empty-body defeasible rules
    for rule in indexed.theory().rules() {
        if rule.rule_type == RuleType::Defeasible && rule.body.is_empty() {
            fired.insert(rule.label.clone());
            for head_lit in &rule.head {
                let head_key = literal_to_key(head_lit);
                let comp_key = complement_key(&head_key);

                // Condition: complement not in delta
                if !delta.contains(&comp_key) && !lambda.contains(&head_key) {
                    lambda.insert(head_key.clone());
                    worklist.push_back(head_key);
                }
            }
        }
    }

    // Forward chaining for defeasible rules
    while let Some(lit_key) = worklist.pop_front() {
        let lit = key_to_literal(&lit_key);

        for rule in indexed.rules_with_body(&lit) {
            if fired.contains(&rule.label) {
                continue;
            }

            // Only strict and defeasible rules contribute to lambda
            if rule.rule_type != RuleType::Strict && rule.rule_type != RuleType::Defeasible {
                continue;
            }

            // Check if this literal is actually in the rule's body
            let in_body = rule.body.iter().any(|b| literal_to_key(b) == lit_key);
            if !in_body {
                continue;
            }

            let remaining = lambda_remaining.get_mut(&rule.label).unwrap();
            if *remaining > 0 {
                *remaining -= 1;

                if *remaining == 0 {
                    fired.insert(rule.label.clone());

                    for head_lit in &rule.head {
                        let head_key = literal_to_key(head_lit);
                        let comp_key = complement_key(&head_key);

                        // Add to lambda if complement not in delta
                        if !delta.contains(&comp_key) && !lambda.contains(&head_key) {
                            lambda.insert(head_key.clone());
                            worklist.push_back(head_key);
                        }
                    }
                }
            }
        }
    }

    lambda
}

/// Phase 3: Compute partial closure (defeasible conclusions with conflict resolution)
///
/// A literal q is in partial closure if:
/// (1) q is in delta, OR
/// (2) All of:
///     (a) Some strict/defeasible rule for q has body satisfied in partial
///     (b) ~q not in delta
///     (c) All attacking rules are either:
///         - Body not fully in lambda (can't attack), OR
///         - Defeated by a superior supporting rule
fn compute_partial_closure(
    indexed: &IndexedTheory,
    theory: &Theory,
    delta: &HashSet<String>,
    lambda: &HashSet<String>,
) -> HashSet<String> {
    let mut partial: HashSet<String> = delta.clone();

    // Candidates: literals in lambda but not in delta
    let candidates: Vec<String> = lambda
        .iter()
        .filter(|k| !delta.contains(*k))
        .cloned()
        .collect();

    // Helper: check if rule body is satisfied in partial
    let body_satisfied = |rule: &Rule, partial: &HashSet<String>| -> bool {
        rule.body
            .iter()
            .all(|b| partial.contains(&literal_to_key(b)))
    };

    // Helper: check if attack body is NOT fully in lambda (attack fails)
    let attack_unsatisfied_lambda = |rule: &Rule| -> bool {
        rule.body
            .iter()
            .any(|b| !lambda.contains(&literal_to_key(b)))
    };

    // Helper: check superiority
    let is_superior = |sup_label: &str, inf_label: &str| -> bool {
        theory
            .superiorities()
            .iter()
            .any(|s| s.superior == sup_label && s.inferior == inf_label)
    };

    // Helper: find attacking rules (rules for ~q)
    let get_attacking_rules = |lit_key: &str| -> Vec<&Rule> {
        let comp_key = complement_key(lit_key);
        let comp_lit = key_to_literal(&comp_key);
        indexed.rules_with_head(&comp_lit)
    };

    // Helper: find supporting rules (strict/defeasible rules for q)
    let get_supporting_rules = |lit_key: &str| -> Vec<&Rule> {
        let lit = key_to_literal(lit_key);
        indexed
            .rules_with_head(&lit)
            .into_iter()
            .filter(|r| r.rule_type == RuleType::Strict || r.rule_type == RuleType::Defeasible)
            .collect()
    };

    // Helper: can we defeat the attacker?
    let team_defeats = |lit_key: &str, attacker: &Rule, partial: &HashSet<String>| -> bool {
        for defender in get_supporting_rules(lit_key) {
            if body_satisfied(defender, partial) && is_superior(&defender.label, &attacker.label) {
                return true;
            }
        }
        false
    };

    // Helper: all attacks defeated?
    let all_attacks_defeated = |lit_key: &str, partial: &HashSet<String>| -> bool {
        for attacker in get_attacking_rules(lit_key) {
            // Skip facts
            if attacker.rule_type == RuleType::Fact {
                continue;
            }

            // Attack fails if body not in lambda
            if attack_unsatisfied_lambda(attacker) {
                continue;
            }

            // Attack fails if we have a superior defender
            if team_defeats(lit_key, attacker, partial) {
                continue;
            }

            // This attack succeeds - literal can't be proven
            return false;
        }
        true
    };

    // Helper: can literal be proven defeasibly?
    let can_prove = |lit_key: &str, partial: &HashSet<String>| -> bool {
        // Already in delta
        if delta.contains(lit_key) {
            return true;
        }

        // Complement in delta blocks
        let comp_key = complement_key(lit_key);
        if delta.contains(&comp_key) {
            return false;
        }

        // Need a supporting rule with satisfied body
        let has_support = get_supporting_rules(lit_key)
            .iter()
            .any(|r| body_satisfied(r, partial));

        if !has_support {
            return false;
        }

        // All attacks must be defeated
        all_attacks_defeated(lit_key, partial)
    };

    // Fixpoint iteration
    let mut changed = true;
    while changed {
        changed = false;
        for lit_key in &candidates {
            if !partial.contains(lit_key) && can_prove(lit_key, &partial) {
                partial.insert(lit_key.clone());
                changed = true;
            }
        }
    }

    partial
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_closure_facts() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");

        let result = reason_scalable(&theory);
        assert!(result.delta.contains("a"));
        assert!(result.delta.contains("b"));
        assert_eq!(result.delta.len(), 2);
    }

    #[test]
    fn test_delta_closure_strict_chain() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("q");
        theory.add_strict_rule(&["p", "q"], "r");
        theory.add_strict_rule(&["r"], "s");
        theory.add_defeasible_rule(&["r"], "t"); // Should NOT be in delta

        let result = reason_scalable(&theory);
        assert!(result.delta.contains("p"));
        assert!(result.delta.contains("q"));
        assert!(result.delta.contains("r"));
        assert!(result.delta.contains("s"));
        assert!(!result.delta.contains("t")); // Defeasible not in delta
    }

    #[test]
    fn test_lambda_includes_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");
        theory.add_defeasible_rule(&["q"], "r");
        theory.add_defeasible_rule(&["r"], "s");

        let result = reason_scalable(&theory);

        // Delta: {p, q}
        assert!(result.delta.contains("p"));
        assert!(result.delta.contains("q"));
        assert!(!result.delta.contains("r"));
        assert!(!result.delta.contains("s"));

        // Lambda: {p, q, r, s}
        assert!(result.lambda.contains("p"));
        assert!(result.lambda.contains("q"));
        assert!(result.lambda.contains("r"));
        assert!(result.lambda.contains("s"));
    }

    #[test]
    fn test_lambda_blocks_on_complement() {
        let mut theory = Theory::new();
        theory.add_fact("~q"); // ~q is strictly proven
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let result = reason_scalable(&theory);

        // ~q in delta
        assert!(result.delta.contains("~q"));
        // q should NOT be in lambda (blocked by condition 2.2)
        assert!(!result.lambda.contains("q"));
    }

    #[test]
    fn test_partial_no_conflict() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let result = reason_scalable(&theory);

        // All should be defeasibly provable
        assert!(result.partial.contains("a"));
        assert!(result.partial.contains("b"));
        assert!(result.partial.contains("c"));
    }

    #[test]
    fn test_partial_ambiguity_block() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");

        let result = reason_scalable(&theory);

        // Both q and ~q in lambda (over-approximation)
        assert!(result.lambda.contains("q"));
        assert!(result.lambda.contains("~q"));

        // Neither in partial (ambiguity block)
        assert!(result.partial.contains("p"));
        assert!(!result.partial.contains("q"));
        assert!(!result.partial.contains("~q"));
    }

    #[test]
    fn test_partial_superiority_resolves() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");
        theory.add_superiority(&r1, &r2); // q wins

        let result = reason_scalable(&theory);

        // q should win via superiority
        assert!(result.partial.contains("q"));
        assert!(!result.partial.contains("~q"));
    }

    #[test]
    fn test_tweety_triangle() {
        let mut theory = Theory::new();
        theory.add_fact("bird_tweety");
        theory.add_fact("penguin_tweety");
        theory.add_fact("bird_eddie");

        let r1 = theory.add_defeasible_rule(&["bird_tweety"], "flies_tweety");
        let r2 = theory.add_defeasible_rule(&["penguin_tweety"], "~flies_tweety");
        theory.add_defeasible_rule(&["bird_eddie"], "flies_eddie");

        theory.add_superiority(&r2, &r1); // Penguin beats bird

        let result = reason_scalable(&theory);

        // Eddie flies (no conflict)
        assert!(result.partial.contains("flies_eddie"));
        // Tweety doesn't fly (penguin wins)
        assert!(result.partial.contains("~flies_tweety"));
        assert!(!result.partial.contains("flies_tweety"));
    }

    #[test]
    fn test_attack_fails_body_not_in_lambda() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["x"], "~q"); // x never proven

        let result = reason_scalable(&theory);

        // x not in lambda, so attack fails
        assert!(!result.lambda.contains("x"));
        // q should be proven
        assert!(result.partial.contains("q"));
    }

    // ==========================================================================
    // SEMANTIC EQUIVALENCE TESTS (Standard vs Scalable)
    // ==========================================================================

    use crate::reason::reason;

    /// Helper to extract defeasibly provable literals from standard reason()
    fn extract_defeasible_provable(conclusions: &[Conclusion]) -> HashSet<String> {
        conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
            .map(|c| c.literal.canonical_name())
            .collect()
    }

    /// Helper to extract definitely provable literals from standard reason()
    fn extract_definite_provable(conclusions: &[Conclusion]) -> HashSet<String> {
        conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
            .map(|c| c.literal.canonical_name())
            .collect()
    }

    #[test]
    fn test_semantic_empty_theory() {
        let theory = Theory::new();

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);
        let scl_def = scalable.partial.clone();

        assert_eq!(std_def.len(), 0, "Standard: empty theory -> no +d");
        assert_eq!(scl_def.len(), 0, "Scalable: empty theory -> no +d");
    }

    #[test]
    fn test_semantic_single_fact() {
        let mut theory = Theory::new();
        theory.add_fact("p");

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);

        assert!(std_def.contains("p"), "Standard: p should be +d");
        assert!(scalable.partial.contains("p"), "Scalable: p should be +d");
        assert!(scalable.delta.contains("p"), "Scalable: p should be +D");
    }

    #[test]
    fn test_semantic_simple_chain() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);

        for lit in &["a", "b", "c"] {
            assert!(
                std_def.contains(*lit),
                "Standard: {} should be +d",
                lit
            );
            assert!(
                scalable.partial.contains(*lit),
                "Scalable: {} should be +d",
                lit
            );
        }
    }

    #[test]
    fn test_semantic_definite_match() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");
        theory.add_strict_rule(&["q"], "r");

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_definite = extract_definite_provable(&standard);

        for lit in &["p", "q", "r"] {
            assert!(
                std_definite.contains(*lit),
                "Standard: {} should be +D",
                lit
            );
            assert!(
                scalable.delta.contains(*lit),
                "Scalable: {} should be +D (in delta)",
                lit
            );
        }
    }

    #[test]
    fn test_semantic_mixed_chain() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");
        theory.add_defeasible_rule(&["q"], "r");

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);
        let std_definite = extract_definite_provable(&standard);

        // p, q definitely provable
        assert!(std_definite.contains("p"));
        assert!(std_definite.contains("q"));
        assert!(scalable.delta.contains("p"));
        assert!(scalable.delta.contains("q"));

        // r defeasibly provable
        assert!(std_def.contains("r"), "Standard: r should be +d");
        assert!(scalable.partial.contains("r"), "Scalable: r should be +d");
    }

    #[test]
    fn test_semantic_superiority_resolves() {
        let mut theory = Theory::new();
        theory.add_fact("trigger");

        let r1 = theory.add_defeasible_rule(&["trigger"], "result");
        let r2 = theory.add_defeasible_rule(&["trigger"], "~result");

        theory.add_superiority(&r1, &r2);

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);

        // r1 > r2, so result should win
        assert!(
            std_def.contains("result"),
            "Standard: result should be +d (superior)"
        );
        assert!(
            scalable.partial.contains("result"),
            "Scalable: result should be +d (superior)"
        );
        assert!(
            !scalable.partial.contains("~result"),
            "Scalable: ~result should NOT be +d"
        );
    }

    #[test]
    fn test_semantic_tweety_triangle() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");

        theory.add_superiority(&r2, &r1);

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);

        // Penguin wins - ~flies provable
        assert!(
            std_def.contains("~flies"),
            "Standard: ~flies should be +d"
        );
        assert!(
            scalable.partial.contains("~flies"),
            "Scalable: ~flies should be +d"
        );
        assert!(
            !scalable.partial.contains("flies"),
            "Scalable: flies should NOT be +d"
        );
    }

    #[test]
    fn test_semantic_long_chain() {
        let mut theory = Theory::new();
        theory.add_fact("l0");

        for i in 0..10 {
            theory.add_defeasible_rule(&[&format!("l{}", i)], &format!("l{}", i + 1));
        }

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);

        // All chain literals should be provable in both modes
        for i in 0..=10 {
            let lit = format!("l{}", i);
            assert!(
                std_def.contains(&lit),
                "Standard: {} should be +d",
                lit
            );
            assert!(
                scalable.partial.contains(&lit),
                "Scalable: {} should be +d",
                lit
            );
        }
    }

    #[test]
    fn test_semantic_wide_theory() {
        let mut theory = Theory::new();

        for i in 0..20 {
            theory.add_fact(&format!("fact{}", i));
            theory.add_defeasible_rule(&[&format!("fact{}", i)], &format!("derived{}", i));
        }

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);

        for i in 0..20 {
            let fact = format!("fact{}", i);
            let derived = format!("derived{}", i);

            assert!(std_def.contains(&fact), "Standard: {} should be +d", fact);
            assert!(
                std_def.contains(&derived),
                "Standard: {} should be +d",
                derived
            );
            assert!(
                scalable.partial.contains(&fact),
                "Scalable: {} should be +d",
                fact
            );
            assert!(
                scalable.partial.contains(&derived),
                "Scalable: {} should be +d",
                derived
            );
        }
    }

    #[test]
    fn test_closure_relationships() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "d");

        let result = reason_scalable(&theory);

        // Delta ⊆ Partial (definite implies defeasible)
        for lit in &result.delta {
            assert!(
                result.partial.contains(lit),
                "Delta {} should be in Partial",
                lit
            );
        }

        // Partial ⊆ Lambda (partial is refined from lambda)
        for lit in &result.partial {
            assert!(
                result.lambda.contains(lit),
                "Partial {} should be in Lambda",
                lit
            );
        }
    }

    // ==========================================================================
    // STRESS TESTS
    // ==========================================================================

    #[test]
    fn test_scalable_long_chain_performance() {
        let mut theory = Theory::new();
        theory.add_fact("l0");

        for i in 0..100 {
            theory.add_defeasible_rule(&[&format!("l{}", i)], &format!("l{}", i + 1));
        }

        let result = reason_scalable(&theory);

        assert!(
            result.partial.contains("l100"),
            "l100 should be provable through long chain"
        );
    }

    #[test]
    fn test_scalable_wide_theory_performance() {
        let mut theory = Theory::new();

        for i in 0..200 {
            theory.add_fact(&format!("fact{}", i));
            theory.add_defeasible_rule(&[&format!("fact{}", i)], &format!("derived{}", i));
        }

        let result = reason_scalable(&theory);

        assert!(
            result.partial.contains("derived199"),
            "derived199 should be provable"
        );
    }

    #[test]
    fn test_scalable_many_conflicts_with_superiority() {
        let mut theory = Theory::new();
        theory.add_fact("trigger");

        // Create 50 conflicts, all resolved by superiority
        for i in 0..50 {
            let r1 = theory.add_defeasible_rule(&["trigger"], &format!("q{}", i));
            let r2 = theory.add_defeasible_rule(&["trigger"], &format!("~q{}", i));
            theory.add_superiority(&r1, &r2);
        }

        let result = reason_scalable(&theory);

        // All positive conclusions should win
        for i in 0..50 {
            assert!(
                result.partial.contains(&format!("q{}", i)),
                "q{} should be provable via superiority",
                i
            );
            assert!(
                !result.partial.contains(&format!("~q{}", i)),
                "~q{} should NOT be provable",
                i
            );
        }
    }
}
