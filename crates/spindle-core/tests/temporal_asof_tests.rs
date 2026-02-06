use spindle_core::Rule;
use spindle_core::literal::Literal;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::temporal::{Temporal, TimePoint};
use spindle_core::theory::Theory;

#[test]
fn test_disjoint_complements_do_not_conflict_at_timepoint() {
    let mut theory = Theory::new();

    let p = Literal::new(
        "p",
        false,
        Default::default(),
        Temporal::from_bounds(0, 10),
        vec![],
    );
    let not_p = Literal::new(
        "p",
        true,
        Default::default(),
        Temporal::from_bounds(20, 30),
        vec![],
    );

    theory.add_rule(Rule::fact("f1", p));
    theory.add_rule(Rule::fact("f2", not_p));

    let prepared = prepare(
        &theory,
        PrepareOptions {
            reference_time: Some(TimePoint::from_millis(5)),
            ..Default::default()
        },
    )
    .unwrap();

    let conclusions = prepared.theory.reason().unwrap();

    let has_p = conclusions
        .iter()
        .any(|c| c.literal.name() == "p" && !c.literal.negation && c.is_positive());
    let has_not_p = conclusions
        .iter()
        .any(|c| c.literal.name() == "p" && c.literal.negation && c.is_positive());

    assert!(has_p, "Expected p to be active at t=5");
    assert!(
        !has_not_p,
        "Expected ~p to be inactive at t=5 due to disjoint temporal bounds"
    );
}

#[test]
fn test_overlapping_complements_are_active_at_timepoint() {
    let mut theory = Theory::new();

    let p = Literal::new(
        "p",
        false,
        Default::default(),
        Temporal::from_bounds(0, 30),
        vec![],
    );
    let not_p = Literal::new(
        "p",
        true,
        Default::default(),
        Temporal::from_bounds(20, 40),
        vec![],
    );

    theory.add_rule(Rule::fact("f1", p));
    theory.add_rule(Rule::fact("f2", not_p));

    let prepared = prepare(
        &theory,
        PrepareOptions {
            reference_time: Some(TimePoint::from_millis(25)),
            ..Default::default()
        },
    )
    .unwrap();

    let conclusions = prepared.theory.reason().unwrap();

    let has_p = conclusions
        .iter()
        .any(|c| c.literal.name() == "p" && !c.literal.negation && c.is_positive());
    let has_not_p = conclusions
        .iter()
        .any(|c| c.literal.name() == "p" && c.literal.negation && c.is_positive());

    assert!(has_p, "Expected p to be active at t=25");
    assert!(
        has_not_p,
        "Expected ~p to be active at t=25 due to overlapping temporal bounds"
    );
}
