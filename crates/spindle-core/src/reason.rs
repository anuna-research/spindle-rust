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
use crate::pipeline::{PrepareOptions, prepare};
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
    ///
    /// Automatically grows the bitset if needed, preventing silent data loss
    /// when new atoms are interned after the bitset is initially sized.
    #[inline]
    fn insert(&mut self, id: LitId) {
        let idx = Self::to_index(id);
        if idx >= self.bits.len() {
            self.bits.grow(idx + 1);
        }
        self.bits.insert(idx);
    }
}

/// Perform defeasible reasoning on a theory
pub fn reason(theory: &Theory) -> Result<Vec<Conclusion>> {
    reason_with_options(theory, PrepareOptions::default())
}

/// Perform defeasible reasoning on a theory with custom options
///
/// This is the primary API for as-of reasoning. Use `reference_time` in options
/// to reason at a specific point in time:
///
/// ```rust
/// use spindle_core::prelude::*;
/// use spindle_core::reason::reason_with_options;
/// use spindle_core::pipeline::PrepareOptions;
/// use spindle_core::temporal::TimePoint;
///
/// let mut theory = Theory::new();
/// theory.add_fact("bird");
///
/// // Reason at a specific time (milliseconds since epoch)
/// let opts = PrepareOptions {
///     reference_time: Some(TimePoint::from_millis(1707220800000)),
///     ..Default::default()
/// };
/// let conclusions = reason_with_options(&theory, opts).unwrap();
/// ```
pub fn reason_with_options(theory: &Theory, opts: PrepareOptions) -> Result<Vec<Conclusion>> {
    // Phase 0: Pipeline (Filtering + Validation + Grounding)
    let prepared = prepare(theory, opts)?;

    reason_prepared(&prepared.theory)
}

