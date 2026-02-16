//! Adversarial property-based tests targeting reasoning engine edge cases.
//!
//! These tests probe specific potential bug areas:
//! - Duplicate body literals and body counter logic
//! - Diamond conflict propagation through chains
//! - Multi-attacker/superiority interactions
//! - Empty-body rule interactions
//! - Strict chain blocking defeasible conclusions
//! - Self-referential and circular rules
//! - Cascading ambiguity blocking

mod proptest_helpers;

use proptest::prelude::*;
use std::collections::HashSet;

use spindle_core::conclusion::ConclusionType;
use spindle_core::literal::Literal;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::reason::reason;
use spindle_core::rule::{Rule, RuleType};
use spindle_core::theory::Theory;

use proptest_helpers::{ATOMS, conclusion_set};

// =============================================================================
// Helper: check that no literal has both +d and +d for its complement
// (mutual exclusion at the defeasible level)
// =============================================================================

fn check_defeasible_mutual_exclusion(conclusions: &[spindle_core::conclusion::Conclusion]) -> bool {
    let defeasible: HashSet<_> = conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
        .map(|c| c.literal.canonical_name())
        .collect();

    let definite: HashSet<_> = conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
        .map(|c| c.literal.canonical_name())
        .collect();

    for lit_name in &defeasible {
        let complement = if lit_name.starts_with('~') {
            lit_name.trim_start_matches('~').to_string()
        } else {
            format!("~{lit_name}")
        };
        if defeasible.contains(&complement) {
            // Both +d L and +d ~L: only legal if at least one has +D backing
            if !definite.contains(lit_name) && !definite.contains(&complement) {
                return false;
            }
        }
    }
    true
}

// =============================================================================
// 1. Duplicate body literals: [p, p, q] => r
// The body counter should correctly handle duplicate literals without
// double-firing or failing to fire.
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Rules with duplicate body literals should behave like set-semantics:
    /// [p, p, q] => r should fire iff p AND q are both proven.
    #[test]
    fn duplicate_body_literals_fire_correctly(
        fact1 in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        fact2 in proptest::sample::select(&["e", "f", "g", "h"]).prop_map(String::from),
        head in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
    ) {
        // Theory with rule [fact1, fact1, fact2] => head
        let mut theory = Theory::new();
        theory.add_fact(&fact1);
        theory.add_fact(&fact2);

        let body = vec![
            Literal::simple(&fact1),
            Literal::simple(&fact1), // duplicate
            Literal::simple(&fact2),
        ];
        let rule = Rule::defeasible("r-dup", body, Literal::simple(&head));
        theory.add_rule(rule);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        // Both facts are present, rule should fire
        prop_assert!(
            defeasible.contains(&head),
            "Rule with dup body [{}, {}, {}] => {} should fire when both facts present, got: {:?}",
            fact1, fact1, fact2, head, defeasible
        );
    }

    /// Rules with duplicate body literals where one fact is missing should NOT fire.
    #[test]
    fn duplicate_body_missing_fact_does_not_fire(
        fact1 in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        missing in proptest::sample::select(&["e", "f", "g", "h"]).prop_map(String::from),
        head in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_fact(&fact1);
        // `missing` is NOT a fact

        let body = vec![
            Literal::simple(&fact1),
            Literal::simple(&fact1),
            Literal::simple(&missing),
        ];
        let rule = Rule::defeasible("r-dup", body, Literal::simple(&head));
        theory.add_rule(rule);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        prop_assert!(
            !defeasible.contains(&head),
            "Rule with missing body literal should not fire"
        );
    }
}

// =============================================================================
// 2. Diamond conflict propagation: ambiguity should block downstream
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// When two defeasible rules conflict on an intermediate literal, downstream
    /// rules depending on the ambiguous literal should NOT produce +d conclusions.
    #[test]
    fn ambiguity_blocks_downstream(
        fact in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        mid in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
        downstream in proptest::sample::select(&["w", "x", "y", "z"]).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_fact(&fact);
        // Conflicting rules on `mid`
        theory.add_defeasible_rule(&[fact.as_str()], &mid);
        theory.add_defeasible_rule(&[fact.as_str()], &format!("~{mid}"));
        // Downstream rule depending on ambiguous `mid`
        theory.add_defeasible_rule(&[mid.as_str()], &downstream);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        // mid should be blocked
        prop_assert!(
            !defeasible.contains(&mid),
            "Ambiguous {} should not be +d",
            mid
        );

        // downstream should also be blocked (depends on ambiguous mid)
        prop_assert!(
            !defeasible.contains(&downstream),
            "Downstream {} should not be +d when {} is ambiguous",
            downstream, mid
        );
    }
}

