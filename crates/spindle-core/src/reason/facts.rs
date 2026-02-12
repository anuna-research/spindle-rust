//! Phase 1 initialization: fact seeding and body counter setup.
//!
//! Seeds the [`ReasoningState`] worklist and proven sets with:
//!
//! 1. **Facts** -- each fact is marked as definitely proven (+D) and
//!    its head literal is enqueued for strict-rule forward chaining.
//! 2. **Empty-body strict rules** -- fire unconditionally, same as facts.
//!
//! Also populates `definite_body_remaining`, `defeasible_body_remaining`,
//! and `rule_discarded` counters for all rules in the theory.
//!
//! Note: empty-body **defeasible** rules are handled in Phase 2
//! ([`super::defeasible::resolve_defeasible`]), not here, because they
//! require the full ambiguity blocking check.

use crate::conclusion::Conclusion;
use crate::index::IndexedTheory;
use crate::rule::RuleType;
use crate::theory::Theory;

use super::state::ReasoningState;

/// Initialize the reasoning state with facts, empty-body strict rules,
/// and body counters for all rules.
pub(crate) fn initialize_facts<'a>(
    theory: &'a Theory,
    indexed: &mut IndexedTheory<'_>,
    state: &mut ReasoningState<'a>,
) {
    // Populate body_remaining and rule_discarded for all rules
    for rule in theory.rules() {
        state
            .definite_body_remaining
            .insert(&rule.label, rule.body.len());
        state
            .defeasible_body_remaining
            .insert(&rule.label, rule.body.len());
        state.rule_discarded.insert(&rule.label, false);
    }

    // Phase 1a: Initialize with facts (deduplicated)
    for fact in theory.facts() {
        let lit = fact.head_literal().clone();
        let lit_id = indexed.intern_literal(&lit);

        // Skip duplicate facts -- only process each literal once
        if state.enqueued.contains(lit_id) {
            continue;
        }
        state.enqueued.insert(lit_id);
        state.definite_proven.insert(lit_id);

        state.add_conclusion(Conclusion::definitely_provable(lit.clone()).with_rule(&fact.label));
        state.worklist.push_back(lit);
    }

    // Phase 1b: Initialize empty-body strict rules
    // These rules have no body literals so forward chaining never triggers them.
    // We must seed their heads into the worklist explicitly.
    // (Empty-body defeasible rules are handled in Phase 2.)
    for rule in theory.rules() {
        if rule.body.is_empty() && rule.rule_type == RuleType::Strict {
            let head_lit = rule.head_literal().clone();
            let head_id = indexed.intern_literal(&head_lit);

            if !state.is_definitely_proven(head_id) {
                state.definite_proven.insert(head_id);
                state.add_conclusion(
                    Conclusion::definitely_provable(head_lit.clone()).with_rule(&rule.label),
                );
            }
            if !state.enqueued.contains(head_id) {
                state.enqueued.insert(head_id);
                state.worklist.push_back(head_lit);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::index::IndexedTheory;
    use crate::literal::Literal;
    use crate::rule::Rule;

    /// Helper: build state + indexed theory, run initialize_facts, return state.
    fn run_init(theory: &Theory) -> (ReasoningState<'_>, IndexedTheory<'_>) {
        let mut indexed = IndexedTheory::build(theory);
        let atom_count = indexed.atom_count();
        let rule_count = theory.rule_count();
        let estimated = rule_count * 2 + indexed.all_literal_ids().count() * 2;
        let mut state = ReasoningState::new(atom_count, rule_count, estimated);

        initialize_facts(theory, &mut indexed, &mut state);

        (state, indexed)
    }

    // ======================================================================
    // BASIC FACT INITIALIZATION
    // ======================================================================

    #[test]
    fn test_single_fact_produces_definite() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let (state, _indexed) = run_init(&theory);

        let has_definite = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "bird"
        });

        assert!(has_definite, "Fact should produce +D conclusion");
        // Phase 1 does NOT emit +d — that is Phase 2's job after condition (2) check
    }

    #[test]
    fn test_negated_fact() {
        let mut theory = Theory::new();
        theory.add_fact("~guilty");

        let (state, _indexed) = run_init(&theory);

        let has_definite = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.literal.name() == "guilty"
                && c.literal.negation
        });

        assert!(has_definite, "Negated fact should produce +D ~guilty");
    }

    #[test]
    fn test_multiple_facts() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_fact("c");

        let (state, _indexed) = run_init(&theory);

        let definite_count = state
            .conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
            .count();

        assert_eq!(
            definite_count, 3,
            "Three facts should produce three +D conclusions"
        );
        assert_eq!(
            state.worklist.len(),
            3,
            "Three facts should enqueue three literals"
        );
    }

    #[test]
    fn test_duplicate_facts_deduplicated() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("bird"); // duplicate

        let (state, _indexed) = run_init(&theory);

        let definite_count = state
            .conclusions
            .iter()
            .filter(|c| {
                c.conclusion_type == ConclusionType::DefinitelyProvable
                    && c.literal.name() == "bird"
            })
            .count();

        assert_eq!(
            definite_count, 1,
            "Duplicate facts should produce exactly one +D conclusion"
        );
        assert_eq!(
            state.worklist.len(),
            1,
            "Duplicate facts should enqueue only once"
        );
    }

    #[test]
    fn test_fact_enqueues_literal_in_worklist() {
        let mut theory = Theory::new();
        theory.add_fact("p");

        let (state, _indexed) = run_init(&theory);

        assert_eq!(state.worklist.len(), 1);
        assert_eq!(state.worklist[0].name(), "p");
    }

    // ======================================================================
    // BODY_REMAINING INITIALIZATION
    // ======================================================================

    #[test]
    fn test_body_remaining_populated_for_all_rules() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p", "q"], "r");
        theory.add_strict_rule(&["p"], "s");

        let (state, _indexed) = run_init(&theory);

        // Both body_remaining maps should have entries for all rules
        let def_has_2 = state.definite_body_remaining.values().any(|&v| v == 2);
        let def_has_1 = state.definite_body_remaining.values().any(|&v| v == 1);
        let defeas_has_2 = state.defeasible_body_remaining.values().any(|&v| v == 2);

        assert!(def_has_2, "definite_body_remaining should have count 2");
        assert!(def_has_1, "definite_body_remaining should have count 1");
        assert!(
            defeas_has_2,
            "defeasible_body_remaining should have count 2"
        );
    }

    // ======================================================================
    // EMPTY-BODY NON-FACT RULES
    // ======================================================================

    #[test]
    fn test_empty_body_strict_rule_fires() {
        let mut theory = Theory::new();
        let rule = Rule::new(
            "axiom",
            RuleType::Strict,
            vec![],
            vec![Literal::simple("truth")],
        );
        theory.add_rule(rule);

        let (state, _indexed) = run_init(&theory);

        let has_definite = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "truth"
        });

        assert!(
            has_definite,
            "Empty-body strict rule should produce +D conclusion"
        );
    }

    #[test]
    fn test_empty_body_defeasible_rule_not_fired_in_phase1() {
        // Empty-body defeasible rules should NOT be fired during Phase 1.
        // They are handled in Phase 2 with proper ambiguity blocking.
        let mut theory = Theory::new();
        let rule = Rule::new(
            "axiom",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("maybe")],
        );
        theory.add_rule(rule);

        let (state, _indexed) = run_init(&theory);

        let has_defeasible = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "maybe"
        });

        assert!(
            !has_defeasible,
            "Phase 1 should not fire empty-body defeasible rules"
        );
    }

    #[test]
    fn test_empty_theory_produces_no_conclusions() {
        let theory = Theory::new();

        let (state, _indexed) = run_init(&theory);

        assert!(
            state.conclusions.is_empty(),
            "Empty theory should produce no conclusions in Phase 1"
        );
        assert!(
            state.worklist.is_empty(),
            "Empty theory should have empty worklist"
        );
    }

    #[test]
    fn test_fact_plus_empty_body_strict_no_duplicate() {
        // A fact already proves +D p. An empty-body strict rule for p should
        // not emit a second +D p conclusion.
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_rule(Rule::new(
            "axiom",
            RuleType::Strict,
            vec![],
            vec![Literal::simple("p")],
        ));

        let (state, _indexed) = run_init(&theory);

        let definite_p_count = state
            .conclusions
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
    fn test_rules_with_body_not_fired_in_phase1() {
        // Rules with non-empty bodies should NOT be fired during Phase 1 init
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        let (state, _indexed) = run_init(&theory);

        let has_q = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "q"
        });

        assert!(
            !has_q,
            "Phase 1 should not fire rules with non-empty bodies (that's Phase 2)"
        );
    }
}
