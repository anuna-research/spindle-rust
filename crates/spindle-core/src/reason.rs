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

    // Track rule body satisfaction - separate counters for each phase
    let rule_count = theory.rule_count();
    let mut definite_body_remaining: FxHashMap<&str, usize> =
        FxHashMap::with_capacity_and_hasher(rule_count, Default::default());
    let mut defeasible_body_remaining: FxHashMap<&str, usize> =
        FxHashMap::with_capacity_and_hasher(rule_count, Default::default());
    for rule in theory.rules() {
        definite_body_remaining.insert(&rule.label, rule.body.len());
        defeasible_body_remaining.insert(&rule.label, rule.body.len());
    }

    // ====================================================================
    // PHASE 1: Definite provability (+D) — facts and strict rules only
    // ====================================================================
    {
        let mut worklist: VecDeque<Literal> = VecDeque::with_capacity(rule_count);
        let mut enqueued = LiteralBitSet::new(atom_count);

        // Seed facts
        for fact in theory.facts() {
            let lit = fact.head_literal().clone();
            let lit_id = indexed.intern_literal(&lit);

            if enqueued.contains(lit_id) {
                continue;
            }
            enqueued.insert(lit_id);
            definite_proven.insert(lit_id);

            conclusions
                .push(Conclusion::definitely_provable(lit.clone()).with_rule(&fact.label));
            worklist.push_back(lit);
        }

        // Seed empty-body strict rules
        for rule in theory.rules() {
            if rule.body.is_empty() && rule.rule_type == RuleType::Strict {
                let head_lit = rule.head_literal().clone();
                let head_id = indexed.intern_literal(&head_lit);

                if !definite_proven.contains(head_id) {
                    definite_proven.insert(head_id);
                    conclusions.push(
                        Conclusion::definitely_provable(head_lit.clone())
                            .with_rule(&rule.label),
                    );
                }
                if !enqueued.contains(head_id) {
                    enqueued.insert(head_id);
                    worklist.push_back(head_lit);
                }
            }
        }

        // Definite forward chaining — strict rules only
        while let Some(lit) = worklist.pop_front() {
            for rule in indexed.rules_with_body(&lit) {
                if rule.rule_type != RuleType::Strict {
                    continue;
                }
                let remaining = definite_body_remaining
                    .get_mut(rule.label.as_str())
                    .unwrap();
                if *remaining > 0 {
                    *remaining -= 1;
                    if *remaining == 0 {
                        let head_lit = rule.head_literal().clone();
                        let head_id = indexed
                            .get_lit_id(&head_lit)
                            .expect("Head literal missing from index");

                        if !definite_proven.contains(head_id) {
                            definite_proven.insert(head_id);
                            conclusions.push(
                                Conclusion::definitely_provable(head_lit.clone())
                                    .with_rule(&rule.label),
                            );
                            if !enqueued.contains(head_id) {
                                enqueued.insert(head_id);
                                worklist.push_back(head_lit);
                            }
                        }
                    }
                }
            }
        }
    }

    // ====================================================================
    // PHASE 2: Defeasible provability (+d / -d) — uses complete +D results
    // ====================================================================
    // Implements the spec's compute_defeasible with both positive and negative
    // conclusion propagation via try_prove_defeasible / try_disprove_defeasible.
    let mut defeasible_disproven = LiteralBitSet::new(atom_count);

    // Per-rule tracking: has any body literal been proved -d?
    let mut rule_discarded: FxHashMap<&str, bool> =
        FxHashMap::with_capacity_and_hasher(rule_count, Default::default());
    for rule in theory.rules() {
        rule_discarded.insert(&rule.label, false);
    }

    {
        // Worklist carries (LitId, proved: bool) — true = +d, false = -d
        let mut worklist: VecDeque<(LitId, bool)> = VecDeque::with_capacity(rule_count * 2);

        // --- Seed +d from +D (subsumption), but respect condition (2) ---
        // +D q → +d q ONLY IF -D ~q (condition 2). When +D q AND +D ~q,
        // neither gets +d.
        let all_ids: Vec<LitId> = indexed.all_literal_ids().cloned().collect();
        for &lit_id in &all_ids {
            if definite_proven.contains(lit_id) {
                let comp_id = lit_id.complement();
                if !definite_proven.contains(comp_id) {
                    // Normal case: +D q and -D ~q → +d q
                    defeasible_proven.insert(lit_id);
                    let lit = indexed.resolve_literal(lit_id);
                    conclusions.push(Conclusion::defeasibly_provable(lit));
                    worklist.push_back((lit_id, true));
                }
                // If +D q AND +D ~q: don't seed +d (condition 2 fails).
                // Will be handled by -d seeding below.
            }
        }

        // --- Seed -d for literals that can never be defeasibly proved ---
        for &lit_id in &all_ids {
            if defeasible_proven.contains(lit_id) || defeasible_disproven.contains(lit_id) {
                continue;
            }

            // Must be -D to be -d (if +D, it would have been seeded +d above,
            // unless condition 2 failed)
            if definite_proven.contains(lit_id) {
                // +D q but condition (2) failed (+D ~q too) → -d q
                defeasible_disproven.insert(lit_id);
                worklist.push_back((lit_id, false));
                continue;
            }

            // -D q: check if Rsd[q] is empty (no strict/defeasible rules can fire)
            let has_sd_rule = indexed
                .rules_with_head_id(lit_id)
                .iter()
                .any(|r| matches!(r.rule_type, RuleType::Strict | RuleType::Defeasible | RuleType::Fact));

            if !has_sd_rule {
                defeasible_disproven.insert(lit_id);
                worklist.push_back((lit_id, false));
            }
        }

        // --- Seed empty-body defeasible/strict rules not yet decided ---
        // These rules have body_remaining=0 from initialization but nothing
        // in the body index triggers them, so we must explicitly try to prove
        // their heads.
        for rule in theory.rules() {
            if rule.body.is_empty()
                && matches!(
                    rule.rule_type,
                    RuleType::Defeasible | RuleType::Strict | RuleType::Fact
                )
            {
                let head_id = indexed
                    .get_lit_id(rule.head_literal())
                    .expect("Head literal missing from index");
                try_prove_defeasible(
                    head_id,
                    &indexed,
                    theory,
                    &definite_proven,
                    &mut defeasible_proven,
                    &mut defeasible_disproven,
                    &defeasible_body_remaining,
                    &rule_discarded,
                    &mut worklist,
                    &mut conclusions,
                );
            }
        }

        // --- Fixed-point loop ---
        while let Some((q_id, proved)) = worklist.pop_front() {
            // Update rule counters for ALL rules containing q in body
            let rules_with_q: Vec<String> = indexed
                .rules_with_body_id(q_id)
                .iter()
                .map(|r| r.label.clone())
                .collect();

            for rule_label in &rules_with_q {
                if proved {
                    let remaining = defeasible_body_remaining
                        .get_mut(rule_label.as_str())
                        .unwrap();
                    if *remaining > 0 {
                        *remaining -= 1;
                    }
                } else {
                    *rule_discarded.get_mut(rule_label.as_str()).unwrap() = true;
                }
            }

            if proved {
                // q just proved +d
                // 1. Try to prove/disprove heads of newly-applicable rules
                for rule_label in &rules_with_q {
                    let rule = theory.get_rule(rule_label).unwrap();
                    if !matches!(rule.rule_type, RuleType::Strict | RuleType::Defeasible | RuleType::Fact) {
                        continue;
                    }
                    let remaining = defeasible_body_remaining[rule_label.as_str()];
                    let discarded = rule_discarded[rule_label.as_str()];
                    if remaining == 0 && !discarded {
                        let head_id = indexed
                            .get_lit_id(rule.head_literal())
                            .expect("Head literal missing from index");
                        // Try to prove the rule's head
                        try_prove_defeasible(
                            head_id,
                            &indexed,
                            theory,
                            &definite_proven,
                            &mut defeasible_proven,
                            &mut defeasible_disproven,
                            &defeasible_body_remaining,
                            &rule_discarded,
                            &mut worklist,
                            &mut conclusions,
                        );
                        // This rule is now a resolved attacker for ~head.
                        // Re-check ~head: a previous try_prove might have
                        // returned early because this attacker wasn't resolved.
                        let comp_head = head_id.complement();
                        try_prove_defeasible(
                            comp_head,
                            &indexed,
                            theory,
                            &definite_proven,
                            &mut defeasible_proven,
                            &mut defeasible_disproven,
                            &defeasible_body_remaining,
                            &rule_discarded,
                            &mut worklist,
                            &mut conclusions,
                        );
                        try_disprove_defeasible(
                            comp_head,
                            &indexed,
                            theory,
                            &definite_proven,
                            &mut defeasible_proven,
                            &mut defeasible_disproven,
                            &defeasible_body_remaining,
                            &rule_discarded,
                            &mut worklist,
                            &mut conclusions,
                        );
                    }
                }

                // Also handle defeaters that become newly applicable
                for rule_label in &rules_with_q {
                    let rule = theory.get_rule(rule_label).unwrap();
                    if rule.rule_type != RuleType::Defeater {
                        continue;
                    }
                    let remaining = defeasible_body_remaining[rule_label.as_str()];
                    let discarded = rule_discarded[rule_label.as_str()];
                    if remaining == 0 && !discarded {
                        // Defeater is now applicable — re-check the complement
                        let head_id = indexed
                            .get_lit_id(rule.head_literal())
                            .expect("Head literal missing from index");
                        let comp_head = head_id.complement();
                        try_prove_defeasible(
                            comp_head,
                            &indexed,
                            theory,
                            &definite_proven,
                            &mut defeasible_proven,
                            &mut defeasible_disproven,
                            &defeasible_body_remaining,
                            &rule_discarded,
                            &mut worklist,
                            &mut conclusions,
                        );
                        try_disprove_defeasible(
                            comp_head,
                            &indexed,
                            theory,
                            &definite_proven,
                            &mut defeasible_proven,
                            &mut defeasible_disproven,
                            &defeasible_body_remaining,
                            &rule_discarded,
                            &mut worklist,
                            &mut conclusions,
                        );
                    }
                }

                // 2. q being +d may cause ~q to become -d
                let comp_id = q_id.complement();
                try_disprove_defeasible(
                    comp_id,
                    &indexed,
                    theory,
                    &definite_proven,
                    &mut defeasible_proven,
                    &mut defeasible_disproven,
                    &defeasible_body_remaining,
                    &rule_discarded,
                    &mut worklist,
                    &mut conclusions,
                );
            } else {
                // q just proved -d
                // 1. Rules with q in body are now discarded → try -d for their heads
                //    and re-check ~head (attacker removed → might unblock)
                for rule_label in &rules_with_q {
                    let rule = theory.get_rule(rule_label).unwrap();
                    let head_id = indexed
                        .get_lit_id(rule.head_literal())
                        .expect("Head literal missing from index");

                    if matches!(rule.rule_type, RuleType::Strict | RuleType::Defeasible | RuleType::Fact) {
                        try_disprove_defeasible(
                            head_id,
                            &indexed,
                            theory,
                            &definite_proven,
                            &mut defeasible_proven,
                            &mut defeasible_disproven,
                            &defeasible_body_remaining,
                            &rule_discarded,
                            &mut worklist,
                            &mut conclusions,
                        );
                    }

                    // Rule is now discarded as attacker → re-check complement
                    let comp_head = head_id.complement();
                    try_prove_defeasible(
                        comp_head,
                        &indexed,
                        theory,
                        &definite_proven,
                        &mut defeasible_proven,
                        &mut defeasible_disproven,
                        &defeasible_body_remaining,
                        &rule_discarded,
                        &mut worklist,
                        &mut conclusions,
                    );
                }

                // 2. q being -d means attackers using q in body are discarded
                //    → try +d for complement(q)
                let comp_id = q_id.complement();
                try_prove_defeasible(
                    comp_id,
                    &indexed,
                    theory,
                    &definite_proven,
                    &mut defeasible_proven,
                    &mut defeasible_disproven,
                    &defeasible_body_remaining,
                    &rule_discarded,
                    &mut worklist,
                    &mut conclusions,
                );
            }
        }
    }

    // ====================================================================
    // PHASE 3: Emit remaining conclusions (-D, -d)
    // ====================================================================
    let all_ids: Vec<LitId> = indexed.all_literal_ids().cloned().collect();

    for lit_id in all_ids {
        if !definite_proven.contains(lit_id) {
            let lit = indexed.resolve_literal(lit_id);
            conclusions.push(Conclusion::new(ConclusionType::DefinitelyNotProvable, lit));
        }

        if !defeasible_proven.contains(lit_id) {
            if !defeasible_disproven.contains(lit_id) {
                // Safety net: anything still undecided is -d
                defeasible_disproven.insert(lit_id);
            }
            let lit = indexed.resolve_literal(lit_id);
            conclusions.push(Conclusion::new(ConclusionType::DefeasiblyNotProvable, lit));
        }
    }

    Ok(conclusions)
}

