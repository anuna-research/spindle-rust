use spindle_core::Rule;
use spindle_core::literal::Literal;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::reason::reason;
use spindle_core::scalable::reason_scalable;
use spindle_core::theory::Theory;

#[test]
fn test_grounded_superiority_parity_standard_vs_scalable() {
    let mut theory = Theory::new();

    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "bird",
            false,
            Default::default(),
            Default::default(),
            vec!["tweety".to_string()],
        ),
    ));
    theory.add_rule(Rule::fact(
        "f2",
        Literal::new(
            "penguin",
            false,
            Default::default(),
            Default::default(),
            vec!["tweety".to_string()],
        ),
    ));

    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::new(
            "bird",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string()],
        )],
        Literal::new(
            "flies",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string()],
        ),
    ));

    theory.add_rule(Rule::defeasible(
        "r2",
        vec![Literal::new(
            "penguin",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string()],
        )],
        Literal::new(
            "flies",
            true,
            Default::default(),
            Default::default(),
            vec!["?x".to_string()],
        ),
    ));

    theory.add_superiority("r2", "r1");

    let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
    let standard = reason(&prepared.theory).unwrap();

    let indexed = spindle_core::index::IndexedTheory::build(&prepared.theory);
    let scalable = reason_scalable(&indexed);

    let neg_flies = Literal::new(
        "flies",
        true,
        Default::default(),
        Default::default(),
        vec!["tweety".to_string()],
    );
    let pos_flies = Literal::new(
        "flies",
        false,
        Default::default(),
        Default::default(),
        vec!["tweety".to_string()],
    );

    let standard_has_neg = standard
        .iter()
        .any(|c| c.conclusion_type.is_positive() && c.literal == neg_flies);
    let standard_has_pos = standard
        .iter()
        .any(|c| c.conclusion_type.is_positive() && c.literal == pos_flies);

    let neg_id = indexed
        .get_lit_id(&neg_flies)
        .expect("negated flies literal should be indexed");
    let pos_id = indexed
        .get_lit_id(&pos_flies)
        .expect("positive flies literal should be indexed");

    let scalable_has_neg = scalable.partial.contains(&neg_id);
    let scalable_has_pos = scalable.partial.contains(&pos_id);

    assert!(
        standard_has_neg,
        "Standard reasoner should prove ~flies(tweety)"
    );
    assert!(
        !standard_has_pos,
        "Standard reasoner should not prove flies(tweety)"
    );
    assert!(
        scalable_has_neg,
        "Scalable reasoner should match standard for ~flies(tweety)"
    );
    assert!(
        !scalable_has_pos,
        "Scalable reasoner should match standard for flies(tweety)"
    );
}
