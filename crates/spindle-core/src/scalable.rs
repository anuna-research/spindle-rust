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
//! # Performance
//!
//! Uses `LiteralId` (4-byte Copy type) for HashSet keys instead of `String`,
//! eliminating heap allocations in the hot closure computation loops.
//!
//! References:
//! - SPINdle-Racket v1.7.0 scalable/closures.rkt
//! - Allen, J.F. "Maintaining Knowledge about Temporal Intervals" (1983)

use std::collections::{HashMap, HashSet, VecDeque};

use crate::conclusion::{Conclusion, ConclusionType};
use crate::index::IndexedTheory;
use crate::intern::LiteralId;
use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel, RuleType};
use crate::theory::Theory;

/// Result of scalable reasoning: three closure sets
///
/// Uses `LiteralId` (4-byte Copy type) for efficient set operations.
#[derive(Debug, Clone)]
pub struct ScalableResult {
    /// Delta closure: definitely provable (+Δ)
    pub delta: HashSet<LiteralId>,
    /// Lambda closure: potentially provable (over-approximation)
    pub lambda: HashSet<LiteralId>,
    /// Partial closure: defeasibly provable (+∂||)
    pub partial: HashSet<LiteralId>,
}

impl ScalableResult {
    /// Convert to conclusions list
    pub fn to_conclusions(&self, indexed: &IndexedTheory) -> Vec<Conclusion> {
        let mut conclusions = Vec::new();

        for &lit_id in &self.delta {
            let lit = id_to_literal(lit_id);
            conclusions.push(Conclusion::definitely_provable(lit.clone()));
        }

        for &lit_id in &self.partial {
            if !self.delta.contains(&lit_id) {
                let lit = id_to_literal(lit_id);
                conclusions.push(Conclusion::defeasibly_provable(lit));
            }
        }

        // Add defeasibly provable for delta items too
        for &lit_id in &self.delta {
            let lit = id_to_literal(lit_id);
            conclusions.push(Conclusion::defeasibly_provable(lit));
        }

        // Negative conclusions
        for &lit_id in indexed.all_literal_ids() {
            let lit = id_to_literal(lit_id);
            if !self.delta.contains(&lit_id) {
                conclusions.push(Conclusion::new(
                    ConclusionType::DefinitelyNotProvable,
                    lit.clone(),
                ));
            }
            if !self.partial.contains(&lit_id) {
                conclusions.push(Conclusion::new(
                    ConclusionType::DefeasiblyNotProvable,
                    lit,
                ));
            }
        }

        conclusions
    }

    /// Check if a literal (by canonical name string) is in delta
    ///
    /// This is a convenience method for backward compatibility with tests.
    pub fn contains_delta(&self, canonical: &str) -> bool {
        let lit = key_to_literal(canonical);
        self.delta.contains(&lit.literal_id())
    }

    /// Check if a literal (by canonical name string) is in lambda
    pub fn contains_lambda(&self, canonical: &str) -> bool {
        let lit = key_to_literal(canonical);
        self.lambda.contains(&lit.literal_id())
    }

    /// Check if a literal (by canonical name string) is in partial
    pub fn contains_partial(&self, canonical: &str) -> bool {
        let lit = key_to_literal(canonical);
        self.partial.contains(&lit.literal_id())
    }
}

/// Convert a LiteralId back to a Literal
#[inline]
fn id_to_literal(id: LiteralId) -> Literal {
    let name = crate::intern::resolve(id.symbol());
    if id.is_negated() {
        Literal::negated(name)
    } else {
        Literal::simple(name)
    }
}

