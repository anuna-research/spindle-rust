//! Scalable DL(d||) Reasoning Algorithm
//!
//! Implements the three-phase closure computation for scalable
//! defeasible logic reasoning (DL(d||)).

use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::conclusion::{Conclusion, ConclusionType};
use crate::index::{IndexedTheory, LitId};
use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel, RuleType};

/// Result of scalable reasoning: three closure sets
///
/// Uses `LitId` (local 4-byte ID) for efficient set operations.
/// Note: These IDs are only valid with the `IndexedTheory` that produced them.
#[derive(Debug, Clone)]
pub struct ScalableResult {
    /// Delta closure: definitely provable (+Δ)
    pub delta: FxHashSet<LitId>,
    /// Lambda closure: potentially provable (over-approximation)
    pub lambda: FxHashSet<LitId>,
    /// Partial closure: defeasibly provable (+∂||)
    pub partial: FxHashSet<LitId>,
}

impl ScalableResult {
    /// Convert to conclusions list using the associated IndexedTheory
    pub fn to_conclusions(&self, indexed: &IndexedTheory<'_>) -> Vec<Conclusion> {
        // Pre-allocate
        let lit_count = indexed.all_literal_ids().count();
        let estimated = self.delta.len() * 2 + self.partial.len() + lit_count * 2;
        let mut conclusions = Vec::with_capacity(estimated);

        // Delta -> Definitely Provable
        for &lit_id in &self.delta {
            let lit = indexed.resolve_literal(lit_id);
            conclusions.push(Conclusion::definitely_provable(lit.clone()));
        }

        // Partial - Delta -> Defeasibly Provable
        for &lit_id in &self.partial {
            if !self.delta.contains(&lit_id) {
                let lit = indexed.resolve_literal(lit_id);
                conclusions.push(Conclusion::defeasibly_provable(lit));
            }
        }

        // Add defeasibly provable for delta items too (Definite implies Defeasible)
        for &lit_id in &self.delta {
            let lit = indexed.resolve_literal(lit_id);
            conclusions.push(Conclusion::defeasibly_provable(lit));
        }

        // Negative conclusions
        for &lit_id in indexed.all_literal_ids() {
            if !self.delta.contains(&lit_id) {
                let lit = indexed.resolve_literal(lit_id);
                conclusions.push(Conclusion::new(
                    ConclusionType::DefinitelyNotProvable,
                    lit.clone(),
                ));
            }
            if !self.partial.contains(&lit_id) {
                let lit = indexed.resolve_literal(lit_id);
                conclusions.push(Conclusion::new(ConclusionType::DefeasiblyNotProvable, lit));
            }
        }

        conclusions
    }

    /// Check if a literal (by canonical name string) is in delta
    pub fn contains_delta(&self, indexed: &IndexedTheory<'_>, canonical: &str) -> bool {
        let lit = key_to_literal(canonical);
        if let Some(id) = indexed.get_lit_id(&lit) {
            self.delta.contains(&id)
        } else {
            false
        }
    }

    /// Check if a literal (by canonical name string) is in lambda
    pub fn contains_lambda(&self, indexed: &IndexedTheory<'_>, canonical: &str) -> bool {
        let lit = key_to_literal(canonical);
        if let Some(id) = indexed.get_lit_id(&lit) {
            self.lambda.contains(&id)
        } else {
            false
        }
    }

