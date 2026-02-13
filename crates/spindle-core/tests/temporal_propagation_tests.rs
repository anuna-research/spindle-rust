//! Integration tests for temporal variable propagation.
//!
//! Tests the full pipeline: SPL parsing → grounding → temporal variable
//! resolution → reasoning → temporal-annotated conclusions.

use spindle_core::conclusion::ConclusionType;
use spindle_core::grounding::ground_theory;
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::pipeline::PrepareOptions;
use spindle_core::reason::{reason, reason_with_options};
use spindle_core::rule::{Rule, RuleType};
use spindle_core::temporal::{Temporal, TemporalExpr, TimeExpr, TimePoint};
use spindle_core::theory::Theory;

// =========================================================================
// SPL Parser: temporal variable endpoints
// =========================================================================

#[test]
fn test_spl_parse_temporal_variable_during() {
    let input = r#"
        (given (during (p a) ?t1 ?t2))
    "#;
    let theory = spindle_parser::parse_spl(input).unwrap();
    let facts: Vec<_> = theory.facts().collect();
    assert_eq!(facts.len(), 1);

    let head = facts[0].head_literal();
    assert_eq!(head.name(), "p");
    assert_eq!(head.predicates(), vec!["a"]);
    assert!(
        head.has_temporal_variables(),
        "Parsed temporal variable should produce temporal_expr"
    );
}

#[test]
fn test_spl_parse_temporal_mixed_const_var() {
    let input = r#"
        (given (during (p) 100 ?t))
    "#;
    let theory = spindle_parser::parse_spl(input).unwrap();
    let head = theory.facts().next().unwrap().head_literal();
    assert!(
        head.has_temporal_variables(),
        "Mixed const/var should produce temporal_expr"
    );
}

#[test]
fn test_spl_parse_temporal_fully_concrete_during() {
    let input = r#"
        (given (during (p) 100 200))
    "#;
    let theory = spindle_parser::parse_spl(input).unwrap();
    let head = theory.facts().next().unwrap().head_literal();
    assert!(
        !head.has_temporal_variables(),
        "Fully concrete during should resolve to concrete temporal, not temporal_expr"
    );
    assert_eq!(head.temporal.start, TimePoint::Moment(100));
    assert_eq!(head.temporal.end, TimePoint::Moment(200));
}

// =========================================================================
// Grounding: temporal variable binding and propagation
// =========================================================================

#[test]
fn test_grounding_temporal_variable_propagation_simple() {
    let mut theory = Theory::new();

    // Fact: p(a)[100, 200]
    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(100), TimePoint::Moment(200)),
            vec!["a".to_string()],
        ),
    ));

    // Rule: (during (p ?x) ?t1 ?t2) => (during (q ?x) ?t1 ?t2)
    let body = Literal::new_with_temporal_expr(
        "p",
        false,
        Mode::empty(),
        TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
        vec!["?x".to_string()],
    );
    let head = Literal::new_with_temporal_expr(
        "q",
        false,
        Mode::empty(),
        TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
        vec!["?x".to_string()],
    );
    theory.add_rule(Rule::new(
        "r1".to_string(),
        RuleType::Defeasible,
        vec![body],
        vec![head],
    ));

    let grounded = ground_theory(&theory);

    // Verify grounded rule has concrete temporal on head
    let grounded_rule = grounded
        .rules()
        .find(|r| r.label.starts_with("r1_"))
        .expect("Should have grounded r1");

    let grounded_head = grounded_rule.head_literal();
    assert_eq!(grounded_head.name(), "q");
    assert_eq!(grounded_head.predicates(), vec!["a"]);
    assert!(
        !grounded_head.has_temporal_variables(),
        "Temporal variables should be fully resolved"
    );
    assert_eq!(grounded_head.temporal.start, TimePoint::Moment(100));
    assert_eq!(grounded_head.temporal.end, TimePoint::Moment(200));
}

