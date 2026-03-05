//! Integration tests: full SPL → parse → prepare → reason pipeline with temporal types.

use spindle_core::conclusion::{Conclusion, ConclusionType};
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::reason::reason_from_prepared;
use spindle_parser::parse_spl;

fn run_spl(input: &str) -> Vec<Conclusion> {
    let theory = parse_spl(input).expect("parse should succeed");
    let result = prepare(&theory, PrepareOptions::default()).expect("prepare should succeed");
    reason_from_prepared(&result).expect("reasoning should succeed")
}

fn has_conclusion(conclusions: &[Conclusion], name: &str, positive: bool) -> bool {
    conclusions.iter().any(|c| {
        let lit_str = format!("{}", c.literal);
        lit_str.contains(name)
            && (c.conclusion_type == ConclusionType::DefeasiblyProvable) == positive
    })
}

// ===================== Basic temporal facts =====================

#[test]
fn temporal_facts_in_theory() {
    let input = r#"
(given (shift-date alice #d:2025-07-15))
(given (shift-start alice #t:09:00))
(given (shift-end alice #t:17:00))
"#;
    let theory = parse_spl(input).unwrap();
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 3);
}

// ===================== Temporal values in defeasible rules =====================

#[test]
fn temporal_comparison_in_rule() {
    let input = r#"
(given (start alice #t:05:00))
(given (start bob #t:09:00))
(normally r1
  (and (start ?who ?time)
       (< ?time #t:06:00))
  (early-shift ?who))
"#;
    let conclusions = run_spl(input);
    assert!(
        has_conclusion(&conclusions, "early-shift(alice)", true),
        "alice starts at 05:00, should be early-shift. Got: {:?}",
        conclusions
            .iter()
            .map(|c| format!("{}", c.literal))
            .collect::<Vec<_>>()
    );
    assert!(
        !has_conclusion(&conclusions, "early-shift(bob)", true),
        "bob starts at 09:00, should not be early-shift"
    );
}

// ===================== Temporal bind expressions =====================

#[test]
fn temporal_bind_day_of_week() {
    let input = r#"
(given (shift-date alice #d:2025-07-14))
(normally r1
  (and (shift-date ?who ?date)
       (bind ?day (day-of-week ?date)))
  (works-on ?who ?day))
"#;
    let conclusions = run_spl(input);
    // 2025-07-14 is a Monday
    assert!(
        has_conclusion(&conclusions, "works-on(alice, :monday)", true),
        "Got: {:?}",
        conclusions
            .iter()
            .map(|c| format!("{}", c.literal))
            .collect::<Vec<_>>()
    );
}

// ===================== Date component extraction =====================

#[test]
fn date_year_extraction() {
    let input = r#"
(given (event #d:2025-12-25))
(normally r-year
  (and (event ?date)
       (bind ?y (year-of ?date)))
  (event-year ?y))
"#;
    let conclusions = run_spl(input);
    assert!(
        has_conclusion(&conclusions, "event-year(2025)", true),
        "Got: {:?}",
        conclusions
            .iter()
            .map(|c| format!("{}", c.literal))
            .collect::<Vec<_>>()
    );
}

// ===================== Duration arithmetic =====================

#[test]
fn duration_sum_in_bind() {
    let input = r#"
(given (task-a #dur:2h))
(given (task-b #dur:1h30m))
(normally r1
  (and (task-a ?d1)
       (task-b ?d2)
       (bind ?total (+ ?d1 ?d2)))
  (total-time ?total))
"#;
    let conclusions = run_spl(input);
    assert!(
        has_conclusion(&conclusions, "total-time(#dur:3h30m)", true),
        "Got: {:?}",
        conclusions
            .iter()
            .map(|c| format!("{}", c.literal))
            .collect::<Vec<_>>()
    );
}

// ===================== Days between =====================

#[test]
fn days_between_in_rule() {
    let input = r#"
(given (period #d:2025-07-01 #d:2025-07-15))
(normally r1
  (and (period ?start ?end)
       (bind ?days (days-between ?start ?end)))
  (period-length ?days))
"#;
    let conclusions = run_spl(input);
    assert!(
        has_conclusion(&conclusions, "period-length(14)", true),
        "Got: {:?}",
        conclusions
            .iter()
            .map(|c| format!("{}", c.literal))
            .collect::<Vec<_>>()
    );
}

// ===================== Add months =====================

#[test]
fn add_months_clamped() {
    let input = r#"
(given (contract-start #d:2025-01-31))
(normally r1
  (and (contract-start ?date)
       (bind ?review (add-months ?date 1)))
  (first-review ?review))
"#;
    let conclusions = run_spl(input);
    assert!(
        has_conclusion(&conclusions, "first-review(#d:2025-02-28)", true),
        "Got: {:?}",
        conclusions
            .iter()
            .map(|c| format!("{}", c.literal))
            .collect::<Vec<_>>()
    );
}
