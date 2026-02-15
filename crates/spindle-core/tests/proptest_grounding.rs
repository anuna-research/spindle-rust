//! Property-based tests for variable grounding correctness.
//!
//! Tests verify actual grounding invariants, not just "no panic":
//! - Grounded rules have no remaining variables
//! - Grounding preserves rule type
//! - Grounding is idempotent (ground twice == ground once)
//! - Ground facts are preserved through grounding
//! - Binding completeness: n facts => n ground instances
//! - Binding correctness: head args match the bound fact args
//! - Defeaters do not seed new facts

mod proptest_helpers;

use proptest::prelude::*;
use std::collections::HashSet;

use spindle_core::grounding::{ground_theory, has_variables, literal_has_variables};
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::rule::{Rule, RuleType};
use spindle_core::temporal::Temporal;
use spindle_core::theory::Theory;

use proptest_helpers::ATOMS;

// =============================================================================
// Helper: build theories with variables for grounding tests
// =============================================================================

/// Generate a theory with at least one variable rule and matching facts.
/// Returns (theory, fact_args, functor, head_name) so tests can verify outputs.
fn arb_theory_with_variables() -> impl Strategy<Value = (Theory, Vec<String>, String, String)> {
    (
        // 2-4 fact atoms to serve as bindings (deduplicated)
        proptest::collection::hash_set(
            proptest::sample::select(ATOMS).prop_map(String::from),
            2..=4,
        ),
        // A functor name for the predicate
        proptest::sample::select(ATOMS).prop_map(String::from),
        // A head atom name — must differ from functor to avoid ambiguity
        proptest::sample::select(ATOMS).prop_map(String::from),
    )
        .prop_filter("functor and head must differ", |(_, functor, head)| {
            functor != head
        })
        .prop_map(|(fact_names_set, functor, head_name)| {
            let fact_names: Vec<String> = fact_names_set.into_iter().collect();
            let mut theory = Theory::new();

            // Add ground facts: functor(alice), functor(bob), etc.
            for name in &fact_names {
                let lit = Literal::new(
                    functor.as_str(),
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    vec![name.clone()],
                );
                let label = format!("f_{functor}_{name}");
                theory.add_rule(Rule::fact(label, lit));
            }

            // Add a variable rule: functor(?x) => head(?x)
            let body_lit = Literal::new(
                functor.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string()],
            );
            let head_lit = Literal::new(
                head_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string()],
            );
            let rule = Rule::defeasible("r_var", vec![body_lit], head_lit);
            theory.add_rule(rule);

            (theory, fact_names, functor, head_name)
        })
}

/// Generate a theory with a defeater variable rule (should not seed new facts).
fn arb_theory_with_defeater_variable()
-> impl Strategy<Value = (Theory, Vec<String>, String, String)> {
    (
        proptest::collection::hash_set(
            proptest::sample::select(ATOMS).prop_map(String::from),
            2..=4,
        ),
        proptest::sample::select(ATOMS).prop_map(String::from),
        proptest::sample::select(ATOMS).prop_map(String::from),
    )
        .prop_filter("functor and head must differ", |(_, functor, head)| {
            functor != head
        })
        .prop_map(|(fact_names_set, functor, head_name)| {
            let fact_names: Vec<String> = fact_names_set.into_iter().collect();
            let mut theory = Theory::new();

            for name in &fact_names {
                let lit = Literal::new(
                    functor.as_str(),
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    vec![name.clone()],
                );
                let label = format!("f_{functor}_{name}");
                theory.add_rule(Rule::fact(label, lit));
            }

            // Defeater rule: functor(?x) ~> head(?x)
            let body_lit = Literal::new(
                functor.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string()],
            );
            let head_lit = Literal::new(
                head_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string()],
            );
            let rule = Rule::defeater("r_def", vec![body_lit], head_lit);
            theory.add_rule(rule);

            (theory, fact_names, functor, head_name)
        })
}

