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

// ===================== Cross-offset datetime comparison =====================

#[test]
fn cross_offset_datetime_comparison() {
    // AEST (UTC+10) 09:00 is earlier than UTC+8 09:00 (because AEST 09:00 = 23:00 UTC previous day,
    // while UTC+8 09:00 = 01:00 UTC same day)
    let input = r#"
(given (ev a #dt:2025-07-15T09:00:00+10:00))
(given (ev b #dt:2025-07-15T09:00:00+08:00))
(normally r1
  (and (ev a ?dt-a)
       (ev b ?dt-b)
       (< ?dt-a ?dt-b))
  (a-before-b))
"#;
    let conclusions = run_spl(input);
    assert!(
        has_conclusion(&conclusions, "a-before-b", true),
        "AEST 09:00 (UTC-1d 23:00) should be before UTC+8 09:00 (UTC 01:00). Got: {:?}",
        conclusions
            .iter()
            .map(|c| format!("{}", c.literal))
            .collect::<Vec<_>>()
    );
}

// ===================== Mixed temporal min/max error =====================

#[test]
fn mixed_temporal_min_max_error() {
    // min with Date and Time should fail
    let input = r#"
(given (d #d:2025-01-01))
(given (t #t:09:00))
(normally r1
  (and (d ?d)
       (t ?t)
       (bind ?m (min ?d ?t)))
  (result ?m))
"#;
    let conclusions = run_spl(input);
    // Should NOT produce a result (error in bind)
    assert!(
        !has_conclusion(&conclusions, "result", true),
        "min of Date and Time should fail. Got: {:?}",
        conclusions
            .iter()
            .map(|c| format!("{}", c.literal))
            .collect::<Vec<_>>()
    );
}

// ===================== Fold with durations =====================

#[test]
fn fold_sum_durations() {
    let input = r#"
(given (task a #dur:2h))
(given (task b #dur:1h30m))
(given (task c #dur:45m))
(normally r-total
  (fold ?sum #dur:0m + ?d (task ?name ?d))
  (total-duration ?sum))
"#;
    let conclusions = run_spl(input);
    // 2h + 1h30m + 45m = 120 + 90 + 45 = 255 minutes = 4h15m
    assert!(
        has_conclusion(&conclusions, "total-duration(#dur:4h15m)", true),
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