/// Try to prove `+d q`. Implements spec condition (3) of `+d q`:
/// for EVERY applicable attacker `s ∈ R[~q]`, either `s` is discarded,
/// or there exists `t ∈ Rsd[q]` with `t` applicable AND `t > s`.
#[allow(clippy::too_many_arguments)]
fn try_prove_defeasible(
    q: LitId,
    indexed: &IndexedTheory<'_>,
    theory: &Theory,
    definite_proven: &LiteralBitSet,
    defeasible_proven: &mut LiteralBitSet,
    defeasible_disproven: &mut LiteralBitSet,
    body_remaining: &FxHashMap<&str, usize>,
    rule_discarded: &FxHashMap<&str, bool>,
    worklist: &mut VecDeque<(LitId, bool)>,
    conclusions: &mut Vec<Conclusion>,
) {
    if defeasible_proven.contains(q) || defeasible_disproven.contains(q) {
        return; // already decided
    }

    let nq = q.complement();

    // Condition (1): ∃r ∈ Rsd[q] that is applicable
    let supporting_rules = indexed.rules_with_head_id(q);
    let has_applicable = supporting_rules.iter().any(|r| {
        matches!(r.rule_type, RuleType::Strict | RuleType::Defeasible | RuleType::Fact)
            && body_remaining
                .get(r.label.as_str())
                .is_some_and(|&rem| rem == 0)
            && !rule_discarded.get(r.label.as_str()).copied().unwrap_or(false)
    });
    if !has_applicable {
        return;
    }

    // Condition (2): -D ~q (complement is not definitely proved)
    if definite_proven.contains(nq) {
        return;
    }

    // Condition (3): every attacker for ~q is countered
    let attacking_rules = indexed.rules_with_head_id(nq);
    let applicable_supporters: Vec<&crate::rule::Rule> = supporting_rules
        .iter()
        .filter(|r| {
            matches!(r.rule_type, RuleType::Strict | RuleType::Defeasible | RuleType::Fact)
                && body_remaining
                    .get(r.label.as_str())
                    .is_some_and(|&rem| rem == 0)
                && !rule_discarded.get(r.label.as_str()).copied().unwrap_or(false)
        })
        .copied()
        .collect();

    for attacker in &attacking_rules {
        let att_discarded = rule_discarded
            .get(attacker.label.as_str())
            .copied()
            .unwrap_or(false);
        if att_discarded {
            continue; // attacker is inapplicable
        }

        let att_remaining = body_remaining
            .get(attacker.label.as_str())
            .copied()
            .unwrap_or(0);
        if att_remaining > 0 {
            // Attacker's body not fully decided — can't conclude yet
            return;
        }

        // Strict attackers always block
        if attacker.rule_type == RuleType::Strict {
            return;
        }

        // Attacker is applicable. Need ∃t ∈ Rsd[q]: t applicable AND t > s
        let defeated = applicable_supporters.iter().any(|t| {
            theory.is_superior(t.template_label(), attacker.template_label())
        });

        if !defeated {
            return; // undefeated attacker
        }
    }

    // All conditions met — pick the first applicable supporter as the deriving rule
    defeasible_proven.insert(q);
    let lit = indexed.resolve_literal(q);
    let conclusion = if let Some(supporter) = applicable_supporters.first() {
        Conclusion::defeasibly_provable(lit).with_rule(&supporter.label)
    } else {
        Conclusion::defeasibly_provable(lit)
    };
    conclusions.push(conclusion);
    worklist.push_back((q, true));
}