// =============================================================================
// 3. Superiority chain: r3 > r2 > r1 on three-way conflict
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// With three conflicting rules and a chain of superiority,
    /// the topmost superior rule should win.
    #[test]
    fn superiority_chain_three_rules(
        fact in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        head in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_fact(&fact);

        // r1: fact => head
        // r2: fact => ~head
        // r3: fact => head (superior to r2)
        let _r1 = theory.add_defeasible_rule(&[fact.as_str()], &head);
        let r2 = theory.add_defeasible_rule(&[fact.as_str()], &format!("~{head}"));
        let r3 = theory.add_defeasible_rule(&[fact.as_str()], &head);
        theory.add_superiority(&r3, &r2);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        // r3 > r2 means head should win via cross-rule superiority
        prop_assert!(
            defeasible.contains(&head),
            "{} should be +d (r3 > r2 defeats attacker), got: {:?}",
            head, defeasible
        );
    }
}

// =============================================================================
// 4. Empty-body defeasible rules in conflict
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Two empty-body defeasible rules producing opposite conclusions
    /// should trigger ambiguity blocking.
    #[test]
    fn empty_body_defeasible_conflict(
        head in proptest::sample::select(ATOMS).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_rule(Rule::new(
            "r1",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple(&head)],
        ));
        theory.add_rule(Rule::new(
            "r2",
            RuleType::Defeasible,
            vec![],
            vec![Literal::negated(&head)],
        ));

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        prop_assert!(
            !defeasible.contains(&head),
            "{} should be blocked (empty-body conflict)",
            head
        );
        let neg = format!("~{head}");
        prop_assert!(
            !defeasible.contains(&neg),
            "~{} should be blocked (empty-body conflict)",
            head
        );
    }

    /// Empty-body defeasible rule with superiority resolving conflict
    #[test]
    fn empty_body_superiority_resolves(
        head in proptest::sample::select(ATOMS).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        let r1_label = "r_pos".to_string();
        let r2_label = "r_neg".to_string();
        theory.add_rule(Rule::new(
            r1_label.clone(),
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple(&head)],
        ));
        theory.add_rule(Rule::new(
            r2_label.clone(),
            RuleType::Defeasible,
            vec![],
            vec![Literal::negated(&head)],
        ));
        theory.add_superiority(&r1_label, &r2_label);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        prop_assert!(
            defeasible.contains(&head),
            "{} should be +d (r_pos > r_neg)",
            head
        );
    }
}

// =============================================================================
// 5. Self-referential rules: p => p with and without fact
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Self-referential rule `p => p` should be harmless when p is a fact
    /// (p should be provable, no infinite loop).
    #[test]
    fn self_referential_with_fact(
        atom in proptest::sample::select(ATOMS).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_fact(&atom);
        theory.add_defeasible_rule(&[atom.as_str()], &atom);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);
        let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);

        prop_assert!(
            definite.contains(&atom),
            "Fact {} should be +D",
            atom
        );
        prop_assert!(
            defeasible.contains(&atom),
            "Fact {} should also be +d (self-referential rule shouldn't break this)",
            atom
        );
    }

    /// Self-referential rule `p => p` without fact should NOT prove p.
    #[test]
    fn self_referential_without_fact(
        atom in proptest::sample::select(ATOMS).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&[atom.as_str()], &atom);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        prop_assert!(
            !defeasible.contains(&atom),
            "Self-referential rule without fact should NOT prove {}",
            atom
        );
    }
}

// =============================================================================
// 6. Strict chain blocks defeasible at various depths
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// A strict chain `a -> b -> ~q` should block `p => q` regardless
    /// of chain depth.
    #[test]
    fn strict_chain_blocks_defeasible(
        depth in 1_usize..=5,
    ) {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("trigger");

        // Build strict chain: a -> c1 -> c2 -> ... -> ~q
        let mut prev = "a".to_string();
        for i in 0..depth {
            let next = if i == depth - 1 {
                "~q".to_string()
            } else {
                format!("chain{i}")
            };
            theory.add_strict_rule(&[prev.as_str()], &next);
            prev = next;
        }

        // Defeasible rule trying to prove q
        theory.add_defeasible_rule(&["trigger"], "q");

        let conclusions = reason(&theory).unwrap();
        let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        prop_assert!(
            definite.contains("~q"),
            "+D ~q should hold from strict chain of depth {}",
            depth
        );
        prop_assert!(
            !defeasible.contains("q"),
            "q should NOT be +d when +D ~q from strict chain of depth {}",
            depth
        );
    }
}

