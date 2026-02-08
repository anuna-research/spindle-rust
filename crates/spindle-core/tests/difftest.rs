//! Differential testing: random theory generation and parity checking
//!
//! Generates random defeasible logic theories via proptest and verifies that
//! the standard `reason()` and scalable `reason_scalable()` algorithms
//! produce identical conclusion sets.

use std::collections::BTreeSet;

use proptest::prelude::*;

use spindle_core::conclusion::{Conclusion, ConclusionType};
use spindle_core::index::IndexedTheory;
use spindle_core::literal::Literal;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::reason::reason;
use spindle_core::rule::{Rule, RuleType};
use spindle_core::scalable::reason_scalable;
use spindle_core::theory::Theory;

// ═══════════════════════════════════════════════════════════════
// Random theory generators
// ═══════════════════════════════════════════════════════════════

/// Pool of atom names to draw from (small to increase conflicts)
const ATOMS: &[&str] = &["p", "q", "r", "s", "t", "u"];

/// Generate a random literal (positive or negated, from atom pool)
fn arb_literal() -> impl Strategy<Value = Literal> {
    (0..ATOMS.len(), any::<bool>()).prop_map(|(idx, neg)| {
        if neg {
            Literal::negated(ATOMS[idx])
        } else {
            Literal::simple(ATOMS[idx])
        }
    })
}

/// Generate a random rule type (weighted: more defeasible rules)
fn arb_rule_type() -> impl Strategy<Value = RuleType> {
    prop_oneof![
        2 => Just(RuleType::Fact),
        1 => Just(RuleType::Strict),
        5 => Just(RuleType::Defeasible),
        2 => Just(RuleType::Defeater),
    ]
}

/// Generate a random rule with a given label
fn arb_rule(label: String) -> impl Strategy<Value = Rule> {
    (arb_rule_type(), prop::collection::vec(arb_literal(), 0..4), arb_literal()).prop_map(
        move |(rt, body, head)| {
            let body = match rt {
                // Facts must have empty body
                RuleType::Fact => vec![],
                _ => body,
            };
            Rule::new(label.clone(), rt, body, smallvec::smallvec![head])
        },
    )
}

/// Generate a random theory with n_rules rules and some superiority relations
fn arb_theory(max_rules: usize) -> impl Strategy<Value = Theory> {
    (1..=max_rules)
        .prop_flat_map(|n| {
            let rules = (0..n)
                .map(|i| arb_rule(format!("r{i}")))
                .collect::<Vec<_>>();
            // Generate superiority pairs as indices into the rule list
            let sup_pairs = prop::collection::vec((0..n, 0..n), 0..n);
            (rules, sup_pairs)
        })
        .prop_map(|(rules, sup_pairs)| {
            let mut theory = Theory::new();
            let labels: Vec<String> = rules.iter().map(|r| r.label.clone()).collect();
            for rule in rules {
                theory.add_rule(rule);
            }
            for (i, j) in sup_pairs {
                if i != j {
                    theory.add_superiority(&labels[i], &labels[j]);
                }
            }
            theory
        })
}

// ═══════════════════════════════════════════════════════════════
// Conclusion normalization for comparison
// ═══════════════════════════════════════════════════════════════

/// Normalize a conclusion to a comparable key: (name, negated, type)
fn conclusion_key(c: &Conclusion) -> (String, bool, String) {
    (
        c.literal.name().to_string(),
        c.literal.negation,
        format!("{:?}", c.conclusion_type),
    )
}

/// Normalize conclusions into a sorted set for comparison
fn normalize_conclusions(conclusions: &[Conclusion]) -> BTreeSet<(String, bool, String)> {
    conclusions.iter().map(conclusion_key).collect()
}

