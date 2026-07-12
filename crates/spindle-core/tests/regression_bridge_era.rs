//! Regression tests for bridge-era failure modes.
//!
//! The old temporal bridge approach synthesized `__bridge::*` rules to connect
//! temporal literals to their atemporal families. This caused recurring bugs
//! in strength preservation, polarity, defeater identity, trust provenance,
//! and deduplication. The new projection-token approach (SPEC-020) eliminates
//! these failure modes by construction.
//!
//! Each test in this file targets a specific bridge-era bug category and
//! asserts the correct behavior that the projection redesign must maintain.

mod fixtures;

use std::collections::HashSet;

use spindle_core::conclusion::{Conclusion, ConclusionType};
use spindle_core::index::IndexedTheory;
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::projection::{FamilyId, ProjectionEngine, ProjectionToken};
use spindle_core::reason::{reason, reason_full};
use spindle_core::rule::{Rule, RuleType};
use spindle_core::temporal::{Temporal, TimePoint};
use spindle_core::theory::Theory;

// ===========================================================================
// Helpers
// ===========================================================================

fn has_conclusion(
    conclusions: &[Conclusion],
    name: &str,
    negated: bool,
    ct: ConclusionType,
) -> bool {
    conclusions.iter().any(|c| {
        c.conclusion_type == ct && c.literal.name() == name && c.literal.negation == negated
    })
}

fn conclusion_set(conclusions: &[Conclusion]) -> HashSet<String> {
    conclusions.iter().map(|c| format!("{c}")).collect()
}

fn project_rule_tokens(theory: &Theory, label: &str) -> Vec<ProjectionToken> {
    let mut idx = IndexedTheory::build(theory);
    let rule = theory.get_rule(label).unwrap();
    let mut engine = ProjectionEngine::new();
    engine.project_rule(rule, &mut idx);
    engine.into_tokens()
}

// ===========================================================================
// 1. Strength preservation
//
// Bridge-era bug: generated bridge rules sometimes lost the strength
// (defeasible/strict/defeater) of the original temporal rule.
// Fix commit: 81feb71
// ===========================================================================