// =============================================================================
// 7. Defeater interactions
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// A defeater should block a defeasible conclusion, but superiority
    /// of the defeasible rule over the defeater should unblock it.
    #[test]
    fn defeater_blocked_by_superiority(
        fact in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        head in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
    ) {
        // Without superiority: defeater blocks
        {
            let mut theory = Theory::new();
            theory.add_fact(&fact);
            theory.add_defeasible_rule(&[fact.as_str()], &head);
            theory.add_defeater(&[fact.as_str()], &format!("~{head}"));

            let conclusions = reason(&theory).unwrap();
            let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

            prop_assert!(
                !defeasible.contains(&head),
                "{} should be blocked by defeater (no superiority)",
                head
            );
        }

        // With superiority: defeasible rule wins
        {
            let mut theory = Theory::new();
            theory.add_fact(&fact);
            let r1 = theory.add_defeasible_rule(&[fact.as_str()], &head);
            let d1 = theory.add_defeater(&[fact.as_str()], &format!("~{head}"));
            theory.add_superiority(&r1, &d1);

            let conclusions = reason(&theory).unwrap();
            let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

            prop_assert!(
                defeasible.contains(&head),
                "{} should be +d (rule superior to defeater)",
                head
            );
        }
    }
}

// =============================================================================
// 8. No contradictory defeasible conclusions (without definite backing)
// =============================================================================

/// Generate a complex theory with multiple conflicts and check invariants.
fn arb_adversarial_theory() -> impl Strategy<Value = Theory> {
    (
        proptest::collection::vec(proptest::sample::select(ATOMS), 2..=6),
        proptest::collection::vec(
            (
                proptest::sample::select(&["a", "b", "c", "d"]),
                proptest::sample::select(&["m", "n", "o", "p"]),
                proptest::bool::ANY,
            ),
            1..=8,
        ),
        proptest::collection::vec(
            (
                proptest::sample::select(&["a", "b", "c", "d"]),
                proptest::sample::select(&["m", "n", "o", "p"]),
            ),
            0..=3,
        ),
    )
        .prop_map(|(facts, rules, defeaters)| {
            let mut theory = Theory::new();
            let mut labels = Vec::new();

            for &atom in &facts {
                theory.add_fact(atom);
            }

            for (body, head, negated) in &rules {
                let head_str = if *negated {
                    format!("~{head}")
                } else {
                    head.to_string()
                };
                let label = theory.add_defeasible_rule(&[body], &head_str);
                labels.push(label);
            }

            for (body, head) in &defeaters {
                theory.add_defeater(&[body], &format!("~{head}"));
            }

            // Add some superiority relations
            if labels.len() >= 2 {
                theory.add_superiority(&labels[0], &labels[1]);
            }

            theory
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(300))]

    /// No theory should ever produce both +d L and +d ~L unless one
    /// has definite (+D) backing.
    #[test]
    fn no_contradictory_defeasible_without_definite(theory in arb_adversarial_theory()) {
        let conclusions = reason(&theory).unwrap();
        prop_assert!(
            check_defeasible_mutual_exclusion(&conclusions),
            "Found +d L and +d ~L without definite backing"
        );
    }

    /// Every +d conclusion should have at least one applicable supporting rule
    /// whose body is fully satisfied by the proven conclusions.
    #[test]
    fn defeasible_conclusion_has_support(theory in arb_adversarial_theory()) {
        let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
        let conclusions = spindle_core::reason::reason_prepared(&prepared.theory).unwrap();

        let defeasible: HashSet<_> = conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
            .map(|c| c.literal.canonical_name())
            .collect();

        let definite: HashSet<_> = conclusions
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
            .map(|c| c.literal.canonical_name())
            .collect();

        // All proven literals (+d or +D) — a body literal is satisfied if proven
        let proven: HashSet<_> = defeasible.union(&definite).cloned().collect();

        // Every +d literal must either:
        // 1. Be +D (subsumption), OR
        // 2. Have at least one non-defeater rule with head=literal whose
        //    body is fully satisfied (every body literal is proven)
        for lit_name in &defeasible {
            if definite.contains(lit_name) {
                continue; // subsumption covers it
            }

            let has_applicable_support = prepared.theory.rules().any(|r| {
                matches!(r.rule_type, RuleType::Strict | RuleType::Defeasible | RuleType::Fact)
                    && !r.head.is_empty()
                    && r.head_literal().canonical_name() == *lit_name
                    && r.body.iter().all(|body_lit| proven.contains(&body_lit.canonical_name()))
            });

            prop_assert!(
                has_applicable_support,
                "+d {} has no applicable supporting rule (head match + body satisfied) in prepared theory",
                lit_name
            );
        }
    }
}

// =============================================================================
// 9. Conflicting facts: both +D p and +D ~p should block defeasible level
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// When both `p` and `~p` are facts, neither should be defeasibly provable
    /// (condition 2 of +d fails for both).
    #[test]
    fn conflicting_facts_block_defeasible(
        atom in proptest::sample::select(ATOMS).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_fact(&atom);
        theory.add_fact(&format!("~{atom}"));

        let conclusions = reason(&theory).unwrap();
        let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        let neg = format!("~{atom}");

        prop_assert!(definite.contains(&atom), "+D {} expected", atom);
        prop_assert!(definite.contains(&neg), "+D ~{} expected", atom);

        // With +D p AND +D ~p, condition (2) of +d fails for both
        prop_assert!(
            !defeasible.contains(&atom),
            "{} should NOT be +d when +D ~{0}",
            atom
        );
        prop_assert!(
            !defeasible.contains(&neg),
            "~{} should NOT be +d when +D {}",
            atom, atom
        );
    }
}

