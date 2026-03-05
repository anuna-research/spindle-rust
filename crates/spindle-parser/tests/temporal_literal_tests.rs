//! Parser tests for temporal literal syntax.

use spindle_parser::spl::parse_spl;

// ===================== Date literals =====================

#[test]
fn parse_date_in_fact() {
    let input = "(given (shift-date alice #d:2025-07-15))";
    let theory = parse_spl(input).expect("should parse date literal");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
    let args = facts[0].head[0].predicate_args();
    assert_eq!(args.len(), 2);
    assert_eq!(format!("{}", args[1]), "#d:2025-07-15");
}

#[test]
fn parse_date_invalid() {
    let input = "(given (bad-date #d:2025-13-01))";
    assert!(parse_spl(input).is_err());
}

// ===================== Time literals =====================

#[test]
fn parse_time_in_fact() {
    let input = "(given (shift-start alice #t:09:00))";
    let theory = parse_spl(input).expect("should parse time literal");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
    let args = facts[0].head[0].predicate_args();
    assert_eq!(format!("{}", args[1]), "#t:09:00");
}

#[test]
fn parse_time_invalid_hours() {
    let input = "(given (bad-time #t:25:00))";
    assert!(parse_spl(input).is_err());
}

#[test]
fn parse_time_invalid_minutes() {
    let input = "(given (bad-time #t:12:60))";
    assert!(parse_spl(input).is_err());
}

// ===================== Datetime literals =====================

#[test]
fn parse_datetime_with_offset() {
    let input = "(given (event #dt:2025-07-15T14:00:00+10:00))";
    let theory = parse_spl(input).expect("should parse datetime literal");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
}

#[test]
fn parse_datetime_utc() {
    let input = "(given (event #dt:2025-07-15T14:00:00Z))";
    let theory = parse_spl(input).expect("should parse datetime with Z");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
}

// ===================== Duration literals =====================

#[test]
fn parse_duration_full() {
    let input = "(given (shift-len #dur:1d2h30m))";
    let theory = parse_spl(input).expect("should parse duration literal");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
    let args = facts[0].head[0].predicate_args();
    // 1d2h30m = 24*60 + 2*60 + 30 = 1590 minutes
    assert_eq!(format!("{}", args[0]), "#dur:1d2h30m");
}

#[test]
fn parse_duration_hours_only() {
    let input = "(given (shift-len #dur:8h))";
    let theory = parse_spl(input).expect("should parse hours-only duration");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
}

#[test]
fn parse_duration_minutes_only() {
    let input = "(given (shift-len #dur:30m))";
    let theory = parse_spl(input).expect("should parse minutes-only duration");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
}

#[test]
fn parse_duration_invalid() {
    let input = "(given (bad #dur:abc))";
    assert!(parse_spl(input).is_err());
}

// ===================== Offset literals =====================

#[test]
fn parse_offset_positive() {
    let input = "(given (tz melbourne #off:+10:00))";
    let theory = parse_spl(input).expect("should parse positive offset");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
    let args = facts[0].head[0].predicate_args();
    assert_eq!(format!("{}", args[1]), "#off:+10:00");
}

#[test]
fn parse_offset_negative() {
    let input = "(given (tz new-york #off:-05:00))";
    let theory = parse_spl(input).expect("should parse negative offset");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
}

#[test]
fn parse_offset_z() {
    let input = "(given (tz utc #off:Z))";
    let theory = parse_spl(input).expect("should parse Z offset");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
    let args = facts[0].head[0].predicate_args();
    assert_eq!(format!("{}", args[1]), "#off:Z");
}

// ===================== Temporal literals in bind expressions =====================

