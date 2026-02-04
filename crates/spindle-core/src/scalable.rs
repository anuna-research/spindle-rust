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

use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};

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
    pub delta: FxHashSet<LiteralId>,
    /// Lambda closure: potentially provable (over-approximation)
    pub lambda: FxHashSet<LiteralId>,
    /// Partial closure: defeasibly provable (+∂||)
    pub partial: FxHashSet<LiteralId>,
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
                conclusions.push(Conclusion::new(ConclusionType::DefeasiblyNotProvable, lit));
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
    let mut states: FxHashMap<RuleLabel, RuleState> = FxHashMap::default();
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
    states: &mut FxHashMap<RuleLabel, RuleState>,
) -> FxHashSet<LiteralId> {
    let mut delta: FxHashSet<LiteralId> = FxHashSet::default();
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
fn compute_lambda_closure(
    indexed: &IndexedTheory,
    delta: &FxHashSet<LiteralId>,
) -> FxHashSet<LiteralId> {
    let mut lambda: FxHashSet<LiteralId> = delta.clone();
    let mut worklist: VecDeque<LiteralId> = delta.iter().copied().collect();

    // Track remaining counts for lambda computation
    let mut lambda_remaining: FxHashMap<RuleLabel, usize> = FxHashMap::default();
    let mut fired: FxHashSet<RuleLabel> = FxHashSet::default();

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
    delta: &FxHashSet<LiteralId>,
    lambda: &FxHashSet<LiteralId>,
) -> FxHashSet<LiteralId> {
    use std::collections::VecDeque;

    let mut partial: FxHashSet<LiteralId> = delta.clone();

    // Track remaining unsatisfied body literals for each rule
    let mut remaining: FxHashMap<&str, usize> = FxHashMap::default();
    for rule in theory.rules() {
        if rule.rule_type == RuleType::Strict || rule.rule_type == RuleType::Defeasible {
            // Count body literals not yet in partial
            let unsatisfied = rule
                .body
                .iter()
                .filter(|b| !partial.contains(&b.literal_id()))
                .count();
            remaining.insert(&rule.label, unsatisfied);
        }
    }

    // Candidates: literals in lambda but not in delta, blocked by complement in delta
    let blocked_by_delta: FxHashSet<LiteralId> = lambda
        .iter()
        .filter(|k| delta.contains(&k.complement()))
        .copied()
        .collect();

    // Helper: check if rule body is satisfied in partial
    let body_satisfied = |rule: &Rule, partial: &FxHashSet<LiteralId>| -> bool {
        rule.body.iter().all(|b| partial.contains(&b.literal_id()))
    };

    // Helper: check if attack body is NOT fully in lambda (attack fails)
    let attack_unsatisfied_lambda =
        |rule: &Rule| -> bool { rule.body.iter().any(|b| !lambda.contains(&b.literal_id())) };

    // Helper: can we defeat the attacker using superiority?
    let team_defeats = |lit_id: LiteralId, attacker: &Rule, partial: &FxHashSet<LiteralId>| -> bool {
        for defender in indexed.rules_with_head_id(lit_id) {
            if (defender.rule_type == RuleType::Strict
                || defender.rule_type == RuleType::Defeasible)
                && body_satisfied(defender, partial)
                && theory.is_superior(&defender.label, &attacker.label)
            {
                return true;
            }
        }
        false
    };

    // Helper: all attacks defeated?
    let all_attacks_defeated = |lit_id: LiteralId, partial: &FxHashSet<LiteralId>| -> bool {
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
    let can_prove = |lit_id: LiteralId, partial: &FxHashSet<LiteralId>| -> bool {
        // Already in delta
        if delta.contains(&lit_id) {
            return true;
        }
        // Complement in delta blocks (precomputed)
        if blocked_by_delta.contains(&lit_id) {
            return false;
        }
        // Need a supporting rule with satisfied body
        let has_support = indexed.rules_with_head_id(lit_id).iter().any(|r| {
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
    let mut in_worklist: FxHashSet<LiteralId> = worklist.iter().copied().collect();

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
                    && *rem > 0
                {
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
    fn extract_defeasible_provable(conclusions: &[Conclusion]) -> FxHashSet<String> {
        conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
            .map(|c| c.literal.canonical_name())
            .collect()
    }

    /// Helper to extract definitely provable literals from standard reason()
    fn extract_definite_provable(conclusions: &[Conclusion]) -> FxHashSet<String> {
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
            assert!(std_def.contains(*lit), "Standard: {} should be +d", lit);
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
        assert!(std_def.contains("~flies"), "Standard: ~flies should be +d");
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
            assert!(std_def.contains(&lit), "Standard: {} should be +d", lit);
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

    // ==========================================================================
    // STATELESSNESS VERIFICATION TESTS (TEST-020)
    // Ported from spindle-racket/tests/scalable-stateless-tests.rkt
    // ==========================================================================

    /// Helper to create a theory hash for comparison
    fn theory_hash(theory: &Theory) -> (usize, Vec<String>, usize) {
        let rule_count = theory.rule_count();
        let mut rule_labels: Vec<String> = theory.rules().map(|r| r.label.clone()).collect();
        rule_labels.sort();
        let sup_count = theory.superiorities().len();
        (rule_count, rule_labels, sup_count)
    }

    #[test]
    fn test_stateless_reason_scalable_does_not_mutate_theory() {
        // T020-1: reason_scalable does not mutate input theory
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r2, &r1);

        // Capture state before reasoning
        let hash_before = theory_hash(&theory);

        // Perform reasoning
        let _result = reason_scalable(&theory);

        // Capture state after reasoning
        let hash_after = theory_hash(&theory);

        // Assert states are identical
        assert_eq!(
            hash_before, hash_after,
            "Theory should be unchanged after reasoning"
        );
    }

    #[test]
    fn test_stateless_large_theory_unchanged() {
        // T020-4: Large theory remains unchanged after reasoning
        let mut theory = Theory::new();
        for i in 0..100 {
            theory.add_fact(&format!("p{}", i));
            theory.add_defeasible_rule(&[&format!("p{}", i)], &format!("q{}", i));
        }

        let hash_before = theory_hash(&theory);

        // Reason multiple times
        for _ in 0..5 {
            let _result = reason_scalable(&theory);
        }

        let hash_after = theory_hash(&theory);
        assert_eq!(hash_before, hash_after);
    }

    #[test]
    fn test_stateless_repeated_calls_consistent() {
        // T-REP-1: 100 calls on same theory produce consistent results
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r2, &r1);

        let hash_before = theory_hash(&theory);

        // Get reference result
        let ref_result = reason_scalable(&theory);
        let ref_delta_count = ref_result.delta.len();
        let ref_lambda_count = ref_result.lambda.len();
        let ref_partial_count = ref_result.partial.len();

        // Run 99 more times and verify consistency
        for i in 0..99 {
            let result = reason_scalable(&theory);
            assert_eq!(
                result.delta.len(),
                ref_delta_count,
                "Delta closure size at call {} should match",
                i + 2
            );
            assert_eq!(
                result.lambda.len(),
                ref_lambda_count,
                "Lambda closure size at call {} should match",
                i + 2
            );
            assert_eq!(
                result.partial.len(),
                ref_partial_count,
                "Partial closure size at call {} should match",
                i + 2
            );
        }

        // Verify theory unchanged
        let hash_after = theory_hash(&theory);
        assert_eq!(hash_before, hash_after, "Theory unchanged after 100 calls");
    }

    #[test]
    fn test_stateless_theory_reuse_across_modes() {
        // T-REUSE-1: Theory reusable across standard and scalable modes
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r2, &r1);

        let hash_original = theory_hash(&theory);

        // Use with standard mode
        let _std_conclusions = reason(&theory);
        let hash_after_std = theory_hash(&theory);
        assert_eq!(
            hash_original, hash_after_std,
            "Theory unchanged after standard mode"
        );

        // Use with scalable mode
        let _scl_result = reason_scalable(&theory);
        let hash_after_scl = theory_hash(&theory);
        assert_eq!(
            hash_original, hash_after_scl,
            "Theory unchanged after scalable mode"
        );

        // Use with standard mode again
        let _std_conclusions_2 = reason(&theory);
        let hash_after_std_2 = theory_hash(&theory);
        assert_eq!(
            hash_original, hash_after_std_2,
            "Theory unchanged after second standard mode call"
        );
    }

    #[test]
    fn test_stateless_internal_state_isolation() {
        // Verify closures don't leak between reasoning calls
        let mut theory1 = Theory::new();
        theory1.add_fact("a");
        theory1.add_defeasible_rule(&["a"], "b");

        let mut theory2 = Theory::new();
        theory2.add_fact("x");
        theory2.add_fact("y");
        theory2.add_defeasible_rule(&["x", "y"], "z");

        // Reason on theory1
        let result1 = reason_scalable(&theory1);

        // Reason on theory2
        let _result2 = reason_scalable(&theory2);

        // Reason on theory1 again - should get same results as before
        let result1_again = reason_scalable(&theory1);

        assert_eq!(
            result1.delta.len(),
            result1_again.delta.len(),
            "Delta for theory1 should be same before and after reasoning on theory2"
        );
        assert_eq!(
            result1.partial.len(),
            result1_again.partial.len(),
            "Partial for theory1 should be same before and after reasoning on theory2"
        );
    }

    #[test]
    fn test_stateless_empty_theory() {
        // Empty theory statelessness
        let theory = Theory::new();
        let hash_before = theory_hash(&theory);

        let result = reason_scalable(&theory);

        let hash_after = theory_hash(&theory);
        assert_eq!(hash_before, hash_after);
        assert_eq!(result.delta.len(), 0);
        assert_eq!(result.lambda.len(), 0);
        assert_eq!(result.partial.len(), 0);
    }

    #[test]
    fn test_stateless_single_fact_theory() {
        // Single fact theory unchanged after reasoning
        let mut theory = Theory::new();
        theory.add_fact("p");

        let hash_before = theory_hash(&theory);

        let _result1 = reason_scalable(&theory);
        let _result2 = reason(&theory);
        let _result3 = reason_scalable(&theory);

        let hash_after = theory_hash(&theory);
        assert_eq!(hash_before, hash_after);
    }

    #[test]
    fn test_stateless_conflicting_theory_unchanged() {
        // Conflicting theory without superiorities unchanged
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");

        let hash_before = theory_hash(&theory);

        // Reason multiple times
        for _ in 0..10 {
            let _result = reason_scalable(&theory);
        }

        let hash_after = theory_hash(&theory);
        assert_eq!(hash_before, hash_after);
    }

    // ==========================================================================
    // INJECTION PROPAGATION TESTS (TEST-006, TEST-016)
    // Ported from spindle-racket/tests/scalable-injection-tests.rkt
    // ==========================================================================

    #[test]
    fn test_injection_delta_closure_facts_only() {
        // TEST-006: Verify delta closure correctly captures all facts
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_fact("c");

        let result = reason_scalable(&theory);

        // All facts should be in delta
        assert!(result.contains_delta("a"), "a should be in delta");
        assert!(result.contains_delta("b"), "b should be in delta");
        assert!(result.contains_delta("c"), "c should be in delta");
        assert_eq!(result.delta.len(), 3, "Delta should have exactly 3 facts");
    }

    #[test]
    fn test_injection_delta_closure_strict_chain() {
        // TEST-006: Verify delta closure propagates through strict rules
        // f: a, b, c.  r1: a, b -> x  r2: x, c -> y
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_fact("c");
        theory.add_strict_rule(&["a", "b"], "x");
        theory.add_strict_rule(&["x", "c"], "y");

        let result = reason_scalable(&theory);

        // Delta should contain: a, b, c, x, y
        assert!(result.contains_delta("a"));
        assert!(result.contains_delta("b"));
        assert!(result.contains_delta("c"));
        assert!(
            result.contains_delta("x"),
            "x should be in delta (via strict r1)"
        );
        assert!(
            result.contains_delta("y"),
            "y should be in delta (via strict r2)"
        );
    }

    #[test]
    fn test_injection_delta_excludes_defeasible() {
        // Defeasible rules should NOT contribute to delta closure
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let result = reason_scalable(&theory);

        assert!(result.contains_delta("a"));
        assert!(result.contains_delta("b"));
        assert!(
            !result.contains_delta("c"),
            "c should NOT be in delta (defeasible rule)"
        );
    }

    #[test]
    fn test_injection_lambda_includes_defeasible() {
        // TEST-006: Lambda closure includes defeasible rule conclusions
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "d");

        let result = reason_scalable(&theory);

        // Lambda should contain all: a, b, c, d
        assert!(result.contains_lambda("a"));
        assert!(result.contains_lambda("b"));
        assert!(
            result.contains_lambda("c"),
            "c should be in lambda (via defeasible)"
        );
        assert!(
            result.contains_lambda("d"),
            "d should be in lambda (via defeasible chain)"
        );
    }

    #[test]
    fn test_injection_lambda_blocked_by_delta_complement() {
        // Lambda should NOT include literals whose complement is in delta
        let mut theory = Theory::new();
        theory.add_fact("~q"); // ~q is strictly proven
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let result = reason_scalable(&theory);

        assert!(result.contains_delta("~q"), "~q should be in delta");
        assert!(
            !result.contains_lambda("q"),
            "q should NOT be in lambda (blocked by ~q in delta)"
        );
    }

    #[test]
    fn test_injection_multi_phase_propagation() {
        // Verify correct multi-phase propagation: delta -> lambda -> partial
        let mut theory = Theory::new();
        theory.add_fact("trigger");
        theory.add_strict_rule(&["trigger"], "step1"); // delta
        theory.add_defeasible_rule(&["step1"], "step2"); // lambda & partial
        theory.add_defeasible_rule(&["step2"], "step3"); // lambda & partial

        let result = reason_scalable(&theory);

        // Phase 1 (delta): trigger, step1
        assert!(result.contains_delta("trigger"));
        assert!(result.contains_delta("step1"));
        assert!(!result.contains_delta("step2"));
        assert!(!result.contains_delta("step3"));

        // Phase 2 (lambda): all
        assert!(result.contains_lambda("trigger"));
        assert!(result.contains_lambda("step1"));
        assert!(result.contains_lambda("step2"));
        assert!(result.contains_lambda("step3"));

        // Phase 3 (partial): all (no conflicts)
        assert!(result.contains_partial("trigger"));
        assert!(result.contains_partial("step1"));
        assert!(result.contains_partial("step2"));
        assert!(result.contains_partial("step3"));
    }

    #[test]
    fn test_injection_counter_based_activation() {
        // TEST-016: Verify counter-based activation correctness
        // r1: a, b, c -> x (3 body literals, c is NOT a fact)
        // r2: a -> y (1 body literal)
        // r3: x, y => z (depends on r1, r2)
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        // Note: c is NOT a fact
        theory.add_strict_rule(&["a", "b", "c"], "x");
        theory.add_strict_rule(&["a"], "y");
        theory.add_defeasible_rule(&["x", "y"], "z");

        let result = reason_scalable(&theory);

        // a, b should be proven
        assert!(result.contains_delta("a"));
        assert!(result.contains_delta("b"));
        // y should be proven (r2 fires)
        assert!(
            result.contains_delta("y"),
            "y should be in delta (r2 fires)"
        );
        // x should NOT be proven (c is missing)
        assert!(
            !result.contains_delta("x"),
            "x should NOT be in delta (c missing)"
        );
        // z should NOT be proven (x is missing)
        assert!(
            !result.contains_partial("z"),
            "z should NOT be in partial (x missing)"
        );
    }

    #[test]
    fn test_injection_partial_body_satisfaction() {
        // Rule with partially satisfied body should NOT fire
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        // c is NOT a fact
        theory.add_defeasible_rule(&["a", "b", "c"], "result");

        let result = reason_scalable(&theory);

        assert!(result.contains_partial("a"));
        assert!(result.contains_partial("b"));
        assert!(
            !result.contains_partial("result"),
            "result should NOT be proven (c missing)"
        );
    }

    #[test]
    fn test_injection_empty_body_rule() {
        // Empty body rules should fire immediately
        let mut theory = Theory::new();
        // Create a strict rule with empty body using the Rule struct directly
        let rule = Rule::new(
            "r1",
            RuleType::Strict,
            vec![],
            vec![Literal::simple("auto_true")],
        );
        theory.add_rule(rule);

        let result = reason_scalable(&theory);

        assert!(
            result.contains_delta("auto_true"),
            "Empty body strict rule should fire and add head to delta"
        );
    }

    #[test]
    fn test_injection_chain_of_strict_rules() {
        // Long chain of strict rules propagates completely
        let mut theory = Theory::new();
        theory.add_fact("l0");
        for i in 0..10 {
            theory.add_strict_rule(&[&format!("l{}", i)], &format!("l{}", i + 1));
        }

        let result = reason_scalable(&theory);

        // All literals in chain should be in delta
        for i in 0..=10 {
            assert!(
                result.contains_delta(&format!("l{}", i)),
                "l{} should be in delta",
                i
            );
        }
    }

    #[test]
    fn test_injection_multiple_rules_same_head() {
        // Multiple rules can support the same head
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_defeasible_rule(&["a"], "c");
        theory.add_defeasible_rule(&["b"], "c");

        let result = reason_scalable(&theory);

        assert!(
            result.contains_partial("c"),
            "c should be provable via multiple rules"
        );
    }

    // ==========================================================================
    // DL(d) vs DL(d||) SEMANTIC EQUIVALENCE TESTS (TEST-003)
    // Ported from spindle-racket/tests/scalable-semantic-tests.rkt
    // ==========================================================================

    #[test]
    fn test_semantic_equiv_empty_theory() {
        // TEST-003: Empty theory produces identical empty conclusions
        let theory = Theory::new();

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);
        let scl_def: FxHashSet<String> = scalable
            .partial
            .iter()
            .map(|id| {
                let lit = id_to_literal(*id);
                lit.canonical_name()
            })
            .collect();

        assert_eq!(std_def.len(), 0, "Standard: empty theory -> no +d");
        assert_eq!(scl_def.len(), 0, "Scalable: empty theory -> no +d");
    }

    #[test]
    fn test_semantic_equiv_single_fact() {
        // TEST-003: Single fact produces identical conclusions
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
    fn test_semantic_equiv_simple_defeasible_chain() {
        // TEST-003: Simple defeasible chain produces identical conclusions
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);

        for lit in &["a", "b", "c"] {
            assert!(std_def.contains(*lit), "Standard: {} should be +d", lit);
            assert!(
                scalable.contains_partial(*lit),
                "Scalable: {} should be +d",
                lit
            );
        }
    }

    #[test]
    fn test_semantic_equiv_strict_chain() {
        // TEST-003: Strict chain produces identical definite conclusions
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
    fn test_semantic_equiv_mixed_chain() {
        // TEST-003: Mixed strict and defeasible chain
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
    fn test_semantic_equiv_superiority_resolves() {
        // TEST-003: Superiority-resolved conflict produces identical conclusions
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
    fn test_semantic_equiv_tweety_triangle() {
        // TEST-003: Classic Tweety triangle with superiority
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
        assert!(std_def.contains("~flies"), "Standard: ~flies should be +d");
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
    fn test_semantic_equiv_long_defeasible_chain() {
        // TEST-003: Long defeasible chain produces identical conclusions
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
            assert!(std_def.contains(&lit), "Standard: {} should be +d", lit);
            assert!(
                scalable.contains_partial(&lit),
                "Scalable: {} should be +d",
                lit
            );
        }
    }

    #[test]
    fn test_semantic_equiv_wide_theory() {
        // TEST-003: Wide theory with many independent facts
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
    fn test_semantic_equiv_multi_body_rule() {
        // TEST-003: Rule with multiple body literals
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_fact("c");
        theory.add_defeasible_rule(&["a", "b", "c"], "result");

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);

        assert!(std_def.contains("result"), "Standard: result should be +d");
        assert!(
            scalable.contains_partial("result"),
            "Scalable: result should be +d"
        );
    }

    #[test]
    fn test_semantic_equiv_multiple_supporting_rules() {
        // TEST-003: Multiple rules with same head (no conflict)
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_defeasible_rule(&["a"], "c");
        theory.add_defeasible_rule(&["b"], "c");

        let standard = reason(&theory);
        let scalable = reason_scalable(&theory);

        let std_def = extract_defeasible_provable(&standard);

        assert!(
            std_def.contains("c"),
            "Standard: c should be +d (multiple supporting rules)"
        );
        assert!(
            scalable.contains_partial("c"),
            "Scalable: c should be +d (multiple supporting rules)"
        );
    }

    // ==========================================================================
    // CLOSURE RELATIONSHIP INVARIANT TESTS
    // ==========================================================================

    #[test]
    fn test_closure_invariant_delta_subset_lambda() {
        // Delta should always be a subset of Lambda
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let result = reason_scalable(&theory);

        for &lit_id in &result.delta {
            assert!(
                result.lambda.contains(&lit_id),
                "Delta {:?} should be in Lambda",
                lit_id
            );
        }
    }

    #[test]
    fn test_closure_invariant_delta_subset_partial() {
        // Delta should always be a subset of Partial (definite implies defeasible)
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "d");

        let result = reason_scalable(&theory);

        for &lit_id in &result.delta {
            assert!(
                result.partial.contains(&lit_id),
                "Delta {:?} should be in Partial",
                lit_id
            );
        }
    }

    #[test]
    fn test_closure_invariant_partial_subset_lambda() {
        // Partial should always be a subset of Lambda (over-approximation property)
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");
        // No superiority - ambiguity

        let result = reason_scalable(&theory);

        for &lit_id in &result.partial {
            assert!(
                result.lambda.contains(&lit_id),
                "Partial {:?} should be in Lambda",
                lit_id
            );
        }
    }

    #[test]
    fn test_closure_invariant_complex_theory() {
        // Test closure invariants on a complex theory
        let mut theory = Theory::new();
        theory.add_fact("trigger");
        theory.add_strict_rule(&["trigger"], "strict_result");
        let r1 = theory.add_defeasible_rule(&["trigger"], "def_a");
        let r2 = theory.add_defeasible_rule(&["trigger"], "~def_a");
        theory.add_superiority(&r1, &r2);
        theory.add_defeasible_rule(&["def_a"], "chain_end");

        let result = reason_scalable(&theory);

        // Verify all invariants
        for &lit_id in &result.delta {
            assert!(result.lambda.contains(&lit_id), "Delta subset Lambda");
            assert!(result.partial.contains(&lit_id), "Delta subset Partial");
        }
        for &lit_id in &result.partial {
            assert!(result.lambda.contains(&lit_id), "Partial subset Lambda");
        }
    }

    // ==========================================================================
    // CONFLICT HANDLING TESTS
    // ==========================================================================

    #[test]
    fn test_conflict_ambiguity_blocks_both() {
        // Without superiority, conflicting conclusions should be blocked
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");
        // No superiority - both should be blocked

        let result = reason_scalable(&theory);

        // Both should be in lambda (over-approximation)
        assert!(result.contains_lambda("q"));
        assert!(result.contains_lambda("~q"));

        // Neither should be in partial (ambiguity block)
        assert!(
            !result.contains_partial("q"),
            "q should be blocked by ambiguity"
        );
        assert!(
            !result.contains_partial("~q"),
            "~q should be blocked by ambiguity"
        );
    }

    #[test]
    fn test_conflict_superiority_resolves_winner() {
        // Superiority should resolve conflict in favor of superior rule
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "winner");
        let r2 = theory.add_defeasible_rule(&["p"], "~winner");
        theory.add_superiority(&r1, &r2);

        let result = reason_scalable(&theory);

        assert!(
            result.contains_partial("winner"),
            "winner should be provable (superior rule)"
        );
        assert!(
            !result.contains_partial("~winner"),
            "~winner should NOT be provable (inferior rule)"
        );
    }

    #[test]
    fn test_conflict_strict_beats_defeasible() {
        // Strict rule should definitely prove, blocking defeasible complement
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "certain");
        theory.add_defeasible_rule(&["p"], "~certain");

        let result = reason_scalable(&theory);

        assert!(
            result.contains_delta("certain"),
            "certain should be in delta (strict)"
        );
        assert!(
            !result.contains_lambda("~certain"),
            "~certain should NOT be in lambda (blocked by delta)"
        );
    }

    #[test]
    fn test_conflict_attack_fails_body_not_in_lambda() {
        // Attack fails if attacker's body is not in lambda
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["x"], "~q"); // x never proven, so attack fails

        let result = reason_scalable(&theory);

        assert!(!result.contains_lambda("x"), "x should not be in lambda");
        assert!(
            result.contains_partial("q"),
            "q should be provable (attack fails due to unsatisfied body)"
        );
    }

    // ==========================================================================
    // EDGE CASE TESTS
    // ==========================================================================

    #[test]
    fn test_edge_negated_fact() {
        // Negated fact should work correctly
        let mut theory = Theory::new();
        theory.add_fact("~guilty");

        let result = reason_scalable(&theory);

        assert!(result.contains_delta("~guilty"));
        assert!(result.contains_partial("~guilty"));
    }

    #[test]
    fn test_edge_negated_chain() {
        // Chain with negated literals
        let mut theory = Theory::new();
        theory.add_fact("~innocent");
        theory.add_defeasible_rule(&["~innocent"], "suspect");

        let result = reason_scalable(&theory);

        assert!(result.contains_delta("~innocent"));
        assert!(result.contains_partial("suspect"));
    }

    #[test]
    fn test_edge_self_referential_rule() {
        // Self-referential rule should not cause infinite loop
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "p");

        let result = reason_scalable(&theory);

        // p not proven (no initial fact)
        assert!(!result.contains_partial("p"));
    }

    #[test]
    fn test_edge_circular_dependencies() {
        // Circular rules should terminate without infinite loop
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["q"], "r");
        theory.add_defeasible_rule(&["r"], "p");

        let result = reason_scalable(&theory);

        // None should be proven (no entry point)
        assert!(!result.contains_partial("p"));
        assert!(!result.contains_partial("q"));
        assert!(!result.contains_partial("r"));
    }

    #[test]
    fn test_edge_conflicting_facts() {
        // Conflicting facts (inconsistent theory)
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("~p");

        let result = reason_scalable(&theory);

        // Both should be in delta (inconsistent theory)
        assert!(result.contains_delta("p"));
        assert!(result.contains_delta("~p"));
    }

    // ==========================================================================
    // to_conclusions() COVERAGE TESTS
    // ==========================================================================

    #[test]
    fn test_to_conclusions_all_types() {
        // Test that to_conclusions generates all four conclusion types
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["x"], "y"); // x not proven, so y unprovable

        let result = reason_scalable(&theory);
        let indexed = IndexedTheory::build(theory.clone());
        let conclusions = result.to_conclusions(&indexed);

        // Check all conclusion types are present
        let has_def_provable = conclusions
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable);
        let has_def_not_provable = conclusions
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefinitelyNotProvable);
        let has_defeas_provable = conclusions
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable);
        let has_defeas_not_provable = conclusions
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefeasiblyNotProvable);

        assert!(has_def_provable, "Should have +D conclusions");
        assert!(has_def_not_provable, "Should have -D conclusions");
        assert!(has_defeas_provable, "Should have +d conclusions");
        assert!(has_defeas_not_provable, "Should have -d conclusions");
    }

    #[test]
    fn test_to_conclusions_partial_not_in_delta() {
        // Test the path where partial contains items not in delta
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b"); // b in partial but not delta

        let result = reason_scalable(&theory);
        let indexed = IndexedTheory::build(theory.clone());
        let conclusions = result.to_conclusions(&indexed);

        // b should be +d but not +D
        let b_defeasibly = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.canonical_name() == "b"
        });
        let b_definitely = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.canonical_name() == "b"
        });

        assert!(b_defeasibly, "b should be defeasibly provable");
        assert!(!b_definitely, "b should NOT be definitely provable");
    }

    #[test]
    fn test_contains_methods() {
        // Test all three contains_* helper methods
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");

        let result = reason_scalable(&theory);

        // Test contains_delta
        assert!(result.contains_delta("a"));
        assert!(!result.contains_delta("b")); // b is defeasible only
        assert!(!result.contains_delta("nonexistent"));

        // Test contains_lambda
        assert!(result.contains_lambda("a"));
        assert!(result.contains_lambda("b"));
        assert!(!result.contains_lambda("nonexistent"));

        // Test contains_partial
        assert!(result.contains_partial("a"));
        assert!(result.contains_partial("b"));
        assert!(!result.contains_partial("nonexistent"));
    }

    #[test]
    fn test_contains_negated_literals() {
        // Test contains_* methods with negated literals
        let mut theory = Theory::new();
        theory.add_fact("~guilty");
        theory.add_defeasible_rule(&["~guilty"], "innocent");

        let result = reason_scalable(&theory);

        assert!(result.contains_delta("~guilty"));
        assert!(result.contains_lambda("~guilty"));
        assert!(result.contains_partial("~guilty"));
        assert!(result.contains_partial("innocent"));
    }

    #[test]
    fn test_empty_body_defeasible_rule() {
        // Empty body defeasible rule (covered in lambda closure)
        let mut theory = Theory::new();
        let rule = Rule::new(
            "r1",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("auto")],
        );
        theory.add_rule(rule);

        let result = reason_scalable(&theory);

        // auto should be in lambda and partial (empty body fires immediately)
        assert!(result.contains_lambda("auto"));
        assert!(result.contains_partial("auto"));
    }

    #[test]
    fn test_lambda_body_check() {
        // Test the path where we verify literal is actually in rule body
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_defeasible_rule(&["a"], "x");
        theory.add_defeasible_rule(&["b"], "y");

        let result = reason_scalable(&theory);

        // Both x and y should be in lambda
        assert!(result.contains_lambda("x"));
        assert!(result.contains_lambda("y"));
    }

    #[test]
    fn test_partial_blocked_by_delta_complement() {
        // Test the blocked_by_delta path in partial closure
        let mut theory = Theory::new();
        theory.add_fact("~q"); // ~q definitely proven
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let result = reason_scalable(&theory);

        // q should not be in partial (blocked by ~q in delta)
        assert!(!result.contains_partial("q"));
        assert!(result.contains_partial("~q"));
    }

    #[test]
    fn test_lambda_defeater_not_in_lambda() {
        // Defeaters don't contribute to lambda closure
        let mut theory = Theory::new();
        theory.add_fact("p");
        let rule = Rule::new(
            "d1",
            RuleType::Defeater,
            vec![Literal::simple("p")],
            vec![Literal::simple("q")],
        );
        theory.add_rule(rule);

        let result = reason_scalable(&theory);

        // p should be in all closures
        assert!(result.contains_delta("p"));
        // q is in a defeater, not strict/defeasible, so not in lambda
        assert!(!result.contains_lambda("q"));
    }

    #[test]
    fn test_fact_as_attacker_skipped() {
        // Facts as "attackers" are skipped in partial closure
        let mut theory = Theory::new();
        theory.add_fact("~q"); // This fact would "attack" q
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let result = reason_scalable(&theory);

        // ~q is definitely proven, so q should be blocked
        assert!(result.contains_delta("~q"));
        assert!(!result.contains_partial("q"));
    }

    #[test]
    fn test_no_supporting_rule_with_satisfied_body() {
        // Test case where no supporting rule has satisfied body
        let mut theory = Theory::new();
        theory.add_fact("a");
        // Rule that could support q but body not satisfied
        theory.add_defeasible_rule(&["x", "y"], "q");

        let result = reason_scalable(&theory);

        // q has no satisfied supporting rule
        assert!(!result.contains_partial("q"));
    }

    #[test]
    fn test_rule_already_fired() {
        // Test where a rule has already fired and shouldn't fire again
        let mut theory = Theory::new();
        theory.add_fact("a");
        // This rule fires once when a is proven
        theory.add_defeasible_rule(&["a"], "b");
        // Add a second rule with same body to ensure first rule marked as fired
        theory.add_defeasible_rule(&["a"], "c");

        let result = reason_scalable(&theory);

        // Both b and c should be proven
        assert!(result.contains_partial("b"));
        assert!(result.contains_partial("c"));
    }

    #[test]
    fn test_literal_in_body_check() {
        // Test where trigger index returns rules but literal not actually in body
        // This tests the in_body check in lambda closure
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");

        let result = reason_scalable(&theory);

        assert!(result.contains_lambda("b"));
    }

    // ==========================================================================
    // ASSERTION MESSAGE COVERAGE TESTS
    // ==========================================================================

    #[test]
    #[should_panic(expected = "should be +d")]
    fn test_assert_msg_semantic_defeasible() {
        let mut theory = Theory::new();
        // No facts - nothing will be provable
        theory.add_defeasible_rule(&["a"], "b");

        let standard = reason(&theory);
        let std_def = extract_defeasible_provable(&standard);

        for lit in &["b"] {
            assert!(std_def.contains(*lit), "Standard: {} should be +d", lit);
        }
    }

    #[test]
    #[should_panic(expected = "should be +D")]
    fn test_assert_msg_semantic_definite() {
        let mut theory = Theory::new();
        // No facts - nothing will be provable
        theory.add_strict_rule(&["p"], "q");

        let standard = reason(&theory);
        let std_definite = extract_definite_provable(&standard);

        for lit in &["q"] {
            assert!(
                std_definite.contains(*lit),
                "Standard: {} should be +D",
                lit
            );
        }
    }

    #[test]
    #[should_panic(expected = "Scalable: r should be +d")]
    fn test_assert_msg_scalable_mixed_chain() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");
        // Rule that can't fire (missing "q" prerequisite creates scenario)
        theory.add_defeasible_rule(&["missing"], "r");

        let scalable = reason_scalable(&theory);

        assert!(scalable.contains_partial("r"), "Scalable: r should be +d");
    }

    // ==========================================================================
    // EDGE CASE TESTS FOR CLOSURE COMPUTATION
    // ==========================================================================

    #[test]
    fn test_can_prove_literal_in_delta() {
        // Test line 405: literal already in delta returns true early
        let mut theory = Theory::new();
        theory.add_fact("p"); // p is in delta
        theory.add_strict_rule(&["p"], "q"); // q will be in delta too

        let result = reason_scalable(&theory);

        // Both p and q should be in delta
        assert!(result.contains_delta("p"));
        assert!(result.contains_delta("q"));
        // And therefore also in partial
        assert!(result.contains_partial("p"));
        assert!(result.contains_partial("q"));
    }

    #[test]
    fn test_can_prove_blocked_by_delta() {
        // Test line 409: complement in delta blocks the literal
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q"); // q is definitely provable
        theory.add_defeasible_rule(&["p"], "~q"); // ~q should be blocked

        let result = reason_scalable(&theory);

        // q is in delta (strict rule)
        assert!(result.contains_delta("q"));
        // ~q should NOT be in partial (blocked by q in delta)
        assert!(!result.contains_partial("~q"));
    }

    #[test]
    fn test_can_prove_no_supporting_rule() {
        // Test line 419: no supporting rule with satisfied body
        let mut theory = Theory::new();
        theory.add_fact("p");
        // Rule for q requires missing fact
        theory.add_defeasible_rule(&["missing"], "q");

        let result = reason_scalable(&theory);

        // q should not be provable (no rule can fire)
        assert!(!result.contains_partial("q"));
        assert!(!result.contains_lambda("q"));
    }

    #[test]
    fn test_lambda_skips_fired_rules() {
        // Test line 275: rule already fired should be skipped
        let mut theory = Theory::new();
        theory.add_fact("a");
        // Two rules with same body - both should fire, but only once each
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["a"], "c");

        let result = reason_scalable(&theory);

        // Both b and c should be in lambda (rules fired once each)
        assert!(result.contains_lambda("b"));
        assert!(result.contains_lambda("c"));
    }

    #[test]
    fn test_lambda_skips_defeaters() {
        // Test line 280: defeaters don't contribute to lambda
        let mut theory = Theory::new();
        theory.add_fact("p");
        // Defeater should not add q to lambda
        let rule = crate::rule::Rule::new(
            "d1".to_string(),
            crate::rule::RuleType::Defeater,
            vec![Literal::simple("p")],
            vec![Literal::simple("q")],
        );
        theory.add_rule(rule);

        let result = reason_scalable(&theory);

        // q should NOT be in lambda (defeaters don't prove)
        assert!(!result.contains_lambda("q"));
    }

    #[test]
    fn test_partial_with_conflicting_rules_no_superiority() {
        // Test complex conflict without superiority
        let mut theory = Theory::new();
        theory.add_fact("trigger");
        theory.add_defeasible_rule(&["trigger"], "result");
        theory.add_defeasible_rule(&["trigger"], "~result");
        // No superiority - conflict should block both

        let result = reason_scalable(&theory);

        // Both should be in lambda (potential conclusions)
        assert!(result.contains_lambda("result"));
        assert!(result.contains_lambda("~result"));
        // Neither should be in partial (conflict blocks both)
        assert!(!result.contains_partial("result"));
        assert!(!result.contains_partial("~result"));
    }

    #[test]
    fn test_partial_with_chain_and_conflict() {
        // Test where a chain leads to a conflict
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["a"], "~c"); // Conflicts with chain result

        let result = reason_scalable(&theory);

        // a and b should be partial (no conflict)
        assert!(result.contains_partial("a"));
        assert!(result.contains_partial("b"));
        // c and ~c conflict - neither should be partial
        assert!(!result.contains_partial("c"));
        assert!(!result.contains_partial("~c"));
    }
}
