use spindle_core::temporal::{Temporal, TimePoint};
use spindle_core::{Literal, Rule, Theory, conclusion::ConclusionType, reason::reason};

#[test]
fn test_arg_discrimination() {
    // Motivation: 1.1 Predicate arguments are not respected by the reasoner
    // Observed: A rule requiring needs_classify("doc_1","h1") fires when only needs_classify("doc_2","h2") is present.
    let mut theory = Theory::new();

    // Fact: p(a)
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

    // Rule: p(b) => q
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

    let conclusions = reason(&theory).unwrap();

    // Expectation: q is NOT proven because p(a) != p(b)
    let q_proven = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "q"
    });

    assert!(!q_proven, "Regression: p(a) satisfied p(b), proving q!");
}

#[test]
fn test_grounding_end_to_end() {
    // Motivation: 1.2 Variables are not grounded in outputs
    let mut theory = Theory::new();

    // Fact: p(a)
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

    // Rule: p(?x) => q(?x)
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

    // reason() currently doesn't call ground_theory, so this will likely fail to produce q(a)
    let conclusions = reason(&theory).unwrap();

    let qa_proven = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "q"
            && c.literal.predicates() == vec!["a".to_string()]
    });

    assert!(
        qa_proven,
        "Regression: Variables were not grounded/reasoned over. Expected q(a)."
    );
}

#[test]
fn test_superiority_grounded() {
    // Motivation: 1.3 Grounding, as implemented, does not preserve superiority semantics across instances
    let mut theory = Theory::new();

    // Fact: bird(tweety)
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
    // Fact: penguin(tweety)
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

    // Rule r1: bird(?x) => flies(?x)
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

    // Rule r2: penguin(?x) => ~flies(?x)
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

    // Prefer r2 over r1
    theory.add_superiority("r2", "r1");

    let conclusions = reason(&theory).unwrap();

    let flies_neg_proven = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "flies"
            && c.literal.negation
            && c.literal.predicates() == vec!["tweety".to_string()]
    });

    let flies_pos_proven = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "flies"
            && !c.literal.negation
            && c.literal.predicates() == vec!["tweety".to_string()]
    });

    assert!(
        flies_neg_proven,
        "Regression: Superiority failed on grounded instance (expected ~flies(tweety))"
    );
    assert!(
        !flies_pos_proven,
        "Regression: Inferior rule fired despite superiority (found flies(tweety))"
    );
}

#[test]
fn test_temporal_default_safety() {
    // Motivation: 1.5 ... temporal default values produce noisy [-inf,-inf] suffixes

    let t: Temporal = Default::default();

    // We want the default to be the Unbounded/Empty temporal ([-inf, +inf])
    assert!(
        t.is_empty(),
        "Temporal::default() should be the Empty/Unbounded temporal [-inf, +inf]"
    );
    assert_eq!(t.start, TimePoint::NegInf);
    assert_eq!(
        t.end,
        TimePoint::PosInf,
        "Temporal::default().end should be +inf"
    );
}
