use spindle_core::{Rule, RuleType};
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

#[test]
fn test_rule_level_temporal_filtered_when_inactive() {
    // Rule with temporal=[100,200], query at t=50 → rule excluded, head not derived
    let mut theory = Theory::new();

    // Fact: bird (always active)
    theory.add_rule(Rule::fact(
        "f1",
        Literal::new("bird", false, Default::default(), Default::default(), vec![]),
    ));

    // Rule: bird => flies, but only active in [100,200]
    let mut rule = Rule::new(
        "r1".to_string(),
        RuleType::Defeasible,
        vec![Literal::new(
            "bird",
            false,
            Default::default(),
            Default::default(),
            vec![],
        )],
        vec![Literal::new(
            "flies",
            false,
            Default::default(),
            Default::default(),
            vec![],
        )],
    );
    rule.temporal = Temporal::from_bounds(100, 200);
    theory.add_rule(rule);

    let prepared = prepare(
        &theory,
        PrepareOptions {
            reference_time: Some(TimePoint::from_millis(50)),
            ..Default::default()
        },
    )
    .unwrap();

    // The rule should have been filtered out
    let has_flies_rule = prepared
        .theory
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "flies"));
    assert!(
        !has_flies_rule,
        "Rule r1 should be filtered out at t=50 (outside [100,200])"
    );
}

#[test]
fn test_rule_level_temporal_kept_when_active() {
    // Same rule, query at t=150 → rule kept, head derived
    let mut theory = Theory::new();

    theory.add_rule(Rule::fact(
        "f1",
        Literal::new("bird", false, Default::default(), Default::default(), vec![]),
    ));

    let mut rule = Rule::new(
        "r1".to_string(),
        RuleType::Defeasible,
        vec![Literal::new(
            "bird",
            false,
            Default::default(),
            Default::default(),
            vec![],
        )],
        vec![Literal::new(
            "flies",
            false,
            Default::default(),
            Default::default(),
            vec![],
        )],
    );
    rule.temporal = Temporal::from_bounds(100, 200);
    theory.add_rule(rule);

    let prepared = prepare(
        &theory,
        PrepareOptions {
            reference_time: Some(TimePoint::from_millis(150)),
            ..Default::default()
        },
    )
    .unwrap();

    // The rule should be kept
    let has_flies_rule = prepared
        .theory
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "flies"));
    assert!(
        has_flies_rule,
        "Rule r1 should be kept at t=150 (inside [100,200])"
    );

    // And reasoning should derive flies
    let conclusions = prepared.theory.reason().unwrap();
    let has_flies = conclusions
        .iter()
        .any(|c| c.literal.name() == "flies" && c.is_positive());
    assert!(has_flies, "Expected flies to be derived at t=150");
}

#[test]
fn test_rule_unbounded_temporal_always_active() {
    // Rule with empty temporal → always active
    let mut theory = Theory::new();

    theory.add_rule(Rule::fact(
        "f1",
        Literal::new("bird", false, Default::default(), Default::default(), vec![]),
    ));

    // Rule with default (empty) temporal
    let rule = Rule::new(
        "r1".to_string(),
        RuleType::Defeasible,
        vec![Literal::new(
            "bird",
            false,
            Default::default(),
            Default::default(),
            vec![],
        )],
        vec![Literal::new(
            "flies",
            false,
            Default::default(),
            Default::default(),
            vec![],
        )],
    );
    theory.add_rule(rule);

    // Query at arbitrary time — should still work
    let prepared = prepare(
        &theory,
        PrepareOptions {
            reference_time: Some(TimePoint::from_millis(999)),
            ..Default::default()
        },
    )
    .unwrap();

    let has_flies_rule = prepared
        .theory
        .rules()
        .any(|r| r.head.iter().any(|h| h.name() == "flies"));
    assert!(
        has_flies_rule,
        "Rule with empty temporal should always be active"
    );
}
