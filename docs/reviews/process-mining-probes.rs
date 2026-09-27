// Review probes: assert the desired behavior; failures reproduce review findings.
// See process-mining.md for execution instructions and interpretation.
use spindle_core::mining::*;
use spindle_core::prelude::{Literal, Rule, Theory};
use std::collections::HashMap;

fn positives(log: &EventLog, fact: &str) -> Vec<String> {
    let result = mine_rules(log, 1, 0.0);
    let mut theory = Theory::new();
    theory.add_fact(fact);
    for learned in result.rules {
        theory.add_rule(learned.rule);
    }
    theory
        .reason()
        .unwrap()
        .into_iter()
        .filter(|c| c.is_positive())
        .map(|c| c.literal.name().to_string())
        .collect()
}

#[test]
fn alpha_retains_all_maximal_pairs() {
    let log = make_log_from_traces(&[
        &["s", "a", "x", "e"],
        &["s", "a", "y", "e"],
        &["s", "b", "x", "e"],
        &["s", "b", "y", "e"],
        &["s", "c", "x", "e"],
        &["s", "d", "y", "e"],
    ]);
    let net = AlphaMiner::new().mine(&log);
    let labels: Vec<_> = net.places.iter().map(|p| p.label.as_str()).collect();
    assert!(
        labels.contains(&"p(a,b,x,y)"),
        "missing ({{a,b}},{{x,y}}): {labels:?}"
    );
}

#[test]
fn timestamp_offsets_preserve_chronology() {
    let case = Case::new(
        "offsets",
        vec![
            Event::new("2026-01-17T10:00:00+02:00", "first", HashMap::new()),
            Event::new("2026-01-17T09:00:00Z", "second", HashMap::new()),
        ],
    );
    assert_eq!(case.activities(), vec!["first", "second"]);
}

#[test]
fn timestamp_fractional_seconds_preserve_chronology() {
    let case = Case::new(
        "fractions",
        vec![
            Event::new("2026-01-17T10:00:00Z", "first", HashMap::new()),
            Event::new("2026-01-17T10:00:00.100Z", "second", HashMap::new()),
        ],
    );
    assert_eq!(case.activities(), vec!["first", "second"]);
}

#[test]
fn learned_rules_preserve_exclusive_choice() {
    let log = make_log_from_traces(&[&["start", "approve", "end"], &["start", "reject", "end"]]);
    let conclusions = positives(&log, "start");
    assert!(
        !(conclusions.contains(&"approve".into()) && conclusions.contains(&"reject".into())),
        "both exclusive branches derived: {conclusions:?}"
    );
}

#[test]
fn learned_rules_preserve_and_join() {
    let log = make_log_from_traces(&[&["start", "a", "b", "end"], &["start", "b", "a", "end"]]);
    let conclusions = positives(&log, "a");
    assert!(
        !conclusions.contains(&"end".into()),
        "completed with only a: {conclusions:?}"
    );
}

#[test]
fn metrics_do_not_treat_negated_activity_as_positive() {
    let log = make_repeated_log(5, &["a", "b"]);
    let rule = Rule::defeasible(
        "negative",
        vec![Literal::simple("a")],
        Literal::negated("b"),
    );
    let learned = rules_with_metrics(&log, &[rule], 1, 1.0);
    assert!(
        learned.is_empty(),
        "a => -b receives positive a -> b evidence: {learned:?}"
    );
}

#[test]
fn sequential_helper_preserves_long_trace_order() {
    let names: Vec<_> = (0..100).map(|i| format!("activity_{i}")).collect();
    let refs: Vec<_> = names.iter().map(String::as_str).collect();
    let case = make_sequential_trace("long", &refs);
    assert_eq!(case.activities(), refs);
}

#[test]
fn support_and_confidence_match_documented_trace_counts() {
    let log = make_log_from_traces(&[&["a", "b", "a", "b"], &["a", "c"], &["a"]]);
    assert_eq!(calculate_support(&log, "a", "b"), 1);
    assert_eq!(calculate_confidence(&log, "a", "b"), 0.5);
}
