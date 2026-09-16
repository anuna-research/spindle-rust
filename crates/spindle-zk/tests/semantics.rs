//! SPEC-025 TEST-001 / TEST-006: independent engine comparison and profile checks.
use spindle_core::{conclusion::ConclusionType, literal::Literal, rule::Rule};
use spindle_parser::parse_spl;
use spindle_zk::Program;
use std::collections::BTreeSet;

fn assert_agrees(source: &str, inputs: &[Literal], selected: &[Literal]) {
    let policy = parse_spl(source).unwrap();
    let compiled = Program::compile(&policy, inputs).unwrap();
    let actual = compiled.evaluate(selected).unwrap();
    let mut theory = policy.clone();
    for (i, literal) in selected.iter().enumerate() {
        theory.add_rule(Rule::fact(format!("zk_input_{i}"), literal.clone()));
    }
    let expected = spindle_core::reason::reason(&theory).unwrap();
    let expected: BTreeSet<_> = expected
        .into_iter()
        .map(|c| (c.literal.to_spl(), c.conclusion_type.symbol().to_string()))
        .collect();
    // Compare atoms present in the instantiated reference theory. The compiler
    // additionally tracks absent candidates from its fixed input universe.
    let mentioned: BTreeSet<_> = theory
        .rules()
        .flat_map(|r| {
            r.head
                .iter()
                .cloned()
                .chain(r.body.iter().filter_map(|b| match b {
                    spindle_core::body::BodyLiteral::Logic(l) => Some(l.to_literal()),
                    _ => None,
                }))
                .map(|l| l.to_spl())
                .collect::<Vec<_>>()
        })
        .collect();
    let actual: BTreeSet<_> = actual
        .into_iter()
        .filter(|(l, _)| mentioned.contains(&l.to_spl()))
        .map(|(l, t)| (l.to_spl(), t.symbol().to_string()))
        .collect();
    let expected: BTreeSet<_> = expected
        .into_iter()
        .filter(|(l, _)| mentioned.contains(l))
        .collect();
    assert_eq!(actual, expected, "source: {source}; facts: {selected:?}");
}

#[test]
fn traditional_four_tag_regressions() {
    for source in [
        "(normally r () p)",
        "(always r p p)",
        "(normally r p p)",
        "(normally r () p) (normally s () (not p))",
        "(given p) (given (not p))",
        "(always r (not q) (not q)) (normally s () q)",
        "(normally r () p) (normally s () (not p)) (except a p (not q)) (normally b () q)",
        "(normally r () p) (normally s () (not p)) (prefer r s)",
        "(except r () p)",
        "(always r p q) (always s q p)",
        "(normally r () p) (normally s () (not p)) (normally t () p) (prefer r s) (prefer s t)",
        "(normally r a p) (normally s b (not p)) (prefer r s)",
    ] {
        assert_agrees(source, &[], &[]);
    }
}

#[test]
fn private_inputs_are_presence_bits_not_negative_proofs() {
    let source = "(normally r bird flies) (normally s penguin (not flies)) (prefer s r)";
    let inputs = [Literal::simple("bird"), Literal::simple("penguin")];
    for mask in 0..4 {
        let selected: Vec<_> = inputs
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, l)| l.clone())
            .collect();
        assert_agrees(source, &inputs, &selected);
    }
}

#[test]
fn explicit_negation_is_an_independent_private_input() {
    let inputs = [Literal::simple("p"), Literal::negated("p")];
    for mask in 0..4 {
        let selected: Vec<_> = inputs
            .iter()
            .enumerate()
            .filter(|(i, _)| mask & (1 << i) != 0)
            .map(|(_, l)| l.clone())
            .collect();
        assert_agrees(
            "(always r p q) (always s (not p) (not q))",
            &inputs,
            &selected,
        );
    }
}

#[test]
fn undeclared_private_facts_are_rejected() {
    let policy = parse_spl("(normally r p q)").unwrap();
    let compiled = Program::compile(&policy, &[Literal::simple("p")]).unwrap();
    assert!(compiled.evaluate(&[Literal::simple("q")]).is_err());
}

#[test]
fn unseeded_strict_cycle_has_no_constructive_negative() {
    let compiled = Program::compile(&parse_spl("(always r p p)").unwrap(), &[]).unwrap();
    let conclusions = compiled.evaluate(&[]).unwrap();
    assert!(!conclusions.iter().any(|(l, t)| l == &Literal::simple("p")
        && matches!(
            t,
            ConclusionType::DefinitelyNotProvable | ConclusionType::DefeasiblyNotProvable
        )));
}

#[test]
fn unsupported_source_never_silently_loses_constraints() {
    for source in [
        "(normally r (bird ?x) (flies ?x))",
        "(normally r (must p) q)",
        "(normally r (> 2 1) q)",
    ] {
        let policy = parse_spl(source).unwrap();
        assert!(Program::compile(&policy, &[]).is_err());
    }
}

#[test]
fn numeric_symbols_do_not_satisfy_integer_premises() {
    use spindle_core::{intern::intern, mode::Mode, temporal::Temporal, term::Term};
    let symbol = Literal::from_ids(
        intern("p"),
        false,
        Mode::empty(),
        Temporal::empty(),
        vec![Term::Symbol(intern("1"))],
    );
    let integer = Literal::from_ids(
        intern("p"),
        false,
        Mode::empty(),
        Temporal::empty(),
        vec![Term::Integer(1)],
    );
    let mut theory = spindle_core::Theory::new();
    theory.add_rule(Rule::fact("symbol", symbol.clone()));
    theory.add_rule(Rule::strict(
        "rule",
        vec![integer.clone()],
        Literal::simple("q"),
    ));
    let program = Program::compile(&theory, &[symbol, integer]).unwrap();
    let tags = program.evaluate(&[]).unwrap();
    assert!(
        !tags
            .iter()
            .any(|(l, tag)| l == &Literal::simple("q") && tag.is_positive())
    );
}

#[test]
fn temporal_witness_does_not_match_an_ordinary_input() {
    let p = Literal::simple("p");
    let policy = Program::compile(
        &parse_spl("(normally r p q)").unwrap(),
        std::slice::from_ref(&p),
    )
    .unwrap();
    let mut temporal = p;
    temporal.interval_var = Some(spindle_core::intern::intern("?time"));
    assert!(policy.evaluate(&[temporal]).is_err());
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(96))]
    #[test]
    fn generated_rules_match_the_existing_engine(
        choices in proptest::collection::vec((0u8..3, 0u8..6, 0u8..7), 0..8),
        facts in 0u8..16,
    ) {
        let names = ["a", "(not a)", "b", "(not b)", "c", "(not c)"];
        let mut source = String::new();
        for (i, (kind, head, body)) in choices.iter().enumerate() {
            let operator = ["always", "normally", "except"][*kind as usize];
            let premise = if *body == 6 { "()" } else { names[*body as usize] };
            source.push_str(&format!("({operator} r{i} {premise} {})\n", names[*head as usize]));
        }
        let inputs = [Literal::simple("a"), Literal::negated("a"), Literal::simple("b"), Literal::negated("b")];
        let selected: Vec<_> = inputs.iter().enumerate().filter(|(i, _)| facts & (1 << i) != 0)
            .map(|(_, l)| l.clone()).collect();
        assert_agrees(&source, &inputs, &selected);
    }
}