/// Generate a theory with a two-variable join rule.
/// facts: rel(a, b), rel(b, c)  rule: rel(?x, ?y), rel(?y, ?z) => chain(?x, ?z)
fn arb_theory_with_join() -> impl Strategy<Value = (Theory, String, String)> {
    (
        proptest::sample::select(ATOMS).prop_map(String::from),
        proptest::sample::select(ATOMS).prop_map(String::from),
    )
        .prop_filter("rel and chain must differ", |(rel, chain)| rel != chain)
        .prop_map(|(rel_name, chain_name)| {
            let mut theory = Theory::new();

            // rel(a, b) and rel(b, c)
            let lit_ab = Literal::new(
                rel_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["a_val".to_string(), "b_val".to_string()],
            );
            theory.add_rule(Rule::fact("f1", lit_ab));

            let lit_bc = Literal::new(
                rel_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["b_val".to_string(), "c_val".to_string()],
            );
            theory.add_rule(Rule::fact("f2", lit_bc));

            // rel(?x, ?y), rel(?y, ?z) => chain(?x, ?z)
            let body1 = Literal::new(
                rel_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string(), "?y".to_string()],
            );
            let body2 = Literal::new(
                rel_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?y".to_string(), "?z".to_string()],
            );
            let head = Literal::new(
                chain_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string(), "?z".to_string()],
            );
            let rule = Rule::defeasible("r_join", vec![body1, body2], head);
            theory.add_rule(rule);

            (theory, rel_name, chain_name)
        })
}

// =============================================================================
// Grounded rules have no remaining variables
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// After grounding, every rule in the output theory must be fully ground
    /// (no ?-prefixed variables remain in any literal).
    #[test]
    fn grounded_rules_have_no_variables(
        (theory, _, _, _) in arb_theory_with_variables()
    ) {
        let grounded = ground_theory(&theory);

        for rule in grounded.rules() {
            prop_assert!(
                !has_variables(rule),
                "Grounded rule '{}' still has variables: {:?}",
                rule.label, rule
            );
            for lit in &rule.body {
                prop_assert!(
                    !literal_has_variables(lit),
                    "Body literal '{}' in rule '{}' has variables",
                    lit, rule.label
                );
            }
            for lit in &rule.head {
                prop_assert!(
                    !literal_has_variables(lit),
                    "Head literal '{}' in rule '{}' has variables",
                    lit, rule.label
                );
            }
        }
    }
}

// =============================================================================
// Grounding preserves rule types
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// The rule type (Fact, Defeasible, Strict, Defeater) must be preserved
    /// through grounding. A defeasible variable rule should produce defeasible
    /// ground instances.
    #[test]
    fn grounding_preserves_rule_type(
        (theory, _, _, _) in arb_theory_with_variables()
    ) {
        let grounded = ground_theory(&theory);

        // All generated instances of "r_var" should be Defeasible
        for rule in grounded.rules() {
            if rule.label.starts_with("r_var") {
                prop_assert_eq!(
                    rule.rule_type,
                    RuleType::Defeasible,
                    "Ground instance '{}' should be Defeasible, got {:?}",
                    rule.label, rule.rule_type
                );
            }
        }

        // All original facts should remain as Facts
        for rule in grounded.rules() {
            if rule.label.starts_with("f_") {
                prop_assert_eq!(
                    rule.rule_type,
                    RuleType::Fact,
                    "Fact '{}' should remain a Fact, got {:?}",
                    rule.label, rule.rule_type
                );
            }
        }
    }
}

// =============================================================================
// Grounding is idempotent
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Grounding an already-grounded theory should produce an identical theory
    /// (no new rules generated on the second pass).
    #[test]
    fn grounding_is_idempotent(
        (theory, _, _, _) in arb_theory_with_variables()
    ) {
        let once = ground_theory(&theory);
        let twice = ground_theory(&once);

        let rules_once: HashSet<_> = once.rules()
            .map(|r| (r.label.clone(), r.rule_type))
            .collect();
        let rules_twice: HashSet<_> = twice.rules()
            .map(|r| (r.label.clone(), r.rule_type))
            .collect();

        prop_assert_eq!(
            rules_once.len(),
            rules_twice.len(),
            "Grounding twice produced {} rules vs {} rules on first pass",
            rules_twice.len(), rules_once.len()
        );
        prop_assert_eq!(rules_once, rules_twice, "Grounding should be idempotent");
    }
}

// =============================================================================
// Ground facts are preserved through grounding
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// All original ground facts must appear in the grounded theory.
    #[test]
    fn ground_facts_preserved(
        (theory, fact_args, functor, _) in arb_theory_with_variables()
    ) {
        let grounded = ground_theory(&theory);

        let grounded_fact_labels: HashSet<_> = grounded.rules()
            .filter(|r| r.rule_type == RuleType::Fact)
            .map(|r| r.label.clone())
            .collect();

        for arg in &fact_args {
            let label = format!("f_{functor}_{arg}");
            prop_assert!(
                grounded_fact_labels.contains(&label),
                "Original fact '{}' missing from grounded theory. Present: {:?}",
                label, grounded_fact_labels
            );
        }
    }
}