/// Perform defeasible reasoning on an already-prepared theory.
///
/// Use this when you have already called [`prepare()`] and want to avoid
/// redundant pipeline work. The theory must have been prepared with the
/// desired options (grounding, temporal filtering, etc.) before calling
/// this function.
pub fn reason_prepared(theory: &Theory) -> Result<Vec<Conclusion>> {
    // Use the theory directly for indexing and reasoning
    let mut indexed = IndexedTheory::build(theory);

    // Pre-allocate conclusions vector
    let estimated_size = theory.rule_count() * 2 + indexed.all_literal_ids().count() * 2;
    let mut conclusions = Vec::with_capacity(estimated_size);

    // Track what we've proven using LiteralBitSet for O(1) operations
    let atom_count = indexed.atom_count();
    let mut definite_proven = LiteralBitSet::new(atom_count);
    let mut defeasible_proven = LiteralBitSet::new(atom_count);

    // Track rule body satisfaction - pre-allocate for all rules
    let rule_count = theory.rule_count();
    let mut body_remaining: FxHashMap<&str, usize> =
        FxHashMap::with_capacity_and_hasher(rule_count, Default::default());
    for rule in theory.rules() {
        body_remaining.insert(&rule.label, rule.body.len());
    }

    // Worklist for forward chaining
    let mut worklist: VecDeque<Literal> = VecDeque::with_capacity(rule_count);
    // Track which literals have been enqueued to prevent duplicate processing
    let mut enqueued = LiteralBitSet::new(atom_count);

    // Phase 1: Initialize with facts (deduplicated)
    for fact in theory.facts() {
        let lit = fact.head_literal().clone();
        // Interning here is safe as facts are already in the theory
        let lit_id = indexed.intern_literal(&lit);

        // Skip duplicate facts — only process each literal once
        if enqueued.contains(lit_id) {
            continue;
        }
        enqueued.insert(lit_id);

        definite_proven.insert(lit_id);
        defeasible_proven.insert(lit_id);

        conclusions.push(Conclusion::definitely_provable(lit.clone()).with_rule(&fact.label));
        conclusions.push(Conclusion::defeasibly_provable(lit.clone()).with_rule(&fact.label));

        worklist.push_back(lit);
    }

    // Phase 1b: Initialize empty-body non-fact rules
    // These rules have no body literals so forward chaining never triggers them.
    // We must seed their heads into the worklist explicitly.
    for rule in theory.rules() {
        if rule.body.is_empty() && rule.rule_type != RuleType::Fact {
            let head_lit = rule.head_literal().clone();
            let head_id = indexed.intern_literal(&head_lit);

            match rule.rule_type {
                RuleType::Strict => {
                    // Even if the literal was already enqueued/proven defeasibly,
                    // a strict empty-body rule must still upgrade it to definite.
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
                    }
                    if !enqueued.contains(head_id) {
                        enqueued.insert(head_id);
                        worklist.push_back(head_lit);
                    }
                }
                RuleType::Defeasible => {
                    // Empty-body defeasible rules fire immediately but can still be blocked
                    if !defeasible_proven.contains(head_id) {
                        let blocked = is_blocked_by_superior(
                            &indexed,
                            theory,
                            rule,
                            &defeasible_proven,
                        );
                        if !blocked {
                            defeasible_proven.insert(head_id);
                            conclusions.push(
                                Conclusion::defeasibly_provable(head_lit.clone())
                                    .with_rule(&rule.label),
                            );
                        }
                    }
                    if defeasible_proven.contains(head_id) && !enqueued.contains(head_id) {
                        enqueued.insert(head_id);
                        worklist.push_back(head_lit);
                    }
                }
                _ => {}
            }
        }
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

                                if !enqueued.contains(head_id) {
                                    enqueued.insert(head_id);
                                    worklist.push_back(head_lit);
                                }
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
                                    theory,
                                    rule,
                                    &defeasible_proven,
                                );

                                if !blocked {
                                    defeasible_proven.insert(head_id);
                                    conclusions.push(
                                        Conclusion::defeasibly_provable(head_lit.clone())
                                            .with_rule(&rule.label),
                                    );
                                    if !enqueued.contains(head_id) {
                                        enqueued.insert(head_id);
                                        worklist.push_back(head_lit);
                                    }
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

        // Ambiguity blocking (skeptical semantics): if neither rule is superior
        // over the other and both have satisfied bodies, block the conclusion.
        // This matches scalable.rs behavior and standard DL(d) semantics.
        if !attacker_superior && !rule_superior {
            return true;
        }
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

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "bird")
        );
    }

    #[test]
    fn test_negated_fact() {
        let mut theory = Theory::new();
        theory.add_fact("~guilty");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "guilty"
                    && c.literal.negation)
        );
    }

    #[test]
    fn test_strict_chain() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_strict_rule(&["bird"], "animal");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "animal")
        );
    }

    #[test]
    fn test_defeasible_rule() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies")
        );
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
        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && c.literal.negation)
        );
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
                "{lit_name} should be defeasibly provable via chain"
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
            theory.add_defeasible_rule(&[&format!("l{i}")], &format!("l{}", i + 1));
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
            theory.add_fact(&format!("fact{i}"));
            theory.add_defeasible_rule(&[&format!("fact{i}")], &format!("derived{i}"));
        }

        let conclusions = reason(&theory).unwrap();

        // Check a sample of derived conclusions
        for i in [0, 25, 50, 75, 99].iter() {
            assert!(
                conclusions
                    .iter()
                    .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == format!("derived{i}")),
                "derived{i} should be defeasibly provable"
            );
        }
    }

    #[test]
    fn test_many_facts() {
        let mut theory = Theory::new();

        for i in 0..100 {
            theory.add_fact(&format!("p{i}"));
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
        // Neither rule is superior - ambiguity blocking (skeptical semantics)
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");
        // No superiority relation

        let conclusions = reason(&theory).unwrap();

        // Neither q nor ~q should be defeasibly provable (ambiguity blocking)
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

        assert!(
            !has_q,
            "q should NOT be defeasibly provable under ambiguity blocking"
        );
        assert!(
            !has_not_q,
            "~q should NOT be defeasibly provable under ambiguity blocking"
        );
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
        // Empty-body strict rules should fire and prove their head
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

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "truth"),
            "Empty-body strict rule should prove its head"
        );
    }

    #[test]
    fn test_empty_body_defeasible_rule() {
        use crate::rule::Rule;

        let mut theory = Theory::new();
        let rule = Rule::new(
            "axiom",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("maybe")],
        );
        theory.add_rule(rule);

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "maybe"),
            "Empty-body defeasible rule should prove its head"
        );
    }

    #[test]
    fn test_empty_body_rule_chains() {
        // Empty-body rule should seed forward chaining
        use crate::rule::Rule;

        let mut theory = Theory::new();
        let axiom = Rule::new(
            "axiom",
            RuleType::Strict,
            vec![],
            vec![Literal::simple("base")],
        );
        theory.add_rule(axiom);
        theory.add_defeasible_rule(&["base"], "derived");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "derived"),
            "Forward chaining should work from empty-body rule heads"
        );
    }

    #[test]
    fn test_empty_body_strict_not_lost_when_defeasible_same_head_exists() {
        // Regression: if an empty-body defeasible rule for `p` is seen before an
        // empty-body strict rule for `p`, strict derivation must still be emitted.
        // We iterate labels to avoid depending on HashMap iteration order.
        use crate::rule::Rule;

        for i in 0..128 {
            let mut theory = Theory::new();
            theory.add_rule(Rule::new(
                format!("d{i}"),
                RuleType::Defeasible,
                vec![],
                vec![Literal::simple("p")],
            ));
            theory.add_rule(Rule::new(
                format!("s{i}"),
                RuleType::Strict,
                vec![],
                vec![Literal::simple("p")],
            ));

            let conclusions = reason(&theory).unwrap();
            let has_definite_p = conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            });

            assert!(has_definite_p, "missing +D p for label pair d{i} / s{i}");
        }
    }

    #[test]
    fn test_fact_plus_empty_body_strict_no_duplicate_conclusions() {
        // A fact already proves +D p. An empty-body strict rule for p should not
        // emit a second +D p conclusion.
        use crate::rule::Rule;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_rule(Rule::new(
            "axiom",
            RuleType::Strict,
            vec![],
            vec![Literal::simple("p")],
        ));

        let conclusions = reason(&theory).unwrap();
        let definite_p_count = conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            })
            .count();

        assert_eq!(definite_p_count, 1, "Should have exactly one +D p");
    }

    #[test]
    fn test_fact_plus_empty_body_defeasible_no_duplicate_and_chains() {
        // A fact already proves p. An empty-body defeasible rule for p should
        // not produce a duplicate, and forward chaining from p should still work.
        use crate::rule::Rule;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_rule(Rule::new(
            "d_axiom",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));
        theory.add_defeasible_rule(&["p"], "q");

        let conclusions = reason(&theory).unwrap();

        let defeasible_p_count = conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            })
            .count();
        assert_eq!(defeasible_p_count, 1, "Should have exactly one +d p");

        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });
        assert!(has_q, "Forward chaining from p to q should still work");
    }

    #[test]
    fn test_two_empty_body_defeasible_same_head_only_one_conclusion() {
        // Two empty-body defeasible rules for the same head should produce
        // exactly one +d conclusion, not two.
        use crate::rule::Rule;

        let mut theory = Theory::new();
        theory.add_rule(Rule::new(
            "d1",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));
        theory.add_rule(Rule::new(
            "d2",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));

        let conclusions = reason(&theory).unwrap();
        let defeasible_p_count = conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            })
            .count();

        assert_eq!(defeasible_p_count, 1, "Should have exactly one +d p");
    }

    // ==========================================================================
    // REGRESSION TESTS: Duplicate fact deduplication
    // ==========================================================================

    #[test]
    fn test_duplicate_facts_no_double_conclusions() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("bird"); // duplicate

        let conclusions = reason(&theory).unwrap();

        let bird_definite_count = conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "bird"
            })
            .count();

        assert_eq!(
            bird_definite_count, 1,
            "Duplicate facts should produce exactly one +D conclusion, got {bird_definite_count}"
        );
    }

    #[test]
    fn test_duplicate_facts_no_premature_rule_firing() {
        // Critical regression test: duplicate facts must not cause
        // multi-body rules to fire with unsatisfied body literals.
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("p"); // duplicate
        theory.add_defeasible_rule(&["p", "q"], "r");

        let conclusions = reason(&theory).unwrap();

        let has_r = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "r"
        });

        assert!(
            !has_r,
            "Rule with body [p, q] must NOT fire when only p is proven (even with duplicate p facts)"
        );
    }

    // ==========================================================================
    // REGRESSION TESTS: Ambiguity blocking (skeptical semantics)
    // ==========================================================================

    #[test]
    fn test_ambiguity_blocking_with_superiority() {
        // When superiority resolves the conflict, the superior rule should win
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");
        theory.add_superiority(&r1, &r2);

        let conclusions = reason(&theory).unwrap();

        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(has_q, "Superior rule should prove q");
    }

    #[test]
    fn test_ambiguity_blocking_no_conflict_when_attacker_unsatisfied() {
        // If attacker's body is not satisfied, no blocking should occur
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["unproven"], "~q");

        let conclusions = reason(&theory).unwrap();

        let has_q = conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(
            has_q,
            "q should be provable when attacker body is unsatisfied"
        );
    }

    #[test]
    fn test_standard_scalable_parity_ambiguity() {
        // Standard and scalable algorithms should agree on ambiguity blocking
        use crate::index::IndexedTheory;
        use crate::scalable::reason_scalable;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");

        let standard = reason(&theory).unwrap();
        let indexed = IndexedTheory::build(&theory);
        let scalable = reason_scalable(&indexed);
        let scalable_conclusions = scalable.to_conclusions(&indexed);

        let std_has_q = standard.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });
        let scl_has_q = scalable_conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert_eq!(
            std_has_q, scl_has_q,
            "Standard and scalable should agree on ambiguity blocking for q"
        );
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
                "{lit_name} should be defeasibly provable via chain"
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

    // ==========================================================================
    // reason_with_options API TESTS (spec §3.2, Milestone 5)
    // ==========================================================================

    #[test]
    fn test_reason_with_options_default_matches_reason() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let conclusions_default = reason(&theory).unwrap();
        let conclusions_with_opts =
            reason_with_options(&theory, PrepareOptions::default()).unwrap();

        // Both should produce the same positive conclusions
        let pos_default: Vec<_> = conclusions_default
            .iter()
            .filter(|c| c.is_positive())
            .map(|c| c.literal.name())
            .collect();

        let pos_with_opts: Vec<_> = conclusions_with_opts
            .iter()
            .filter(|c| c.is_positive())
            .map(|c| c.literal.name())
            .collect();

        assert_eq!(pos_default, pos_with_opts);
    }

    #[test]
    fn test_reason_with_options_accepts_reference_time() {
        use crate::temporal::{Temporal, TimePoint};

        let mut theory = Theory::new();

        // Add a fact with a specific temporal window
        let bird_lit = Literal::new(
            "bird",
            false,
            crate::mode::Mode::default(),
            Temporal::from_bounds(1000, 2000), // active from 1000 to 2000
            vec![],
        );
        theory.add_rule(crate::rule::Rule::fact("f1", bird_lit));

        // Reason at time 1500 (inside the window)
        let opts_inside = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(1500)),
            ..Default::default()
        };
        let conclusions_inside = reason_with_options(&theory, opts_inside).unwrap();
        let has_bird_inside = conclusions_inside
            .iter()
            .any(|c| c.literal.name() == "bird" && c.is_positive());
        assert!(has_bird_inside, "bird should be provable at time 1500");

        // Reason at time 3000 (outside the window)
        let opts_outside = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(3000)),
            ..Default::default()
        };
        let conclusions_outside = reason_with_options(&theory, opts_outside).unwrap();
        let has_bird_outside = conclusions_outside
            .iter()
            .any(|c| c.literal.name() == "bird" && c.is_positive());
        assert!(
            !has_bird_outside,
            "bird should NOT be provable at time 3000 (outside temporal window)"
        );
    }

    #[test]
    fn test_reason_with_options_grounding_can_be_disabled() {
        use crate::pipeline::GroundingOptions;

        let mut theory = Theory::new();
        theory.add_fact("p");

        // With grounding disabled, variables won't be instantiated
        let opts = PrepareOptions {
            grounding: GroundingOptions {
                enabled: false,
                ..Default::default()
            },
            ..Default::default()
        };

        // Should still work for ground theories
        let conclusions = reason_with_options(&theory, opts).unwrap();
        assert!(
            conclusions.iter().any(|c| c.is_positive()),
            "should have positive conclusions"
        );
    }

    // ==========================================================================
    // REGRESSION TESTS: LiteralBitSet auto-grow
    // ==========================================================================

    #[test]
    fn test_bitset_grows_on_insert_beyond_capacity() {
        use crate::index::AtomId;

        // Create a small bitset (capacity for 2 atoms = 4 bits)
        let mut bitset = LiteralBitSet::new(2);

        // Insert at atom 10, well beyond initial capacity
        let lit_id = LitId::new(AtomId::from_raw(10), false);
        bitset.insert(lit_id);

        assert!(
            bitset.contains(lit_id),
            "Bitset should contain the inserted literal after growing"
        );
    }

    #[test]
    fn test_bitset_preserves_existing_on_grow() {
        use crate::index::AtomId;

        let mut bitset = LiteralBitSet::new(2);

        // Insert at atom 0
        let lit_0 = LitId::new(AtomId::from_raw(0), false);
        bitset.insert(lit_0);
        assert!(bitset.contains(lit_0));

        // Grow by inserting at atom 10
        let lit_10 = LitId::new(AtomId::from_raw(10), true);
        bitset.insert(lit_10);

        // Atom 0 should still be present
        assert!(
            bitset.contains(lit_0),
            "Existing bit at atom 0 should be preserved after grow"
        );
        assert!(
            bitset.contains(lit_10),
            "New bit at atom 10 should be present after grow"
        );
    }
}
