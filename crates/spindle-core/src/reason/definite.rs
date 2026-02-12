//! Phase 2: Strict forward chaining for the DL(d) algorithm.
//!
//! Drains the worklist populated by Phase 1 (fact initialization), firing
//! rules whose body literals are fully satisfied. Strict rules produce +D
//! and +d conclusions; defeasible rules produce +d conclusions (subject to
//! conflict/superiority checks); defeaters block but never prove.

use crate::conclusion::Conclusion;
use crate::index::IndexedTheory;
use crate::rule::RuleType;
use crate::theory::Theory;

use super::defeasible::is_blocked_by_superior;
use super::state::ReasoningState;

/// Drain the worklist, decrement body_remaining counters for rules containing
/// each proven literal, fire rules when all body literals are satisfied
/// (body_remaining reaches 0), and emit the appropriate conclusions.
///
/// - **Strict rules**: mark head as definitely proven, emit +D and +d.
/// - **Defeasible rules**: mark head as defeasibly proven (unless blocked by
///   a superior attacker or defeater), emit +d.
/// - **Defeaters**: do not prove anything; blocking is handled by
///   [`is_blocked_by_superior`].
pub(crate) fn forward_chain_strict(
    theory: &Theory,
    indexed: &IndexedTheory<'_>,
    state: &mut ReasoningState<'_>,
) {
    while let Some(lit) = state.drain_worklist() {
        // Find rules where this literal appears in body
        // using immutable lookup
        for rule in indexed.rules_with_body(&lit) {
            let remaining = state.body_remaining.get_mut(rule.label.as_str()).unwrap();
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
                            if !state.is_definitely_proven(head_id) {
                                state.mark_definitely_proven(head_id);

                                state.add_conclusion(
                                    Conclusion::definitely_provable(head_lit.clone())
                                        .with_rule(&rule.label),
                                );
                                state.add_conclusion(
                                    Conclusion::defeasibly_provable(head_lit.clone())
                                        .with_rule(&rule.label),
                                );

                                state.try_enqueue(head_id, head_lit);
                            }
                        }

                        RuleType::Defeasible => {
                            // Check for conflicts and superiority
                            let comp_id = head_id.complement();

                            // Only prove if complement isn't definitely proven
                            if !state.is_definitely_proven(comp_id)
                                && !state.is_defeasibly_proven(head_id)
                            {
                                // Check if we're blocked by superior rules
                                let blocked = is_blocked_by_superior(
                                    indexed,
                                    theory,
                                    rule,
                                    &state.defeasible_proven,
                                );

                                if !blocked {
                                    state.defeasible_proven.insert(head_id);
                                    state.add_conclusion(
                                        Conclusion::defeasibly_provable(head_lit.clone())
                                            .with_rule(&rule.label),
                                    );
                                    state.try_enqueue(head_id, head_lit);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::index::IndexedTheory;
    use crate::reason::facts;
    use crate::reason::state::ReasoningState;

    /// Helper: build state + indexed theory, run Phase 1, then Phase 2.
    fn run_phases(theory: &Theory) -> (ReasoningState<'_>, IndexedTheory<'_>) {
        let mut indexed = IndexedTheory::build(theory);
        let atom_count = indexed.atom_count();
        let rule_count = theory.rule_count();
        let estimated = rule_count * 2 + indexed.all_literal_ids().count() * 2;
        let mut state = ReasoningState::new(atom_count, rule_count, estimated);

        // Phase 1
        facts::initialize_facts(theory, &mut indexed, &mut state);
        // Phase 2
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
        let has_defeasible_b = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "b"
                && !c.literal.negation
        });

        assert!(has_definite_b, "Strict rule a -> b should produce +D b");
        assert!(has_defeasible_b, "Strict rule a -> b should produce +d b");
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
        // Two strict rules proving the same head should not duplicate conclusions
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
    // DEFEASIBLE RULES IN FORWARD CHAIN
    // ======================================================================

    #[test]
    fn test_defeasible_rule_fires_in_phase2() {
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
            has_defeasible_q,
            "Defeasible rule p => q should produce +d q"
        );
    }

    #[test]
    fn test_defeasible_chain() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "d");

        let (state, _) = run_phases(&theory);

        for name in &["b", "c", "d"] {
            let has_defeasible = state.conclusions.iter().any(|c| {
                c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == *name
            });
            assert!(
                has_defeasible,
                "{name} should be defeasibly provable via chain"
            );
        }
    }

    // ======================================================================
    // DEFEATER HANDLING
    // ======================================================================

    #[test]
    fn test_defeater_does_not_prove() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeater(&["p"], "q");

        let (state, _) = run_phases(&theory);

        let has_q = state.conclusions.iter().any(|c| {
            (c.conclusion_type == ConclusionType::DefinitelyProvable
                || c.conclusion_type == ConclusionType::DefeasiblyProvable)
                && c.literal.name() == "q"
        });
        assert!(!has_q, "Defeater should not prove q");
    }

    // ======================================================================
    // MIXED RULE TYPES
    // ======================================================================

    #[test]
    fn test_strict_feeds_defeasible_chain() {
        // strict a -> b, then defeasible b => c
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
            has_defeasible_c,
            "Defeasible rule should defeasibly prove c from strict b"
        );
    }

    #[test]
    fn test_no_rules_no_forward_chaining() {
        // Only facts, no rules to fire
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");

        let (state, _) = run_phases(&theory);

        // Only fact conclusions (from Phase 1), no additional from Phase 2
        let all_positive: Vec<_> = state
            .conclusions
            .iter()
            .filter(|c| c.conclusion_type.is_positive())
            .collect();

        // Should be 4: +D a, +d a, +D b, +d b (all from Phase 1)
        assert_eq!(
            all_positive.len(),
            4,
            "Only fact conclusions, no forward-chained ones"
        );
    }
}
