//! Issue #44: single-head strong permission profile.
use spindle_core::query::{QueryStatus, query, why_not};
use spindle_core::{ConclusionType, Literal, reason};
use spindle_parser::parse_spl;

fn literal(source: &str) -> Literal {
    parse_spl(&format!("(given {source})"))
        .unwrap()
        .rules()
        .next()
        .unwrap()
        .head_literal()
        .clone()
}

fn tag(source: &str, goal: &str, tag: ConclusionType) -> bool {
    let conclusions = reason(&parse_spl(source).unwrap()).unwrap();
    let goal = literal(goal);
    conclusions.iter().any(|c| {
        c.conclusion_type == tag
            && spindle_core::projection::FamilyId::from(&c.literal)
                == spindle_core::projection::FamilyId::from(&goal)
    })
}

#[test]
fn marks_preferences_and_missing_hat() {
    let rules = "(normally no-play () (forbidden play)) (normally hat-play hat (may play))";
    for (extra, permission, prohibition) in [
        ("(given hat) (prefer hat-play no-play)", true, false),
        ("(given hat) (prefer no-play hat-play)", false, true),
        ("(given hat)", false, false),
        ("(prefer hat-play no-play)", false, true),
    ] {
        let source = format!("{rules} {extra}");
        for (goal, expected) in [
            ("(may play)", permission),
            ("(forbidden play)", prohibition),
        ] {
            assert_eq!(
                tag(&source, goal, ConclusionType::DefeasiblyProvable),
                expected,
                "{source}: {goal}"
            );
            assert_eq!(
                tag(&source, goal, ConclusionType::DefeasiblyNotProvable),
                !expected,
                "{source}: {goal} negative"
            );
        }
    }
}

#[test]
fn opposite_permissions_coexist_but_obligations_conflict() {
    let permissions = "(normally a () (may play)) (normally b () (may (not play)))";
    assert!(tag(
        permissions,
        "(may play)",
        ConclusionType::DefeasiblyProvable
    ));
    assert!(tag(
        permissions,
        "(may (not play))",
        ConclusionType::DefeasiblyProvable
    ));
    let obligations = "(normally a () (must play)) (normally b () (must (not play)))";
    assert!(!tag(
        obligations,
        "(must play)",
        ConclusionType::DefeasiblyProvable
    ));
    assert!(!tag(
        obligations,
        "(must (not play))",
        ConclusionType::DefeasiblyProvable
    ));
}

#[test]
fn prohibition_alias_satisfies_modal_premises() {
    let forbidden = literal("(forbidden play)");
    assert_eq!(
        spindle_core::projection::FamilyId::from(&forbidden),
        spindle_core::projection::FamilyId::new(
            forbidden.interned_name(),
            Vec::new(),
            forbidden.mode.clone(),
            forbidden.negation,
        )
    );
    let source = "(normally a () (forbidden play)) (normally b (must (not play)) stop)";
    assert!(tag(source, "stop", ConclusionType::DefeasiblyProvable));
    assert_eq!(
        query(&parse_spl(source).unwrap(), &literal("(must (not play))"))
            .unwrap()
            .status,
        QueryStatus::Provable
    );
}

#[test]
fn modal_queries_and_blocker_explanations() {
    let theory =
        parse_spl("(normally a () (forbidden play)) (normally b () (may play)) (prefer b a)")
            .unwrap();
    assert_eq!(
        query(&theory, &literal("(forbidden play)")).unwrap().status,
        QueryStatus::Refuted
    );
    assert!(
        why_not(&theory, &literal("(forbidden play)"))
            .unwrap()
            .blocked_by
            .iter()
            .any(|b| b.blocking_rule.as_deref() == Some("b"))
    );
    let theory = parse_spl("(normally a () (may play))").unwrap();
    assert_eq!(
        query(&theory, &literal("(may (not play))")).unwrap().status,
        QueryStatus::Unknown
    );
}

