//! SPEC-025 TEST-009/010: constrained aggregate lowering and checked arithmetic.
use spindle_core::{ConclusionType, Rule, aggregation};
use spindle_parser::parse_spl;
use spindle_zk::{Claim, Policy, Program, Tag, parse_facts};
use std::collections::BTreeSet;

#[test]
fn aggregate_membership_requires_snapshot_evidence() {
    let theory = parse_spl(
        "(aggregate-domain 0 1)
        (normally total (bind ?n (fold + 1 :from row :initial 0)) (total ?n))",
    )
    .unwrap();
    let inputs = parse_facts("(given row)").unwrap();
    let program = Program::compile(&theory, &inputs).unwrap();
    let absent = program.evaluate(&[]).unwrap();
    let present = program.evaluate(&inputs).unwrap();
    let zero = parse_facts("(given (total 0))").unwrap().remove(0);
    let one = parse_facts("(given (total 1))").unwrap().remove(0);
    assert!(
        absent.contains(&(zero, ConclusionType::DefeasiblyProvable)),
        "absent row was counted"
    );
    assert!(
        !absent.contains(&(one.clone(), ConclusionType::DefeasiblyProvable)),
        "fabricated row established a sum"
    );
    assert!(present.contains(&(one, ConclusionType::DefeasiblyProvable)));
}

fn compare(source: &str, schema: &str) {
    let theory = parse_spl(source).unwrap();
    let inputs = parse_facts(schema).unwrap();
    let program = Program::compile(&theory, &inputs).unwrap();
    for mask in 0..(1 << inputs.len()) {
        let selected: Vec<_> = inputs
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, l)| l.clone())
            .collect();
        let actual = program.evaluate(&selected).unwrap();
        let mut instance = theory.clone();
        for (i, l) in selected.iter().enumerate() {
            instance.add_rule(Rule::fact(format!("input_{i}"), l.clone()));
        }
        let reference = aggregation::source::program(&instance).unwrap();
        let expected =
            aggregation::evaluate(&reference, theory.aggregate_domain().unwrap()).unwrap();
        let mentioned: BTreeSet<_> = expected
            .conclusions
            .iter()
            .map(|c| c.atom.clone())
            .collect();
        let actual: BTreeSet<_> = actual
            .iter()
            .filter_map(|(l, t)| {
                let atom = aggregation::Pattern {
                    name: l.name().into(),
                    negation: l.negation,
                    args: l
                        .predicate_args()
                        .iter()
                        .map(aggregation::source::term)
                        .collect::<Result<_, _>>()
                        .unwrap(),
                };
                mentioned
                    .contains(&atom)
                    .then_some((atom, t.symbol().to_owned()))
            })
            .collect();
        let expected: BTreeSet<_> = expected
            .conclusions
            .into_iter()
            .map(|c| (c.atom, c.tag.symbol().to_owned()))
            .collect();
        assert_eq!(actual, expected, "mask {mask}: {source}");
    }
}

#[test]
fn private_aggregate_rows_match_reference() {
    for name in ["sum", "count", "min-of", "max-of"] {
        compare(
            &format!(
                "(aggregate-domain 0 1 2 3) (normally total (agg ?n {name} ?v (row ?v)) (total ?n))"
            ),
            "(given (row 1)) (given (row 2))",
        );
    }
}

#[test]
fn defeat_deduplication_and_chained_strata_match_reference() {
    compare(
        "(aggregate-domain 0 1 2 3)
        (normally row-a a (row 1)) (normally row-b b (row 1))
        (normally attack c (not (row 1))) (prefer attack row-a) (prefer attack row-b)
        (normally total (agg ?n count ?v (row ?v)) (total ?n))
        (normally next (agg ?n sum ?v (total ?v)) (next ?n))",
        "(given a) (given b) (given c)",
    );
}

#[test]
fn grouping_and_strict_aggregate_strength_match_reference() {
    compare(
        "(aggregate-domain 0 1 2 3)
        (always total (agg ?n sum ?v (row ?group ?v)) (total ?group ?n))",
        "(given (row 0 1)) (given (row 1 2))",
    );
    let p =
        parse_spl("(aggregate-domain 0 1) (always total (agg ?n count ?v (row ?v)) (total ?n))")
            .unwrap();
    let tags = Program::compile(&p, &[]).unwrap().evaluate(&[]).unwrap();
    assert!(tags.iter().any(|(l, t)| l.name() == "total"
        && l.predicate_args() == [spindle_core::term::Term::Integer(0)]
        && *t == ConclusionType::DefeasiblyProvable));
    assert!(
        !tags
            .iter()
            .any(|(l, t)| l.name() == "total" && *t == ConclusionType::DefinitelyProvable)
    );
}

#[test]
fn overflow_and_out_of_domain_results_fail_closed() {
    for (domain, schema) in [
        (
            "0 1 9223372036854775807",
            "(given (row 9223372036854775807)) (given (row 1))",
        ),
        ("0 1 2", "(given (row 1)) (given (row 2))"),
    ] {
        let source = format!(
            "(aggregate-domain {domain}) (normally total (agg ?n sum ?v (row ?v)) (total ?n))"
        );
        let inputs = parse_facts(schema).unwrap();
        let program = Program::compile(&parse_spl(&source).unwrap(), &inputs).unwrap();
        assert!(program.evaluate(&inputs).is_err());
        assert!(program.evaluate(&[]).is_ok());
    }
}