#[test]
fn strength_defeasible_temporal_projects_defeasible_support() {
    // A defeasible rule with temporal head must project FamilySupport
    // with RuleType::Defeasible — not Strict or Fact.
    let head = Literal::new(
        "q",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_rule(Rule::defeasible("r1", vec![Literal::simple("a")], head));

    let tokens = project_rule_tokens(&theory, "r1");
    let family_tok = tokens
        .iter()
        .find_map(|t| match t {
            ProjectionToken::Family(fs) => Some(fs),
            _ => None,
        })
        .expect("should emit FamilySupport");

    assert_eq!(
        family_tok.rule_type,
        RuleType::Defeasible,
        "bridge-era regression: temporal defeasible rule lost its strength in projection"
    );
}

#[test]
fn strength_strict_temporal_projects_strict_support() {
    let head = Literal::new(
        "q",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_rule(Rule::strict("s1", vec![Literal::simple("a")], head));

    let tokens = project_rule_tokens(&theory, "s1");
    let family_tok = tokens
        .iter()
        .find_map(|t| match t {
            ProjectionToken::Family(fs) => Some(fs),
            _ => None,
        })
        .expect("should emit FamilySupport");

    assert_eq!(
        family_tok.rule_type,
        RuleType::Strict,
        "bridge-era regression: temporal strict rule lost its strength in projection"
    );
}

#[test]
fn strength_defeater_temporal_projects_defeater_attack() {
    let head = Literal::new(
        "q",
        true,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_rule(Rule::defeater("d1", vec![Literal::simple("a")], head));

    let tokens = project_rule_tokens(&theory, "d1");
    let attack_tok = tokens
        .iter()
        .find_map(|t| match t {
            ProjectionToken::Attack(fa) => Some(fa),
            _ => None,
        })
        .expect("should emit FamilyAttack");

    assert_eq!(
        attack_tok.rule_type,
        RuleType::Defeater,
        "bridge-era regression: temporal defeater lost its strength in projection"
    );
}

// ===========================================================================
// 2. Polarity preservation
//
// Bridge-era bug: bridge rules inverted or dropped the negation of
// conclusions during synthesis.
// Fix commit: abcb29a
// ===========================================================================

#[test]
fn polarity_positive_temporal_projects_to_positive_family() {
    let head = Literal::new(
        "q",
        false, // positive
        Mode::empty(),
        Temporal::new(TimePoint::Moment(5), TimePoint::Moment(15)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_rule(Rule::defeasible("r1", vec![Literal::simple("a")], head));

    let tokens = project_rule_tokens(&theory, "r1");
    let family_tok = tokens
        .iter()
        .find_map(|t| match t {
            ProjectionToken::Family(fs) => Some(fs),
            _ => None,
        })
        .unwrap();

    assert!(
        !family_tok.family_id.negated(),
        "bridge-era regression: positive temporal rule projected to negated family"
    );
}

#[test]
fn polarity_negated_temporal_projects_to_negated_family() {
    let head = Literal::new(
        "q",
        true, // negated
        Mode::empty(),
        Temporal::new(TimePoint::Moment(5), TimePoint::Moment(15)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_rule(Rule::defeasible("r1", vec![Literal::simple("a")], head));

    let tokens = project_rule_tokens(&theory, "r1");
    let family_tok = tokens
        .iter()
        .find_map(|t| match t {
            ProjectionToken::Family(fs) => Some(fs),
            _ => None,
        })
        .unwrap();

    assert!(
        family_tok.family_id.negated(),
        "bridge-era regression: negated temporal rule projected to positive family (polarity inversion)"
    );
}

#[test]
fn polarity_negated_defeater_attacks_negated_family() {
    // A defeater ~q[1,10] should attack the ~q family, not the q family.
    let head = Literal::new(
        "q",
        true,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_rule(Rule::defeater("d1", vec![Literal::simple("a")], head));

    let tokens = project_rule_tokens(&theory, "d1");
    let attack_tok = tokens
        .iter()
        .find_map(|t| match t {
            ProjectionToken::Attack(fa) => Some(fa),
            _ => None,
        })
        .unwrap();

    let expected_family = FamilyId::from(&Literal::negated("q"));
    assert_eq!(
        attack_tok.family_id, expected_family,
        "bridge-era regression: negated defeater attacked wrong family (polarity lost)"
    );
}

// ===========================================================================
// 3. Defeater applicability preservation
//
// Bridge-era bug: temporal defeaters lost their original body constraints
// when synthesized as bridge rules, making them fire unconditionally.
// Fix commit: 40be048
// ===========================================================================

#[test]
fn defeater_body_conditions_respected() {
    // Theory: fact bird, rule r1: bird => flies,
    //         defeater d1: broken_wing ~> ~flies
    // Without the fact "broken_wing", the defeater should NOT block flies.
    let mut theory = Theory::new();
    theory.add_fact("bird");
    theory.add_defeasible_rule(&["bird"], "flies");
    theory.add_defeater(&["broken_wing"], "~flies");

    let conclusions = reason(&theory).unwrap();

    assert!(
        has_conclusion(
            &conclusions,
            "flies",
            false,
            ConclusionType::DefeasiblyProvable
        ),
        "bridge-era regression: defeater blocked conclusion despite unsatisfied body"
    );
}

#[test]
fn defeater_body_conditions_block_when_satisfied() {
    // Same theory but with broken_wing fact present — defeater should block.
    let mut theory = Theory::new();
    theory.add_fact("bird");
    theory.add_fact("broken_wing");
    theory.add_defeasible_rule(&["bird"], "flies");
    theory.add_defeater(&["broken_wing"], "~flies");

    let conclusions = reason(&theory).unwrap();

    assert!(
        !has_conclusion(
            &conclusions,
            "flies",
            false,
            ConclusionType::DefeasiblyProvable
        ),
        "defeater should block +d flies when body is satisfied"
    );
}

// ===========================================================================
// 4. Temporal defeater attack identity (label distinctness)
//
// Bridge-era bug: distinct temporal defeaters with different labels but
// similar structure were collapsed into one attack via deduplication.
// Fix commit: 6ecd3ad
// ===========================================================================

#[test]
fn distinct_defeaters_produce_distinct_attacks() {
    // Two defeaters d1 and d2 with same body/head but different labels
    // must remain distinct for superiority resolution.
    let mut theory = Theory::new();
    theory.add_rule(Rule::defeater(
        "d1",
        vec![Literal::simple("a")],
        Literal::negated("q"),
    ));
    theory.add_rule(Rule::defeater(
        "d2",
        vec![Literal::simple("a")],
        Literal::negated("q"),
    ));

    let mut idx = IndexedTheory::build(&theory);
    let mut engine = ProjectionEngine::new();
    engine.project_rule(theory.get_rule("d1").unwrap(), &mut idx);
    engine.project_rule(theory.get_rule("d2").unwrap(), &mut idx);

    let attack_labels: Vec<_> = engine
        .tokens()
        .iter()
        .filter_map(|t| match t {
            ProjectionToken::Attack(a) => Some(a.rule_label.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(
        attack_labels.len(),
        2,
        "bridge-era regression: distinct defeaters collapsed into single attack"
    );
    assert!(attack_labels.contains(&"d1".to_string()));
    assert!(attack_labels.contains(&"d2".to_string()));
}

#[test]
fn distinct_temporal_defeaters_produce_distinct_attacks() {
    // Same test but with temporal heads — bridge dedup was especially fragile here.
    let head = |start, end| {
        Literal::new(
            "q",
            true,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(start), TimePoint::Moment(end)),
            vec![],
        )
    };

    let mut theory = Theory::new();
    theory.add_rule(Rule::defeater(
        "d1",
        vec![Literal::simple("a")],
        head(1, 10),
    ));
    theory.add_rule(Rule::defeater(
        "d2",
        vec![Literal::simple("a")],
        head(1, 10),
    ));

    let mut idx = IndexedTheory::build(&theory);
    let mut engine = ProjectionEngine::new();
    engine.project_rule(theory.get_rule("d1").unwrap(), &mut idx);
    engine.project_rule(theory.get_rule("d2").unwrap(), &mut idx);

    let attack_labels: HashSet<_> = engine
        .tokens()
        .iter()
        .filter_map(|t| match t {
            ProjectionToken::Attack(a) => Some(a.rule_label.clone()),
            _ => None,
        })
        .collect();

    assert_eq!(
        attack_labels.len(),
        2,
        "bridge-era regression: temporal defeaters with same shape collapsed"
    );
}

// ===========================================================================
// 5. Trust provenance (label preservation)
//
// Bridge-era bug: trust degrees resolved through synthetic __bridge::*
// labels instead of original authored rule labels.
// Fix commits: d6ce7f1, 07ab8f9
// ===========================================================================

#[test]
fn projection_tokens_carry_original_label() {
    // A rule with a known label must project tokens using that same label,
    // never a synthetic __bridge:: label.
    let head = Literal::new(
        "result",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(100)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_rule(Rule::defeasible(
        "my_rule",
        vec![Literal::simple("input")],
        head,
    ));

    let tokens = project_rule_tokens(&theory, "my_rule");

    for token in &tokens {
        let label = match token {
            ProjectionToken::Exact(s) => &s.rule_label,
            ProjectionToken::Family(s) => &s.rule_label,
            ProjectionToken::Attack(a) => &a.rule_label,
        };
        assert_eq!(
            label, "my_rule",
            "bridge-era regression: projection token uses synthetic label instead of original"
        );
        assert!(
            !label.starts_with("__bridge"),
            "bridge-era regression: synthetic __bridge label leaked into projection tokens"
        );
    }
}

#[test]
fn grounded_instance_projection_uses_grounded_label() {
    // Grounded instances should keep their grounded label in projection
    // output so consumers can join tokens back to conclusions and bodies.
    let mut rule = Rule::defeasible(
        "r1__ground_0",
        vec![Literal::simple("a")],
        Literal::simple("b"),
    );
    rule.template_label = Some("r1".to_string());

    let mut theory = Theory::new();
    theory.add_rule(rule);

    let tokens = project_rule_tokens(&theory, "r1__ground_0");

    for token in &tokens {
        let label = match token {
            ProjectionToken::Exact(s) => &s.rule_label,
            ProjectionToken::Family(s) => &s.rule_label,
            ProjectionToken::Attack(a) => &a.rule_label,
        };
        assert_eq!(
            label, "r1__ground_0",
            "bridge-era regression: grounded instance label was rewritten, breaking joins against conclusions"
        );
    }
}

// ===========================================================================
// 6. Deduplication stability
//
// Bridge-era bug: multiple temporal facts with equivalent heads created
// duplicate bridge rules, and deduplication logic was fragile.
// Fix commits: 0a2b789, 1095dde
// ===========================================================================

#[test]
fn duplicate_temporal_facts_no_duplicate_family_support() {
    // Two facts for the same literal (but added twice) should not create
    // duplicate family support tokens for the same rule label.
    let lit = Literal::new(
        "sensor",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", lit.clone()));

    // Projecting the same rule twice should produce identical tokens.
    let tokens_1 = project_rule_tokens(&theory, "f1");
    let tokens_2 = project_rule_tokens(&theory, "f1");

    assert_eq!(
        tokens_1.len(),
        tokens_2.len(),
        "bridge-era regression: repeated projection produced inconsistent token counts"
    );
}

#[test]
fn different_temporal_windows_share_family() {
    // Two temporal variants p[1,10] and p[20,30] must share the same FamilyId
    // but produce distinct ExactLitIds — this is the foundation that replaces
    // bridge dedup.
    let lit_a = Literal::new(
        "p",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let lit_b = Literal::new(
        "p",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30)),
        vec![],
    );

    let family_a = FamilyId::from(&lit_a);
    let family_b = FamilyId::from(&lit_b);
    assert_eq!(
        family_a, family_b,
        "bridge-era regression: temporal variants assigned different families"
    );

    // But they should produce distinct exact IDs in the index.
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", lit_a));
    theory.add_rule(Rule::fact("f2", lit_b));

    let mut idx = IndexedTheory::build(&theory);
    let exact_a = idx.exact_lit_id(&theory.get_rule("f1").unwrap().head[0]);
    let exact_b = idx.exact_lit_id(&theory.get_rule("f2").unwrap().head[0]);
    assert_ne!(
        exact_a, exact_b,
        "bridge-era regression: different temporal windows collapsed to same ExactLitId"
    );
}

// ===========================================================================
// 7. Atemporal body matching via family projection
//
// Bridge-era bug: atemporal body literals like `p` in `p => q` required
// synthetic bridge rules to match temporal proofs like `p[1,10]`.
// Now family-level projection handles this by construction.
// ===========================================================================

#[test]
fn atemporal_body_compiled_as_family_key() {
    // An atemporal body literal should compile to a Family match key,
    // which means any temporal variant of that family can satisfy it.
    use spindle_core::compilation::{BodyMatchKey, compile_rule};

    let mut theory = Theory::new();
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::simple("p")],
        Literal::simple("q"),
    ));

    let mut idx = IndexedTheory::build(&theory);
    let rule = theory.get_rule("r1").unwrap();
    let compiled = compile_rule(rule, &mut idx);

    assert!(
        compiled.keys()[0].is_family(),
        "bridge-era regression: atemporal body not compiled as family key"
    );
    let expected = FamilyId::from(&Literal::simple("p"));
    assert_eq!(
        compiled.keys()[0],
        BodyMatchKey::Family(expected),
        "bridge-era regression: wrong family for atemporal body"
    );
}

#[test]
fn temporal_body_compiled_as_exact_key() {
    // A temporal body literal should compile to an Exact match key,
    // requiring precise temporal window matching (not family-level).
    use spindle_core::compilation::compile_rule;

    let temporal_lit = Literal::new(
        "sensor",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![temporal_lit],
        Literal::simple("alert"),
    ));

    let mut idx = IndexedTheory::build(&theory);
    let rule = theory.get_rule("r1").unwrap();
    let compiled = compile_rule(rule, &mut idx);

    assert!(
        compiled.keys()[0].is_exact(),
        "bridge-era regression: temporal body not compiled as exact key"
    );
}

// ===========================================================================
// 8. No synthetic bridge rules in theory
//
// The projection redesign must not inject any synthetic rules into the
// prepared theory. This was the root cause of all bridge-era bugs.
// ===========================================================================

#[test]
fn no_bridge_rules_in_theory() {
    // After reasoning, no rule in the theory should have a __bridge:: label.
    let mut theory = Theory::new();
    theory.add_fact("p");
    theory.add_defeasible_rule(&["p"], "q");

    let conclusions = reason(&theory).unwrap();
    assert!(!conclusions.is_empty());

    for rule in theory.rules() {
        assert!(
            !rule.label.starts_with("__bridge"),
            "bridge-era regression: synthetic __bridge rule found in theory: {}",
            rule.label
        );
    }
}

#[test]
fn no_bridge_rules_with_temporal_theory() {
    let head = Literal::new(
        "q",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_rule(Rule::defeasible("r1", vec![Literal::simple("a")], head));

    let conclusions = reason(&theory).unwrap();
    assert!(!conclusions.is_empty());

    for rule in theory.rules() {
        assert!(
            !rule.label.starts_with("__bridge"),
            "bridge-era regression: synthetic __bridge rule in temporal theory: {}",
            rule.label
        );
    }
}

// ===========================================================================
// 9. Shadow mode consistency for bridge-era scenarios
//
// End-to-end tests verifying shadow mode reports zero divergences for
// scenarios that previously triggered bridge bugs.
// ===========================================================================

#[test]
fn reason_full_temporal_defeasible_projection() {
    // Temporal defeasible rule projects to atemporal family with correct
    // strength — reason and reason_full must agree on conclusions.
    let head = Literal::new(
        "q",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_rule(Rule::defeasible("r1", vec![Literal::simple("a")], head));

    let standard = reason(&theory).unwrap();
    let full = reason_full(&theory).unwrap();
    assert_eq!(
        conclusion_set(&standard),
        conclusion_set(&full.conclusions),
        "reason and reason_full should agree on temporal defeasible projection"
    );
    assert!(
        !full.projection_tokens.is_empty(),
        "temporal defeasible rule should produce projection tokens"
    );
}

#[test]
fn reason_full_temporal_defeater_with_superiority() {
    // A temporal defeater that interacts with superiority — this was a
    // bridge-era pain point because __bridge labels broke superiority lookup.
    let mut theory = Theory::new();
    theory.add_fact("bird");
    theory.add_fact("penguin");
    theory.add_fact("injured");

    let r1 = theory.add_defeasible_rule(&["bird"], "flies");
    let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
    theory.add_superiority(&r2, &r1);
    theory.add_defeater(&["injured"], "~flies");

    let standard = reason(&theory).unwrap();
    let full = reason_full(&theory).unwrap();
    assert_eq!(
        conclusion_set(&standard),
        conclusion_set(&full.conclusions),
        "reason and reason_full should agree on temporal defeater + superiority"
    );

    // Penguin rule should still win via superiority.
    assert!(
        has_conclusion(
            &full.conclusions,
            "flies",
            true,
            ConclusionType::DefeasiblyProvable
        ),
        "expected +d ~flies via superiority"
    );
}

#[test]
fn reason_full_multiple_temporal_windows() {
    // Multiple temporal windows for the same predicate — bridge dedup
    // used to fail here.
    let mut theory = Theory::new();
    theory.add_fact("sensor");

    let head_early = Literal::new(
        "alert",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );
    let head_late = Literal::new(
        "alert",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30)),
        vec![],
    );
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::simple("sensor")],
        head_early,
    ));
    theory.add_rule(Rule::defeasible(
        "r2",
        vec![Literal::simple("sensor")],
        head_late,
    ));

    let standard = reason(&theory).unwrap();
    let full = reason_full(&theory).unwrap();
    assert_eq!(
        conclusion_set(&standard),
        conclusion_set(&full.conclusions),
        "reason and reason_full should agree on multiple temporal windows"
    );
}

#[test]
fn reason_full_modal_temporal_interaction() {
    // Modal + temporal combination — the bridge had trouble with modes.
    let mut theory = Theory::new();
    theory.add_fact("citizen");

    let head = Literal::new(
        "pay_taxes",
        false,
        Mode::obligation(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(365)),
        vec![],
    );
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::simple("citizen")],
        head,
    ));

    let standard = reason(&theory).unwrap();
    let full = reason_full(&theory).unwrap();
    assert_eq!(
        conclusion_set(&standard),
        conclusion_set(&full.conclusions),
        "reason and reason_full should agree on modal+temporal interaction"
    );
}

// ===========================================================================
// 10. Family identity invariants
//
// Correct family grouping is the foundation of the projection approach.
// Bridge-era bugs often stemmed from incorrect grouping.
// ===========================================================================

#[test]
fn family_ignores_temporal_bounds() {
    let lit_unbounded = Literal::simple("p");
    let lit_bounded = Literal::new(
        "p",
        false,
        Mode::empty(),
        Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
        vec![],
    );

    assert_eq!(
        FamilyId::from(&lit_unbounded),
        FamilyId::from(&lit_bounded),
        "bridge-era regression: temporal bounds affected family identity"
    );
}

#[test]
fn family_separates_negation() {
    let pos = Literal::simple("p");
    let neg = Literal::negated("p");

    assert_ne!(
        FamilyId::from(&pos),
        FamilyId::from(&neg),
        "bridge-era regression: positive and negative literals share family"
    );
}

#[test]
fn family_separates_args() {
    let lit_a = Literal::new(
        "parent",
        false,
        Mode::empty(),
        Temporal::empty(),
        vec!["alice".into()],
    );
    let lit_b = Literal::new(
        "parent",
        false,
        Mode::empty(),
        Temporal::empty(),
        vec!["bob".into()],
    );

    assert_ne!(
        FamilyId::from(&lit_a),
        FamilyId::from(&lit_b),
        "bridge-era regression: different predicate args share family"
    );
}

#[test]
fn family_separates_modes() {
    let plain = Literal::simple("pay");
    let obligatory = Literal::new("pay", false, Mode::obligation(), Temporal::empty(), vec![]);

    assert_ne!(
        FamilyId::from(&plain),
        FamilyId::from(&obligatory),
        "bridge-era regression: different modal operators share family"
    );
}

#[test]
fn family_complement_roundtrips() {
    let family = FamilyId::from(&Literal::simple("p"));
    let comp = family.complement();
    assert!(comp.negated());
    assert_eq!(comp.complement(), family, "complement should be involutive");
}

// ===========================================================================
// 11. Diagnostics and observability (OBS-001, OBS-002)
//
// The projection approach provides structured diagnostics that the bridge
// approach lacked, making debugging transparent rather than opaque.
// ===========================================================================

#[test]
fn diagnostics_count_all_token_types() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", Literal::simple("bird")));
    theory.add_rule(Rule::defeater(
        "d1",
        vec![Literal::simple("broken")],
        Literal::negated("flies"),
    ));

    let mut idx = IndexedTheory::build(&theory);
    let mut engine = ProjectionEngine::new();
    engine.project_rule(theory.get_rule("f1").unwrap(), &mut idx);
    engine.project_rule(theory.get_rule("d1").unwrap(), &mut idx);

    let diag = engine.diagnostics();
    assert_eq!(diag.exact_supports, 2, "expected 2 exact supports");
    assert_eq!(diag.family_supports, 1, "expected 1 family support");
    assert_eq!(diag.family_attacks, 1, "expected 1 family attack");
    assert_eq!(diag.total_tokens(), 4, "expected 4 total tokens");
    assert!(diag.contributing_labels.contains("f1"));
    assert!(diag.contributing_labels.contains("d1"));
}