/// Try to disprove `q` (prove `-d q`). Implements the dual/mirror of `+d`.
#[allow(clippy::too_many_arguments)]
fn try_disprove_defeasible(
    q: LitId,
    indexed: &IndexedTheory<'_>,
    theory: &Theory,
    definite_proven: &LiteralBitSet,
    defeasible_proven: &mut LiteralBitSet,
    defeasible_disproven: &mut LiteralBitSet,
    body_remaining: &FxHashMap<&str, usize>,
    rule_discarded: &FxHashMap<&str, bool>,
    worklist: &mut VecDeque<(LitId, bool)>,
    _conclusions: &mut Vec<Conclusion>,
) {
    if defeasible_proven.contains(q) || defeasible_disproven.contains(q) {
        return; // already decided
    }

    // Precondition: must be -D q (if +D q, it would already be +d or -d)
    if definite_proven.contains(q) {
        return;
    }

    let nq = q.complement();

    // Disjunct (2): +D ~q → -d q
    if definite_proven.contains(nq) {
        defeasible_disproven.insert(q);
        worklist.push_back((q, false));
        return;
    }

    // Disjunct (1): all Rsd[q] rules are discarded → -d q (no support)
    let supporting_rules = indexed.rules_with_head_id(q);
    let sd_rules: Vec<&crate::rule::Rule> = supporting_rules
        .iter()
        .filter(|r| matches!(r.rule_type, RuleType::Strict | RuleType::Defeasible | RuleType::Fact))
        .copied()
        .collect();

    let all_discarded = sd_rules.iter().all(|r| {
        rule_discarded
            .get(r.label.as_str())
            .copied()
            .unwrap_or(false)
    });

    if all_discarded {
        defeasible_disproven.insert(q);
        worklist.push_back((q, false));
        return;
    }

    // Disjunct (3): ∃ applicable attacker s that no t in Rsd[q] can beat
    let attacking_rules = indexed.rules_with_head_id(nq);
    for attacker in &attacking_rules {
        let att_remaining = body_remaining
            .get(attacker.label.as_str())
            .copied()
            .unwrap_or(0);
        let att_discarded = rule_discarded
            .get(attacker.label.as_str())
            .copied()
            .unwrap_or(false);

        if att_remaining > 0 || att_discarded {
            continue; // attacker not applicable
        }

        // Attacker s is applicable. Check: ∀t ∈ Rsd[q]: t discarded OR ¬(t > s)
        // But if any t is undecided (not discarded, not applicable), can't conclude
        let any_t_undecided = sd_rules.iter().any(|t| {
            let t_discarded = rule_discarded
                .get(t.label.as_str())
                .copied()
                .unwrap_or(false);
            let t_remaining = body_remaining
                .get(t.label.as_str())
                .copied()
                .unwrap_or(0);
            !t_discarded
                && t_remaining > 0
                && theory.is_superior(t.template_label(), attacker.template_label())
        });

        if any_t_undecided {
            continue; // can't conclude yet for this attacker
        }

        let all_t_fail = sd_rules.iter().all(|t| {
            let t_discarded = rule_discarded
                .get(t.label.as_str())
                .copied()
                .unwrap_or(false);
            t_discarded || !theory.is_superior(t.template_label(), attacker.template_label())
        });

        if all_t_fail {
            defeasible_disproven.insert(q);
            worklist.push_back((q, false));
            return;
        }
    }
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

        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "c"
                    && c.literal.negation
            }),
            "~c should not be defeasibly provable against a strict attacker"
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
    fn test_ambiguity_blocking_no_superiority() {
        // SDL ambiguity blocking: conflicting defeasible rules with no superiority
        // relation should block BOTH conclusions (neither is +d).
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");
        // No superiority relation

        let conclusions = reason(&theory).unwrap();

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
            "q should NOT be defeasibly provable (ambiguity blocking)"
        );
        assert!(
            !has_not_q,
            "~q should NOT be defeasibly provable (ambiguity blocking)"
        );

        // Both should be -d
        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "-d q expected"
        );
        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                    && c.literal.name() == "q"
                    && c.literal.negation
            }),
            "-d ~q expected"
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
    // REGRESSION TESTS: Defeasible conflict handling
    // ==========================================================================

    #[test]
    fn test_conflict_resolution_with_superiority() {
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
    fn test_conflict_resolution_no_conflict_when_attacker_unsatisfied() {
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

    // ==========================================================================
    // SDL SPEC COMPLIANCE TESTS (specs/DEFEASIBLE-LOGIC-SEMANTICS.md)
    // ==========================================================================

    #[test]
    fn test_spec_worked_example_conflicting_facts() {
        // Spec worked example (lines 114-134):
        //   f1: >> p
        //   f2: >> -p
        // Both are facts. No superiority. Result: -d p, -d -p.
        //
        // Note: our implementation treats facts as +D (definitively provable).
        // With +D p and +D -p, condition (2) of +d fails for both:
        //   +d p requires -D -p, but +D -p, so blocked.
        //   +d -p requires -D p, but +D p, so blocked.
        // So neither is defeasibly provable even though both are definite.
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("~p");

        let conclusions = reason(&theory).unwrap();

        // Both should be +D (they are axioms)
        assert!(conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.name() == "p"
                && !c.literal.negation
        }));
        assert!(conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.name() == "p"
                && c.literal.negation
        }));
    }

    #[test]
    fn test_spec_ambiguity_blocking_localized() {
        // Spec section "Ambiguity Blocking" (lines 143-148):
        // When p and ~p are blocked, downstream rules that don't depend on p
        // proceed normally.
        //
        //   r1: => p
        //   r2: => -p
        //   r3: => q          (independent support for q)
        //   r4: p => q         (depends on ambiguous p)
        //
        // Result: -d p, -d -p, but +d q (via r3, uncontested)
        use crate::rule::Rule;

        let mut theory = Theory::new();
        theory.add_rule(Rule::new(
            "r1",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));
        theory.add_rule(Rule::new(
            "r2",
            RuleType::Defeasible,
            vec![],
            vec![Literal::new("p", true, Default::default(), Default::default(), vec![])],
        ));
        theory.add_rule(Rule::new(
            "r3",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("q")],
        ));
        theory.add_rule(Rule::defeasible(
            "r4",
            vec![Literal::simple("p")],
            Literal::simple("q"),
        ));

        let conclusions = reason(&theory).unwrap();

        // p and ~p are ambiguous → -d
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            }),
            "p should NOT be +d (ambiguity blocked)"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && c.literal.negation
            }),
            "~p should NOT be +d (ambiguity blocked)"
        );

        // q should be +d via r3 (uncontested, independent of ambiguous p)
        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "q should be +d via independent rule r3 (ambiguity blocking is localized)"
        );
    }

    #[test]
    fn test_spec_superiority_defeats_attacker() {
        // Spec condition (3): an applicable attacker s is defeated if
        // ∃t ∈ Rsd[q]: t applicable AND t > s.
        //
        //   >> bird, >> penguin
        //   r1: bird => flies
        //   r2: penguin => ~flies
        //   r2 > r1
        //
        // Result: +d ~flies (r2 wins), -d flies (r1 is defeated)
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r2, &r1);

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && c.literal.negation
            }),
            "+d ~flies expected (superior rule)"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "flies"
                    && !c.literal.negation
            }),
            "flies should NOT be +d (defeated by superior r2)"
        );
    }

    #[test]
    fn test_spec_defeater_uniform_blocking() {
        // Defeaters and defeasible attackers use the same blocking condition.
        //   >> p
        //   r1: p => q
        //   d1: p ~> ~q     (defeater)
        //
        // d1 is an applicable attacker. Need t ∈ Rsd[q] with t > d1.
        // r1 exists but no superiority → blocked.
        // Result: -d q AND -d ~q (defeater blocks but can't prove)
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeater(&["p"], "~q");

        let conclusions = reason(&theory).unwrap();

        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "q should NOT be +d (blocked by defeater)"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && c.literal.negation
            }),
            "~q should NOT be +d (defeater can't prove)"
        );
    }

    #[test]
    fn test_spec_cross_rule_superiority() {
        // Issue 5: is_blocked_by_superior must check ALL applicable Rsd[q] rules.
        // Rule t (different from the triggering rule) may be the one that defeats
        // the attacker.
        //
        //   >> a, >> b
        //   r1: a => q
        //   r2: b => ~q
        //   r3: b => q         (r3 > r2)
        //
        // r1 fires first. Attacker r2 is applicable. r1 is NOT superior to r2,
        // but r3 IS superior to r2. So the attacker is defeated, +d q.
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");

        theory.add_defeasible_rule(&["a"], "q");
        let r2 = theory.add_defeasible_rule(&["b"], "~q");
        let r3 = theory.add_defeasible_rule(&["b"], "q");
        theory.add_superiority(&r3, &r2);

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "+d q expected (r3 > r2 defeats the attacker, even though r1 triggered first)"
        );
    }

    #[test]
    fn test_spec_strict_always_blocks_defeasible() {
        // Strict attackers always block defeasible conclusions, regardless of
        // superiority. (+D ~q means condition (2) of +d q fails.)
        //
        //   >> a
        //   r1: a => q
        //   r2: a -> ~q     (strict)
        //
        // Result: +D ~q, -d q
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "q");
        theory.add_strict_rule(&["a"], "~q");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "q"
                    && c.literal.negation
            }),
            "+D ~q expected"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            }),
            "q should NOT be +d (strict rule proves ~q)"
        );
    }

    #[test]
    fn test_spec_cascading_chain_uncontested() {
        // Uncontested defeasible chain should propagate.
        //   >> a
        //   r1: a => b
        //   r2: b => c
        //   r3: c => d
        //
        // Result: +d a, +d b, +d c, +d d
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "d");

        let conclusions = reason(&theory).unwrap();

        for lit in &["b", "c", "d"] {
            assert!(
                conclusions.iter().any(|c| {
                    c.conclusion_type == ConclusionType::DefeasiblyProvable
                        && c.literal.name() == *lit
                        && !c.literal.negation
                }),
                "+d {lit} expected in uncontested chain"
            );
        }
    }

    #[test]
    fn test_two_phase_strict_chain_blocks_defeasible() {
        // Regression test for Issue 3 (two-phase reasoning).
        // A multi-hop strict chain to ~q must complete in phase 1 before
        // phase 2 checks definite_proven for the complement.
        //
        //   >> a, >> b
        //   r1: a -> c       (strict)
        //   r2: c -> ~p      (strict chain: a → c → ~p)
        //   r3: b => p       (defeasible)
        //
        // Phase 1 must produce +D ~p. Then phase 2: condition (2) of +d p
        // requires -D ~p, which is false → -d p.
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_strict_rule(&["a"], "c");
        theory.add_strict_rule(&["c"], "~p");
        theory.add_defeasible_rule(&["b"], "p");

        let conclusions = reason(&theory).unwrap();

        assert!(
            conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "p"
                    && c.literal.negation
            }),
            "+D ~p expected from strict chain"
        );
        assert!(
            !conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "p"
                    && !c.literal.negation
            }),
            "p should NOT be +d (blocked by +D ~p from strict chain)"
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
