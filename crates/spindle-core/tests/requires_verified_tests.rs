//! Verified requires API regression and contract tests.

use spindle_core::literal::Literal;
use spindle_core::query::{RequiresOptions, RequiresSearchStatus, requires, requires_with_options};
use spindle_core::rule::Rule;
use spindle_core::temporal::{Temporal, TimePoint};
use spindle_core::theory::Theory;

fn temporal_lit(name: &str, start: i64, end: i64) -> Literal {
    Literal::new(
        name,
        false,
        spindle_core::mode::Mode::empty(),
        Temporal::new(TimePoint::Moment(start), TimePoint::Moment(end)),
        vec![],
    )
}

#[test]
fn test_requires_with_options_already_provable_has_zero_verification_counts() {
    let mut theory = Theory::new();
    theory.add_fact("p");

    let result = requires_with_options(
        &theory,
        &Literal::simple("p"),
        RequiresOptions {
            max_solutions: 3,
            max_raw_candidates: 5,
        },
    )
    .unwrap();

    assert!(result.already_provable);
    assert!(result.solutions.is_empty());
    assert_eq!(result.search_status, RequiresSearchStatus::BoundedComplete);
    assert_eq!(result.verification.raw_examined, 0);
    assert_eq!(result.verification.accepted, 0);
    assert_eq!(result.verification.rejected, 0);
}

#[test]
fn test_requires_with_options_reports_budget_exhausted() {
    let mut theory = Theory::new();
    theory.add_defeasible_rule(&["a"], "goal");
    theory.add_defeasible_rule(&["b"], "goal");
    theory.add_defeasible_rule(&["c"], "goal");
    theory.add_defeater(&["a"], "~goal");
    theory.add_defeater(&["b"], "~goal");
    theory.add_defeater(&["c"], "~goal");

    let result = requires_with_options(
        &theory,
        &Literal::simple("goal"),
        RequiresOptions {
            max_solutions: 5,
            max_raw_candidates: 1,
        },
    )
    .unwrap();

    assert!(!result.already_provable);
    assert!(result.solutions.is_empty());
    assert_eq!(result.search_status, RequiresSearchStatus::BudgetExhausted);
    assert_eq!(result.verification.raw_examined, 1);
    assert_eq!(result.verification.accepted, 0);
    assert_eq!(result.verification.rejected, 1);
}

#[test]
fn test_requires_with_options_treats_limit_as_raw_candidate_budget() {
    let mut theory = Theory::new();
    theory.add_defeasible_rule(&["a"], "goal");
    theory.add_defeasible_rule(&["b", "c"], "goal");
    theory.add_defeater(&["a"], "~goal");

    let result = requires_with_options(
        &theory,
        &Literal::simple("goal"),
        RequiresOptions {
            max_solutions: 5,
            max_raw_candidates: 1,
        },
    )
    .unwrap();

    assert!(!result.already_provable);
    assert!(result.solutions.is_empty());
    assert_eq!(result.search_status, RequiresSearchStatus::BudgetExhausted);
    assert_eq!(result.verification.raw_examined, 1);
    assert_eq!(result.verification.accepted, 0);
    assert_eq!(result.verification.rejected, 1);
}

#[test]
fn test_requires_with_options_reports_budget_exhausted_with_duplicate_candidates() {
    let mut theory = Theory::new();
    theory.add_defeasible_rule(&["a"], "goal");
    theory.add_defeasible_rule(&["a"], "goal");

    let result = requires_with_options(
        &theory,
        &Literal::simple("goal"),
        RequiresOptions {
            max_solutions: 5,
            max_raw_candidates: 1,
        },
    )
    .unwrap();

    assert!(!result.already_provable);
    // abduce() merges duplicate fact-sets before returning, so two identical
    // rules produce only one raw candidate — within budget.
    assert_eq!(result.search_status, RequiresSearchStatus::BoundedComplete);
}

#[test]
fn test_requires_with_options_deduplicates_and_orders_solutions() {
    let mut theory = Theory::new();
    theory.add_defeasible_rule(&["a"], "goal");
    theory.add_defeasible_rule(&["a"], "goal"); // duplicate fact-set candidate
    theory.add_defeasible_rule(&["b"], "goal");

    let result = requires_with_options(
        &theory,
        &Literal::simple("goal"),
        RequiresOptions {
            max_solutions: 10,
            max_raw_candidates: 10,
        },
    )
    .unwrap();

    assert_eq!(result.search_status, RequiresSearchStatus::BoundedComplete);
    assert_eq!(
        result.solutions.len(),
        2,
        "Duplicate candidates should be removed"
    );

    let mut fact_sets: Vec<Vec<String>> = result
        .solutions
        .iter()
        .map(|s| {
            let mut facts: Vec<_> = s.facts.iter().map(Literal::to_spl).collect();
            facts.sort();
            facts
        })
        .collect();
    fact_sets.sort();

    assert_eq!(
        fact_sets,
        vec![vec!["(a)".to_string()], vec!["(b)".to_string()]]
    );
}

#[test]
fn test_requires_wrapper_matches_first_verified_solution() {
    let mut theory = Theory::new();
    theory.add_defeasible_rule(&["a"], "goal");
    theory.add_defeasible_rule(&["b"], "goal");

    let wrapper_result = requires(&theory, &Literal::simple("goal")).unwrap();
    let verified_result = requires_with_options(
        &theory,
        &Literal::simple("goal"),
        RequiresOptions {
            max_solutions: 1,
            max_raw_candidates: 1000,
        },
    )
    .unwrap();

    let expected: std::collections::HashSet<_> = verified_result
        .solutions
        .first()
        .map(|s| s.facts.iter().cloned().collect())
        .unwrap_or_default();
    let actual: std::collections::HashSet<_> = wrapper_result.into_iter().collect();
    assert_eq!(actual, expected);
}

#[test]
fn test_requires_with_options_rejects_zero_limits() {
    let theory = Theory::new();
    let goal = Literal::simple("goal");

    let err = requires_with_options(
        &theory,
        &goal,
        RequiresOptions {
            max_solutions: 0,
            max_raw_candidates: 1,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "VALIDATION_ERROR");

    let err = requires_with_options(
        &theory,
        &goal,
        RequiresOptions {
            max_solutions: 1,
            max_raw_candidates: 0,
        },
    )
    .unwrap_err();
    assert_eq!(err.code(), "VALIDATION_ERROR");
}

#[test]
fn test_requires_accepts_family_matched_temporal_head_for_atemporal_goal() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::simple("a")],
        temporal_lit("p", 1, 10),
    ));

    let result = requires_with_options(
        &theory,
        &Literal::simple("p"),
        RequiresOptions {
            max_solutions: 3,
            max_raw_candidates: 10,
        },
    )
    .unwrap();

    assert!(!result.already_provable);
    assert_eq!(result.verification.accepted, 1);
    let solution = result.solutions.first().unwrap();
    assert!(solution.facts.contains(&Literal::simple("a")));
}

#[test]
fn test_requires_bounded_goal_does_not_treat_other_window_as_provable() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", temporal_lit("p", 20, 30)));

    let result = requires_with_options(
        &theory,
        &temporal_lit("p", 1, 10),
        RequiresOptions {
            max_solutions: 3,
            max_raw_candidates: 10,
        },
    )
    .unwrap();

    assert!(!result.already_provable);
    let solution = result.solutions.first().unwrap();
    let fact = solution.facts.iter().next().unwrap();
    assert_eq!(fact.to_spl(), temporal_lit("p", 1, 10).to_spl());
}
