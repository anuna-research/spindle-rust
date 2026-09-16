//! Phase 2: Defeasible provability (+d / -d) with SDL ambiguity blocking.
//!
//! After Phase 1 has completed all definite (+D) derivations, this module
//! runs a fixed-point loop that propagates both positive (+d) and negative
//! (-d) defeasible conclusions.
//!
//! Phase 3 reports only constructively derived negative tags.
//!
//! # Algorithm
//!
//! 1. Derive definite negative proofs from discarded strict rules.
//! 2. Seed +d unconditionally from +D.
//! 3. Compute constructive +d/-d until no further proof can be added.
//! 4. Emit derived tags; undecided cycles receive no synthetic negative tag.

use std::collections::{BTreeMap, VecDeque};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::conclusion::{Conclusion, ConclusionType};
use crate::index::{IndexedTheory, LitId};
use crate::projection::FamilyId;
use crate::rule::RuleType;
use crate::theory::Theory;

use super::state::{LiteralBitSet, ReasoningState};

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

    // Definite failure is a proof, not the complement of definite success.
    super::definite::derive_negative(indexed, state);
    let mut all_ids: Vec<LitId> = indexed.all_literal_ids().cloned().collect();
    all_ids.sort_by_key(|id| indexed.resolve_literal(*id).to_spl());
    for &lit_id in &all_ids {
        if state.definite_proven.contains(lit_id) {
            // Traditional DL(partial): +D q implies +d q unconditionally.
            state.defeasible_proven.insert(lit_id);
            let (lit, label) = state
                .conclusions
                .iter()
                .find_map(|c| {
                    (c.conclusion_type == ConclusionType::DefinitelyProvable
                        && indexed.get_lit_id(&c.literal) == Some(lit_id))
                    .then(|| (c.literal.clone(), c.rule_label.clone()))
                })
                .unwrap_or_else(|| (indexed.resolve_literal(lit_id), None));
            let mut conclusion = Conclusion::defeasibly_provable(lit);
            conclusion.rule_label = label;
            state.conclusions.push(conclusion);
            worklist.push_back((lit_id, true));
        }
    }
    // All family members remain possible until constructively disproved.
    // Cyclic support is not a negative proof.
    let mut family_live: FxHashMap<FamilyId, usize> = FxHashMap::default();
    for &id in &all_ids {
        *family_live
            .entry(FamilyId::from(&indexed.resolve_literal(id)))
            .or_insert(0) += 1;
    }
    let family_members: FxHashSet<LitId> = all_ids.iter().copied().collect();
    let mut counted_dead: FxHashSet<LitId> = FxHashSet::default();

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
                &state.definite_disproven,
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

    // --- Fixed-point loop (with battle-resolution sweeps) ---
    //
    // The inner loop is event-driven. Battles among rules that never
    // receive a triggering event (e.g. competing empty-body rules in a
    // fact-free theory) would otherwise stay undecided, leaving
    // spec-derivable -d conclusions (disjunct (3): an applicable,
    // unbeaten attacker) underived — and the discards that depend on
    // them unapplied, blocking valid downstream conclusions. After the
    // event queue drains, sweep every undecided literal with the +d/-d
    // inference conditions; any new decision queues events, and the
    // event loop re-enters. Terminates: each productive sweep decides at
    // least one literal.
    // SIMPLIFY: sweep restarts make Phase 2 worst-case quadratic in the
    // literal count — replace with dependency-tracked scheduling if
    // profiling ever shows this on theories within NFR bounds
    // (trace: SPEC-019).
    loop {
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

            let event_lit = indexed.resolve_literal(q_id);
            let event_family = FamilyId::from(&event_lit);
            // A constructive -d event shrinks its
            // family's live support, once per member (idempotent). Done once
            // per event rather than per rule so a defeated member with no body
            // occurrence still decrements its family's live count.
            if !proved
                && family_members.contains(&q_id)
                && counted_dead.insert(q_id)
                && let Some(c) = family_live.get_mut(&event_family)
            {
                *c = c.saturating_sub(1);
            }

            for rule_label in &rules_with_q {
                let rule = theory
                    .get_rule(rule_label)
                    .expect("rule label from body index must exist");
                if proved {
                    // Mark the body slots this +d literal satisfies. Per-slot
                    // tracking makes the decrement idempotent, so several
                    // temporal members of one family satisfying a single
                    // atemporal body slot count once — a rule becomes
                    // applicable only when every distinct slot has a satisfier
                    // (prevents unsound +d — see SPEC-020 regression).
                    let satisfied = state
                        .defeasible_slots_satisfied
                        .get_mut(rule_label.as_str())
                        .expect("rule slot bitset must exist");
                    let remaining = state
                        .defeasible_body_remaining
                        .get_mut(rule_label.as_str())
                        .expect("rule body counter must exist");
                    super::cover_body_slots(rule, &event_lit, satisfied, remaining);
                } else {
                    // Family-aware discard (SPEC-020; famSat semantics). A -d
                    // event for literal L only discards a rule when it removes
                    // the LAST way to satisfy a body literal:
                    //  - a TEMPORAL body literal requires exactly L (same
                    //    family AND same window), so an exact match discards;
                    //  - an ATEMPORAL body literal is family-satisfiable, so it
                    //    is dead only when its family has no LIVE member left —
                    //    members stop counting only when constructively defeated. A
                    //    family with a live member may still fire the rule
                    //    later, and as an attacker it keeps blocking meanwhile.
                    let should_discard = rule.body.iter().any(|bl| match bl.as_logic() {
                        Some(logic) => {
                            let b = logic.to_literal();
                            if b.is_temporal() {
                                // Temporal body: requires exactly this literal.
                                // FamilyId excludes the window (as does Literal
                                // PartialEq), so compare the window explicitly
                                // instead of `b == event_lit`.
                                FamilyId::from(&b) == event_family
                                    && b.temporal == event_lit.temporal
                            } else {
                                // Atemporal body: dead only when its family has
                                // no live member. (This event reaches the rule
                                // via the exact or family index, so b is in the
                                // event's family.)
                                FamilyId::from(&b) == event_family
                                    && family_live.get(&event_family).copied().unwrap_or(0) == 0
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
                            &state.definite_disproven,
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
                            &state.definite_disproven,
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
                            &state.definite_disproven,
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
                            &state.definite_disproven,
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
                            &state.definite_disproven,
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
                    &state.definite_disproven,
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
                            &state.definite_disproven,
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
                        &state.definite_disproven,
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
                    &state.definite_disproven,
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

        // Battle-resolution sweep: try to decide every remaining literal.
        for &lit_id in &all_ids {
            try_prove_defeasible(
                lit_id,
                indexed,
                theory,
                &state.definite_proven,
                &state.definite_disproven,
                &mut state.defeasible_proven,
                &mut state.defeasible_disproven,
                &state.defeasible_body_remaining,
                &state.rule_discarded,
                &mut state.projection_labels,
                &mut worklist,
                &mut state.conclusions,
            );
            try_disprove_defeasible(
                lit_id,
                indexed,
                theory,
                &state.definite_proven,
                &state.definite_disproven,
                &mut state.defeasible_proven,
                &mut state.defeasible_disproven,
                &state.defeasible_body_remaining,
                &state.rule_discarded,
                &mut worklist,
            );
        }
        if worklist.is_empty() {
            break;
        }
    }

    // ====================================================================
    // PHASE 3: Emit remaining conclusions (-D, -d)
    // ====================================================================
    let mut all_ids: Vec<LitId> = indexed.all_literal_ids().cloned().collect();
    all_ids.sort_by_key(|id| indexed.resolve_literal(*id).to_spl());

    for lit_id in all_ids {
        if state.definite_disproven.contains(lit_id) {
            let lit = indexed.resolve_literal(lit_id);
            state
                .conclusions
                .push(Conclusion::new(ConclusionType::DefinitelyNotProvable, lit));
        }

        if state.defeasible_disproven.contains(lit_id) {
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
    definite_disproven: &LiteralBitSet,
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
    if !definite_proven.contains(q) && !definite_disproven.contains(nq) {
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
    definite_disproven: &LiteralBitSet,
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
    if !definite_disproven.contains(q) {
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

        // Note: strict attackers are NOT special-cased here. A DEFINITELY
        // applicable strict attacker already blocks via +D ~q (disjunct (2)
        // above); a merely defeasibly-applicable strict attacker is subject to
        // the same superiority check as any other attacker, mirroring
        // `try_prove_defeasible` and the Lean `canDisprove2` model (see
        // lean/DIVERGENCES.md). Special-casing it here made +d/-d
        // order-dependent.

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