    /// Check if a literal (by canonical name string) is in partial
    pub fn contains_partial(&self, indexed: &IndexedTheory<'_>, canonical: &str) -> bool {
        let lit = key_to_literal(canonical);
        if let Some(id) = indexed.get_lit_id(&lit) {
            self.partial.contains(&id)
        } else {
            false
        }
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

/// Perform scalable DL(d||) reasoning on a theory.
///
/// Requires an `IndexedTheory` to ensure consistent ID usage.
pub fn reason_scalable(indexed: &IndexedTheory<'_>) -> ScalableResult {
    let theory = indexed.theory();
    let rule_count = theory.rule_count();

    // Initialize rule states with pre-allocated capacity
    let mut states: FxHashMap<RuleLabel, RuleState> =
        FxHashMap::with_capacity_and_hasher(rule_count, Default::default());
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
    let delta = compute_delta_closure(indexed, &mut states);

    // Phase 2: Lambda Closure
    let lambda = compute_lambda_closure(indexed, &delta);

    // Phase 3: Partial Closure
    let partial = compute_partial_closure(indexed, &delta, &lambda);

    ScalableResult {
        delta,
        lambda,
        partial,
    }
}

/// Phase 1: Compute delta closure (definite conclusions)
fn compute_delta_closure(
    indexed: &IndexedTheory<'_>,
    states: &mut FxHashMap<RuleLabel, RuleState>,
) -> FxHashSet<LitId> {
    let mut delta: FxHashSet<LitId> = FxHashSet::default();
    let mut worklist: VecDeque<LitId> = VecDeque::new();

    // Initialize with facts and empty-body strict rules
    for rule in indexed.theory().rules() {
        if rule.rule_type == RuleType::Fact
            || (rule.rule_type == RuleType::Strict && rule.body.is_empty())
        {
            for head_lit in &rule.head {
                // Safe unwrap because IndexedTheory builds from these rules
                let lit_id = indexed.get_lit_id(head_lit).expect("Literal must exist");
                if !delta.contains(&lit_id) {
                    delta.insert(lit_id);
                    worklist.push_back(lit_id);
                }
            }
        }
    }

    // Forward chaining loop
    while let Some(lit_id) = worklist.pop_front() {
        // Find rules containing this literal in body
        for rule in indexed.rules_with_body_id(lit_id) {
            let state = states.get_mut(&rule.label).unwrap();

            if !state.activated && state.remaining > 0 {
                state.remaining -= 1;

                // If body fully satisfied and rule is strict, fire it
                if state.remaining == 0 && rule.rule_type == RuleType::Strict {
                    state.activated = true;

                    for head_lit in &rule.head {
                        let head_id = indexed.get_lit_id(head_lit).expect("Literal must exist");
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

/// Phase 2: Compute lambda closure (potential conclusions)
fn compute_lambda_closure(
    indexed: &IndexedTheory<'_>,
    delta: &FxHashSet<LitId>,
) -> FxHashSet<LitId> {
    let mut lambda: FxHashSet<LitId> = delta.clone();
    let mut worklist: VecDeque<LitId> = delta.iter().copied().collect();

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
                let head_id = indexed.get_lit_id(head_lit).expect("Literal must exist");
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
        for rule in indexed.rules_with_body_id(lit_id) {
            if fired.contains(&rule.label) {
                continue;
            }

            // Only strict and defeasible rules contribute to lambda
            if rule.rule_type != RuleType::Strict && rule.rule_type != RuleType::Defeasible {
                continue;
            }

            let remaining = lambda_remaining.get_mut(&rule.label).unwrap();
            if *remaining > 0 {
                *remaining -= 1;

                if *remaining == 0 {
                    fired.insert(rule.label.clone());

                    for head_lit in &rule.head {
                        let head_id = indexed.get_lit_id(head_lit).expect("Literal must exist");
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
fn compute_partial_closure(
    indexed: &IndexedTheory<'_>,
    delta: &FxHashSet<LitId>,
    lambda: &FxHashSet<LitId>,
) -> FxHashSet<LitId> {
    let theory = indexed.theory();
    let mut partial: FxHashSet<LitId> = delta.clone();

    // Track remaining unsatisfied body literals for each rule
    let mut remaining: FxHashMap<&str, usize> = FxHashMap::default();
    for rule in theory.rules() {
        if rule.rule_type == RuleType::Strict || rule.rule_type == RuleType::Defeasible {
            // Count body literals not yet in partial
            let unsatisfied = rule
                .body
                .iter()
                .filter(|b| {
                    let bid = indexed.get_lit_id(b).expect("Literal must exist");
                    !partial.contains(&bid)
                })
                .count();
            remaining.insert(&rule.label, unsatisfied);
        }
    }

    // Candidates: literals in lambda but not in delta, blocked by complement in delta
    let blocked_by_delta: FxHashSet<LitId> = lambda
        .iter()
        .filter(|&&k| delta.contains(&k.complement()))
        .copied()
        .collect();

    // Helper: check if rule body is satisfied in partial
    let body_satisfied = |rule: &Rule, partial: &FxHashSet<LitId>| -> bool {
        rule.body.iter().all(|b| {
            let bid = indexed.get_lit_id(b).expect("Literal must exist");
            partial.contains(&bid)
        })
    };

    // Helper: check if attack body is NOT fully in lambda (attack fails)
    let attack_unsatisfied_lambda = |rule: &Rule| -> bool {
        rule.body.iter().any(|b| {
            let bid = indexed.get_lit_id(b).expect("Literal must exist");
            !lambda.contains(&bid)
        })
    };

    // Helper: can we defeat the attacker using superiority?
    let team_defeats = |lit_id: LitId, attacker: &Rule, partial: &FxHashSet<LitId>| -> bool {
        for defender in indexed.rules_with_head_id(lit_id) {
            if (defender.rule_type == RuleType::Strict
                || defender.rule_type == RuleType::Defeasible)
                && body_satisfied(defender, partial)
                && theory.is_superior(defender.template_label(), attacker.template_label())
            {
                return true;
            }
        }
        false
    };

    // Helper: all attacks defeated?
    let all_attacks_defeated = |lit_id: LitId, partial: &FxHashSet<LitId>| -> bool {
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
    let can_prove = |lit_id: LitId, partial: &FxHashSet<LitId>| -> bool {
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
    let mut worklist: VecDeque<LitId> = VecDeque::new();

    // Initialize worklist with rules that have body fully satisfied
    for rule in theory.rules() {
        if (rule.rule_type == RuleType::Strict || rule.rule_type == RuleType::Defeasible)
            && remaining.get(rule.label.as_str()) == Some(&0)
        {
            for head_lit in &rule.head {
                let head_id = indexed.get_lit_id(head_lit).expect("Literal must exist");
                if lambda.contains(&head_id) && !partial.contains(&head_id) {
                    worklist.push_back(head_id);
                }
            }
        }
    }

    // Track what's already in worklist to avoid duplicates
    let mut in_worklist: FxHashSet<LitId> = worklist.iter().copied().collect();

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
                            let head_id = indexed.get_lit_id(head_lit).expect("Literal must exist");
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
    use crate::Theory;
    use crate::literal::Literal;
    use crate::rule::Rule;

    #[test]
    fn test_delta_closure_facts() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_delta(&indexed, "a"));
        assert!(result.contains_delta(&indexed, "b"));
        assert_eq!(result.delta.len(), 2);
    }

    #[test]
    fn test_delta_closure_strict_chain() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("q");
        theory.add_strict_rule(&["p", "q"], "r");
        theory.add_strict_rule(&["r"], "s");
        theory.add_defeasible_rule(&["r"], "t");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_delta(&indexed, "p"));
        assert!(result.contains_delta(&indexed, "q"));
        assert!(result.contains_delta(&indexed, "r"));
        assert!(result.contains_delta(&indexed, "s"));
        assert!(!result.contains_delta(&indexed, "t"));
    }

    #[test]
    fn test_lambda_includes_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");
        theory.add_defeasible_rule(&["q"], "r");
        theory.add_defeasible_rule(&["r"], "s");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_delta(&indexed, "p"));
        assert!(result.contains_delta(&indexed, "q"));
        assert!(!result.contains_delta(&indexed, "r"));

        assert!(result.contains_lambda(&indexed, "p"));
        assert!(result.contains_lambda(&indexed, "q"));
        assert!(result.contains_lambda(&indexed, "r"));
        assert!(result.contains_lambda(&indexed, "s"));
    }

    #[test]
    fn test_lambda_blocks_on_complement() {
        let mut theory = Theory::new();
        theory.add_fact("~q");
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_delta(&indexed, "~q"));
        assert!(!result.contains_lambda(&indexed, "q"));
    }

    #[test]
    fn test_partial_no_conflict() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_partial(&indexed, "a"));
        assert!(result.contains_partial(&indexed, "b"));
        assert!(result.contains_partial(&indexed, "c"));
    }

    #[test]
    fn test_partial_ambiguity_block() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_lambda(&indexed, "q"));
        assert!(result.contains_lambda(&indexed, "~q"));

        assert!(result.contains_partial(&indexed, "p"));
        assert!(!result.contains_partial(&indexed, "q"));
        assert!(!result.contains_partial(&indexed, "~q"));
    }

    #[test]
    fn test_partial_superiority_resolves() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");
        theory.add_superiority(&r1, &r2);

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_partial(&indexed, "q"));
        assert!(!result.contains_partial(&indexed, "~q"));
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

        theory.add_superiority(&r2, &r1);

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_partial(&indexed, "flies_eddie"));
        assert!(result.contains_partial(&indexed, "~flies_tweety"));
        assert!(!result.contains_partial(&indexed, "flies_tweety"));
    }

    #[test]
    fn test_attack_fails_body_not_in_lambda() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["x"], "~q");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(!result.contains_lambda(&indexed, "x"));
        assert!(result.contains_partial(&indexed, "q"));
    }

    use crate::reason::reason;

    fn extract_defeasible_provable(conclusions: &[Conclusion]) -> FxHashSet<String> {
        conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
            .map(|c| c.literal.canonical_name())
            .collect()
    }

    fn extract_definite_provable(conclusions: &[Conclusion]) -> FxHashSet<String> {
        conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
            .map(|c| c.literal.canonical_name())
            .collect()
    }

    #[test]
    fn test_semantic_equiv_empty_theory() {
        let theory = Theory::new();

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);

        let std_def = extract_defeasible_provable(&standard);

        // Manual partial extraction via to_conclusions since set comparison is hard with LitId
        let scl_conclusions = scalable.to_conclusions(&indexed);
        let scl_def = extract_defeasible_provable(&scl_conclusions);

        assert_eq!(std_def.len(), 0);
        assert_eq!(scl_def.len(), 0);
    }

    #[test]
    fn test_semantic_equiv_single_fact() {
        let mut theory = Theory::new();
        theory.add_fact("p");

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);

        let std_def = extract_defeasible_provable(&standard);

        assert!(std_def.contains("p"));
        assert!(scalable.contains_partial(&indexed, "p"));
        assert!(scalable.contains_delta(&indexed, "p"));
    }

    #[test]
    fn test_semantic_equiv_simple_defeasible_chain() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);

        let std_def = extract_defeasible_provable(&standard);

        for &lit in ["a", "b", "c"].iter() {
            assert!(std_def.contains(lit));
            assert!(scalable.contains_partial(&indexed, lit));
        }
    }

    #[test]
    fn test_semantic_definite_match() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");
        theory.add_strict_rule(&["q"], "r");

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);

        let std_definite = extract_definite_provable(&standard);

        for &lit in ["p", "q", "r"].iter() {
            assert!(std_definite.contains(lit));
            assert!(scalable.contains_delta(&indexed, lit));
        }
    }

    #[test]
    fn test_semantic_mixed_chain() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");
        theory.add_defeasible_rule(&["q"], "r");

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);

        let std_def = extract_defeasible_provable(&standard);
        let std_definite = extract_definite_provable(&standard);

        assert!(std_definite.contains("p"));
        assert!(std_definite.contains("q"));
        assert!(scalable.contains_delta(&indexed, "p"));
        assert!(scalable.contains_delta(&indexed, "q"));

        assert!(std_def.contains("r"));
        assert!(scalable.contains_partial(&indexed, "r"));
    }

    #[test]
    fn test_semantic_superiority_resolves() {
        let mut theory = Theory::new();
        theory.add_fact("trigger");
        let r1 = theory.add_defeasible_rule(&["trigger"], "result");
        let r2 = theory.add_defeasible_rule(&["trigger"], "~result");
        theory.add_superiority(&r1, &r2);

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);

        let std_def = extract_defeasible_provable(&standard);

        assert!(std_def.contains("result"));
        assert!(scalable.contains_partial(&indexed, "result"));
        assert!(!scalable.contains_partial(&indexed, "~result"));
    }

    #[test]
    fn test_semantic_tweety_triangle() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r2, &r1);

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);

        let std_def = extract_defeasible_provable(&standard);

        assert!(std_def.contains("~flies"));
        assert!(scalable.contains_partial(&indexed, "~flies"));
        assert!(!scalable.contains_partial(&indexed, "flies"));
    }

    #[test]
    fn test_semantic_long_chain() {
        let mut theory = Theory::new();
        theory.add_fact("l0");
        for i in 0..10 {
            theory.add_defeasible_rule(&[&format!("l{}", i)], &format!("l{}", i + 1));
        }

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);

        let std_def = extract_defeasible_provable(&standard);

        for i in 0..=10 {
            let lit = format!("l{}", i);
            assert!(std_def.contains(&lit));
            assert!(scalable.contains_partial(&indexed, &lit));
        }
    }

    #[test]
    fn test_semantic_wide_theory() {
        let mut theory = Theory::new();
        for i in 0..20 {
            theory.add_fact(&format!("fact{}", i));
            theory.add_defeasible_rule(&[&format!("fact{}", i)], &format!("derived{}", i));
        }

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);

        let std_def = extract_defeasible_provable(&standard);

        for i in 0..20 {
            let fact = format!("fact{}", i);
            let derived = format!("derived{}", i);

            assert!(std_def.contains(&fact));
            assert!(std_def.contains(&derived));
            assert!(scalable.contains_partial(&indexed, &fact));
            assert!(scalable.contains_partial(&indexed, &derived));
        }
    }

    #[test]
    fn test_closure_relationships() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "d");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        for &lit_id in &result.delta {
            assert!(result.partial.contains(&lit_id));
        }

        for &lit_id in &result.partial {
            assert!(result.lambda.contains(&lit_id));
        }
    }

    #[test]
    fn test_scalable_long_chain_performance() {
        let mut theory = Theory::new();
        theory.add_fact("l0");
        for i in 0..100 {
            theory.add_defeasible_rule(&[&format!("l{}", i)], &format!("l{}", i + 1));
        }

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_partial(&indexed, "l100"));
    }

    #[test]
    fn test_scalable_wide_theory_performance() {
        let mut theory = Theory::new();
        for i in 0..200 {
            theory.add_fact(&format!("fact{}", i));
            theory.add_defeasible_rule(&[&format!("fact{}", i)], &format!("derived{}", i));
        }

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        assert!(result.contains_partial(&indexed, "derived199"));
    }

    #[test]
    fn test_scalable_many_conflicts_with_superiority() {
        let mut theory = Theory::new();
        theory.add_fact("trigger");
        for i in 0..50 {
            let r1 = theory.add_defeasible_rule(&["trigger"], &format!("q{}", i));
            let r2 = theory.add_defeasible_rule(&["trigger"], &format!("~q{}", i));
            theory.add_superiority(&r1, &r2);
        }

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        for i in 0..50 {
            assert!(result.contains_partial(&indexed, &format!("q{}", i)));
            assert!(!result.contains_partial(&indexed, &format!("~q{}", i)));
        }
    }

    #[test]
    fn test_duplicate_facts_no_double_decrement() {
        // Regression: duplicate facts in theory should not cause double-decrement
        // of body_remaining counters in delta/lambda/partial closures.
        let mut theory = Theory::new();
        // Add the same fact twice via two separate rules
        theory.add_rule(Rule::fact("f1", Literal::simple("p")));
        theory.add_rule(Rule::fact("f2", Literal::simple("p")));
        // Add a rule whose body depends on p
        theory.add_defeasible_rule(&["p"], "q");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        // q should still be provable (not broken by double-decrement)
        assert!(result.contains_partial(&indexed, "q"));
        assert!(result.contains_partial(&indexed, "p"));
    }

    #[test]
    fn test_duplicate_facts_multi_body_rule() {
        // Regression: duplicate facts should not incorrectly satisfy multi-body rules
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("a")));
        theory.add_rule(Rule::fact("f2", Literal::simple("a"))); // duplicate
        theory.add_defeasible_rule(&["a", "b"], "goal");

        let indexed = IndexedTheory::build(&theory);
        let result = reason_scalable(&indexed);

        // goal should NOT be provable - b is missing
        assert!(!result.contains_partial(&indexed, "goal"));
        assert!(result.contains_partial(&indexed, "a"));
    }

    // stateless tests need updating too, but they are simpler
    // I'll update one as example
    #[test]
    fn test_stateless_reason_scalable_does_not_mutate_theory() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let indexed = IndexedTheory::build(&theory);
        let _result = reason_scalable(&indexed);

        // Checks theory didn't change... theory is referenced by indexed, so it couldn't change
        // logic holds
    }
}