/// Convert a key string back to a literal (for backward compatibility)
fn key_to_literal(key: &str) -> Literal {
    if let Some(name) = key.strip_prefix('~') {
        Literal::negated(name)
    } else {
        Literal::simple(key)
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
) -> HashSet<LiteralId> {
    let mut delta: HashSet<LiteralId> = HashSet::new();
    let mut worklist: VecDeque<LiteralId> = VecDeque::new();

    // Initialize with facts and empty-body strict rules
    for rule in indexed.theory().rules() {
        if rule.rule_type == RuleType::Fact
            || (rule.rule_type == RuleType::Strict && rule.body.is_empty())
        {
            for head_lit in &rule.head {
                let lit_id = head_lit.literal_id();
                if !delta.contains(&lit_id) {
                    delta.insert(lit_id);
                    worklist.push_back(lit_id);
                }
            }
        }
    }

    // Forward chaining loop
    while let Some(lit_id) = worklist.pop_front() {
        let lit = id_to_literal(lit_id);

        // Find rules containing this literal in body
        for rule in indexed.rules_with_body(&lit) {
            let state = states.get_mut(&rule.label).unwrap();

            if !state.activated && state.remaining > 0 {
                state.remaining -= 1;

                // If body fully satisfied and rule is strict, fire it
                if state.remaining == 0 && rule.rule_type == RuleType::Strict {
                    state.activated = true;

                    for head_lit in &rule.head {
                        let head_id = head_lit.literal_id();
                        if !delta.contains(&head_id) {
                            delta.insert(head_id);
                            worklist.push_back(head_id);
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
fn compute_lambda_closure(indexed: &IndexedTheory, delta: &HashSet<LiteralId>) -> HashSet<LiteralId> {
    let mut lambda: HashSet<LiteralId> = delta.clone();
    let mut worklist: VecDeque<LiteralId> = delta.iter().copied().collect();

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
                let head_id = head_lit.literal_id();
                let comp_id = head_id.complement();

                // Condition: complement not in delta
                if !delta.contains(&comp_id) && !lambda.contains(&head_id) {
                    lambda.insert(head_id);
                    worklist.push_back(head_id);
                }
            }
        }
    }

    // Forward chaining for defeasible rules
    while let Some(lit_id) = worklist.pop_front() {
        let lit = id_to_literal(lit_id);

        for rule in indexed.rules_with_body(&lit) {
            if fired.contains(&rule.label) {
                continue;
            }

            // Only strict and defeasible rules contribute to lambda
            if rule.rule_type != RuleType::Strict && rule.rule_type != RuleType::Defeasible {
                continue;
            }

            // Check if this literal is actually in the rule's body
            let in_body = rule.body.iter().any(|b| b.literal_id() == lit_id);
            if !in_body {
                continue;
            }

            let remaining = lambda_remaining.get_mut(&rule.label).unwrap();
            if *remaining > 0 {
                *remaining -= 1;

                if *remaining == 0 {
                    fired.insert(rule.label.clone());

                    for head_lit in &rule.head {
                        let head_id = head_lit.literal_id();
                        let comp_id = head_id.complement();

                        // Add to lambda if complement not in delta
                        if !delta.contains(&comp_id) && !lambda.contains(&head_id) {
                            lambda.insert(head_id);
                            worklist.push_back(head_id);
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
/// Uses semi-naive evaluation: when a new literal is proven, only rules
/// triggered by that literal are re-evaluated, rather than all candidates.
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
    delta: &HashSet<LiteralId>,
    lambda: &HashSet<LiteralId>,
) -> HashSet<LiteralId> {
    use std::collections::VecDeque;
    use rustc_hash::FxHashMap;

    let mut partial: HashSet<LiteralId> = delta.clone();

    // Track remaining unsatisfied body literals for each rule
    let mut remaining: FxHashMap<&str, usize> = FxHashMap::default();
    for rule in theory.rules() {
        if rule.rule_type == RuleType::Strict || rule.rule_type == RuleType::Defeasible {
            // Count body literals not yet in partial
            let unsatisfied = rule.body.iter()
                .filter(|b| !partial.contains(&b.literal_id()))
                .count();
            remaining.insert(&rule.label, unsatisfied);
        }
    }

    // Candidates: literals in lambda but not in delta, blocked by complement in delta
    let blocked_by_delta: HashSet<LiteralId> = lambda
        .iter()
        .filter(|k| delta.contains(&k.complement()))
        .copied()
        .collect();

    // Helper: check if rule body is satisfied in partial
    let body_satisfied = |rule: &Rule, partial: &HashSet<LiteralId>| -> bool {
        rule.body.iter().all(|b| partial.contains(&b.literal_id()))
    };

    // Helper: check if attack body is NOT fully in lambda (attack fails)
    let attack_unsatisfied_lambda = |rule: &Rule| -> bool {
        rule.body.iter().any(|b| !lambda.contains(&b.literal_id()))
    };

    // Helper: can we defeat the attacker using superiority?
    let team_defeats = |lit_id: LiteralId, attacker: &Rule, partial: &HashSet<LiteralId>| -> bool {
        for defender in indexed.rules_with_head_id(lit_id) {
            if (defender.rule_type == RuleType::Strict || defender.rule_type == RuleType::Defeasible)
                && body_satisfied(defender, partial)
                && theory.is_superior(&defender.label, &attacker.label)
            {
                return true;
            }
        }
        false
    };

    // Helper: all attacks defeated?
    let all_attacks_defeated = |lit_id: LiteralId, partial: &HashSet<LiteralId>| -> bool {
        for attacker in indexed.rules_with_head_id(lit_id.complement()) {
            // Skip facts
            if attacker.rule_type == RuleType::Fact {
                continue;
            }
            // Attack fails if body not in lambda
            if attack_unsatisfied_lambda(attacker) {
                continue;
            }
            // Attack fails if we have a superior defender
            if team_defeats(lit_id, attacker, partial) {
                continue;
            }
            // This attack succeeds - literal can't be proven
            return false;
        }
        true
    };

    // Helper: can literal be proven defeasibly?
    let can_prove = |lit_id: LiteralId, partial: &HashSet<LiteralId>| -> bool {
        // Already in delta
        if delta.contains(&lit_id) {
            return true;
        }
        // Complement in delta blocks (precomputed)
        if blocked_by_delta.contains(&lit_id) {
            return false;
        }
        // Need a supporting rule with satisfied body
        let has_support = indexed.rules_with_head_id(lit_id)
            .iter()
            .any(|r| {
                (r.rule_type == RuleType::Strict || r.rule_type == RuleType::Defeasible)
                    && body_satisfied(r, partial)
            });
        if !has_support {
            return false;
        }
        // All attacks must be defeated
        all_attacks_defeated(lit_id, partial)
    };

    // Worklist of literals to process (semi-naive: only process triggered literals)
    let mut worklist: VecDeque<LiteralId> = VecDeque::new();

    // Initialize worklist with rules that have body fully satisfied
    for rule in theory.rules() {
        if (rule.rule_type == RuleType::Strict || rule.rule_type == RuleType::Defeasible)
            && remaining.get(rule.label.as_str()) == Some(&0)
        {
            for head_lit in &rule.head {
                let head_id = head_lit.literal_id();
                if lambda.contains(&head_id) && !partial.contains(&head_id) {
                    worklist.push_back(head_id);
                }
            }
        }
    }

    // Track what's already in worklist to avoid duplicates
    let mut in_worklist: HashSet<LiteralId> = worklist.iter().copied().collect();

    // Semi-naive iteration
    while let Some(lit_id) = worklist.pop_front() {
        in_worklist.remove(&lit_id);

        // Skip if already proven or not a candidate
        if partial.contains(&lit_id) || !lambda.contains(&lit_id) {
            continue;
        }

        // Try to prove this literal
        if can_prove(lit_id, &partial) {
            partial.insert(lit_id);

            // Trigger rules that have this literal in body
            for rule in indexed.rules_with_body_id(lit_id) {
                if let Some(rem) = remaining.get_mut(rule.label.as_str())
                    && *rem > 0 {
                    *rem -= 1;
                    if *rem == 0 {
                        // Rule body now satisfied - add head to worklist
                        for head_lit in &rule.head {
                            let head_id = head_lit.literal_id();
                            if lambda.contains(&head_id)
                                && !partial.contains(&head_id)
                                && !in_worklist.contains(&head_id)
                            {
                                worklist.push_back(head_id);
                                in_worklist.insert(head_id);
                            }
                        }
                    }
                }
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
        assert!(result.contains_delta("a"));
        assert!(result.contains_delta("b"));
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
        assert!(result.contains_delta("p"));
        assert!(result.contains_delta("q"));
        assert!(result.contains_delta("r"));
        assert!(result.contains_delta("s"));
        assert!(!result.contains_delta("t")); // Defeasible not in delta
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
        assert!(result.contains_delta("p"));
        assert!(result.contains_delta("q"));
        assert!(!result.contains_delta("r"));
        assert!(!result.contains_delta("s"));

        // Lambda: {p, q, r, s}
        assert!(result.contains_lambda("p"));
        assert!(result.contains_lambda("q"));
        assert!(result.contains_lambda("r"));
        assert!(result.contains_lambda("s"));
    }

    #[test]
    fn test_lambda_blocks_on_complement() {
        let mut theory = Theory::new();
        theory.add_fact("~q"); // ~q is strictly proven
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let result = reason_scalable(&theory);

        // ~q in delta
        assert!(result.contains_delta("~q"));
        // q should NOT be in lambda (blocked by condition 2.2)
        assert!(!result.contains_lambda("q"));
    }

    #[test]
    fn test_partial_no_conflict() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let result = reason_scalable(&theory);

        // All should be defeasibly provable
        assert!(result.contains_partial("a"));
        assert!(result.contains_partial("b"));
        assert!(result.contains_partial("c"));
    }

    #[test]
    fn test_partial_ambiguity_block() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");

        let result = reason_scalable(&theory);

        // Both q and ~q in lambda (over-approximation)
        assert!(result.contains_lambda("q"));
        assert!(result.contains_lambda("~q"));

        // Neither in partial (ambiguity block)
        assert!(result.contains_partial("p"));
        assert!(!result.contains_partial("q"));
        assert!(!result.contains_partial("~q"));
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
        assert!(result.contains_partial("q"));
        assert!(!result.contains_partial("~q"));
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
        assert!(result.contains_partial("flies_eddie"));
        // Tweety doesn't fly (penguin wins)
        assert!(result.contains_partial("~flies_tweety"));
        assert!(!result.contains_partial("flies_tweety"));
    }

    #[test]
    fn test_attack_fails_body_not_in_lambda() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["x"], "~q"); // x never proven

        let result = reason_scalable(&theory);

        // x not in lambda, so attack fails
        assert!(!result.contains_lambda("x"));
        // q should be proven
        assert!(result.contains_partial("q"));
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
        assert!(scalable.contains_partial("p"), "Scalable: p should be +d");
        assert!(scalable.contains_delta("p"), "Scalable: p should be +D");
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
                scalable.contains_partial(*lit),
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
                scalable.contains_delta(*lit),
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
        assert!(scalable.contains_delta("p"));
        assert!(scalable.contains_delta("q"));

        // r defeasibly provable
        assert!(std_def.contains("r"), "Standard: r should be +d");
        assert!(scalable.contains_partial("r"), "Scalable: r should be +d");
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
            scalable.contains_partial("result"),
            "Scalable: result should be +d (superior)"
        );
        assert!(
            !scalable.contains_partial("~result"),
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
            scalable.contains_partial("~flies"),
            "Scalable: ~flies should be +d"
        );
        assert!(
            !scalable.contains_partial("flies"),
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
                scalable.contains_partial(&lit),
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
                scalable.contains_partial(&fact),
                "Scalable: {} should be +d",
                fact
            );
            assert!(
                scalable.contains_partial(&derived),
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
        for &lit_id in &result.delta {
            assert!(
                result.partial.contains(&lit_id),
                "Delta {:?} should be in Partial",
                lit_id
            );
        }

        // Partial ⊆ Lambda (partial is refined from lambda)
        for &lit_id in &result.partial {
            assert!(
                result.lambda.contains(&lit_id),
                "Partial {:?} should be in Lambda",
                lit_id
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
            result.contains_partial("l100"),
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
            result.contains_partial("derived199"),
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
                result.contains_partial(&format!("q{}", i)),
                "q{} should be provable via superiority",
                i
            );
            assert!(
                !result.contains_partial(&format!("~q{}", i)),
                "~q{} should NOT be provable",
                i
            );
        }
    }
}