// =============================================================================
// 10. Circular rule dependencies
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Circular defeasible rules should terminate without infinite loop
    /// and produce no positive conclusions.
    #[test]
    fn circular_rules_terminate(
        cycle_len in 2_usize..=6,
    ) {
        let mut theory = Theory::new();
        let atoms: Vec<String> = (0..cycle_len).map(|i| format!("c{i}")).collect();

        for i in 0..cycle_len {
            let body = &atoms[i];
            let head = &atoms[(i + 1) % cycle_len];
            theory.add_defeasible_rule(&[body.as_str()], head);
        }

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        for atom in &atoms {
            prop_assert!(
                !defeasible.contains(atom),
                "Circular rule should not prove {} (cycle length {})",
                atom, cycle_len
            );
        }
    }

    /// Circular rules with one fact in the cycle should prove all downstream.
    #[test]
    fn circular_with_fact_proves_chain(
        cycle_len in 2_usize..=6,
    ) {
        let mut theory = Theory::new();
        let atoms: Vec<String> = (0..cycle_len).map(|i| format!("c{i}")).collect();

        // Seed c0 as a fact
        theory.add_fact(&atoms[0]);

        for i in 0..cycle_len {
            let body = &atoms[i];
            let head = &atoms[(i + 1) % cycle_len];
            theory.add_defeasible_rule(&[body.as_str()], head);
        }

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        // All atoms in the chain should be +d (chain from fact c0)
        for atom in &atoms {
            prop_assert!(
                defeasible.contains(atom),
                "{} should be +d in cycle with fact seed (cycle length {})",
                atom, cycle_len
            );
        }
    }
}

// =============================================================================
// 11. Wide theories: many independent conclusions
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// A theory with N independent fact->conclusion pairs should prove all N.
    #[test]
    fn wide_theory_all_conclusions(
        width in 10_usize..=50,
    ) {
        let mut theory = Theory::new();
        let mut expected: HashSet<String> = HashSet::new();

        for i in 0..width {
            let fact = format!("fact{i}");
            let head = format!("head{i}");
            theory.add_fact(&fact);
            theory.add_defeasible_rule(&[fact.as_str()], &head);
            expected.insert(head);
        }

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        for head in &expected {
            prop_assert!(
                defeasible.contains(head),
                "{} should be +d in wide theory",
                head
            );
        }
    }
}

// =============================================================================
// 12. Mixed strict/defeasible: strict proof prevents defeasible conflict
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// If a strict rule proves `q`, a defeasible attacker for `~q` should
    /// not make `q` ambiguous (strict always wins).
    #[test]
    fn strict_proof_immune_to_defeasible_attacker(
        fact in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        head in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_fact(&fact);
        theory.add_strict_rule(&[fact.as_str()], &head);
        theory.add_defeasible_rule(&[fact.as_str()], &format!("~{head}"));

        let conclusions = reason(&theory).unwrap();
        let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        prop_assert!(
            definite.contains(&head),
            "+D {} should hold from strict rule",
            head
        );

        // The defeasible attacker for ~head should not be +d
        let neg = format!("~{head}");
        prop_assert!(
            !defeasible.contains(&neg),
            "~{} should NOT be +d against strict +D {}",
            head, head
        );
    }
}

// =============================================================================
// 13. Multiple supporters for same conclusion: at least one defeating
//     each attacker is sufficient
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Cross-rule superiority: r3's superiority over attacker r2 should
    /// unblock r1's conclusion (even though r1 itself isn't superior to r2).
    #[test]
    fn cross_rule_superiority_unblocks(
        fact1 in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        fact2 in proptest::sample::select(&["e", "f", "g", "h"]).prop_map(String::from),
        head in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_fact(&fact1);
        theory.add_fact(&fact2);

        // r1: fact1 => head (no superiority)
        theory.add_defeasible_rule(&[fact1.as_str()], &head);
        // r2: fact2 => ~head (attacker)
        let r2 = theory.add_defeasible_rule(&[fact2.as_str()], &format!("~{head}"));
        // r3: fact2 => head (superior to r2)
        let r3 = theory.add_defeasible_rule(&[fact2.as_str()], &head);
        theory.add_superiority(&r3, &r2);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        prop_assert!(
            defeasible.contains(&head),
            "{} should be +d (r3 > r2 defeats attacker, even though r1 doesn't)",
            head
        );
    }
}

