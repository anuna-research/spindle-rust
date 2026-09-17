//! Public proof labels and trust steps must not depend on hash-map seeds.
use spindle_core::{
    Theory,
    pipeline::{PrepareOptions, compute_weighted_conclusions, prepare},
    reason::reason_prepared,
};

#[test]
fn grounding_labels_survive_reordered_theory_insertion() {
    let original =
        spindle_parser::parse_spl(include_str!("fixtures/issue_37_witness.spl")).unwrap();
    let snapshot = |theory: &Theory| {
        let prepared = prepare(theory, PrepareOptions::default()).unwrap();
        let mut rules: Vec<_> = prepared.theory.rules().map(|r| r.to_spl()).collect();
        rules.sort();
        rules
    };
    let expected = snapshot(&original);
    let mut rules: Vec<_> = original.rules().cloned().collect();
    rules.sort_by(|a, b| a.label.cmp(&b.label));
    for rotation in 0..8 {
        rules.rotate_left(rotation);
        let mut reordered = Theory::new();
        for rule in &rules {
            reordered.add_rule(rule.clone());
        }
        for sup in original.superiorities() {
            reordered.add_superiority(&sup.superior, &sup.inferior);
        }
        assert_eq!(snapshot(&reordered), expected);
    }
}

#[test]
fn diminishers_are_applied_in_stable_order() {
    let source = format!(
        "{}\n(claims challenger (except another concern (not approved)))\n(prefer support another)",
        include_str!("../../../examples/trust-diminishment.spl")
    );
    for _ in 0..8 {
        let theory = spindle_parser::parse_spl(&source).unwrap();
        let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
        let conclusions = reason_prepared(&prepared.theory).unwrap();
        let weighted = compute_weighted_conclusions(
            &conclusions,
            &prepared.theory,
            prepared.theory.trust_policy(),
            None,
        );
        let approved = weighted
            .iter()
            .find(|c| {
                c.literal.name() == "approved"
                    && c.conclusion_type == spindle_core::ConclusionType::DefeasiblyProvable
            })
            .unwrap();
        assert_eq!(
            approved
                .diminished_by
                .iter()
                .map(|d| d.defeater_label.as_str())
                .collect::<Vec<_>>(),
            vec!["another", "challenge"]
        );
        assert!((approved.degree - 0.441).abs() < 1e-10);
    }
}
