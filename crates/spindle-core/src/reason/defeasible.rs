//! Phase 2: Defeasible provability (+d / -d) with SDL ambiguity blocking.
//!
//! After Phase 1 has completed all definite (+D) derivations, this module
//! runs a fixed-point loop that propagates both positive (+d) and negative
//! (-d) defeasible conclusions.
//!
//! Also contains Phase 3: emit `-D` and `-d` for all unproven literals.
//!
//! # Algorithm
//!
//! 1. **Seed +d from +D** (subsumption with condition 2): `+D q` yields
//!    `+d q` only if `-D ~q`.
//! 2. **Seed -d** for literals with no Rsd support, and for +D literals
//!    that fail condition (2).
//! 3. **Seed empty-body defeasible rules** via `try_prove_defeasible`.
//! 4. **Fixed-point loop**: process `(LitId, proved)` tuples from a worklist,
//!    updating body counters, trying to prove/disprove heads and complements.
//! 5. **Emit negatives**: `-D` and `-d` for all remaining unproven literals.

use std::collections::{BTreeMap, VecDeque};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::conclusion::{Conclusion, ConclusionType};
use crate::index::{IndexedTheory, LitId};
use crate::projection::FamilyId;
use crate::rule::RuleType;
use crate::theory::Theory;

use super::state::{LiteralBitSet, ReasoningState};

/// Compute the lambda over-approximation: everything that COULD be
/// defeasibly provable, ignoring attacks and superiority. Mirrors
/// `lean/SpindleLean/Closure/Lambda.lean` (`lambdaClose`).
///
/// Seeds from the definite conclusions (+D), then repeatedly fires
/// productive rules (fact/strict/defeasible — not defeaters) whose logic
/// body literals are all already in lambda, excluding heads whose
/// complement is definite. Arithmetic body constraints and un-interned
/// body literals are treated as satisfiable, keeping this a genuine
/// over-approximation.
///
/// Literals OUTSIDE lambda can never be defeasibly proved — in particular,
/// rules with circular, unfounded bodies (e.g. `p => p`) never place their
/// head in lambda. Seeding such literals as -d lets the worklist discard
/// unfounded attackers instead of letting a rule that can never fire block
/// conclusions forever (well-founded reading; see lean/DIVERGENCES.md
/// class 2).
fn compute_lambda<'t>(
    theory: &'t Theory,
    indexed: &IndexedTheory<'t>,
    definite_proven: &LiteralBitSet,
    body_remaining_init: &FxHashMap<&'t str, usize>,
) -> LiteralBitSet {
    let mut lambda = definite_proven.clone();
    // Mirror the main worklist's counter mechanics — including family-aware
    // body matching via rules_with_body_id — with all attack and superiority
    // checks ignored. Starting from a pristine clone of the body counters
    // keeps lambda's notion of "body satisfiable" identical to the fixed
    // point's notion of "body satisfied".
    let mut remaining = body_remaining_init.clone();
    let mut queue: VecDeque<LitId> = VecDeque::new();

    // Seed with all definite literals.
    for &id in indexed.all_literal_ids() {
        if definite_proven.contains(id) {
            queue.push_back(id);
        }
    }

    // Fire rules whose bodies are already fully satisfied (e.g. empty bodies).
    for rule in theory.rules() {
        if !matches!(
            rule.rule_type,
            RuleType::Fact | RuleType::Strict | RuleType::Defeasible
        ) {
            continue;
        }
        if remaining.get(rule.label.as_str()).copied().unwrap_or(0) == 0 {
            for head in &rule.head {
                if let Some(head_id) = indexed.get_lit_id(head)
                    && !lambda.contains(head_id)
                    && !definite_proven.contains(head_id.complement())
                {
                    lambda.insert(head_id);
                    queue.push_back(head_id);
                }
            }
        }
    }

    // Propagate to fixpoint.
    while let Some(id) = queue.pop_front() {
        for rule in indexed.rules_with_body_id(id) {
            if !matches!(
                rule.rule_type,
                RuleType::Fact | RuleType::Strict | RuleType::Defeasible
            ) {
                continue;
            }
            let Some(rem) = remaining.get_mut(rule.label.as_str()) else {
                continue;
            };
            if *rem > 0 {
                *rem -= 1;
            }
            if *rem == 0 {
                for head in &rule.head {
                    if let Some(head_id) = indexed.get_lit_id(head)
                        && !lambda.contains(head_id)
                        && !definite_proven.contains(head_id.complement())
                    {
                        lambda.insert(head_id);
                        queue.push_back(head_id);
                    }
                }
            }
        }
    }

    lambda
}