// =============================================================================
// 14. Deep defeasible chain should propagate
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// A long uncontested defeasible chain should propagate the full depth.
    #[test]
    fn deep_defeasible_chain(
        depth in 5_usize..=20,
    ) {
        let mut theory = Theory::new();
        theory.add_fact("start");

        let atoms: Vec<String> = (0..depth).map(|i| format!("d{i}")).collect();
        theory.add_defeasible_rule(&["start"], &atoms[0]);

        for i in 0..depth - 1 {
            theory.add_defeasible_rule(&[atoms[i].as_str()], &atoms[i + 1]);
        }

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        for atom in &atoms {
            prop_assert!(
                defeasible.contains(atom),
                "{} should be +d in chain of depth {}",
                atom, depth
            );
        }
    }
}

// =============================================================================
// 15. Attacker with unsatisfied body should not block
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// An attacker whose body is not satisfied should not block the conclusion.
    #[test]
    fn unsatisfied_attacker_does_not_block(
        fact in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        head in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
        missing in proptest::sample::select(&["w", "x", "y", "z"]).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_fact(&fact);
        // `missing` is NOT a fact

        theory.add_defeasible_rule(&[fact.as_str()], &head);
        theory.add_defeasible_rule(&[missing.as_str()], &format!("~{head}"));

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        prop_assert!(
            defeasible.contains(&head),
            "{} should be +d (attacker body {} unsatisfied)",
            head, missing
        );
    }
}

// =============================================================================
// 16. Defeater never proves its head literal
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// A defeater's head literal should never appear as +d unless supported
    /// by a non-defeater rule.
    #[test]
    fn defeater_head_never_proven_alone(
        fact in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        head in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
    ) {
        let mut theory = Theory::new();
        theory.add_fact(&fact);
        // Only a defeater for head, no defeasible/strict rule
        theory.add_defeater(&[fact.as_str()], &head);

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);
        let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);

        prop_assert!(
            !defeasible.contains(&head),
            "Defeater alone should not prove +d {}",
            head
        );
        prop_assert!(
            !definite.contains(&head),
            "Defeater alone should not prove +D {}",
            head
        );
    }
}

// =============================================================================
// 17. Mixed empty-body and body rules for same head
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// An empty-body defeasible rule for `p` combined with a body-rule
    /// attacker for `~p` should interact correctly.
    #[test]
    fn empty_body_with_body_attacker(
        fact in proptest::sample::select(&["a", "b", "c", "d"]).prop_map(String::from),
        head in proptest::sample::select(&["m", "n", "o", "p"]).prop_map(String::from),
    ) {
        // Case 1: empty-body for head, body-rule for ~head (body satisfied)
        let mut theory = Theory::new();
        theory.add_fact(&fact);
        theory.add_rule(Rule::new(
            "r_empty",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple(&head)],
        ));
        theory.add_defeasible_rule(&[fact.as_str()], &format!("~{head}"));

        let conclusions = reason(&theory).unwrap();
        let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

        // Both sides should be blocked (ambiguity): neither +d head nor +d ~head
        prop_assert!(
            !defeasible.contains(&head),
            "Ambiguity should block +d {}, but it was provable",
            head
        );
        prop_assert!(
            !defeasible.contains(&format!("~{head}")),
            "Ambiguity should block +d ~{}, but it was provable",
            head
        );
    }
}

// =============================================================================
// 18. Large stress test: reason() never panics on complex theories
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Large theories with many rules, conflicts, and superiority relations
    /// should never panic, and no literal should have both +D and -D.
    #[test]
    fn stress_test_no_panic(
        n_facts in 3_usize..=10,
        n_rules in 5_usize..=15,
        n_sups in 0_usize..=5,
    ) {
        let atoms = &ATOMS[..std::cmp::min(n_facts + n_rules, ATOMS.len())];
        let mut theory = Theory::new();
        let mut labels = Vec::new();

        for i in 0..std::cmp::min(n_facts, atoms.len()) {
            theory.add_fact(atoms[i]);
        }

        for i in 0..n_rules {
            let body_idx = i % atoms.len();
            let head_idx = (i + 1) % atoms.len();
            let negated = i % 3 == 0;
            let head_str = if negated {
                format!("~{}", atoms[head_idx])
            } else {
                atoms[head_idx].to_string()
            };
            let label = theory.add_defeasible_rule(&[atoms[body_idx]], &head_str);
            labels.push(label);
        }

        if labels.len() >= 2 {
            for i in 0..std::cmp::min(n_sups, labels.len() - 1) {
                theory.add_superiority(&labels[i], &labels[i + 1]);
            }
        }

        let result = reason(&theory);
        prop_assert!(result.is_ok(), "reason() panicked on stress test: {:?}", result.err());

        let conclusions = result.unwrap();
        // Basic sanity: every literal should have at most one D verdict and one d verdict
        let mut d_verdicts: HashSet<String> = HashSet::new();
        let mut d_neg_verdicts: HashSet<String> = HashSet::new();

        for c in &conclusions {
            let name = c.literal.canonical_name();
            match c.conclusion_type {
                ConclusionType::DefinitelyProvable => {
                    prop_assert!(
                        !d_neg_verdicts.contains(&name),
                        "Both +D and -D for {}",
                        name
                    );
                    d_verdicts.insert(name);
                }
                ConclusionType::DefinitelyNotProvable => {
                    prop_assert!(
                        !d_verdicts.contains(&name),
                        "Both +D and -D for {}",
                        name
                    );
                    d_neg_verdicts.insert(name);
                }
                _ => {}
            }
        }
    }
}

