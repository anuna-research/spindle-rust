use spindle_core::Rule;
use spindle_core::literal::Literal;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::theory::Theory;

#[test]
fn test_grounded_instance_resolves_template_metadata() {
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
            vec!["?x".to_string()],
        )],
        Literal::new(
            "q",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string()],
        ),
    ));

    theory.add_meta_string("r1", "description", "template metadata");

    let prepared = prepare(&theory, PrepareOptions::default()).unwrap();

    assert!(
        prepared.theory.get_meta("r1").is_some(),
        "Expected grounded theory to preserve template metadata for r1"
    );
}
