use spindle_core::Rule;
use spindle_core::literal::Literal;
use spindle_core::query::{HypotheticalClaim, abduce, requires, what_if};
use spindle_core::theory::Theory;

#[test]
fn test_what_if_does_not_collapse_predicate_args() {
    let mut theory = Theory::new();

    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["a".to_string()],
        ),
    ));

    let hypothetical = HypotheticalClaim::new(Literal::new(
        "p",
        false,
        Default::default(),
        Default::default(),
        vec!["b".to_string()],
    ));

    let goal = Literal::new(
        "p",
        false,
        Default::default(),
        Default::default(),
        vec!["b".to_string()],
    );

    let result = what_if(&theory, vec![hypothetical], &goal).unwrap();

    assert!(
        result.new_conclusions.iter().any(|lit| lit == &goal),
        "Expected p(b) to be treated as a new conclusion distinct from p(a)"
    );
}

#[test]
fn test_why_not_missing_body_uses_full_literal_identity() {
    let mut theory = Theory::new();

    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["a".to_string()],
        ),
    ));

    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["b".to_string()],
        )],
        Literal::simple("q"),
    ));

    let goal = Literal::simple("q");
    let result = spindle_core::query::why_not(&theory, &goal).unwrap();

    let missing = result.get_missing_premises();
    assert!(
        missing
            .iter()
            .any(|lit| lit.name() == "p" && lit.predicates() == vec!["b".to_string()]),
        "Expected p(b) to be reported as missing, not satisfied by p(a)"
    );
}

#[test]
fn test_abduce_requires_respects_predicate_args() {
    let mut theory = Theory::new();

    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["a".to_string()],
        ),
    ));

    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["b".to_string()],
        )],
        Literal::simple("q"),
    ));

    let goal = Literal::simple("q");
    let abducted = abduce(&theory, &goal, 1).unwrap();
    let required = requires(&theory, &goal).unwrap();

    let expected = Literal::new(
        "p",
        false,
        Default::default(),
        Default::default(),
        vec!["b".to_string()],
    );

    assert!(
        abducted
            .solutions
            .iter()
            .any(|s| s.facts.contains(&expected)),
        "Expected abduction to require p(b) rather than treating p(a) as sufficient"
    );

    assert!(
        required.iter().any(|lit| lit == &expected),
        "Expected requires() to include p(b) as the missing fact"
    );
}