// =============================================================================
// HAND-CRAFTED EDGE CASE TESTS
// =============================================================================
// These target specific algorithmic corner cases that random generation
// is unlikely to hit.

/// Team defeat: one attacker defeated, another not → conclusion still blocked.
///
///   >> a, >> b, >> c
///   r1: a => q
///   r2: b => ~q
///   r3: c => ~q
///   r1 > r2 (but NOT r1 > r3)
///
/// r2 is defeated by r1, but r3 is undefeated → +d q should FAIL.
#[test]
fn team_defeat_partial_superiority() {
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_fact("b");
    theory.add_fact("c");

    let r1 = theory.add_defeasible_rule(&["a"], "q");
    let r2 = theory.add_defeasible_rule(&["b"], "~q");
    let _r3 = theory.add_defeasible_rule(&["c"], "~q");
    theory.add_superiority(&r1, &r2); // r1 > r2, but NOT r1 > r3

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    // r3 is an undefeated attacker → +d q should fail
    assert!(
        !defeasible.contains("q"),
        "q should NOT be +d: r3 is an undefeated attacker (team defeat)"
    );
}

/// Superiority is not transitive: r1>r2 and r2>r3 does NOT imply r1>r3.
///
///   >> a, >> b
///   r1: a => q   (r1 > r2)
///   r2: a => ~q  (r2 > r3)
///   r3: b => q
///
/// For +d q: attackers = {r2}. Supporters = {r1, r3}.
///   r1 > r2 → attacker defeated → +d q ✓
/// For +d ~q: attackers = {r1, r3}. Supporters = {r2}.
///   r1: r2 > r1? No → undefeated attacker → fail
/// Result: +d q, -d ~q
#[test]
fn superiority_not_transitive() {
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_fact("b");

    let r1 = theory.add_defeasible_rule(&["a"], "q");
    let r2 = theory.add_defeasible_rule(&["a"], "~q");
    let r3 = theory.add_defeasible_rule(&["b"], "q");
    theory.add_superiority(&r1, &r2);
    theory.add_superiority(&r2, &r3);

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(
        defeasible.contains("q"),
        "+d q expected (r1 > r2 defeats the attacker)"
    );
    assert!(
        !defeasible.contains("~q"),
        "~q should NOT be +d (r1 and r3 are undefeated attackers of ~q)"
    );
}

/// Cascading ambiguity: conflict on intermediate blocks entire downstream chain.
///
///   >> a
///   r1: a => b
///   r2: a => ~b
///   r3: b => c
///   r4: c => d
///   r5: d => e
///
/// b is ambiguous → c, d, e all blocked.
#[test]
fn cascading_ambiguity_blocks_entire_chain() {
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_defeasible_rule(&["a"], "b");
    theory.add_defeasible_rule(&["a"], "~b");
    theory.add_defeasible_rule(&["b"], "c");
    theory.add_defeasible_rule(&["c"], "d");
    theory.add_defeasible_rule(&["d"], "e");

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(!defeasible.contains("b"), "b should be ambiguous");
    assert!(!defeasible.contains("~b"), "~b should be ambiguous");
    assert!(
        !defeasible.contains("c"),
        "c should be blocked (depends on ambiguous b)"
    );
    assert!(
        !defeasible.contains("d"),
        "d should be blocked (depends on blocked c)"
    );
    assert!(
        !defeasible.contains("e"),
        "e should be blocked (depends on blocked d)"
    );
}

/// Ambiguity is localized: independent conclusions unaffected.
///
///   >> a
///   r1: => p
///   r2: => ~p
///   r3: a => q
///
/// p is ambiguous, but q has independent support via r3 → +d q.
#[test]
fn ambiguity_localized_independent_unaffected() {
    let mut theory = Theory::new();
    theory.add_fact("a");
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
        vec![Literal::negated("p")],
    ));
    // Use manual label to avoid collision with auto-generated "r2"
    theory.add_rule(Rule::new(
        "r3",
        RuleType::Defeasible,
        vec![Literal::simple("a")],
        vec![Literal::simple("q")],
    ));

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(!defeasible.contains("p"), "p should be ambiguous");
    assert!(!defeasible.contains("~p"), "~p should be ambiguous");
    assert!(
        defeasible.contains("q"),
        "q should be +d (independent of ambiguous p)"
    );
}