/// Run Phase 2 (defeasible fixed-point) and Phase 3 (negative emission).
pub(crate) fn resolve_defeasible(
    theory: &Theory,
    indexed: &IndexedTheory<'_>,
    state: &mut ReasoningState<'_>,
) {
    // Phase 2 uses its own worklist carrying (LitId, proved: bool)
    let rule_count = theory.rule_count();
    let estimated_size = rule_count * 2;
    let mut worklist: VecDeque<(LitId, bool)> = VecDeque::with_capacity(estimated_size);

    // --- Seed +d from +D (subsumption), but respect condition (2) ---
    // Sort by SPL string for deterministic iteration order. This is
    // lexicographic (not temporal), which is intentional: we only need a
    // stable, reproducible ordering here, not temporal precedence.
    let mut all_ids: Vec<LitId> = indexed.all_literal_ids().cloned().collect();
    all_ids.sort_by_key(|id| indexed.resolve_literal(*id).to_spl());
    for &lit_id in &all_ids {
        if state.definite_proven.contains(lit_id) {
            let comp_id = lit_id.complement();
            if !state.definite_proven.contains(comp_id) {
                // Normal case: +D q and -D ~q → +d q
                state.defeasible_proven.insert(lit_id);
                // Reuse the +D conclusion's literal (preserves temporal) and rule label
                let (lit, definite_label) = state
                    .conclusions
                    .iter()
                    .find_map(|c| {
                        if c.conclusion_type == ConclusionType::DefinitelyProvable
                            && indexed.get_lit_id(&c.literal) == Some(lit_id)
                        {
                            Some((c.literal.clone(), c.rule_label.as_deref().map(String::from)))
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| (indexed.resolve_literal(lit_id), None));
                let mut conclusion = Conclusion::defeasibly_provable(lit);
                if let Some(label) = definite_label {
                    conclusion = conclusion.with_rule(&label);
                }
                state.conclusions.push(conclusion);
                worklist.push_back((lit_id, true));
            }
            // If +D q AND +D ~q: don't seed +d (condition 2 fails).
        }
    }

    // --- Seed -d for literals that can never be defeasibly proved ---
    // Lambda over-approximation: literals outside it are unfounded (no
    // non-circular support path exists), so they seed -d and the worklist
    // discards any attacker that depends on them (well-founded reading,
    // mirrors the Lean model's lambda-based attack discard).
    let lambda = compute_lambda(
        theory,
        indexed,
        &state.definite_proven,
        &state.defeasible_body_remaining,
    );
    // Families with at least one member in lambda: an atemporal body literal
    // whose family is alive may yet be satisfied by a temporal member, so
    // -d events on the exact atemporal literal must not discard its rules.
    let mut alive_families: FxHashSet<FamilyId> = FxHashSet::default();
    for &id in indexed.all_literal_ids() {
        if lambda.contains(id) {
            alive_families.insert(FamilyId::from(&indexed.resolve_literal(id)));
        }
    }
    for &lit_id in &all_ids {
        if state.defeasible_proven.contains(lit_id) || state.defeasible_disproven.contains(lit_id) {
            continue;
        }

        if state.definite_proven.contains(lit_id) {
            // +D q but condition (2) failed (+D ~q too) → -d q
            state.defeasible_disproven.insert(lit_id);
            worklist.push_back((lit_id, false));
            continue;
        }

        // -D q and unfounded: q is outside the lambda over-approximation,
        // so no chain of productive rules can ever establish it. This
        // subsumes the "Rsd[q] is empty" check (a literal with no
        // productive rules never enters lambda unless definite).
        if !lambda.contains(lit_id) {
            state.defeasible_disproven.insert(lit_id);
            worklist.push_back((lit_id, false));
        }
    }

    // --- Seed empty-body defeasible/strict rules not yet decided ---
    let mut empty_body_rules: Vec<_> = theory
        .rules()
        .filter(|rule| {
            rule.body.is_empty()
                && matches!(
                    rule.rule_type,
                    RuleType::Defeasible | RuleType::Strict | RuleType::Fact
                )
        })
        .collect();
    empty_body_rules.sort_by(|lhs, rhs| {
        lhs.head_literal()
            .to_spl()
            .cmp(&rhs.head_literal().to_spl())
            .then_with(|| lhs.label.cmp(&rhs.label))
    });
    for rule in empty_body_rules {
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
                indexed,
                theory,
                &state.definite_proven,
                &mut state.defeasible_proven,
                &mut state.defeasible_disproven,
                &state.defeasible_body_remaining,
                &state.rule_discarded,
                &mut state.projection_labels,
                &mut worklist,
                &mut state.conclusions,
            );
        }
    }

    // --- Fixed-point loop ---
    while let Some((q_id, proved)) = worklist.pop_front() {
        // Update rule counters for ALL rules containing q in body
        let mut rules_with_q: Vec<String> = indexed
            .rules_with_body_id(q_id)
            .iter()
            .map(|r| r.label.clone())
            .collect();
        rules_with_q.sort_by(|lhs, rhs| {
            theory
                .get_rule(lhs)
                .expect("rule label from body index must exist")
                .head_literal()
                .to_spl()
                .cmp(
                    &theory
                        .get_rule(rhs)
                        .expect("rule label from body index must exist")
                        .head_literal()
                        .to_spl(),
                )
                .then_with(|| lhs.cmp(rhs))
        });

        for rule_label in &rules_with_q {
            if proved {
                let remaining = state
                    .defeasible_body_remaining
                    .get_mut(rule_label.as_str())
                    .unwrap();
                if *remaining > 0 {
                    *remaining -= 1;
                }
            } else {
                // Family-aware discard (SPEC-020; mirrors the Lean family
                // model's famSat semantics). A -d event for literal L only
                // discards a rule when it removes the LAST way to satisfy a
                // body literal:
                //  - a TEMPORAL body literal requires exactly L, so an exact
                //    match discards;
                //  - an ATEMPORAL body literal is family-satisfiable, so it
                //    is dead only when its whole family is unfounded (no
                //    member in the lambda over-approximation). Otherwise a
                //    yet-unproven family member may still fire the rule, and
                //    as an attacker the rule keeps blocking (the model's
                //    lambda-based attackReaches).
                // Family events (L temporal, body atemporal in L's family)
                // never discard: one member failing does not kill the family.
                let event_lit = indexed.resolve_literal(q_id);
                let rule = theory
                    .get_rule(rule_label)
                    .expect("rule label from body index must exist");
                let should_discard = rule.body.iter().any(|bl| match bl.as_logic() {
                    Some(logic) => {
                        let b = logic.to_literal();
                        if b == event_lit {
                            if b.temporal.is_empty() {
                                !alive_families.contains(&FamilyId::from(&b))
                            } else {
                                true
                            }
                        } else {
                            false
                        }
                    }
                    None => false,
                });
                if should_discard {
                    *state.rule_discarded.get_mut(rule_label.as_str()).unwrap() = true;
                }
            }
        }

        if proved {
            // q just proved +d
            // 1. Try to prove/disprove heads of newly-applicable rules
            for rule_label in &rules_with_q {
                let rule = theory.get_rule(rule_label).unwrap();
                if !matches!(
                    rule.rule_type,
                    RuleType::Strict | RuleType::Defeasible | RuleType::Fact
                ) {
                    continue;
                }
                let remaining = state.defeasible_body_remaining[rule_label.as_str()];
                let discarded = state.rule_discarded[rule_label.as_str()];
                if remaining == 0 && !discarded {
                    let head_id = indexed
                        .get_lit_id(rule.head_literal())
                        .expect("Head literal missing from index");
                    try_prove_defeasible(
                        head_id,
                        indexed,
                        theory,
                        &state.definite_proven,
                        &mut state.defeasible_proven,
                        &mut state.defeasible_disproven,
                        &state.defeasible_body_remaining,
                        &state.rule_discarded,
                        &mut state.projection_labels,
                        &mut worklist,
                        &mut state.conclusions,
                    );
                    // Re-check complement (resolved attacker may unblock)
                    let comp_head = head_id.complement();
                    try_prove_defeasible(
                        comp_head,
                        indexed,
                        theory,
                        &state.definite_proven,
                        &mut state.defeasible_proven,
                        &mut state.defeasible_disproven,
                        &state.defeasible_body_remaining,
                        &state.rule_discarded,
                        &mut state.projection_labels,
                        &mut worklist,
                        &mut state.conclusions,
                    );
                    try_disprove_defeasible(
                        comp_head,
                        indexed,
                        theory,
                        &state.definite_proven,
                        &mut state.defeasible_proven,
                        &mut state.defeasible_disproven,
                        &state.defeasible_body_remaining,
                        &state.rule_discarded,
                        &mut worklist,
                    );
                }
            }

            // Handle defeaters that become newly applicable
            for rule_label in &rules_with_q {
                let rule = theory.get_rule(rule_label).unwrap();
                if rule.rule_type != RuleType::Defeater {
                    continue;
                }
                let remaining = state.defeasible_body_remaining[rule_label.as_str()];
                let discarded = state.rule_discarded[rule_label.as_str()];
                if remaining == 0 && !discarded {
                    let head_id = indexed
                        .get_lit_id(rule.head_literal())
                        .expect("Head literal missing from index");
                    let comp_head = head_id.complement();
                    try_prove_defeasible(
                        comp_head,
                        indexed,
                        theory,
                        &state.definite_proven,
                        &mut state.defeasible_proven,
                        &mut state.defeasible_disproven,
                        &state.defeasible_body_remaining,
                        &state.rule_discarded,
                        &mut state.projection_labels,
                        &mut worklist,
                        &mut state.conclusions,
                    );
                    try_disprove_defeasible(
                        comp_head,
                        indexed,
                        theory,
                        &state.definite_proven,
                        &mut state.defeasible_proven,
                        &mut state.defeasible_disproven,
                        &state.defeasible_body_remaining,
                        &state.rule_discarded,
                        &mut worklist,
                    );
                }
            }

            // q being +d may cause ~q to become -d
            let comp_id = q_id.complement();
            try_disprove_defeasible(
                comp_id,
                indexed,
                theory,
                &state.definite_proven,
                &mut state.defeasible_proven,
                &mut state.defeasible_disproven,
                &state.defeasible_body_remaining,
                &state.rule_discarded,
                &mut worklist,
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

                if matches!(
                    rule.rule_type,
                    RuleType::Strict | RuleType::Defeasible | RuleType::Fact
                ) {
                    try_disprove_defeasible(
                        head_id,
                        indexed,
                        theory,
                        &state.definite_proven,
                        &mut state.defeasible_proven,
                        &mut state.defeasible_disproven,
                        &state.defeasible_body_remaining,
                        &state.rule_discarded,
                        &mut worklist,
                    );
                }

                // Rule is now discarded as attacker → re-check complement
                let comp_head = head_id.complement();
                try_prove_defeasible(
                    comp_head,
                    indexed,
                    theory,
                    &state.definite_proven,
                    &mut state.defeasible_proven,
                    &mut state.defeasible_disproven,
                    &state.defeasible_body_remaining,
                    &state.rule_discarded,
                    &mut state.projection_labels,
                    &mut worklist,
                    &mut state.conclusions,
                );
            }

            // q being -d means attackers using q in body are discarded
            // → try +d for complement(q)
            let comp_id = q_id.complement();
            try_prove_defeasible(
                comp_id,
                indexed,
                theory,
                &state.definite_proven,
                &mut state.defeasible_proven,
                &mut state.defeasible_disproven,
                &state.defeasible_body_remaining,
                &state.rule_discarded,
                &mut state.projection_labels,
                &mut worklist,
                &mut state.conclusions,
            );
        }
    }

    // ====================================================================
    // PHASE 3: Emit remaining conclusions (-D, -d)
    // ====================================================================
    let mut all_ids: Vec<LitId> = indexed.all_literal_ids().cloned().collect();
    all_ids.sort_by_key(|id| indexed.resolve_literal(*id).to_spl());

    for lit_id in all_ids {
        if !state.definite_proven.contains(lit_id) {
            let lit = indexed.resolve_literal(lit_id);
            state
                .conclusions
                .push(Conclusion::new(ConclusionType::DefinitelyNotProvable, lit));
        }

        if !state.defeasible_proven.contains(lit_id) {
            if !state.defeasible_disproven.contains(lit_id) {
                // Safety net: anything still undecided is -d
                state.defeasible_disproven.insert(lit_id);
            }
            let lit = indexed.resolve_literal(lit_id);
            state
                .conclusions
                .push(Conclusion::new(ConclusionType::DefeasiblyNotProvable, lit));
        }
    }
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
    projection_labels: &mut FxHashSet<String>,
    worklist: &mut VecDeque<(LitId, bool)>,
    conclusions: &mut Vec<Conclusion>,
) {
    if defeasible_disproven.contains(q) {
        return; // already decided
    }
    let already_proven = defeasible_proven.contains(q);

    let nq = q.complement();

    // Condition (1): ∃r ∈ Rsd[q] that is applicable
    let supporting_rules = indexed.rules_with_head_id(q);
    let has_applicable = supporting_rules.iter().any(|r| {
        matches!(
            r.rule_type,
            RuleType::Strict | RuleType::Defeasible | RuleType::Fact
        ) && body_remaining
            .get(r.label.as_str())
            .is_some_and(|&rem| rem == 0)
            && !rule_discarded
                .get(r.label.as_str())
                .copied()
                .unwrap_or(false)
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
            matches!(
                r.rule_type,
                RuleType::Strict | RuleType::Defeasible | RuleType::Fact
            ) && body_remaining
                .get(r.label.as_str())
                .is_some_and(|&rem| rem == 0)
                && !rule_discarded
                    .get(r.label.as_str())
                    .copied()
                    .unwrap_or(false)
        })
        .copied()
        .collect();

    // Keep scanning applicable attackers even after the proof is known to fail.
    // Projection labels drive the projection engine's explanation surface, so
    // blocked literals should retain every grounded attacker/supporter that
    // actually participated instead of whichever blocker happened to appear
    // first in iteration order.
    let mut blocked_by_applicable_attacker = false;

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

        // Check if any applicable supporter is superior to this attacker.
        let defeated_by_superior = applicable_supporters
            .iter()
            .any(|t| theory.is_superior(t.template_label(), attacker.template_label()));

        if att_remaining > 0 {
            // Attacker with undecided body: if a superior applicable rule
            // defeats it, the attacker is countered. Spec condition (3)'s
            // beaten disjunct (∃t ∈ Rsd[q]: t applicable AND t > s) applies
            // to strict attackers too — a strict rule that is DEFINITELY
            // applicable already blocks via condition (2) (+D ~q), so
            // superiority here only ever overrides a merely defeasibly
            // applicable strict attacker.
            if defeated_by_superior {
                continue;
            }
            projection_labels.extend(applicable_supporters.iter().map(|rule| rule.label.clone()));
            return;
        }

        // Attacker is applicable. Need ∃t ∈ Rsd[q]: t applicable AND t > s
        if !defeated_by_superior {
            projection_labels.insert(attacker.label.clone());
            blocked_by_applicable_attacker = true;
        }
    }

    if blocked_by_applicable_attacker {
        projection_labels.extend(applicable_supporters.iter().map(|rule| rule.label.clone()));
        return;
    }

    // All conditions met.
    //
    // Emit one +d conclusion per distinct grounded supporter head literal
    // (e.g., distinct temporal windows). Use deterministic ordering so output
    // does not depend on supporter discovery order.
    //
    // If q is already +d, keep emitting any newly-applicable temporal windows.
    let mut existing_positive_q: FxHashSet<String> = conclusions
        .iter()
        .filter(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && indexed.get_lit_id(&c.literal) == Some(q)
        })
        .map(|c| c.literal.to_spl())
        .collect();

    if applicable_supporters.is_empty() {
        let lit = indexed.resolve_literal(q);
        if existing_positive_q.insert(lit.to_spl()) {
            conclusions.push(Conclusion::defeasibly_provable(lit));
        }
    } else {
        // For duplicate supporters with the same grounded head literal, keep the
        // lexicographically-smallest label for deterministic attribution.
        let mut supporters_by_literal: BTreeMap<String, &crate::rule::Rule> = BTreeMap::new();
        for supporter in applicable_supporters {
            let literal_key = supporter.head_literal().to_spl();
            supporters_by_literal
                .entry(literal_key)
                .and_modify(|selected| {
                    if supporter.label.as_str() < selected.label.as_str() {
                        *selected = supporter;
                    }
                })
                .or_insert(supporter);
        }

        for supporter in supporters_by_literal.values() {
            let lit = supporter.head_literal().clone();
            if existing_positive_q.insert(lit.to_spl()) {
                conclusions.push(Conclusion::defeasibly_provable(lit).with_rule(&supporter.label));
            }
        }
    }

    if !already_proven {
        defeasible_proven.insert(q);
        worklist.push_back((q, true));
    }
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
        .filter(|r| {
            matches!(
                r.rule_type,
                RuleType::Strict | RuleType::Defeasible | RuleType::Fact
            )
        })
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

        // Strict attackers always block
        if attacker.rule_type == RuleType::Strict {
            defeasible_disproven.insert(q);
            worklist.push_back((q, false));
            return;
        }

        // Attacker s is applicable. Check: ∀t ∈ Rsd[q]: t discarded OR ¬(t > s)
        // But if any t is undecided (not discarded, not applicable), can't conclude
        let any_t_undecided = sd_rules.iter().any(|t| {
            let t_discarded = rule_discarded
                .get(t.label.as_str())
                .copied()
                .unwrap_or(false);
            let t_remaining = body_remaining.get(t.label.as_str()).copied().unwrap_or(0);
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
    use crate::conclusion::ConclusionType;
    use crate::index::IndexedTheory;
    use crate::reason::definite;
    use crate::reason::facts;
    use crate::reason::state::ReasoningState;

    /// Helper: build state + indexed theory, run all three phases.
    fn run_all_phases(theory: &Theory) -> (ReasoningState<'_>, IndexedTheory<'_>) {
        let mut indexed = IndexedTheory::build(theory);
        let atom_count = indexed.atom_count();
        let rule_count = theory.rule_count();
        let estimated = rule_count * 2 + indexed.all_literal_ids().count() * 2;
        let mut state = ReasoningState::new(atom_count, rule_count, estimated);

        // Phase 1
        facts::initialize_facts(theory, &mut indexed, &mut state);
        // Phase 1 continued
        definite::forward_chain_strict(theory, &indexed, &mut state);
        // Phase 2 + Phase 3
        resolve_defeasible(theory, &indexed, &mut state);

        (state, indexed)
    }

    // ======================================================================
    // NEGATIVE CONCLUSION GENERATION (-D, -d)
    // ======================================================================

    #[test]
    fn test_unproven_literal_gets_negative_definite() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let (state, _) = run_all_phases(&theory);

        let has_neg_definite_q = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyNotProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(
            has_neg_definite_q,
            "q should get -D since it has no strict derivation"
        );
    }

    #[test]
    fn test_unproven_literal_gets_negative_defeasible() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q"); // p not proven

        let (state, _) = run_all_phases(&theory);

        let has_neg_defeasible_q = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyNotProvable && c.literal.name() == "q"
        });

        assert!(
            has_neg_defeasible_q,
            "q should get -d since its body is unsatisfied"
        );
    }

    #[test]
    fn test_proven_literal_no_negative_definite() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");

        let (state, _) = run_all_phases(&theory);

        let has_neg_definite_q = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyNotProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(
            !has_neg_definite_q,
            "q should NOT get -D since it is strictly proven"
        );
    }

    #[test]
    fn test_defeasibly_proven_no_negative_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let (state, _) = run_all_phases(&theory);

        let has_neg_defeasible_q = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(
            !has_neg_defeasible_q,
            "q should NOT get -d since it is defeasibly proven"
        );
    }

    #[test]
    fn test_empty_theory_no_negative_conclusions() {
        let theory = Theory::new();

        let (state, _) = run_all_phases(&theory);

        assert!(
            state.conclusions.is_empty(),
            "Empty theory should produce no conclusions at all"
        );
    }

    #[test]
    fn test_fact_no_negative_for_fact() {
        let mut theory = Theory::new();
        theory.add_fact("p");

        let (state, _) = run_all_phases(&theory);

        let has_neg_definite_p = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyNotProvable
                && c.literal.name() == "p"
                && !c.literal.negation
        });
        let has_neg_defeasible_p = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyNotProvable
                && c.literal.name() == "p"
                && !c.literal.negation
        });

        assert!(!has_neg_definite_p, "Fact p should NOT get -D");
        assert!(!has_neg_defeasible_p, "Fact p should NOT get -d");
    }

    // ======================================================================
    // AMBIGUITY BLOCKING
    // ======================================================================

    #[test]
    fn test_ambiguity_blocking_no_superiority() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["p"], "~q");

        let (state, _) = run_all_phases(&theory);

        let has_q = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });
        let has_not_q = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && c.literal.negation
        });

        assert!(!has_q, "q should NOT be +d (ambiguity blocking)");
        assert!(!has_not_q, "~q should NOT be +d (ambiguity blocking)");
    }

    #[test]
    fn test_superiority_resolves_ambiguity() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");
        theory.add_superiority(&r1, &r2);

        let (state, _) = run_all_phases(&theory);

        let has_q = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        assert!(has_q, "q should be +d (r1 > r2 resolves ambiguity)");
    }

    #[test]
    fn test_temporal_supporters_emit_all_distinct_positive_windows() {
        use crate::literal::Literal;
        use crate::rule::Rule;
        use crate::temporal::Temporal;

        let mut theory = Theory::new();
        theory.add_rule(Rule::new(
            "r_late",
            RuleType::Defeasible,
            vec![],
            vec![Literal::new(
                "q",
                false,
                Default::default(),
                Temporal::from_bounds(20, 30),
                vec![],
            )],
        ));
        theory.add_rule(Rule::new(
            "r_early",
            RuleType::Defeasible,
            vec![],
            vec![Literal::new(
                "q",
                false,
                Default::default(),
                Temporal::from_bounds(0, 10),
                vec![],
            )],
        ));

        let (state, _) = run_all_phases(&theory);

        let q_windows: Vec<_> = state
            .conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            })
            .map(|c| c.literal.temporal.clone())
            .collect();

        assert_eq!(
            q_windows,
            vec![Temporal::from_bounds(0, 10), Temporal::from_bounds(20, 30)],
            "all distinct grounded supporter windows should be emitted deterministically"
        );
    }

    #[test]
    fn test_temporal_supporters_emit_late_windows_after_initial_proof() {
        use crate::literal::Literal;
        use crate::rule::Rule;
        use crate::temporal::Temporal;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_rule(Rule::new(
            "r_q_early",
            RuleType::Defeasible,
            vec![Literal::simple("p")],
            vec![Literal::new(
                "q",
                false,
                Default::default(),
                Temporal::from_bounds(0, 10),
                vec![],
            )],
        ));
        theory.add_defeasible_rule(&["p"], "s");
        theory.add_rule(Rule::new(
            "r_q_late",
            RuleType::Defeasible,
            vec![Literal::simple("s")],
            vec![Literal::new(
                "q",
                false,
                Default::default(),
                Temporal::from_bounds(20, 30),
                vec![],
            )],
        ));

        let (state, _) = run_all_phases(&theory);

        let q_windows: Vec<_> = state
            .conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "q"
                    && !c.literal.negation
            })
            .map(|c| c.literal.temporal.clone())
            .collect();

        assert_eq!(
            q_windows,
            vec![Temporal::from_bounds(0, 10), Temporal::from_bounds(20, 30)],
            "late supporters should still contribute additional +d temporal windows"
        );
    }
}