#[test]
fn outer_negation_preserves_scope_and_roundtrips() {
    for op in ["must", "may", "forbidden"] {
        let outside = literal(&format!("(not ({op} play))"));
        let inside = literal(&format!("({op} (not play))"));
        assert_ne!(outside, inside);
        assert!(outside.mode.negation);
        assert_eq!(literal(&outside.to_spl()), outside);
        let theory = parse_spl(&format!(
            "(given (not ({op} play))) (normally r (not ({op} play)) ok)"
        ))
        .unwrap();
        assert!(
            reason(&theory)
                .unwrap()
                .iter()
                .any(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.name() == "ok")
        );
    }
}

#[test]
fn cross_mode_team_defense() {
    let source = "(normally p () (may play)) (normally o () (must play)) (normally f () (forbidden play)) (prefer o f)";
    assert!(tag(
        source,
        "(may play)",
        ConclusionType::DefeasiblyProvable
    ));
    assert!(tag(
        source,
        "(must play)",
        ConclusionType::DefeasiblyProvable
    ));
    assert!(!tag(
        source,
        "(forbidden play)",
        ConclusionType::DefeasiblyProvable
    ));
}

#[test]
fn negative_modal_premises_require_constructive_refutation() {
    let source = "(normally o () (must play)) (normally f () (forbidden play))
        (normally weak (not (forbidden play)) weakly-allowed)
        (normally strong (may play) explicitly-allowed)";
    assert!(tag(
        source,
        "weakly-allowed",
        ConclusionType::DefeasiblyProvable
    ));
    assert!(!tag(
        source,
        "explicitly-allowed",
        ConclusionType::DefeasiblyProvable
    ));
    let cycle = "(normally loop (must play) (must play)) (normally r (not (must play)) result)";
    assert!(!tag(cycle, "result", ConclusionType::DefeasiblyProvable));
    assert!(!tag(cycle, "result", ConclusionType::DefeasiblyNotProvable));
    let proved = "(normally o () (must play)) (normally r (not (must play)) result)";
    assert!(tag(proved, "result", ConclusionType::DefeasiblyNotProvable));
}

#[test]
fn facts_block_incompatible_defaults_but_do_not_imply_plain_actions() {
    for (fact, goal) in [
        ("(may play)", "(forbidden play)"),
        ("(forbidden play)", "(may play)"),
        ("(not (may play))", "(may play)"),
    ] {
        let source = format!("(given {fact}) (normally r () {goal})");
        assert!(tag(&source, goal, ConclusionType::DefeasiblyNotProvable));
        assert!(!tag(&source, "play", ConclusionType::DefeasiblyProvable));
    }
}

#[test]
fn temporal_and_predicate_identity_remain_separate() {
    for source in [
        "(normally p () (during (may play) 1 2)) (normally f () (during (forbidden play) 3 4))",
        "(normally p () (may (play alice))) (normally f () (forbidden (play bob)))",
    ] {
        let conclusions = reason(&parse_spl(source).unwrap()).unwrap();
        assert_eq!(
            conclusions
                .iter()
                .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
                .count(),
            2
        );
    }
}

#[test]
fn nested_modal_operators_are_rejected() {
    for expression in [
        "(may (must play))",
        "(forbidden (not (may play)))",
        "(must (forbidden play))",
    ] {
        assert!(parse_spl(&format!("(given {expression})")).is_err());
        assert!(parse_spl(&format!("(normally r {expression} result)")).is_err());
    }
}

#[test]
fn alias_grounding_and_explanations() {
    let source =
        "(given (forbidden (play alice))) (normally r (must (not (play ?who))) (stop ?who))";
    assert!(tag(
        source,
        "(stop alice)",
        ConclusionType::DefeasiblyProvable
    ));
    let theory = parse_spl(source).unwrap();
    assert!(
        spindle_core::explanation::explain(&theory, &literal("(must (not (play alice)))"))
            .unwrap()
            .unwrap()
            .proof_tree
            .is_some()
    );
    assert!(
        spindle_core::explanation::explain(&theory, &literal("(stop alice)"))
            .unwrap()
            .unwrap()
            .proof_tree
            .is_some()
    );
}

#[test]
fn permission_cannot_defend_an_obligation_against_opposite_permission() {
    let source = "(normally o () (must play)) (normally p () (may play)) (normally n () (may (not play))) (prefer p n)";
    assert!(tag(
        source,
        "(must play)",
        ConclusionType::DefeasiblyNotProvable
    ));
    assert!(tag(
        source,
        "(may play)",
        ConclusionType::DefeasiblyProvable
    ));
}