/// Empty-body defeater: blocks complement without proving its own head.
///
///   r1: => q        (empty-body defeasible)
///   d1: ~> ~q       (empty-body defeater)
///
/// d1 attacks q by supporting ~q. For +d q: attacker d1 is applicable,
/// need t > d1 with t supporting q. r1 exists but no superiority → blocked.
#[test]
fn empty_body_defeater_blocks() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![],
        vec![Literal::simple("q")],
    ));
    theory.add_rule(Rule::new(
        "d1",
        RuleType::Defeater,
        vec![],
        vec![Literal::negated("q")],
    ));

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(
        !defeasible.contains("q"),
        "q should be blocked by empty-body defeater"
    );
    assert!(
        !defeasible.contains("~q"),
        "~q should NOT be +d (defeater can't prove)"
    );
}

/// Mutual defeaters: two defeaters blocking each other's complements.
///
///   d1: => ~q    (defeater, attacks q)
///   d2: => q     (defeater, attacks ~q)
///
/// Neither should be +d (defeaters don't prove, only block).
#[test]
fn mutual_defeaters_prove_nothing() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::new(
        "d1",
        RuleType::Defeater,
        vec![],
        vec![Literal::negated("q")],
    ));
    theory.add_rule(Rule::new(
        "d2",
        RuleType::Defeater,
        vec![],
        vec![Literal::simple("q")],
    ));

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(
        !defeasible.contains("q"),
        "q should not be +d (only defeaters)"
    );
    assert!(
        !defeasible.contains("~q"),
        "~q should not be +d (only defeaters)"
    );
}

/// Multi-hop strict chain must complete fully before defeasible phase.
///
///   >> a, >> b
///   s1: a -> c
///   s2: c -> d
///   s3: d -> ~q
///   r1: b => q
///
/// The strict chain a→c→d→~q must complete to produce +D ~q BEFORE
/// the defeasible rule r1 is evaluated. Otherwise, r1 could incorrectly
/// derive +d q.
#[test]
fn multi_hop_strict_chain_completes_before_defeasible() {
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_fact("b");
    theory.add_strict_rule(&["a"], "c");
    theory.add_strict_rule(&["c"], "d");
    theory.add_strict_rule(&["d"], "~q");
    theory.add_defeasible_rule(&["b"], "q");

    let conclusions = reason(&theory).unwrap();
    let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(definite.contains("c"), "+D c expected");
    assert!(definite.contains("d"), "+D d expected");
    assert!(definite.contains("~q"), "+D ~q expected from strict chain");
    assert!(
        !defeasible.contains("q"),
        "q should NOT be +d when +D ~q from strict chain"
    );
}

/// Duplicate facts should not cause body counter double-decrement.
///
///   >> p, >> p  (duplicate fact)
///   r1: p, q => r
///
/// With only p as fact (q missing), r should NOT fire.
/// A buggy implementation might decrement the body counter twice for p,
/// reducing [p, q] => r from remaining=2 to remaining=0 and firing it.
#[test]
fn duplicate_facts_no_spurious_rule_firing() {
    let mut theory = Theory::new();
    theory.add_fact("p");
    theory.add_fact("p"); // duplicate
    theory.add_defeasible_rule(&["p", "q"], "r");

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(
        !defeasible.contains("r"),
        "r should NOT be +d: body literal q is missing (duplicate p must not double-decrement)"
    );
}

/// Multiple duplicate body literals in a single rule.
///
///   >> p
///   r1: p, p, p => q
///
/// Should fire because p is proven (the 3 body slots all match p).
#[test]
fn triple_duplicate_body_fires() {
    let mut theory = Theory::new();
    theory.add_fact("p");
    let body = vec![
        Literal::simple("p"),
        Literal::simple("p"),
        Literal::simple("p"),
    ];
    theory.add_rule(Rule::defeasible("r1", body, Literal::simple("q")));

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(
        defeasible.contains("q"),
        "q should be +d: body [p, p, p] satisfied when p is a fact"
    );
}

/// Conflicting strict chains: both +D q and +D ~q from separate strict chains.
/// This is an inconsistent theory, but the engine must handle it gracefully.
///
///   >> a, >> b
///   s1: a -> q
///   s2: b -> ~q
///
/// Result: +D q, +D ~q, but -d q, -d ~q (condition 2 of +d fails for both).
#[test]
fn conflicting_strict_chains() {
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_fact("b");
    theory.add_strict_rule(&["a"], "q");
    theory.add_strict_rule(&["b"], "~q");

    let conclusions = reason(&theory).unwrap();
    let definite = conclusion_set(&conclusions, ConclusionType::DefinitelyProvable);
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    // Both should be +D (strict rules always fire)
    assert!(definite.contains("q"), "+D q expected from strict rule");
    assert!(definite.contains("~q"), "+D ~q expected from strict rule");

    // Neither should be +d (condition 2 fails when complement is +D)
    assert!(!defeasible.contains("q"), "q should NOT be +d when +D ~q");
    assert!(!defeasible.contains("~q"), "~q should NOT be +d when +D q");
}

