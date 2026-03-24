use std::collections::BTreeSet;

use spindle_core::conclusion::ConclusionType;
use spindle_core::projection::ProjectionToken;
use spindle_core::reason::reason_full;
use spindle_core::temporal::{Temporal, TimePoint};
use spindle_core::{Literal, Rule, Theory};

fn token_label(token: &ProjectionToken) -> &str {
    match token {
        ProjectionToken::Exact(s) => &s.rule_label,
        ProjectionToken::Family(s) => &s.rule_label,
        ProjectionToken::Attack(a) => &a.rule_label,
    }
}

fn token_labels(tokens: &[ProjectionToken]) -> BTreeSet<String> {
    tokens
        .iter()
        .map(|token| token_label(token).to_string())
        .collect()
}

fn grounded_q_rule_label<'a>(conclusions: &'a [spindle_core::conclusion::Conclusion]) -> &'a str {
    conclusions
        .iter()
        .find(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && c.literal.predicates() == vec!["a".to_string()]
        })
        .and_then(|c| c.rule_label.as_deref())
        .expect("expected grounded +d q(a) conclusion with a rule label")
}

fn temporal_literal(name: &str, negated: bool, args: Vec<&str>) -> Literal {
    Literal::new(
        name,
        negated,
        Default::default(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        args.into_iter().map(str::to_string).collect(),
    )
}

#[test]
fn reason_full_projects_ambiguity_blockers_without_positive_conclusions() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact(
        "f1",
        temporal_literal("republican", false, vec![]),
    ));
    theory.add_rule(Rule::fact("f2", temporal_literal("quaker", false, vec![])));
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![temporal_literal("republican", false, vec![])],
        temporal_literal("pacifist", true, vec![]),
    ));
    theory.add_rule(Rule::defeasible(
        "r2",
        vec![temporal_literal("quaker", false, vec![])],
        temporal_literal("pacifist", false, vec![]),
    ));

    let result = reason_full(&theory).unwrap();

    assert!(
        !result.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "pacifist"
        }),
        "ambiguity should block both +d pacifist and +d ~pacifist"
    );
    assert!(
        result.diagnostics.contributing_labels.contains("r1"),
        "ambiguity blocker r1 should still be projected"
    );
    assert!(
        result.diagnostics.contributing_labels.contains("r2"),
        "ambiguity blocker r2 should still be projected"
    );
}

#[test]
fn reason_full_projects_defeater_attack_tokens_when_defeater_only_blocks() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", temporal_literal("bird", false, vec![])));
    theory.add_rule(Rule::fact(
        "f2",
        temporal_literal("broken_wing", false, vec![]),
    ));
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![temporal_literal("bird", false, vec![])],
        temporal_literal("flies", false, vec![]),
    ));
    theory.add_rule(Rule::defeater(
        "d1",
        vec![temporal_literal("broken_wing", false, vec![])],
        temporal_literal("flies", true, vec![]),
    ));

    let result = reason_full(&theory).unwrap();

    assert!(
        !result.conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "flies"
                && !c.literal.negation
        }),
        "the defeater should block +d flies"
    );
    assert!(
        result
            .projection_tokens
            .iter()
            .any(|t| matches!(t, ProjectionToken::Attack(a) if a.rule_label == "d1")),
        "the blocking defeater should emit an attack token"
    );
    assert!(
        result.diagnostics.contributing_labels.contains("d1"),
        "diagnostics should include the blocking defeater label"
    );
}

#[test]
fn reason_full_keeps_grounded_projection_labels_aligned_with_conclusions() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", temporal_literal("p", false, vec!["a"])));
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![temporal_literal("p", false, vec!["?x"])],
        temporal_literal("q", false, vec!["?x"]),
    ));

    let result = reason_full(&theory).unwrap();
    let grounded_rule_label = grounded_q_rule_label(&result.conclusions);
    let projection_labels = token_labels(&result.projection_tokens);

    assert_ne!(
        grounded_rule_label, "r1",
        "the test must exercise a grounded instance rather than the template rule"
    );
    assert!(
        projection_labels.contains(grounded_rule_label),
        "projection tokens should use the same grounded rule label as the conclusion"
    );
    assert!(
        !projection_labels.contains("r1"),
        "projection tokens should not rewrite grounded labels back to the template label"
    );
}

#[test]
fn reason_full_keeps_grounded_projection_labels_aligned_with_tokens() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", temporal_literal("p", false, vec!["a"])));
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![temporal_literal("p", false, vec!["?x"])],
        temporal_literal("q", false, vec!["?x"]),
    ));

    let result = reason_full(&theory).unwrap();
    let grounded_rule_label = grounded_q_rule_label(&result.conclusions);
    let projection_labels = token_labels(&result.projection_tokens);

    assert_ne!(
        grounded_rule_label, "r1",
        "the test must exercise a grounded instance rather than the template rule"
    );
    assert!(
        projection_labels.contains(grounded_rule_label),
        "projection tokens should stay in the grounded label space"
    );
    assert!(
        !projection_labels.contains("r1"),
        "projection tokens should not fall back to the template label"
    );
}
