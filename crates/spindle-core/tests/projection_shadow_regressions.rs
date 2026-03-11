use std::collections::BTreeSet;

use spindle_core::conclusion::ConclusionType;
use spindle_core::index::IndexedTheory;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::projection::ProjectionToken;
use spindle_core::reason::reason_full;
use spindle_core::shadow::ShadowReasoner;
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

#[test]
fn reason_full_projects_ambiguity_blockers_without_positive_conclusions() {
    let mut theory = Theory::new();
    theory.add_fact("republican");
    theory.add_fact("quaker");
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::simple("republican")],
        Literal::negated("pacifist"),
    ));
    theory.add_rule(Rule::defeasible(
        "r2",
        vec![Literal::simple("quaker")],
        Literal::simple("pacifist"),
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
fn shadow_projects_defeater_attack_tokens_when_defeater_only_blocks() {
    let mut theory = Theory::new();
    theory.add_fact("bird");
    theory.add_fact("broken_wing");
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::simple("bird")],
        Literal::simple("flies"),
    ));
    theory.add_rule(Rule::defeater(
        "d1",
        vec![Literal::simple("broken_wing")],
        Literal::negated("flies"),
    ));

    let mut indexed = IndexedTheory::build(&theory);
    let result = ShadowReasoner::enabled()
        .reason_shadow(&mut indexed)
        .unwrap();

    assert!(
        !result.primary.iter().any(|c| {
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
        "shadow diagnostics should include the blocking defeater label"
    );
}

#[test]
fn reason_full_keeps_grounded_projection_labels_aligned_with_conclusions() {
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
fn shadow_keeps_grounded_projection_labels_aligned_with_compiled_bodies() {
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

    let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
    let mut indexed = IndexedTheory::build(&prepared.theory);
    let result = ShadowReasoner::enabled()
        .reason_shadow(&mut indexed)
        .unwrap();
    let grounded_rule_label = grounded_q_rule_label(&result.primary);
    let projection_labels = token_labels(&result.projection_tokens);

    assert_ne!(
        grounded_rule_label, "r1",
        "the test must exercise a grounded instance rather than the template rule"
    );
    assert!(
        projection_labels.contains(grounded_rule_label),
        "shadow projection tokens should stay in the grounded label space"
    );
    assert!(
        result.compiled_bodies.contains_key(grounded_rule_label),
        "compiled bodies should use the same grounded label as the conclusion"
    );
    assert!(
        !projection_labels.contains("r1"),
        "shadow projection tokens should not fall back to the template label"
    );
}