/// Rule with body literal equal to head (self-loop) should not cause infinite loop
/// when there's a conflict.
///
///   >> p
///   r1: p => q
///   r2: q => q  (self-loop)
///   r3: p => ~q (attacker)
///
/// q is blocked by ambiguity (r1 vs r3), self-loop r2 shouldn't change this.
#[test]
fn self_loop_with_conflict() {
    let mut theory = Theory::new();
    theory.add_fact("p");
    theory.add_defeasible_rule(&["p"], "q");
    theory.add_defeasible_rule(&["q"], "q"); // self-loop
    theory.add_defeasible_rule(&["p"], "~q");

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(!defeasible.contains("q"), "q should be blocked (ambiguity)");
    assert!(
        !defeasible.contains("~q"),
        "~q should be blocked (ambiguity)"
    );
}

/// Superiority between rules with different body predicates, one body unsatisfied.
///
///   >> a
///   r1: a => q   (r1 > r2)
///   r2: b => ~q  (body unsatisfied, b not a fact)
///
/// r2 is not applicable (body unsatisfied) → no conflict → +d q.
#[test]
fn superior_rule_with_unsatisfied_attacker() {
    let mut theory = Theory::new();
    theory.add_fact("a");
    let r1 = theory.add_defeasible_rule(&["a"], "q");
    let r2 = theory.add_defeasible_rule(&["b"], "~q"); // b not a fact
    theory.add_superiority(&r1, &r2);

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(
        defeasible.contains("q"),
        "q should be +d (attacker body unsatisfied, superiority irrelevant)"
    );
}

/// Diamond dependency: two paths to same conclusion, one blocked.
///
///   >> a
///   r1: a => b
///   r2: a => c
///   r3: a => ~b (blocks b via ambiguity)
///   r4: b => d
///   r5: c => d
///
/// b is ambiguous (r1 vs r3), but c is uncontested.
/// d should be +d via r5 (c → d), even though r4 is blocked.
#[test]
fn diamond_one_path_blocked_other_succeeds() {
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_defeasible_rule(&["a"], "b");
    theory.add_defeasible_rule(&["a"], "c");
    theory.add_defeasible_rule(&["a"], "~b"); // blocks b
    theory.add_defeasible_rule(&["b"], "d");
    theory.add_defeasible_rule(&["c"], "d");

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(!defeasible.contains("b"), "b should be ambiguous");
    assert!(defeasible.contains("c"), "c should be +d (uncontested)");
    assert!(
        defeasible.contains("d"),
        "d should be +d via c → d path (even though b → d path is blocked)"
    );
}

/// The "floating conclusion" test: a conclusion supported by multiple rules
/// where one supporter's body becomes -d but another's remains applicable.
///
///   >> a, >> b, >> c
///   r1: a => p
///   r2: b => ~p  (blocks p via ambiguity with r1)
///   r3: c => q
///   r4: p => q   (this path is blocked)
///
/// q should be +d via r3 (independent of ambiguous p).
#[test]
fn floating_conclusion_independent_support() {
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_fact("b");
    theory.add_fact("c");
    theory.add_defeasible_rule(&["a"], "p");
    theory.add_defeasible_rule(&["b"], "~p");
    theory.add_defeasible_rule(&["c"], "q");
    theory.add_defeasible_rule(&["p"], "q");

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(!defeasible.contains("p"), "p should be ambiguous");
    assert!(
        defeasible.contains("q"),
        "q should be +d via r3 (independent of ambiguous p)"
    );
}

/// Five-rule star topology: one fact, five rules to different heads, some conflicting.
///
///   >> a
///   r1: a => b
///   r2: a => ~b
///   r3: a => c
///   r4: a => d
///   r5: a => ~d
///
/// b: ambiguous, c: +d, d: ambiguous.
#[test]
fn star_topology_mixed_conflicts() {
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_defeasible_rule(&["a"], "b");
    theory.add_defeasible_rule(&["a"], "~b");
    theory.add_defeasible_rule(&["a"], "c");
    theory.add_defeasible_rule(&["a"], "d");
    theory.add_defeasible_rule(&["a"], "~d");

    let conclusions = reason(&theory).unwrap();
    let defeasible = conclusion_set(&conclusions, ConclusionType::DefeasiblyProvable);

    assert!(!defeasible.contains("b"), "b should be ambiguous");
    assert!(!defeasible.contains("~b"), "~b should be ambiguous");
    assert!(defeasible.contains("c"), "c should be +d (uncontested)");
    assert!(!defeasible.contains("d"), "d should be ambiguous");
    assert!(!defeasible.contains("~d"), "~d should be ambiguous");
}
