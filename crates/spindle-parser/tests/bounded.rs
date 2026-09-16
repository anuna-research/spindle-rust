use spindle_parser::{parse_spl, parse_spl_bounded};

#[test]
fn bounded_recognizer_preserves_valid_programs() {
    for source in [
        "(given p)",
        "(given (p a))",
        "(normally r (and p q) a)",
        "(given (p \"((((\")) ; (((\n",
    ] {
        let ordinary = parse_spl(source).unwrap();
        let bounded = parse_spl_bounded(source, 8).unwrap();
        assert_eq!(ordinary.rule_count(), bounded.rule_count());
    }
}

#[test]
fn bounded_recognizer_rejects_excess_nesting() {
    let source = format!("{}p{}", "(".repeat(10000), ")".repeat(10000));
    assert!(parse_spl_bounded(&source, 64).is_err());
    assert!(parse_spl_bounded("(given p)", 0).is_err());
    assert!(parse_spl_bounded("(given p)", 1).is_ok());
    assert!(parse_spl_bounded("(given (p a))", 1).is_err());
}

#[test]
fn trailing_compound_fact_and_rule_arguments_are_rejected() {
    for source in [
        "(given (p) ignored)",
        "(given must p)",
        "(normally r p q ignored)",
        "(normally (p a) (q a) ignored)",
    ] {
        assert!(parse_spl_bounded(source, 64).is_err(), "accepted: {source}");
    }
}