#[test]
fn snapshot_deterministic_across_insertion_order() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f2", Literal::simple("b")));
    theory.add_rule(Rule::fact("f1", Literal::simple("a")));

    let mut idx = IndexedTheory::build(&theory);
    let mut engine = ProjectionEngine::new();
    // Project in reverse label order.
    engine.project_rule(theory.get_rule("f2").unwrap(), &mut idx);
    engine.project_rule(theory.get_rule("f1").unwrap(), &mut idx);

    // Expected canonical keys, derived through the same public API.
    let id_a = idx.exact_lit_id(&Literal::simple("a"));
    let id_b = idx.exact_lit_id(&Literal::simple("b"));
    let key_a = idx.exact_lit_key(id_a).unwrap();
    let key_b = idx.exact_lit_key(id_b).unwrap();

    let snap = engine.snapshot(&idx);
    // Entries should be sorted by label despite reverse insertion.
    assert_eq!(snap.exact[0].0, "f1");
    assert_eq!(snap.exact[1].0, "f2");
    // Exact literals are rendered by canonical semantic key
    // (process-independent), not by interning slot number.
    assert_eq!(snap.exact[0].1, key_a);
    assert_eq!(snap.exact[1].1, key_b);

    // Repeated calls produce identical results.
    assert_eq!(snap, engine.snapshot(&idx));
}