#[test]
fn defeater_cannot_defend_obligation_against_permission() {
    let source = "(normally o () (must play)) (normally p () (may (not play))) (except d () (must play)) (prefer d p)";
    assert!(tag(
        source,
        "(must play)",
        ConclusionType::DefeasiblyNotProvable
    ));
}

#[test]
fn bound_negative_modal_premise_is_grounded_without_a_negative_fact() {
    let source = "(given (person alice)) (normally r (and (person ?who) (not (forbidden (play ?who)))) (allowed ?who))";
    assert!(tag(
        source,
        "(allowed alice)",
        ConclusionType::DefeasiblyProvable
    ));
    assert!(
        why_not(&parse_spl(source).unwrap(), &literal("(allowed alice)"))
            .unwrap()
            .blocked_by
            .is_empty()
    );
}

#[test]
fn negative_modal_premise_explanations_preserve_negative_tags() {
    let theory = parse_spl("(normally r (not (must play)) result)").unwrap();
    let explanation = spindle_core::explanation::explain(&theory, &literal("result"))
        .unwrap()
        .unwrap();
    let body = &explanation
        .proof_tree
        .as_ref()
        .unwrap()
        .proof_step
        .as_ref()
        .unwrap()
        .body_proofs;
    assert_eq!(body.len(), 1);
    assert_eq!(
        body[0].derivation_type,
        spindle_core::explanation::DerivationType::DefeasibleRefutation
    );
    assert_eq!(body[0].literal, literal("(must play)"));
    assert!(
        explanation
            .to_natural_language()
            .contains("constructively refuted (-d)")
    );
    assert!(explanation.to_dot().contains("-d [O]play"));
    assert!(
        explanation
            .to_json()
            .to_string()
            .contains("defeasible_refutation")
    );
    assert!(
        explanation
            .to_jsonld()
            .to_string()
            .contains("defeasible_refutation")
    );
}

#[test]
fn exact_temporal_prohibition_aliases_across_queries() {
    use spindle_core::query::{QueryArgs, QueryMatchMode, query_with_match_mode};
    let theory = parse_spl("(normally f () (during (forbidden play) 1 2))").unwrap();
    let same = literal("(during (must (not play)) 1 2)");
    let disjoint = literal("(during (must (not play)) 3 4)");
    assert_eq!(
        query_with_match_mode(
            &theory,
            &same,
            QueryMatchMode::ExactTemporal,
            QueryArgs::default().prepare_options
        )
        .unwrap()
        .status,
        QueryStatus::Provable
    );
    assert_eq!(
        query(&theory, &disjoint).unwrap().status,
        QueryStatus::Unknown
    );
    assert!(why_not(&theory, &same).unwrap().blocked_by.is_empty());
    assert!(
        spindle_core::explanation::explain(&theory, &same)
            .unwrap()
            .is_some()
    );
    assert!(
        spindle_core::explanation::explain(&theory, &disjoint)
            .unwrap()
            .is_none()
    );
}

#[test]
fn diagnostics_share_deontic_opposition() {
    let compatible = parse_spl("(given (may play)) (given (may (not play)))").unwrap();
    assert!(compatible.check_consistency().is_empty());
    assert!(spindle_core::analysis::conflicts::find_conflicts(compatible.rules()).is_empty());
    let conflicting = parse_spl("(given (may play)) (given (forbidden play))").unwrap();
    assert!(!conflicting.check_consistency().is_empty());
    assert_eq!(
        spindle_core::analysis::conflicts::find_conflicts(conflicting.rules()).len(),
        1
    );
}

