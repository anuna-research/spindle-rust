//! Reasoning engine for defeasible logic
//!
//! Implements the standard DL(d) forward chaining algorithm.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::conclusion::{Conclusion, ConclusionType};
use crate::index::IndexedTheory;
use crate::literal::Literal;
use crate::rule::RuleType;
use crate::theory::Theory;

/// Perform defeasible reasoning on a theory
pub fn reason(theory: &Theory) -> Vec<Conclusion> {
    let indexed = IndexedTheory::build(theory.clone());
    let mut conclusions = Vec::new();

    // Track what we've proven
    let mut definite_proven: HashSet<String> = HashSet::new();
    let mut defeasible_proven: HashSet<String> = HashSet::new();

    // Track rule body satisfaction
    let mut body_remaining: HashMap<String, usize> = HashMap::new();
    for rule in theory.rules() {
        body_remaining.insert(rule.label.clone(), rule.body.len());
    }

    // Worklist for forward chaining
    let mut worklist: VecDeque<Literal> = VecDeque::new();

    // Phase 1: Initialize with facts
    for fact in theory.facts() {
        let lit = fact.head_literal().clone();
        let key = lit.canonical_name();

        definite_proven.insert(key.clone());
        defeasible_proven.insert(key.clone());

        conclusions.push(Conclusion::definitely_provable(lit.clone()));
        conclusions.push(Conclusion::defeasibly_provable(lit.clone()));

        worklist.push_back(lit);
    }

    // Phase 2: Forward chaining
    while let Some(lit) = worklist.pop_front() {
        // Find rules where this literal appears in body
        for rule in indexed.rules_with_body(&lit) {
            let remaining = body_remaining.get_mut(&rule.label).unwrap();
            if *remaining > 0 {
                *remaining -= 1;

                // If body fully satisfied, try to fire rule
                if *remaining == 0 {
                    let head_lit = rule.head_literal().clone();
                    let head_key = head_lit.canonical_name();

                    match rule.rule_type {
                        RuleType::Fact => unreachable!("Facts have no body"),

                        RuleType::Strict => {
                            if !definite_proven.contains(&head_key) {
                                definite_proven.insert(head_key.clone());
                                defeasible_proven.insert(head_key.clone());

                                conclusions
                                    .push(Conclusion::definitely_provable(head_lit.clone()));
                                conclusions
                                    .push(Conclusion::defeasibly_provable(head_lit.clone()));

                                worklist.push_back(head_lit);
                            }
                        }

                        RuleType::Defeasible => {
                            // Check for conflicts and superiority
                            let complement = head_lit.complement();
                            let comp_key = complement.canonical_name();

                            // Only prove if complement isn't definitely proven
                            if !definite_proven.contains(&comp_key)
                                && !defeasible_proven.contains(&head_key)
                            {
                                // Check if we're blocked by superior rules
                                let blocked = is_blocked_by_superior(
                                    &indexed,
                                    theory,
                                    rule,
                                    &defeasible_proven,
                                );

                                if !blocked {
                                    defeasible_proven.insert(head_key.clone());
                                    conclusions
                                        .push(Conclusion::defeasibly_provable(head_lit.clone()));
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
    for lit_name in indexed.all_literals() {
        let lit = Literal::simple(lit_name);

        if !definite_proven.contains(lit_name) {
            conclusions.push(Conclusion::new(
                ConclusionType::DefinitelyNotProvable,
                lit.clone(),
            ));
        }

        if !defeasible_proven.contains(lit_name) {
            conclusions.push(Conclusion::new(
                ConclusionType::DefeasiblyNotProvable,
                lit,
            ));
        }
    }

    conclusions
}

/// Check if a rule is blocked by a superior rule for the complement
fn is_blocked_by_superior(
    indexed: &IndexedTheory,
    theory: &Theory,
    rule: &crate::rule::Rule,
    proven: &HashSet<String>,
) -> bool {
    let head_lit = rule.head_literal();
    let complement = head_lit.complement();

    // Find rules for the complement
    let attacking_rules = indexed.rules_with_head(&complement);

    for attacker in attacking_rules {
        // Check if attacker's body is satisfied
        let body_satisfied = attacker
            .body
            .iter()
            .all(|b| proven.contains(&b.canonical_name()));

        if !body_satisfied {
            continue;
        }

        // Check superiority: is attacker > rule?
        let attacker_superior = theory
            .superiorities()
            .iter()
            .any(|s| s.superior == attacker.label && s.inferior == rule.label);

        // Check if rule > attacker
        let rule_superior = theory
            .superiorities()
            .iter()
            .any(|s| s.superior == rule.label && s.inferior == attacker.label);

        // If attacker is superior and rule is not superior over it, we're blocked
        if attacker_superior && !rule_superior {
            return true;
        }

        // If neither is superior, we have a conflict (both blocked in ambiguity propagation)
        // For now, we allow both to be proven (skeptical semantics would block both)
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_fact() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let conclusions = reason(&theory);

        assert!(conclusions.iter().any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
            && c.literal.name == "bird"));
    }

    #[test]
    fn test_strict_chain() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_strict_rule(&["bird"], "animal");

        let conclusions = reason(&theory);

        assert!(conclusions.iter().any(|c| c.conclusion_type == ConclusionType::DefinitelyProvable
            && c.literal.name == "animal"));
    }

    #[test]
    fn test_defeasible_rule() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let conclusions = reason(&theory);

        assert!(conclusions.iter().any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name == "flies"));
    }

    #[test]
    fn test_penguin_doesnt_fly() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");

        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");

        theory.add_superiority(&r2, &r1);

        let conclusions = reason(&theory);

        // ~flies should be defeasibly provable (penguins don't fly)
        assert!(conclusions.iter().any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name == "flies"
            && c.literal.negation));
    }
}