// ===========================================================================
// 12. End-to-end reasoning correctness on bridge-era scenarios
//
// Full reasoning tests that exercise the complete pipeline for scenarios
// that historically exposed bridge bugs.
// ===========================================================================

#[test]
fn tweety_triangle_correct_conclusions() {
    let theory = fixtures::tweety_triangle();
    let conclusions = reason(&theory).unwrap();

    // Penguin wins: +d ~flies, NOT +d flies.
    assert!(
        has_conclusion(
            &conclusions,
            "flies",
            true,
            ConclusionType::DefeasiblyProvable
        ),
        "expected +d ~flies"
    );
    assert!(
        !has_conclusion(
            &conclusions,
            "flies",
            false,
            ConclusionType::DefeasiblyProvable
        ),
        "should NOT have +d flies (penguin rule is superior)"
    );
}

#[test]
fn nixon_diamond_ambiguity_blocks_both() {
    let theory = fixtures::nixon_diamond();
    let conclusions = reason(&theory).unwrap();

    // Neither +d pacifist nor +d ~pacifist should hold (ambiguity).
    assert!(
        !has_conclusion(
            &conclusions,
            "pacifist",
            false,
            ConclusionType::DefeasiblyProvable
        ),
        "should NOT have +d pacifist (ambiguity)"
    );
    assert!(
        !has_conclusion(
            &conclusions,
            "pacifist",
            true,
            ConclusionType::DefeasiblyProvable
        ),
        "should NOT have +d ~pacifist (ambiguity)"
    );
}

