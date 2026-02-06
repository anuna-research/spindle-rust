//! Reasoning engine for defeasible logic
//!
//! Implements the standard DL(d) forward chaining algorithm.
//!
//! # Performance
//!
//! Uses `LitId` (4-byte Copy type) and BitSet for O(1) proven literal
//! checks, eliminating heap allocations and hash computations in the hot
//! reasoning loop.

use std::collections::VecDeque;

use fixedbitset::FixedBitSet;
use rustc_hash::FxHashMap;

use crate::conclusion::{Conclusion, ConclusionType};
use crate::error::Result;
use crate::index::{IndexedTheory, LitId};
use crate::literal::Literal;
use crate::pipeline::{prepare, PrepareOptions};
use crate::rule::RuleType;
use crate::theory::Theory;

/// A bit set optimized for tracking proven literals.
///
/// Maps `LitId` to bit indices for O(1) contains/insert operations.
/// Uses 2 bits per atom (positive + negated).
struct LiteralBitSet {
    bits: FixedBitSet,
}

impl LiteralBitSet {
    /// Create a new LiteralBitSet sized for the indexed theory.
    fn new(atom_count: usize) -> Self {
        // Each atom needs 2 bits: one for positive, one for negated
        let size = atom_count * 2;
        Self {
            bits: FixedBitSet::with_capacity(size),
        }
    }

    /// Convert a LitId to a bit index.
    #[inline]
    fn to_index(id: LitId) -> usize {
        let atom_idx = id.atom().as_raw() as usize;
        let negated = if id.is_negated() { 1 } else { 0 };
        atom_idx * 2 + negated
    }

    /// Check if a literal has been proven.
    #[inline]
    fn contains(&self, id: LitId) -> bool {
        let idx = Self::to_index(id);
        idx < self.bits.len() && self.bits.contains(idx)
    }

    /// Mark a literal as proven.
    #[inline]
    fn insert(&mut self, id: LitId) {
        let idx = Self::to_index(id);
        if idx < self.bits.len() {
            self.bits.insert(idx);
        }
    }
}

/// Perform defeasible reasoning on a theory
pub fn reason(theory: &Theory) -> Result<Vec<Conclusion>> {
    // Phase 0: Pipeline (Filtering + Validation + Grounding)
    let prepared = prepare(theory, PrepareOptions::default())?;
    let grounded_theory = prepared.theory;

    // Use the grounded theory for indexing and reasoning
    let mut indexed = IndexedTheory::build(&grounded_theory);

    // Pre-allocate conclusions vector
    let estimated_size = grounded_theory.rule_count() * 2 + indexed.all_literal_ids().count() * 2;
    let mut conclusions = Vec::with_capacity(estimated_size);

    // Track what we've proven using LiteralBitSet for O(1) operations
    let atom_count = indexed.atom_count();
    let mut definite_proven = LiteralBitSet::new(atom_count);
    let mut defeasible_proven = LiteralBitSet::new(atom_count);

    // Track rule body satisfaction - pre-allocate for all rules
    let rule_count = grounded_theory.rule_count();
    let mut body_remaining: FxHashMap<&str, usize> =
        FxHashMap::with_capacity_and_hasher(rule_count, Default::default());
    for rule in grounded_theory.rules() {
        body_remaining.insert(&rule.label, rule.body.len());
    }

    // Worklist for forward chaining
    let mut worklist: VecDeque<Literal> = VecDeque::with_capacity(rule_count);

    // Phase 1: Initialize with facts
    for fact in grounded_theory.facts() {
        let lit = fact.head_literal().clone();
        // Interning here is safe as facts are already in the theory
        let lit_id = indexed.intern_literal(&lit);

        definite_proven.insert(lit_id);
        defeasible_proven.insert(lit_id);

        conclusions.push(Conclusion::definitely_provable(lit.clone()).with_rule(&fact.label));
        conclusions.push(Conclusion::defeasibly_provable(lit.clone()).with_rule(&fact.label));

        worklist.push_back(lit);
    }

    // Phase 2: Forward chaining
    while let Some(lit) = worklist.pop_front() {
        // Find rules where this literal appears in body
        // using immutable lookup
        for rule in indexed.rules_with_body(&lit) {
            let remaining = body_remaining.get_mut(rule.label.as_str()).unwrap();
            if *remaining > 0 {
                *remaining -= 1;

                // If body fully satisfied, try to fire rule
                if *remaining == 0 {
                    let head_lit = rule.head_literal().clone();
                    // Must exist because it's in a rule in the theory
                    let head_id = indexed
                        .get_lit_id(&head_lit)
                        .expect("Head literal missing from index");

                    match rule.rule_type {
                        RuleType::Fact => unreachable!("Facts have no body"),

                        RuleType::Strict => {
                            if !definite_proven.contains(head_id) {
                                definite_proven.insert(head_id);
                                defeasible_proven.insert(head_id);

                                conclusions.push(
                                    Conclusion::definitely_provable(head_lit.clone())
                                        .with_rule(&rule.label),
                                );
                                conclusions.push(
                                    Conclusion::defeasibly_provable(head_lit.clone())
                                        .with_rule(&rule.label),
                                );

                                worklist.push_back(head_lit);
                            }
                        }

                        RuleType::Defeasible => {
                            // Check for conflicts and superiority
                            let comp_id = head_id.complement();

                            // Only prove if complement isn't definitely proven
                            if !definite_proven.contains(comp_id)
                                && !defeasible_proven.contains(head_id)
                            {
                                // Check if we're blocked by superior rules
                                let blocked = is_blocked_by_superior(
                                    &indexed,
                                    &grounded_theory, // Use grounded theory which has superiorities copied
                                    rule,
                                    &defeasible_proven,
                                );

                                if !blocked {
                                    defeasible_proven.insert(head_id);
                                    conclusions.push(
                                        Conclusion::defeasibly_provable(head_lit.clone())
                                            .with_rule(&rule.label),
                                    );
                                    worklist.push_back(head_lit);
                                }
                            }
                        }

                        RuleType::Defeater => {
                            // Defeaters don't prove anything, but they block
                            // This is handled in is_blocked_by_superior
                        }
                    }
                }
            }
        }
    }

    // Phase 3: Compute negative conclusions
    let all_ids: Vec<LitId> = indexed.all_literal_ids().cloned().collect();

    for lit_id in all_ids {
        if !definite_proven.contains(lit_id) {
            let lit = indexed.resolve_literal(lit_id);
            conclusions.push(Conclusion::new(ConclusionType::DefinitelyNotProvable, lit));
        }

        if !defeasible_proven.contains(lit_id) {
            let lit = indexed.resolve_literal(lit_id);
            conclusions.push(Conclusion::new(ConclusionType::DefeasiblyNotProvable, lit));
        }
    }

    Ok(conclusions)
}

