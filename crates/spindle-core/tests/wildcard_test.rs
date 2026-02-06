use spindle_core::literal::Literal;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::rule::Rule;
use spindle_core::theory::Theory;

#[test]
fn test_wildcard_rewriting() {
    let mut theory = Theory::new();

    // Fact: p(a, b)
    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["a".to_string(), "b".to_string()],
        ),
    ));

    // Rule: p(_, _) => q(found)
    // This uses wildcards in the body. If they are treated as constants "_",
    // they won't match "a" or "b".
    // If they are rewritten to variables, they should match.
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["_".to_string(), "_".to_string()],
        )],
        Literal::simple("q_found"),
    ));

    // Rule 2: p(a, _) => q(partial)
    theory.add_rule(Rule::defeasible(
        "r2",
        vec![Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["a".to_string(), "_".to_string()],
        )],
        Literal::simple("q_partial"),
    ));

    let result = prepare(&theory, PrepareOptions::default()).unwrap();
    let conclusions = result.theory.reason().unwrap();

    let found = conclusions
        .iter()
        .any(|c| c.literal.name() == "q_found" && c.is_positive());
    let partial = conclusions
        .iter()
        .any(|c| c.literal.name() == "q_partial" && c.is_positive());

    assert!(found, "Wildcard rule p(_, _) => q_found did not fire!");
    assert!(partial, "Wildcard rule p(a, _) => q_partial did not fire!");
}

#[test]
fn test_wildcard_independence() {
    // Verify that multiple wildcards do not bind to each other
    let mut theory = Theory::new();

    // Fact: p(a, b) where a != b
    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["a".to_string(), "b".to_string()],
        ),
    ));

    // Rule: p(?x, ?x) => same
    // This should NOT fire for p(a, b)
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string(), "?x".to_string()],
        )],
        Literal::simple("same"),
    ));

    // Rule: p(_, _) => diff
    // This SHOULD fire because _ are independent
    theory.add_rule(Rule::defeasible(
        "r2",
        vec![Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["_".to_string(), "_".to_string()],
        )],
        Literal::simple("diff"),
    ));

    let result = prepare(&theory, PrepareOptions::default()).unwrap();
    let conclusions = result.theory.reason().unwrap();

    let same = conclusions.iter().any(|c| c.literal.name() == "same");
    let diff = conclusions.iter().any(|c| c.literal.name() == "diff");

    assert!(
        !same,
        "Variable rule p(?x, ?x) incorrectly fired for p(a,b)"
    );
    assert!(diff, "Wildcard rule p(_, _) failed to fire for p(a,b)");
}