#[test]
fn aggregate_actual_proof_roundtrip() {
    let policy = Policy::compile(
        "(aggregate-domain 0 1 2 3)
        (normally total (agg ?n sum ?v (row ?v)) (total ?n))",
        "(given (row 1)) (given (row 2))",
    )
    .unwrap();
    let claim = Claim::new("(total 3)", Tag::Defeasibly).unwrap();
    assert!(
        policy
            .prove(&parse_facts("(given (row 1))").unwrap(), &claim)
            .is_err()
    );
    let proof = policy
        .prove(
            &parse_facts("(given (row 1)) (given (row 2))").unwrap(),
            &claim,
        )
        .unwrap();
    policy.verify(&proof, &claim).unwrap();
    assert!(
        policy
            .verify(&proof, &Claim::new("(total 2)", Tag::Defeasibly).unwrap())
            .is_err()
    );
}

#[test]
fn malformed_typed_fact_is_rejected_on_aggregate_path() {
    let mut theory =
        parse_spl("(aggregate-domain 0 1) (normally total (agg ?n count ?v (row ?v)) (total ?n))")
            .unwrap();
    let mut malformed = Rule::strict(
        "malformed",
        vec![spindle_core::Literal::simple("missing")],
        spindle_core::Literal::simple("q"),
    );
    malformed.rule_type = spindle_core::RuleType::Fact;
    theory.add_rule(malformed);
    assert!(Program::compile(&theory, &[]).is_err());
    let source = aggregation::source::program(&theory).unwrap();
    assert!(aggregation::evaluate(&source, theory.aggregate_domain().unwrap()).is_err());
}

#[test]
fn symbol_binding_does_not_invalidate_numeric_aggregate() {
    let theory = parse_spl(
        "(aggregate-domain alice 0 1)
        (normally result (and (bind ?who alice) (agg ?n count ?v (row ?v))) (result ?who ?n))",
    )
    .unwrap();
    let program = Program::compile(&theory, &[]).unwrap();
    let conclusions = program.evaluate(&[]).unwrap();
    let wanted = parse_facts("(given (result alice 0))").unwrap();
    assert!(conclusions.contains(&(wanted[0].clone(), ConclusionType::DefeasiblyProvable)));
}

#[test]
fn singleton_domain_does_not_bypass_grounding_work_limit() {
    let args = (0..17)
        .map(|i| format!("?x{i}"))
        .collect::<Vec<_>>()
        .join(" ");
    let source = format!(
        "(aggregate-domain 0) (normally wide (and) (wide {args})) (normally total (agg ?n count ?v (row ?v)) (total ?n))"
    );
    assert!(matches!(
        Program::compile(&parse_spl(&source).unwrap(), &[]),
        Err(spindle_zk::Error::ResourceLimit(_))
    ));
}

#[test]
fn unsupported_functions_are_rejected_before_private_selection() {
    for expression in [
        "(fold custom-reducer ?v :from (row ?v) :initial 0)",
        "(fold + (custom-extract ?v) :from (row ?v) :initial 0)",
    ] {
        let source =
            format!("(aggregate-domain 0 1) (normally total (bind ?n {expression}) (total ?n))");
        if expression.contains("custom-extract") {
            parse_spl(&source).expect("custom calls must reach proof-profile validation");
        }
        for schema in ["", "(given (row 1))"] {
            assert!(
                matches!(
                    Policy::compile(&source, schema),
                    Err(spindle_zk::Error::Unsupported(_) | spindle_zk::Error::Parse(_))
                ),
                "accepted {source}"
            );
        }
    }
    let source = "(aggregate-domain 0 1) (normally total (and (bind ?x (custom-bind 0)) (agg ?n count ?v (row ?v))) (total ?n))";
    for schema in ["", "(given (row 1))"] {
        assert!(matches!(
            Policy::compile(source, schema),
            Err(spindle_zk::Error::Unsupported(_))
        ));
    }
}

#[test]
fn aggregate_input_schema_respects_predicate_arity_limit() {
    let source = "(aggregate-domain 0 1) (normally total (agg ?n count ?v (row ?v)) (total ?n))";
    let args = vec!["0"; 17].join(" ");
    let schema = format!("(given (unrelated {args}))");
    assert!(matches!(
        Policy::compile(source, &schema),
        Err(spindle_zk::Error::ResourceLimit(_))
    ));
}

#[test]
fn seeds_nonempty_and_undecided_cycles_match_reference() {
    for source in [
        "(normally total (bind ?n (fold + ?v :from (row ?v) :initial 1)) (total ?n))",
        "(normally total (bind ?n (fold min ?v :from (row ?v) :require-nonempty)) (total ?n))",
        "(normally cycle (row 1) (row 1)) (normally total (agg ?n count ?v (row ?v)) (total ?n))",
        "(always cycle (not (row 1)) (not (row 1))) (normally row (and) (row 1)) (normally total (agg ?n count ?v (row ?v)) (total ?n))",
    ] {
        compare(
            &format!("(aggregate-domain 0 1 2 3 4) {source}"),
            "(given (row 1)) (given (row 2))",
        );
    }
}