// =============================================================================
// Binding completeness: n distinct facts => n ground instances
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// For a theory with n distinct facts matching a single-body variable rule,
    /// grounding should produce exactly n ground instances of that rule.
    #[test]
    fn binding_completeness(
        (theory, fact_args, _, _) in arb_theory_with_variables()
    ) {
        let grounded = ground_theory(&theory);

        let ground_instances: Vec<_> = grounded.rules()
            .filter(|r| r.label.starts_with("r_var_"))
            .collect();

        prop_assert_eq!(
            ground_instances.len(),
            fact_args.len(),
            "Expected {} ground instances for {} facts, got {}. Instances: {:?}",
            fact_args.len(),
            fact_args.len(),
            ground_instances.len(),
            ground_instances.iter().map(|r| &r.label).collect::<Vec<_>>()
        );
    }
}

// =============================================================================
// Binding correctness: head args match the bound fact args
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// For rule `functor(?x) => head(?x)`, each ground instance's head
    /// argument must equal some fact's argument, and vice versa. The set of
    /// head arguments must exactly equal the set of fact arguments.
    #[test]
    fn binding_correctness(
        (theory, fact_args, functor, head_name) in arb_theory_with_variables()
    ) {
        let grounded = ground_theory(&theory);

        // Collect head args from ground instances of the variable rule
        let mut actual_head_args: HashSet<String> = HashSet::new();
        let mut actual_body_args: HashSet<String> = HashSet::new();

        for rule in grounded.rules() {
            if !rule.label.starts_with("r_var_") {
                continue;
            }

            // Head should be head_name(X) for some ground X
            for head_lit in &rule.head {
                prop_assert_eq!(
                    head_lit.name(), head_name.as_str(),
                    "Ground instance head name should be '{}', got '{}'",
                    head_name, head_lit.name()
                );
                let preds = head_lit.predicates();
                prop_assert_eq!(
                    preds.len(), 1,
                    "Ground instance head should have exactly 1 arg, got {:?}",
                    preds
                );
                actual_head_args.insert(preds[0].to_string());
            }

            // Body should be functor(X) for some ground X
            for body_lit in &rule.body {
                prop_assert_eq!(
                    body_lit.name(), functor.as_str(),
                    "Ground instance body name should be '{}', got '{}'",
                    functor, body_lit.name()
                );
                let preds = body_lit.predicates();
                prop_assert_eq!(
                    preds.len(), 1,
                    "Ground instance body should have exactly 1 arg, got {:?}",
                    preds
                );
                actual_body_args.insert(preds[0].to_string());
            }
        }

        // The head args should be exactly the fact args
        let actual_head_strs: HashSet<String> = actual_head_args.clone();
        let expected_strs: HashSet<String> = fact_args.iter().cloned().collect();
        prop_assert_eq!(
            expected_strs.clone(), actual_head_strs,
            "Head args should match fact args exactly"
        );

        // Body args should also be exactly the fact args (same binding ?x)
        let actual_body_strs: HashSet<String> = actual_body_args.clone();
        prop_assert_eq!(
            expected_strs, actual_body_strs,
            "Body args should match fact args exactly"
        );

        // Each ground instance's head arg should equal its body arg (same ?x binding)
        for rule in grounded.rules() {
            if !rule.label.starts_with("r_var_") {
                continue;
            }
            let head_arg = rule.head[0].predicates()[0];
            let body_arg = rule.body[0].predicates()[0];
            prop_assert_eq!(
                head_arg, body_arg,
                "In rule '{}', head arg '{}' != body arg '{}' (same ?x should bind to same value)",
                rule.label, head_arg, body_arg
            );
        }
    }
}