#[test]
fn test_grounding_multiple_temporal_facts() {
    let mut theory = Theory::new();

    // Two temporal facts for same predicate
    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(100), TimePoint::Moment(200)),
            vec!["a".to_string()],
        ),
    ));
    theory.add_rule(Rule::fact(
        "f2",
        Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(300), TimePoint::Moment(400)),
            vec!["a".to_string()],
        ),
    ));

    // Rule with temporal variables
    let body = Literal::new_with_temporal_expr(
        "p",
        false,
        Mode::empty(),
        TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
        vec!["?x".to_string()],
    );
    let head = Literal::new_with_temporal_expr(
        "q",
        false,
        Mode::empty(),
        TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
        vec!["?x".to_string()],
    );
    theory.add_rule(Rule::new(
        "r1".to_string(),
        RuleType::Defeasible,
        vec![body],
        vec![head],
    ));

    let grounded = ground_theory(&theory);

    // Should produce two grounded instances
    let grounded_rules: Vec<_> = grounded
        .rules()
        .filter(|r| r.label.starts_with("r1_"))
        .collect();
    assert_eq!(
        grounded_rules.len(),
        2,
        "Two temporal facts should produce two grounded instances"
    );

    // Check both temporal windows are present
    let mut windows: Vec<(i64, i64)> = grounded_rules
        .iter()
        .map(|r| {
            let h = r.head_literal();
            match (&h.temporal.start, &h.temporal.end) {
                (TimePoint::Moment(s), TimePoint::Moment(e)) => (*s, *e),
                _ => panic!("Expected concrete temporal"),
            }
        })
        .collect();
    windows.sort();
    assert_eq!(windows, vec![(100, 200), (300, 400)]);
}

#[test]
fn test_grounding_chain_temporal_propagation() {
    // p(a)[100,200] => q(a)[100,200] => r(a)[100,200]
    let mut theory = Theory::new();

    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(100), TimePoint::Moment(200)),
            vec!["a".to_string()],
        ),
    ));

    // r1: (during (p ?x) ?t1 ?t2) => (during (q ?x) ?t1 ?t2)
    theory.add_rule(Rule::new(
        "r1".to_string(),
        RuleType::Defeasible,
        vec![Literal::new_with_temporal_expr(
            "p",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        )],
        vec![Literal::new_with_temporal_expr(
            "q",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        )],
    ));

    // r2: (during (q ?x) ?s1 ?s2) => (during (r ?x) ?s1 ?s2)
    theory.add_rule(Rule::new(
        "r2".to_string(),
        RuleType::Defeasible,
        vec![Literal::new_with_temporal_expr(
            "q",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?s1"), TimeExpr::var("?s2")),
            vec!["?x".to_string()],
        )],
        vec![Literal::new_with_temporal_expr(
            "r",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?s1"), TimeExpr::var("?s2")),
            vec!["?x".to_string()],
        )],
    ));

    let grounded = ground_theory(&theory);

    // Should have a grounded r2 instance with r(a)[100,200]
    let has_r = grounded.rules().any(|r| {
        r.label.starts_with("r2_")
            && r.head.iter().any(|h| {
                h.name() == "r"
                    && h.predicates() == vec!["a"]
                    && h.temporal.start == TimePoint::Moment(100)
                    && h.temporal.end == TimePoint::Moment(200)
            })
    });
    assert!(
        has_r,
        "Chain propagation should produce r(a)[100,200] from p(a)[100,200]"
    );
}

// =========================================================================
// Pipeline: temporal variable validation
// =========================================================================

#[test]
fn test_pipeline_rejects_unresolved_temporal_vars() {
    // A fact p(a) with NO temporal, paired with a rule that has temporal vars
    // in the head only (not in the body). Grounding binds ?x to "a" but
    // temporal vars ?t1, ?t2 remain unresolved.
    let mut theory = Theory::new();

    // Fact: p(a) — no temporal annotation
    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".to_string()],
        ),
    ));

    // Rule: p(?x) => q(?x)[?t1, ?t2]  — temporal vars in head can't bind
    let body = Literal::new(
        "p",
        false,
        Mode::empty(),
        Temporal::empty(),
        vec!["?x".to_string()],
    );
    let head = Literal::new_with_temporal_expr(
        "q",
        false,
        Mode::empty(),
        TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
        vec!["?x".to_string()],
    );
    theory.add_rule(Rule::new(
        "r1".to_string(),
        RuleType::Defeasible,
        vec![body],
        vec![head],
    ));

    // Pipeline should reject due to unresolved temporal vars on grounded head
    let result = reason(&theory);
    assert!(
        result.is_err(),
        "Pipeline should reject unresolved temporal variables"
    );
}

// =========================================================================
// Reasoning: temporal info on conclusions
// =========================================================================

