//! Phase 3: Negative conclusion generation for the DL(d) algorithm.
//!
//! After forward chaining has completed (Phases 1 and 2), this module emits
//! negative conclusions for all unproven literals:
//!
//! - **`-D`** (`DefinitelyNotProvable`) for every literal not in
//!   `definite_proven`.
//! - **`-d`** (`DefeasiblyNotProvable`) for every literal not in
//!   `defeasible_proven`.
//!
//! Also contains the [`is_blocked_by_superior`] helper used by Phases 1, 2,
//! and 3 to check whether a defeasible rule is blocked by a superior attacker
//! or defeater.

use crate::conclusion::{Conclusion, ConclusionType};
use crate::index::{IndexedTheory, LitId};
use crate::rule::RuleType;
use crate::theory::Theory;

use super::state::{LiteralBitSet, ReasoningState};

/// Emit negative conclusions for all unproven literals.
///
/// Iterates over every literal known to the indexed theory and emits:
///
/// - `-D` if the literal is not in `state.definite_proven`
/// - `-d` if the literal is not in `state.defeasible_proven`
pub(crate) fn resolve_defeasible(
    _theory: &Theory,
    indexed: &IndexedTheory<'_>,
    state: &mut ReasoningState<'_>,
) {
    let all_ids: Vec<LitId> = indexed.all_literal_ids().cloned().collect();

    for lit_id in all_ids {
        if !state.is_definitely_proven(lit_id) {
            let lit = indexed.resolve_literal(lit_id);
            state.add_conclusion(Conclusion::new(ConclusionType::DefinitelyNotProvable, lit));
        }

        if !state.is_defeasibly_proven(lit_id) {
            let lit = indexed.resolve_literal(lit_id);
            state.add_conclusion(Conclusion::new(ConclusionType::DefeasiblyNotProvable, lit));
        }
    }
}

/// Check if a rule is blocked by a superior rule or defeater for the complement
pub(crate) fn is_blocked_by_superior(
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

        // Strict attackers always block opposing defeasible conclusions.
        if attacker.rule_type == RuleType::Strict {
            return true;
        }

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

        // No-superiority ties are intentionally not blocked here.
        // In standard mode we allow equal-strength conflicting defeasible rules
        // to remain defeasibly derivable unless explicit superiority/defeaters apply.
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::index::IndexedTheory;
    use crate::reason::definite;
    use crate::reason::facts;
    use crate::reason::state::ReasoningState;

    /// Helper: build state + indexed theory, run Phase 1, Phase 2, then Phase 3.
    fn run_all_phases(theory: &Theory) -> (ReasoningState<'_>, IndexedTheory<'_>) {
        let mut indexed = IndexedTheory::build(theory);
        let atom_count = indexed.atom_count();
        let rule_count = theory.rule_count();
        let estimated = rule_count * 2 + indexed.all_literal_ids().count() * 2;
        let mut state = ReasoningState::new(atom_count, rule_count, estimated);

        // Phase 1
        facts::initialize_facts(theory, &mut indexed, &mut state);
        // Phase 2
        definite::forward_chain_strict(theory, &indexed, &mut state);
        // Phase 3
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

        // q has no strict path, so -D q
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

        // q is strictly proven, so no -D q
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

        // q is defeasibly proven, so no -d q
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
    // is_blocked_by_superior TESTS
    // ======================================================================

    #[test]
    fn test_not_blocked_when_no_attackers() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let (state, indexed) = run_all_phases(&theory);

        let rule = theory
            .rules()
            .find(|r| r.head_literal().name() == "q")
            .unwrap();

        let blocked = is_blocked_by_superior(&indexed, &theory, rule, &state.defeasible_proven);
        assert!(
            !blocked,
            "Rule should not be blocked when no attackers exist"
        );
    }

    #[test]
    fn test_blocked_by_strict_attacker() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_strict_rule(&["p"], "~q");

        let (state, indexed) = run_all_phases(&theory);

        let rule = theory
            .rules()
            .find(|r| {
                r.head_literal().name() == "q"
                    && !r.head_literal().negation
                    && r.rule_type == RuleType::Defeasible
            })
            .unwrap();

        let blocked = is_blocked_by_superior(&indexed, &theory, rule, &state.defeasible_proven);
        assert!(
            blocked,
            "Defeasible rule should be blocked by strict attacker"
        );
    }

    #[test]
    fn test_blocked_by_defeater() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeater(&["p"], "~q");

        let (state, indexed) = run_all_phases(&theory);

        let rule = theory
            .rules()
            .find(|r| {
                r.head_literal().name() == "q"
                    && !r.head_literal().negation
                    && r.rule_type == RuleType::Defeasible
            })
            .unwrap();

        let blocked = is_blocked_by_superior(&indexed, &theory, rule, &state.defeasible_proven);
        assert!(blocked, "Defeasible rule should be blocked by defeater");
    }

    #[test]
    fn test_not_blocked_when_attacker_body_unsatisfied() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");
        theory.add_defeasible_rule(&["unproven"], "~q");

        let (state, indexed) = run_all_phases(&theory);

        let rule = theory
            .rules()
            .find(|r| {
                r.head_literal().name() == "q"
                    && !r.head_literal().negation
                    && r.rule_type == RuleType::Defeasible
            })
            .unwrap();

        let blocked = is_blocked_by_superior(&indexed, &theory, rule, &state.defeasible_proven);
        assert!(
            !blocked,
            "Rule should not be blocked when attacker body is unsatisfied"
        );
    }

    #[test]
    fn test_blocked_by_superior_attacker() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");
        theory.add_superiority(&r2, &r1); // r2 > r1

        let (state, indexed) = run_all_phases(&theory);

        let rule = theory
            .rules()
            .find(|r| {
                r.head_literal().name() == "q"
                    && !r.head_literal().negation
                    && r.rule_type == RuleType::Defeasible
            })
            .unwrap();

        let blocked = is_blocked_by_superior(&indexed, &theory, rule, &state.defeasible_proven);
        assert!(blocked, "Rule should be blocked by superior attacker");
    }

    #[test]
    fn test_not_blocked_when_rule_is_superior() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        let r1 = theory.add_defeasible_rule(&["p"], "q");
        let r2 = theory.add_defeasible_rule(&["p"], "~q");
        theory.add_superiority(&r1, &r2); // r1 > r2

        let (state, indexed) = run_all_phases(&theory);

        let rule = theory
            .rules()
            .find(|r| {
                r.head_literal().name() == "q"
                    && !r.head_literal().negation
                    && r.rule_type == RuleType::Defeasible
            })
            .unwrap();

        let blocked = is_blocked_by_superior(&indexed, &theory, rule, &state.defeasible_proven);
        assert!(
            !blocked,
            "Rule should NOT be blocked when it is superior to attacker"
        );
    }
}