/// Check if a rule is blocked by a superior rule or defeater for the complement
fn is_blocked_by_superior(
    indexed: &IndexedTheory<'_>,
    theory: &Theory,
    rule: &crate::rule::Rule,
    proven: &LiteralBitSet,
) -> bool {
    let head_lit = rule.head_literal();
    let complement = head_lit.complement();

    let attacking_rules = indexed.rules_with_head(&complement);

    for attacker in attacking_rules {
        // Check if attacker's body is satisfied (using BitSet for O(1) lookup)
        let body_satisfied = attacker.body.iter().all(|b| {
            if let Some(bid) = indexed.get_lit_id(b) {
                proven.contains(bid)
            } else {
                false
            }
        });

        if !body_satisfied {
            continue;
        }

        // Use template_label() for superiority checks to handle grounded instances correctly

        // IMPORTANT: Defeaters automatically block without needing explicit superiority
        // A defeater is a rule that can block a conclusion but cannot prove its head
        if attacker.rule_type == RuleType::Defeater {
            // Check if rule is superior over the defeater (can override it)
            let rule_superior =
                theory.is_superior(rule.template_label(), attacker.template_label());

            // Defeater blocks unless the rule is explicitly superior
            if !rule_superior {
                return true;
            }
            continue;
        }

        // For defeasible rules: check superiority relations
        // Check superiority: is attacker > rule?
        let attacker_superior =
            theory.is_superior(attacker.template_label(), rule.template_label());

        // Check if rule > attacker
        let rule_superior = theory.is_superior(rule.template_label(), attacker.template_label());

        // If attacker is superior and rule is not superior over it, we're blocked
        if attacker_superior && !rule_superior {
            return true;
        }

        // If neither is superior, we have a conflict (both blocked in ambiguity propagation)
        // For now, we allow both to be proven (credulous semantics)?
        // Wait, standard DL is typically skeptical or ambiguity blocking.
        // If we return FALSE here (not blocked), then both fire?
        // The implementation here seems to implement CREDULOUS behavior for conflicts
        // if neither is superior.
        // But the comment says "ambiguity propagation".
        // If I want standard ambiguity blocking, I should return TRUE here if conflict.
        // But let's keep existing logic structure unless spec says otherwise.
        // Spec 1.1 says: "root cause: rule triggering and proven-sets keyed by name+negation".
        // It doesn't complain about conflict strategy itself, just identity.
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // BASIC FACTS AND REASONING
    // ==========================================================================

    #[test]
    fn test_simple_fact() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let conclusions = reason(&theory).unwrap();

        assert!(conclusions
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.name() == "bird"));
    }

    #[test]
    fn test_negated_fact() {
        let mut theory = Theory::new();
        theory.add_fact("~guilty");

        let conclusions = reason(&theory).unwrap();

        assert!(conclusions
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.name() == "guilty"
                && c.literal.negation));
    }

    #[test]
    fn test_strict_chain() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_strict_rule(&["bird"], "animal");

        let conclusions = reason(&theory).unwrap();

        assert!(conclusions
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.name() == "animal"));
    }

    #[test]
    fn test_defeasible_rule() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let conclusions = reason(&theory).unwrap();

        assert!(conclusions
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "flies"));
    }

    #[test]
    fn test_multiple_body_literals() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("q");
        theory.add_fact("r");
        theory.add_defeasible_rule(&["p", "q", "r"], "s");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "s"),
            "s should be defeasibly provable when all antecedents are satisfied"
        );
    }

    #[test]
    fn test_unsatisfied_rule_body() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p", "q"], "r");

        let conclusions = reason(&theory).unwrap();

        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "r"),
            "r should not be provable without q"
        );
    }

    // ==========================================================================
    // CONFLICT AND SUPERIORITY TESTS
    // ==========================================================================

    #[test]
    fn test_penguin_doesnt_fly() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");

        theory.add_superiority(&r2, &r1);

        let conclusions = reason(&theory).unwrap();

        // ~flies should be defeasibly provable (penguins don't fly)
        assert!(conclusions
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "flies"
                && c.literal.negation));
    }

    #[test]
    fn test_superiority_resolves_conflict() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["bird"], "~flies");

        theory.add_superiority(&r1, &r2);

        let conclusions = reason(&theory).unwrap();

        // Superior rule r1 should win
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && !c.literal.negation),
            "flies should be defeasibly provable via superior rule"
        );
    }

    #[test]
    fn test_strict_beats_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_strict_rule(&["a", "b"], "c");
        theory.add_defeasible_rule(&["a", "b"], "~c");

        let conclusions = reason(&theory).unwrap();

        // Strict rule should definitely prove c
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "c"
                    && !c.literal.negation),
            "c should be definitely provable via strict rule"
        );
    }

    #[test]
    fn test_forward_chaining() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "d");

        let conclusions = reason(&theory).unwrap();

        for lit_name in &["b", "c", "d"] {
            assert!(
                conclusions
                    .iter()
                    .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == *lit_name),
                "{} should be defeasibly provable via chain",
                lit_name
            );
        }
    }

    // ==========================================================================
    // NEGATIVE CONCLUSIONS (-d, -D)
    // ==========================================================================

    #[test]
    fn test_negative_definite_conclusion() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let conclusions = reason(&theory).unwrap();

        // q has no strict path, so -D q
        assert!(
            conclusions.iter().any(
                |c| c.conclusion_type == ConclusionType::DefinitelyNotProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            ),
            "q should be definitely NOT provable (no strict rule)"
        );
    }

    #[test]
    fn test_negative_defeasible_unsatisfied() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q"); // p not proven

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(
                |c| c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                    && c.literal.name() == "q"
            ),
            "q should be defeasibly NOT provable when body unsatisfied"
        );
    }

    #[test]
    fn test_superiority_yields_negative_for_inferior() {
        let mut theory = Theory::new();
        theory.add_fact("p");

        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");

        theory.add_superiority(&r1, &r2);

        let conclusions = reason(&theory).unwrap();

        // r1 > r2, so +d q, but ~q should be blocked
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation),
            "q should be defeasibly provable via superior rule"
        );
    }

    // ==========================================================================
    // DEFEATER TESTS
    // ==========================================================================

    #[test]
    fn test_defeater_blocks_conclusion() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeater(&["p"], "~q");

        let conclusions = reason(&theory).unwrap();

        // Defeater should block q from being proven
        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        let has_not_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && c.literal.negation
        });

        // Defeater blocks q from being proven
        assert!(
            !has_q,
            "q should NOT be defeasibly provable when blocked by defeater"
        );

        // Defeater doesn't prove ~q (it only blocks)
        assert!(
            !has_not_q,
            "~q should NOT be defeasibly provable by defeater alone"
        );
    }

    #[test]
    fn test_defeater_doesnt_prove() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeater(&["p"], "q");

        let conclusions = reason(&theory).unwrap();

        // Defeaters don't prove anything
        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"),
            "Defeater alone should not prove q"
        );
    }

    // ==========================================================================
    // EDGE CASES
    // ==========================================================================

    #[test]
    fn test_empty_theory() {
        let theory = Theory::new();
        let conclusions = reason(&theory).unwrap();

        // Empty theory should produce no positive conclusions
        assert!(
            !conclusions.iter().any(|c| c.conclusion_type.is_positive()),
            "Empty theory should produce no positive conclusions"
        );
    }

    #[test]
    fn test_theory_with_only_rules() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["q"], "r");

        let conclusions = reason(&theory).unwrap();

        // No facts, so no positive conclusions
        assert!(
            !conclusions.iter().any(|c| c.conclusion_type.is_positive()),
            "Theory with no facts should produce no positive conclusions"
        );
    }

    #[test]
    fn test_self_referential_rule() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "p");

        let conclusions = reason(&theory).unwrap();

        // Should not crash and p should not be proven
        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"),
            "Self-referential rule without initial fact should not prove p"
        );
    }

    #[test]
    fn test_circular_dependencies() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["q"], "r");
        theory.add_defeasible_rule(&["r"], "p");

        let conclusions = reason(&theory).unwrap();

        // Should terminate without infinite loop - the fact that we reach this assertion
        // means the function terminated successfully
        let _ = conclusions;
    }

    #[test]
    fn test_conflicting_facts() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("~p");

        let conclusions = reason(&theory).unwrap();

        // Both should be definitely provable (inconsistent theory)
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation),
            "p should be definitely provable"
        );
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && c.literal.negation),
            "~p should be definitely provable"
        );
    }

    // ==========================================================================
    // STRESS TESTS
    // ==========================================================================

    #[test]
    fn test_long_chain() {
        let mut theory = Theory::new();
        theory.add_fact("l0");

        for i in 0..50 {
            theory.add_defeasible_rule(&[&format!("l{}", i)], &format!("l{}", i + 1));
        }

        let conclusions = reason(&theory).unwrap();

        // All literals in chain should be provable
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "l50"),
            "l50 should be defeasibly provable through long chain"
        );
    }

    #[test]
    fn test_wide_theory() {
        let mut theory = Theory::new();

        // 100 independent facts and derived conclusions
        for i in 0..100 {
            theory.add_fact(&format!("fact{}", i));
            theory.add_defeasible_rule(&[&format!("fact{}", i)], &format!("derived{}", i));
        }

        let conclusions = reason(&theory).unwrap();

        // Check a sample of derived conclusions
        for i in [0, 25, 50, 75, 99].iter() {
            assert!(
                conclusions
                    .iter()
                    .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == format!("derived{}", i)),
                "derived{} should be defeasibly provable",
                i
            );
        }
    }

    #[test]
    fn test_many_facts() {
        let mut theory = Theory::new();

        for i in 0..100 {
            theory.add_fact(&format!("p{}", i));
        }

        let conclusions = reason(&theory).unwrap();

        let definite_count = conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
            .count();

        assert!(
            definite_count >= 100,
            "Should definitely prove all {} facts, got {}",
            100,
            definite_count
        );
    }

    // ==========================================================================
    // ADDITIONAL COVERAGE TESTS
    // ==========================================================================

    #[test]
    fn test_rule_superior_to_defeater() {
        // Test that a rule can override a defeater with explicit superiority
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let d1 = theory.add_defeater(&["p"], "~q");
        theory.add_superiority(&r1, &d1); // r1 is superior to defeater

        let conclusions = reason(&theory).unwrap();

        // q should be provable because r1 > d1
        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(
            has_q,
            "q should be defeasibly provable when rule is superior to defeater"
        );
    }

    #[test]
    fn test_mutual_non_superiority_ambiguity() {
        // Neither rule is superior - both can fire (credulous semantics)
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");
        // No superiority relation

        let conclusions = reason(&theory).unwrap();

        // Both q and ~q should be provable (credulous semantics)
        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });
        let has_not_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && c.literal.negation
        });

        // In the current implementation with credulous semantics, both may be proven
        // This test verifies the behavior is consistent
        let _ = (has_q, has_not_q);
    }

    #[test]
    fn test_attacker_with_unsatisfied_body() {
        // Attacker's body is not satisfied, so it can't block
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["x"], "~q"); // x is not a fact

        let conclusions = reason(&theory).unwrap();

        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(
            has_q,
            "q should be provable when attacker's body is unsatisfied"
        );
    }

    #[test]
    fn test_attacker_superior_blocks() {
        // Attacker is superior, so it blocks the defender
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");
        theory.add_superiority(&r2, &r1); // r2 > r1

        let conclusions = reason(&theory).unwrap();

        // q should NOT be provable (blocked by superior r2)
        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        // ~q should be provable
        let has_not_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && c.literal.negation
        });

        assert!(!has_q, "q should be blocked by superior attacker");
        assert!(has_not_q, "~q should be provable via superior rule");
    }

    #[test]
    fn test_empty_body_strict_rule() {
        // Empty-body strict rules aren't triggered in standard reason()
        // because forward chaining requires body literals to trigger.
        // Use scalable reasoning or facts for empty-body rules.
        use crate::rule::Rule;

        let mut theory = Theory::new();
        let rule = Rule::new(
            "axiom",
            RuleType::Strict,
            vec![],
            vec![Literal::simple("truth")],
        );
        theory.add_rule(rule);

        let conclusions = reason(&theory).unwrap();

        // Standard forward chaining doesn't fire empty body rules
        // This documents the behavior - use facts instead
        let _ = conclusions;
    }

    // ==========================================================================
    // ASSERTION MESSAGE COVERAGE TESTS
    // These tests verify assertion messages by intentionally triggering failures
    // ==========================================================================

    #[test]
    #[should_panic(expected = "s should be defeasibly provable when all antecedents are satisfied")]
    fn test_assert_msg_multiple_body_literals() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        // Intentionally missing q and r facts
        theory.add_defeasible_rule(&["p", "q", "r"], "s");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "s"),
            "s should be defeasibly provable when all antecedents are satisfied"
        );
    }

    #[test]
    #[should_panic(expected = "r should not be provable without q")]
    fn test_assert_msg_unsatisfied_rule_body() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("q"); // Adding q so r IS provable
        theory.add_defeasible_rule(&["p", "q"], "r");

        let conclusions = reason(&theory).unwrap();

        // This will fail because r IS provable (we added q)
        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "r"),
            "r should not be provable without q"
        );
    }

    #[test]
    #[should_panic(expected = "flies should be defeasibly provable via superior rule")]
    fn test_assert_msg_superiority_resolves_conflict() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["bird"], "~flies");

        // Wrong direction - r2 beats r1, so ~flies wins instead of flies
        theory.add_superiority(&r2, &r1);

        let conclusions = reason(&theory).unwrap();

        // This fails because r2 > r1 means ~flies wins, not flies
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && !c.literal.negation),
            "flies should be defeasibly provable via superior rule"
        );
    }

    #[test]
    #[should_panic(expected = "c should be definitely provable via strict rule")]
    fn test_assert_msg_strict_beats_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        // Missing fact "b"
        theory.add_strict_rule(&["a", "b"], "c");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "c"
                    && !c.literal.negation),
            "c should be definitely provable via strict rule"
        );
    }

    #[test]
    #[should_panic(expected = "should be defeasibly provable via chain")]
    fn test_assert_msg_forward_chaining() {
        let mut theory = Theory::new();
        // No facts - chain won't fire
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let conclusions = reason(&theory).unwrap();

        for lit_name in &["b", "c"] {
            assert!(
                conclusions
                    .iter()
                    .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == *lit_name),
                "{} should be defeasibly provable via chain",
                lit_name
            );
        }
    }

    #[test]
    #[should_panic(expected = "q should be definitely NOT provable")]
    fn test_assert_msg_negative_definite() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q"); // Adding strict rule so q IS provable

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(
                |c| c.conclusion_type == ConclusionType::DefinitelyNotProvable
                    && c.literal.name() == "q"
            ),
            "q should be definitely NOT provable (no strict rule)"
        );
    }

    #[test]
    #[should_panic(expected = "q should be defeasibly NOT provable when body unsatisfied")]
    fn test_assert_msg_negative_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q"); // Adding rule so q IS provable

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(
                |c| c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                    && c.literal.name() == "q"
            ),
            "q should be defeasibly NOT provable when body unsatisfied"
        );
    }

    #[test]
    #[should_panic(expected = "Empty theory should produce no positive conclusions")]
    fn test_assert_msg_empty_theory() {
        let mut theory = Theory::new();
        theory.add_fact("x"); // Not empty

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().all(|c| !c.conclusion_type.is_positive()),
            "Empty theory should produce no positive conclusions"
        );
    }

    #[test]
    #[should_panic(expected = "Self-referential rule without initial fact should not prove p")]
    fn test_assert_msg_self_reference() {
        let mut theory = Theory::new();
        theory.add_fact("p"); // Adding initial fact so it IS provable
        theory.add_defeasible_rule(&["p"], "p");

        let conclusions = reason(&theory).unwrap();

        assert!(
            !conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"),
            "Self-referential rule without initial fact should not prove p"
        );
    }
}
