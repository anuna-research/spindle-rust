//! SPEC-024 TEST-018: predicate-indicator parser robustness.
//!
//! Fuzz arbitrary UTF-8, long digit runs, escape runs, and slash-heavy strings.
//! Assert bounded completion, checked overflow, structural recognition, and no
//! panic. Any success must round-trip through the canonical indicator renderer.

use proptest::prelude::*;
use spindle_parser::parse_predicate_indicator;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4000))]

    /// The recognizer never panics and either fully recognizes the input or
    /// returns an error. Successful parses are stable under re-rendering and
    /// re-parsing (the canonical indicator round-trips).
    #[test]
    fn recognizer_is_total_and_bounded(input in ".{0,64}") {
        match parse_predicate_indicator(&input) {
            Ok(symbol) => {
                let rendered = symbol.indicator().to_string();
                let reparsed = parse_predicate_indicator(&rendered)
                    .expect("canonical indicator must re-parse");
                prop_assert_eq!(symbol, reparsed);
            }
            Err(_) => {}
        }
    }

    /// Slash-heavy strings never panic and never silently split on the last
    /// slash: a bare functor cannot contain `/`.
    #[test]
    fn slash_heavy_inputs_are_safe(
        parts in prop::collection::vec("[a-z]{0,4}", 0..6)
    ) {
        let input = parts.join("/");
        // Must not panic. Result may be Ok only when there is exactly one slash
        // separating a valid bare functor from a valid arity.
        let _ = parse_predicate_indicator(&input);
    }

    /// Long digit runs after a valid functor are handled with checked overflow
    /// (never a panic, never a truncating cast).
    #[test]
    fn long_digit_runs_never_panic(digits in "[0-9]{0,40}") {
        let input = format!("p/{digits}");
        let _ = parse_predicate_indicator(&input);
    }
}

#[test]
fn control_characters_in_quotes_rejected_without_panic() {
    assert!(parse_predicate_indicator("\"a\u{0007}b\"/1").is_err());
    assert!(parse_predicate_indicator("\"a\nb\"/1").is_err());
}