#[test]
fn test_reasoning_fact_conclusion_preserves_temporal() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "active",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1000), TimePoint::Moment(2000)),
            vec![],
        ),
    ));

    let conclusions = reason(&theory).unwrap();

    let definite = conclusions
        .iter()
        .find(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "active"
        })
        .expect("Should have +D active");

    assert_eq!(
        definite.literal.temporal.start,
        TimePoint::Moment(1000),
        "Fact conclusion should preserve temporal start"
    );
    assert_eq!(
        definite.literal.temporal.end,
        TimePoint::Moment(2000),
        "Fact conclusion should preserve temporal end"
    );
}

#[test]
fn test_reasoning_defeasible_conclusion_preserves_temporal() {
    let mut theory = Theory::new();

    // Fact with temporal
    theory.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "license",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1000), TimePoint::Moment(2000)),
            vec!["alice".to_string()],
        ),
    ));

    // Rule: license(?x)[?t1,?t2] => authorized(?x)[?t1,?t2]
    theory.add_rule(Rule::new(
        "r1".to_string(),
        RuleType::Defeasible,
        vec![Literal::new_with_temporal_expr(
            "license",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        )],
        vec![Literal::new_with_temporal_expr(
            "authorized",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        )],
    ));

    let conclusions = reason(&theory).unwrap();

    let authorized = conclusions
        .iter()
        .find(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "authorized"
                && !c.literal.negation
        })
        .expect("Should have +d authorized(alice)");

    assert_eq!(
        authorized.literal.temporal.start,
        TimePoint::Moment(1000),
        "Defeasible conclusion should preserve temporal start from grounding"
    );
    assert_eq!(
        authorized.literal.temporal.end,
        TimePoint::Moment(2000),
        "Defeasible conclusion should preserve temporal end from grounding"
    );
}

// =========================================================================
// End-to-end SPL tests
// =========================================================================

#[test]
fn test_spl_end_to_end_temporal_variable_propagation() {
    let input = r#"
        ; Fact with concrete temporal
        (given (during (license alice) 1000 2000))

        ; Rule with temporal variables
        (normally r1
            (during (license ?x) ?t1 ?t2)
            (during (authorized ?x) ?t1 ?t2))
    "#;

    let theory = spindle_parser::parse_spl(input).unwrap();
    let conclusions = reason(&theory).unwrap();

    let authorized = conclusions
        .iter()
        .find(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "authorized"
        })
        .expect("Should derive authorized(alice)");

    assert_eq!(authorized.literal.predicates(), vec!["alice"]);
    assert_eq!(authorized.literal.temporal.start, TimePoint::Moment(1000));
    assert_eq!(authorized.literal.temporal.end, TimePoint::Moment(2000));
}

#[test]
fn test_spl_temporal_with_reference_time() {
    let input = r#"
        (given (during (valid) 1000 2000))
        (normally r1 (valid) (active))
    "#;

    let theory = spindle_parser::parse_spl(input).unwrap();

    // At time 1500 (inside window): valid is active
    let opts_inside = PrepareOptions {
        reference_time: Some(TimePoint::from_millis(1500)),
        ..Default::default()
    };
    let inside = reason_with_options(&theory, opts_inside).unwrap();
    assert!(
        inside
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "active"),
        "active should be provable at t=1500"
    );

    // At time 3000 (outside window): valid is filtered out
    let opts_outside = PrepareOptions {
        reference_time: Some(TimePoint::from_millis(3000)),
        ..Default::default()
    };
    let outside = reason_with_options(&theory, opts_outside).unwrap();
    assert!(
        !outside
            .iter()
            .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "active"),
        "active should NOT be provable at t=3000"
    );
}

// =========================================================================
// Theory detection helper
// =========================================================================

#[test]
fn test_theory_has_temporal_literals() {
    let mut temporal = Theory::new();
    temporal.add_rule(Rule::fact(
        "f1",
        Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(2)),
            vec![],
        ),
    ));
    assert!(temporal.has_temporal_literals());

    let simple = Theory::new();
    assert!(!simple.has_temporal_literals());

    let mut with_vars = Theory::new();
    with_vars.add_rule(Rule::new(
        "r1".to_string(),
        RuleType::Defeasible,
        vec![Literal::new_with_temporal_expr(
            "p",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec![],
        )],
        vec![Literal::simple("q")],
    ));
    assert!(with_vars.has_temporal_literals());
}