#[test]
fn parse_temporal_in_bind() {
    let input = r#"
(normally r1
  (and (shift ?who ?date)
       (bind ?start #t:09:00))
  (shift-start ?who ?start))
"#;
    let theory = parse_spl(input).expect("should parse temporal literal in bind");
    let rules: Vec<_> = theory.rules().filter(|r| !r.is_fact()).collect();
    assert_eq!(rules.len(), 1);
}

// ===================== Negative duration =====================

#[test]
fn parse_negative_duration() {
    let input = "(given (adj #dur:-1h30m))";
    let theory = parse_spl(input).expect("should parse negative duration");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 1);
    let args = facts[0].head[0].predicate_args();
    assert_eq!(format!("{}", args[0]), "#dur:-1h30m");
}

#[test]
fn duration_negative_round_trip() {
    let input = "(given (d #dur:-1h30m))";
    let theory = parse_spl(input).unwrap();
    let arg = &theory.rules().next().unwrap().head[0].predicate_args()[0];
    let displayed = format!("{}", arg);
    assert_eq!(displayed, "#dur:-1h30m");
    // Parse the displayed form back
    let input2 = format!("(given (d {displayed}))");
    let theory2 = parse_spl(&input2).unwrap();
    let arg2 = &theory2.rules().next().unwrap().head[0].predicate_args()[0];
    assert_eq!(arg, arg2);
}

// ===================== Multiple temporal facts =====================

#[test]
fn parse_multiple_temporal_facts() {
    let input = r#"
(given (shift alice #d:2025-07-15 #t:09:00 #t:17:00))
(given (shift bob #d:2025-07-15 #t:14:00 #t:22:00))
"#;
    let theory = parse_spl(input).expect("should parse multiple temporal facts");
    let facts: Vec<_> = theory.rules().filter(|r| r.is_fact()).collect();
    assert_eq!(facts.len(), 2);
}

// ===================== Datetime display round-trip =====================

#[test]
fn datetime_display_round_trip_offset() {
    let input = "(given (e #dt:2025-07-15T14:00:00+10:00))";
    let theory = parse_spl(input).unwrap();
    let arg = &theory.rules().next().unwrap().head[0].predicate_args()[0];
    let displayed = format!("{}", arg);
    assert_eq!(displayed, "#dt:2025-07-15T14:00+10:00");
    // Parse the displayed form back
    let input2 = format!("(given (e {displayed}))");
    let theory2 = parse_spl(&input2).unwrap();
    let arg2 = &theory2.rules().next().unwrap().head[0].predicate_args()[0];
    assert_eq!(arg, arg2);
}

#[test]
fn datetime_display_round_trip_utc() {
    let input = "(given (e #dt:2025-07-15T14:00:00Z))";
    let theory = parse_spl(input).unwrap();
    let arg = &theory.rules().next().unwrap().head[0].predicate_args()[0];
    let displayed = format!("{}", arg);
    assert_eq!(displayed, "#dt:2025-07-15T14:00Z");
    // Parse the displayed form back
    let input2 = format!("(given (e {displayed}))");
    let theory2 = parse_spl(&input2).unwrap();
    let arg2 = &theory2.rules().next().unwrap().head[0].predicate_args()[0];
    assert_eq!(arg, arg2);
}

// ===================== Round-trip: parse → display → parse =====================

#[test]
fn date_display_round_trip() {
    let input = "(given (d #d:2025-07-15))";
    let theory = parse_spl(input).unwrap();
    let arg = &theory.rules().next().unwrap().head[0].predicate_args()[0];
    let displayed = format!("{}", arg);
    assert_eq!(displayed, "#d:2025-07-15");
    // Parse the displayed form back
    let input2 = format!("(given (d {displayed}))");
    let theory2 = parse_spl(&input2).unwrap();
    let arg2 = &theory2.rules().next().unwrap().head[0].predicate_args()[0];
    assert_eq!(arg, arg2);
}

#[test]
fn time_display_round_trip() {
    let input = "(given (t #t:14:30))";
    let theory = parse_spl(input).unwrap();
    let arg = &theory.rules().next().unwrap().head[0].predicate_args()[0];
    let displayed = format!("{}", arg);
    assert_eq!(displayed, "#t:14:30");
    let input2 = format!("(given (t {displayed}))");
    let theory2 = parse_spl(&input2).unwrap();
    let arg2 = &theory2.rules().next().unwrap().head[0].predicate_args()[0];
    assert_eq!(arg, arg2);
}

#[test]
fn offset_display_round_trip() {
    for lit in &["#off:+10:00", "#off:-05:00", "#off:Z"] {
        let input = format!("(given (o {lit}))");
        let theory = parse_spl(&input).unwrap();
        let arg = &theory.rules().next().unwrap().head[0].predicate_args()[0];
        let displayed = format!("{}", arg);
        assert_eq!(&displayed, lit, "round-trip failed for {lit}");
    }
}
