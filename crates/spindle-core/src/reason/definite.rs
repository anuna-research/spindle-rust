//! Phase 1 continued: Strict forward chaining for the DL(d) algorithm.
//!
//! Drains the worklist populated by fact initialization, firing **strict rules
//! only** whose body literals are fully satisfied. Strict rules produce +D
//! conclusions. Defeasible rules and defeaters are skipped entirely — they
//! are handled in Phase 2 ([`super::defeasible`]).
//!
//! This isolation ensures that multi-hop strict chains (e.g., `a → c → ¬p`)
//! complete fully before any defeasible reasoning checks `definite_proven`,
//! preventing order-dependent bugs where a defeasible rule fires before
//! the strict chain that should block it has completed.

use crate::conclusion::Conclusion;
use crate::index::IndexedTheory;
use crate::rule::RuleType;
use crate::theory::Theory;

use super::state::ReasoningState;

/// Drain the Phase 1 worklist, decrement `definite_body_remaining` counters,
/// and fire strict rules when their bodies are fully satisfied.
///
/// Only processes `RuleType::Strict` rules. All other rule types are skipped.
pub(crate) fn forward_chain_strict(
    _theory: &Theory,
    indexed: &IndexedTheory<'_>,
    state: &mut ReasoningState<'_>,
) {
    while let Some(lit) = state.drain_worklist() {
        for rule in indexed.rules_with_body(&lit) {
            if rule.rule_type != RuleType::Strict {
                continue;
            }
            // Mark the body slots this literal satisfies. Family matching can
            // route several temporal members of one family to the same
            // atemporal body slot; per-slot tracking ensures the slot is
            // counted once, so a rule fires only when every distinct slot is
            // satisfied (prevents unsound +D — see SPEC-020 regression).
            let now_applicable = {
                let satisfied = state
                    .definite_slots_satisfied
                    .get_mut(rule.label.as_str())
                    .expect("rule slot bitset must exist");
                let remaining = state
                    .definite_body_remaining
                    .get_mut(rule.label.as_str())
                    .expect("rule body counter must exist");
                let was_unsatisfied = *remaining > 0;
                super::cover_body_slots(rule, &lit, satisfied, remaining);
                was_unsatisfied && *remaining == 0
            };
            if now_applicable {
                let head_lit = rule.head_literal().clone();
                let head_id = indexed
                    .get_lit_id(&head_lit)
                    .expect("Head literal missing from index");

                if !state.is_definitely_proven(head_id) {
                    state.definite_proven.insert(head_id);
                    state.add_conclusion(
                        Conclusion::definitely_provable(head_lit.clone()).with_rule(&rule.label),
                    );
                    state.try_enqueue(head_id, head_lit);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::index::IndexedTheory;
    use crate::reason::facts;
    use crate::reason::state::ReasoningState;

    /// Helper: build state + indexed theory, run Phase 1 init + strict forward chain.
    fn run_phases(theory: &Theory) -> (ReasoningState<'_>, IndexedTheory<'_>) {
        let mut indexed = IndexedTheory::build(theory);
        let atom_count = indexed.atom_count();
        let rule_count = theory.rule_count();
        let estimated = rule_count * 2 + indexed.all_literal_ids().count() * 2;
        let mut state = ReasoningState::new(atom_count, rule_count, estimated);

        // Phase 1 init
        facts::initialize_facts(theory, &mut indexed, &mut state);
        // Phase 1 forward chain
        forward_chain_strict(theory, &indexed, &mut state);

        (state, indexed)
    }

    // ======================================================================
    // STRICT RULE CHAINING
    // ======================================================================

    #[test]
    fn test_single_strict_rule_fires() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");

        let (state, _) = run_phases(&theory);

        let has_definite_b = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.name() == "b"
                && !c.literal.negation
        });

        assert!(has_definite_b, "Strict rule a -> b should produce +D b");
    }

    #[test]
    fn test_strict_chain_three_deep() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");
        theory.add_strict_rule(&["b"], "c");
        theory.add_strict_rule(&["c"], "d");

        let (state, _) = run_phases(&theory);

        for name in &["b", "c", "d"] {
            let has_definite = state.conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == *name
            });
            assert!(
                has_definite,
                "{name} should be definitely provable via strict chain"
            );
        }
    }

    #[test]
    fn test_strict_rule_multi_body() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_fact("q");
        theory.add_strict_rule(&["p", "q"], "r");

        let (state, _) = run_phases(&theory);

        let has_definite_r = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "r"
        });
        assert!(
            has_definite_r,
            "Strict rule with body [p, q] should fire when both are facts"
        );
    }

    #[test]
    fn test_strict_rule_unsatisfied_body_does_not_fire() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p", "q"], "r");

        let (state, _) = run_phases(&theory);

        let has_r = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "r"
        });
        assert!(
            !has_r,
            "Strict rule should not fire when body literal q is missing"
        );
    }

    #[test]
    fn test_strict_rule_no_duplicate_conclusions() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_strict_rule(&["a"], "c");
        theory.add_strict_rule(&["b"], "c");

        let (state, _) = run_phases(&theory);

        let definite_c_count = state
            .conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "c"
            })
            .count();

        assert_eq!(
            definite_c_count, 1,
            "Two strict rules proving the same head should produce exactly one +D conclusion"
        );
    }

    // ======================================================================
    // DEFEASIBLE RULES NOT PROCESSED IN PHASE 1
    // ======================================================================

    #[test]
    fn test_defeasible_rule_not_fired_in_strict_phase() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let (state, _) = run_phases(&theory);

        let has_defeasible_q = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });
        assert!(
            !has_defeasible_q,
            "Defeasible rules should NOT fire in Phase 1 (strict-only)"
        );
    }

    #[test]
    fn test_defeater_does_not_fire_in_strict_phase() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeater(&["p"], "q");

        let (state, _) = run_phases(&theory);

        let has_q = state.conclusions.iter().any(|c| {
            (c.conclusion_type == ConclusionType::DefinitelyProvable
                || c.conclusion_type == ConclusionType::DefeasiblyProvable)
                && c.literal.name() == "q"
        });
        assert!(!has_q, "Defeater should not prove q in Phase 1");
    }

    // ======================================================================
    // MIXED RULE TYPES
    // ======================================================================

    #[test]
    fn test_strict_feeds_worklist_but_defeasible_not_fired() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let (state, _) = run_phases(&theory);

        let has_definite_b = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "b"
        });
        let has_defeasible_c = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "c"
        });

        assert!(has_definite_b, "Strict rule should definitely prove b");
        assert!(
            !has_defeasible_c,
            "Defeasible c should NOT be proved in Phase 1"
        );
    }

    #[test]
    fn test_no_rules_no_forward_chaining() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");

        let (state, _) = run_phases(&theory);

        let all_positive: Vec<_> = state
            .conclusions
            .iter()
            .filter(|c| c.conclusion_type.is_positive())
            .collect();

        // Should be 2: +D a, +D b (Phase 1 only emits +D, not +d)
        assert_eq!(
            all_positive.len(),
            2,
            "Only +D fact conclusions from Phase 1"
        );
    }
}