// Governatori et al., Definition 10(2.3): only obligation rules attack
// explicit permission. An obligation-headed defeater is not such a rule.
#[test]
fn obligation_defeater_does_not_attack_permission() {
    for prohibition in ["(forbidden play)", "(must (not play))"] {
        for preference in ["", "(prefer block permit)", "(prefer permit block)"] {
            let source = format!(
                "(normally permit () (may play)) (except block () {prohibition})
                 (normally use (may play) allowed) {preference}"
            );
            assert!(
                tag(&source, "(may play)", ConclusionType::DefeasiblyProvable),
                "{source}"
            );
            assert!(
                !tag(&source, "(may play)", ConclusionType::DefeasiblyNotProvable),
                "{source}"
            );
            assert!(tag(&source, "allowed", ConclusionType::DefeasiblyProvable));
            assert!(!tag(
                &source,
                prohibition,
                ConclusionType::DefeasiblyProvable
            ));
            let theory = parse_spl(&source).unwrap();
            assert_eq!(
                query(&theory, &literal("(may play)")).unwrap().status,
                QueryStatus::Provable
            );
            assert!(
                why_not(&theory, &literal("(may play)"))
                    .unwrap()
                    .blocked_by
                    .is_empty()
            );
        }
    }
}

#[test]
fn obligation_defeater_still_attacks_obligation() {
    let source = "(normally oblige () (must play)) (except block () (forbidden play))";
    assert!(tag(
        source,
        "(must play)",
        ConclusionType::DefeasiblyNotProvable
    ));
    assert!(!tag(
        source,
        "(must play)",
        ConclusionType::DefeasiblyProvable
    ));
}

#[test]
fn negative_modal_grounding_is_independent_of_binder_position() {
    for modal in ["must", "may", "forbidden"] {
        for body in [
            format!("(and (not ({modal} (pay ?x))) (customer ?x))"),
            format!("(and (customer ?x) (not ({modal} (pay ?x))))"),
        ] {
            let source = format!(
                "(given (customer alice)) (given (customer bob))
                 (given ({modal} (pay bob)))
                 (normally result {body} (may (skip ?x)))"
            );
            assert!(
                tag(
                    &source,
                    "(may (skip alice))",
                    ConclusionType::DefeasiblyProvable
                ),
                "{source}"
            );
            assert!(
                !tag(
                    &source,
                    "(may (skip bob))",
                    ConclusionType::DefeasiblyProvable
                ),
                "{source}"
            );
        }
    }
    // Arithmetic arguments in a deferred premise also resolve after binding.
    let arithmetic = "(given (amount 2))
        (normally r (and (not (must (pay (+ ?n 1)))) (amount ?n)) result)";
    assert!(tag(
        arithmetic,
        "result",
        ConclusionType::DefeasiblyProvable
    ));
    assert!(!tag(
        &format!("{arithmetic} (given (must (pay 3)))"),
        "result",
        ConclusionType::DefeasiblyProvable
    ));
    // An unbound negative modal premise must not invent a binding.
    assert!(!tag(
        "(normally r (not (must (pay ?x))) (may (skip ?x)))",
        "(may (skip alice))",
        ConclusionType::DefeasiblyProvable
    ));
}

#[test]
fn prohibition_aliases_emit_one_positive_conclusion() {
    for source in [
        "(normally a () (forbidden pay)) (normally b () (must (not pay)))",
        "(given (forbidden pay)) (normally b () (must (not pay)))",
        "(normally b () (must (not pay))) (normally a () (forbidden pay))",
    ] {
        let conclusions = reason(&parse_spl(source).unwrap()).unwrap();
        let goal = literal("(forbidden pay)");
        assert_eq!(
            conclusions
                .iter()
                .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable
                    && c.literal.family_id() == goal.family_id())
                .count(),
            1,
            "{source}"
        );
    }
}

#[test]
fn modal_defeater_defends_but_cannot_supply_productive_support() {
    // Definition 8(2.3.2): any rule can counter an obligation attacker.
    let modal = "(normally support () (must pay))
                 (normally attacker () (forbidden pay))
                 (except defender () (must pay)) (prefer defender attacker)";
    assert!(tag(modal, "(must pay)", ConclusionType::DefeasiblyProvable));
    assert!(!tag(
        &modal.replace("(normally support () (must pay))", ""),
        "(must pay)",
        ConclusionType::DefeasiblyProvable
    ));
    let plain = "(normally support () pay) (normally attacker () (not pay))
                 (except defender () pay) (prefer defender attacker)";
    assert!(tag(plain, "pay", ConclusionType::DefeasiblyNotProvable));
}