#[test]
fn defeaters_block_without_proving() {
    let theory = fixtures::conflicting_defeaters();
    let conclusions = reason(&theory).unwrap();

    // Defeaters should block +d flies but NOT prove +d ~flies.
    assert!(
        !has_conclusion(
            &conclusions,
            "flies",
            false,
            ConclusionType::DefeasiblyProvable
        ),
        "defeaters should block +d flies"
    );
    assert!(
        !has_conclusion(
            &conclusions,
            "flies",
            true,
            ConclusionType::DefeasiblyProvable
        ),
        "defeaters should NOT prove +d ~flies (defeaters only block)"
    );
}

#[test]
fn reason_and_reason_full_agree_on_all_fixtures() {
    let fixture_theories: Vec<(&str, Theory)> = vec![
        ("tweety", fixtures::tweety_triangle()),
        ("nixon", fixtures::nixon_diamond()),
        ("chain_5", fixtures::inheritance_chain(5)),
        ("temporal", fixtures::temporal_theory()),
        ("modal", fixtures::modal_theory()),
        ("defeaters", fixtures::conflicting_defeaters()),
    ];

    for (name, theory) in fixture_theories {
        let standard = reason(&theory).unwrap();
        let full = reason_full(&theory).unwrap();

        let standard_set = conclusion_set(&standard);
        let full_set = conclusion_set(&full.conclusions);

        assert_eq!(
            standard_set, full_set,
            "bridge-era regression: reason and reason_full disagree on fixture '{name}'"
        );
    }
}