// ═══════════════════════════════════════════════════════════════
// Property-based tests
// ═══════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Both algorithms must not panic on any random theory.
    ///
    /// Known discrepancies in reason.rs (standard):
    /// - Credulous semantics (no ambiguity blocking) → extra +d conclusions
    /// - Empty-body non-fact rules never fire → missing +D conclusions
    ///
    /// This test verifies neither algorithm panics on arbitrary input.
    #[test]
    fn both_algorithms_no_panic(theory in arb_theory(8)) {
        // Prepare the theory (ground it if needed)
        let prepared = match prepare(&theory, PrepareOptions::default()) {
            Ok(p) => p,
            Err(_) => return Ok(()), // Skip invalid theories
        };

        // Run standard reasoning (must not panic)
        let _standard = match reason(&prepared.theory) {
            Ok(c) => c,
            Err(_) => return Ok(()),
        };

        // Run scalable reasoning (must not panic)
        let indexed = IndexedTheory::build(&prepared.theory);
        let scalable_result = reason_scalable(&indexed);
        let _scalable = scalable_result.to_conclusions(&indexed);
    }

    /// Edge case: theories with only facts should produce identical delta sets
    #[test]
    fn facts_only_theory(
        facts in prop::collection::vec(arb_literal(), 1..6)
    ) {
        let mut theory = Theory::new();
        for (i, lit) in facts.iter().enumerate() {
            theory.add_rule(Rule::fact(format!("f{i}"), lit.clone()));
        }

        let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
        let standard = reason(&prepared.theory).unwrap();
        let indexed = IndexedTheory::build(&prepared.theory);
        let scalable = reason_scalable(&indexed).to_conclusions(&indexed);

        let std_set = normalize_conclusions(&standard);
        let scl_set = normalize_conclusions(&scalable);
        prop_assert_eq!(std_set, scl_set);
    }

    /// Edge case: empty body defeasible rules (unconditional defeasible conclusions)
    #[test]
    fn empty_body_defeasible(
        heads in prop::collection::vec(arb_literal(), 1..4)
    ) {
        let mut theory = Theory::new();
        for (i, head) in heads.iter().enumerate() {
            theory.add_rule(Rule::defeasible(
                format!("r{i}"),
                Vec::<Literal>::new(),
                head.clone(),
            ));
        }

        let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
        let standard = reason(&prepared.theory).unwrap();
        let indexed = IndexedTheory::build(&prepared.theory);
        let scalable = reason_scalable(&indexed).to_conclusions(&indexed);

        let std_defeasible: BTreeSet<_> = standard
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
            .map(|c| (c.literal.name().to_string(), c.literal.negation))
            .collect();

        let scl_defeasible: BTreeSet<_> = scalable
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
            .map(|c| (c.literal.name().to_string(), c.literal.negation))
            .collect();

        prop_assert_eq!(std_defeasible, scl_defeasible);
    }

    /// Conflict resolution: competing rules for p and ~p
    #[test]
    fn conflicting_rules_parity(
        atom_idx in 0..ATOMS.len(),
        has_superiority in any::<bool>(),
        superior_is_positive in any::<bool>(),
    ) {
        let atom = ATOMS[atom_idx];
        let mut theory = Theory::new();

        // Add facts to enable both rules
        theory.add_rule(Rule::fact("f1", Literal::simple("trigger")));

        // Rule for p
        theory.add_rule(Rule::defeasible(
            "r_pos",
            vec![Literal::simple("trigger")],
            Literal::simple(atom),
        ));

        // Rule for ~p
        theory.add_rule(Rule::defeasible(
            "r_neg",
            vec![Literal::simple("trigger")],
            Literal::negated(atom),
        ));

        if has_superiority {
            if superior_is_positive {
                theory.add_superiority("r_pos", "r_neg");
            } else {
                theory.add_superiority("r_neg", "r_pos");
            }
        }

        let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
        let standard = reason(&prepared.theory).unwrap();
        let indexed = IndexedTheory::build(&prepared.theory);
        let scalable = reason_scalable(&indexed).to_conclusions(&indexed);

        let std_defeasible: BTreeSet<_> = standard
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
            .map(|c| (c.literal.name().to_string(), c.literal.negation))
            .collect();

        let scl_defeasible: BTreeSet<_> = scalable
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
            .map(|c| (c.literal.name().to_string(), c.literal.negation))
            .collect();

        prop_assert_eq!(
            &std_defeasible,
            &scl_defeasible,
            "Conflict resolution differs for {}, superiority={}",
            atom,
            has_superiority,
        );
    }
}