// =============================================================================
// Grounding is deterministic
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Grounding the same theory twice should produce identical rules
    /// (same labels, same rule types, same head/body literals).
    #[test]
    fn grounding_deterministic(
        (theory, _, _, _) in arb_theory_with_variables()
    ) {
        let g1 = ground_theory(&theory);
        let g2 = ground_theory(&theory);

        let set1: HashSet<_> = g1.rules()
            .map(|r| (r.label.clone(), r.rule_type, format!("{:?}", r.head), format!("{:?}", r.body)))
            .collect();
        let set2: HashSet<_> = g2.rules()
            .map(|r| (r.label.clone(), r.rule_type, format!("{:?}", r.head), format!("{:?}", r.body)))
            .collect();

        prop_assert_eq!(set1, set2, "Grounding should be deterministic");
    }
}

// =============================================================================
// Predicate argument discrimination
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Two literals with different predicate arguments should NOT be equal.
    #[test]
    fn predicate_args_discriminate(
        name in proptest::sample::select(ATOMS),
        arg1 in proptest::sample::select(&["alice", "bob", "carol"]).prop_map(String::from),
        arg2 in proptest::sample::select(&["dave", "eve", "frank"]).prop_map(String::from),
    ) {
        let lit1 = Literal::new(name, false, Mode::empty(), Temporal::empty(), vec![arg1.clone()]);
        let lit2 = Literal::new(name, false, Mode::empty(), Temporal::empty(), vec![arg2.clone()]);

        prop_assert_ne!(
            lit1, lit2,
            "Literals with different predicate args should not be equal: {} vs {}",
            arg1, arg2
        );
    }
}

// =============================================================================
// Defeaters do not seed new facts for further grounding
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Defeater rules should produce ground instances but their heads should
    /// NOT appear as facts that seed further grounding rounds.
    #[test]
    fn defeaters_do_not_seed_facts(
        (theory, fact_args, _functor, head_name) in arb_theory_with_defeater_variable()
    ) {
        let grounded = ground_theory(&theory);

        // Defeater instances should exist
        let defeater_instances: Vec<_> = grounded.rules()
            .filter(|r| r.rule_type == RuleType::Defeater)
            .collect();
        prop_assert_eq!(
            defeater_instances.len(),
            fact_args.len(),
            "Expected {} defeater instances, got {}",
            fact_args.len(), defeater_instances.len()
        );

        // No facts with head_name should exist (defeaters don't create facts)
        let head_facts: Vec<_> = grounded.rules()
            .filter(|r| r.rule_type == RuleType::Fact && r.head_literal().name() == head_name.as_str())
            .collect();
        prop_assert!(
            head_facts.is_empty(),
            "Defeater heads should not appear as facts, but found: {:?}",
            head_facts.iter().map(|r| &r.label).collect::<Vec<_>>()
        );
    }
}

// =============================================================================
// Join variable correctness: multi-body rules bind shared variables correctly
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// For a join rule `rel(?x, ?y), rel(?y, ?z) => chain(?x, ?z)` with facts
    /// rel(a, b) and rel(b, c), grounding should produce chain(a, c) because
    /// ?y must bind consistently to the same value across body literals.
    #[test]
    fn join_variable_consistency(
        (theory, _, _chain_name) in arb_theory_with_join()
    ) {
        let grounded = ground_theory(&theory);

        let join_instances: Vec<_> = grounded.rules()
            .filter(|r| r.label.starts_with("r_join_"))
            .collect();

        // Should produce exactly one ground instance: chain(a_val, c_val)
        // because ?y must bind to b_val in both body literals
        prop_assert_eq!(
            join_instances.len(), 1,
            "Expected 1 join instance, got {}. Instances: {:?}",
            join_instances.len(),
            join_instances.iter().map(|r| format!("{}: {:?} => {:?}", r.label, r.body, r.head)).collect::<Vec<_>>()
        );

        let instance = &join_instances[0];
        let head_preds = instance.head[0].predicates();
        prop_assert_eq!(head_preds.len(), 2, "chain should have 2 args");
        prop_assert_eq!(
            head_preds[0], "a_val",
            "chain first arg should be a_val (bound from ?x), got {}",
            head_preds[0]
        );
        prop_assert_eq!(
            head_preds[1], "c_val",
            "chain second arg should be c_val (bound from ?z), got {}",
            head_preds[1]
        );

        // Verify body literals have consistent ?y binding
        let body1_preds = instance.body[0].predicates();
        let body2_preds = instance.body[1].predicates();
        prop_assert_eq!(
            body1_preds[1], body2_preds[0],
            "?y should bind consistently: body1 arg2='{}' != body2 arg1='{}'",
            body1_preds[1], body2_preds[0]
        );
    }
}
