//! Phase 1: Fact initialization for the DL(d) forward-chaining algorithm.
//!
//! Seeds the [`ReasoningState`] worklist and proven sets with:
//!
//! 1. **Facts** -- each fact is marked as definitely proven (+D) and
//!    defeasibly proven (+d), and its head literal is enqueued.
//! 2. **Empty-body non-fact rules** -- strict rules fire unconditionally;
//!    defeasible rules fire unless blocked by a superior attacker or defeater.
//!
//! Also populates `body_remaining` counters for all rules in the theory.

use crate::conclusion::Conclusion;
use crate::index::IndexedTheory;
use crate::rule::RuleType;
use crate::theory::Theory;

use super::defeasible::is_blocked_by_superior;
use super::state::ReasoningState;

/// Initialize the reasoning state with facts and empty-body rules.
///
/// This corresponds to Phase 1 (and Phase 1b) of the reasoning pipeline:
///
/// - Populate `body_remaining` counters for every rule.
/// - Iterate over facts, marking each as definitely proven, emitting +D and +d
///   conclusions, and enqueueing its head literal.  Duplicate facts are skipped
///   via the `enqueued` set to prevent double-counting.
/// - Fire empty-body strict rules (unconditionally) and empty-body defeasible
///   rules (unless blocked by a superior attacker).
pub(crate) fn initialize_facts<'a>(
    theory: &'a Theory,
    indexed: &mut IndexedTheory<'_>,
    state: &mut ReasoningState<'a>,
) {
    // Populate body_remaining for all rules
    for rule in theory.rules() {
        state.body_remaining.insert(&rule.label, rule.body.len());
    }

    // Phase 1: Initialize with facts (deduplicated)
    for fact in theory.facts() {
        let lit = fact.head_literal().clone();
        // Interning here is safe as facts are already in the theory
        let lit_id = indexed.intern_literal(&lit);

        // Skip duplicate facts -- only process each literal once
        if state.enqueued.contains(lit_id) {
            continue;
        }
        state.enqueued.insert(lit_id);

        state.mark_definitely_proven(lit_id);

        state.add_conclusion(Conclusion::definitely_provable(lit.clone()).with_rule(&fact.label));
        state.add_conclusion(Conclusion::defeasibly_provable(lit.clone()).with_rule(&fact.label));

        state.worklist.push_back(lit);
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
                    }
                    if !state.enqueued.contains(head_id) {
                        state.enqueued.insert(head_id);
                        state.worklist.push_back(head_lit);
                    }
                }
                RuleType::Defeasible => {
                    // Empty-body defeasible rules fire immediately but can still be blocked
                    if !state.is_defeasibly_proven(head_id) {
                        let blocked =
                            is_blocked_by_superior(indexed, theory, rule, &state.defeasible_proven);
                        if !blocked {
                            state.defeasible_proven.insert(head_id);
                            state.add_conclusion(
                                Conclusion::defeasibly_provable(head_lit.clone())
                                    .with_rule(&rule.label),
                            );
                        }
                    }
                    if state.is_defeasibly_proven(head_id) && !state.enqueued.contains(head_id) {
                        state.enqueued.insert(head_id);
                        state.worklist.push_back(head_lit);
                    }
                }
                _ => {}
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
    fn test_single_fact_produces_definite_and_defeasible() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let (state, _indexed) = run_init(&theory);

        let has_definite = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "bird"
        });
        let has_defeasible = state.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "bird"
        });

        assert!(has_definite, "Fact should produce +D conclusion");
        assert!(has_defeasible, "Fact should produce +d conclusion");
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

        // body_remaining should have entries for all rules
        // The defeasible rule has 2 body literals, strict has 1, fact has 0
        let defeasible_remaining = state.body_remaining.values().find(|&&v| v == 2);
        let strict_remaining = state.body_remaining.values().find(|&&v| v == 1);

        assert!(
            defeasible_remaining.is_some(),
            "body_remaining should have an entry with count 2 for the defeasible rule"
        );
        assert!(
            strict_remaining.is_some(),
            "body_remaining should have an entry with count 1 for the strict rule"
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
    fn test_empty_body_defeasible_rule_fires() {
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
            has_defeasible,
            "Empty-body defeasible rule should produce +d conclusion"
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
        // Rules with non-empty bodies should NOT be fired during Phase 1
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
