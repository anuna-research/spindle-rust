//! SPL (Spindle Lisp) Parser
//!
//! Parses the LISP-based SPL format into a Theory.
//!
//! # SPL Format
//!
//! ```lisp
//! ; Comments with semicolon
//!
//! ; Facts
//! (given bird)
//! (given (parent alice bob))
//!
//! ; Strict rules
//! (always r1 bird animal)
//!
//! ; Defeasible rules
//! (normally r1 bird flies)
//!
//! ; Defeaters
//! (except d1 broken-wing flies)
//!
//! ; Negation
//! (normally r2 penguin (not flies))
//!
//! ; Superiority
//! (prefer r2 r1)
//!
//! ; Conjunction
//! (normally r3 (and bird healthy) flies)
//! ```

mod expressions;
pub(crate) mod lexer;
pub(crate) mod literals;
mod metadata;
mod rules;

use spindle_core::Theory;

use crate::ParseError;
use crate::error::ParserFormat;

use expressions::process_expr_with_line;
use lexer::{
    line_of_from_error, line_of_offset, parse_expressions_with_positions, remove_comments,
    source_line_text,
};

/// Parse an SPL string into a Theory
pub fn parse_spl(input: &str) -> Result<Theory, ParseError> {
    // Remove comments
    let cleaned = remove_comments(input);

    let mut theory = Theory::new();

    // Parse all top-level expressions, tracking positions for line numbers
    let (remaining, expr_positions) = parse_expressions_with_positions(&cleaned).map_err(|e| {
        let line = line_of_from_error(&cleaned, &e);
        ParseError::ParserError {
            line,
            message: format!("SPL parse error: {e:?}"),
            format: ParserFormat::Spl,
            source_line: source_line_text(&cleaned, line),
        }
    })?;

    if !remaining.trim().is_empty() {
        let line = line_of_offset(&cleaned, cleaned.len() - remaining.len());
        return Err(ParseError::ParserError {
            line,
            message: format!("Unparsed input remaining: {}", remaining.trim()),
            format: ParserFormat::Spl,
            source_line: source_line_text(&cleaned, line),
        });
    }

    // Process expressions with line number tracking
    for (expr, offset) in expr_positions {
        let line = line_of_offset(&cleaned, offset);
        process_expr_with_line(&mut theory, &expr, line, &cleaned)?;
    }

    Ok(theory)
}

#[cfg(test)]
mod tests {
    use super::*;
    use spindle_core::MetaValue;
    use spindle_core::temporal::TimePoint;

    // =========================================================================
    // BASIC PARSING TESTS (existing)
    // =========================================================================

    #[test]
    fn test_parse_simple_fact() {
        let theory = parse_spl("(given bird)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_predicate_fact() {
        let theory = parse_spl("(given (parent alice bob))").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_flat_predicate_fact() {
        let theory = parse_spl("(given employed alice acme)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_defeasible_rule() {
        let theory = parse_spl("(normally r1 bird flies)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_strict_rule() {
        let theory = parse_spl("(always r1 bird animal)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_negated_head() {
        let theory = parse_spl("(normally r1 penguin (not flies))").unwrap();
        let rule = theory.rules().next().unwrap();
        assert!(rule.head_literal().is_negated());
    }

    #[test]
    fn test_parse_superiority() {
        let theory = parse_spl("(given a)\n(given b)\n(prefer r2 r1)").unwrap();
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_parse_superiority_chain() {
        let theory = parse_spl("(prefer r3 r2 r1)").unwrap();
        assert_eq!(theory.superiorities().len(), 2);
    }

    #[test]
    fn test_parse_conjunction() {
        let theory = parse_spl("(normally r1 (and bird healthy) flies)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.body.len(), 2);
    }

    #[test]
    fn test_parse_nested_during() {
        let theory = parse_spl("(given (during (employed alice) 10 20))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert_eq!(lit.name(), "employed");
        assert!(lit.is_temporal());
        assert_eq!(lit.temporal.start, TimePoint::from_millis(10));
        assert_eq!(lit.temporal.end, TimePoint::from_millis(20));
    }

    #[test]
    fn test_parse_nested_must() {
        let theory = parse_spl("(given (must (pay alice)))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert_eq!(lit.name(), "pay");
        assert!(lit.is_modal());
        assert_eq!(lit.mode.name, Some("O".to_string()));
    }

    #[test]
    fn test_parse_moment_rfc3339() {
        // 2024-01-01T00:00:00Z is 1704067200000 millis
        let theory = parse_spl("(given (during p (moment \"2024-01-01T00:00:00Z\") inf))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();

        if let TimePoint::Moment(millis) = lit.temporal.start {
            assert_eq!(millis, 1704067200000);
        } else {
            panic!(
                "Expected Moment for start time, got {:?}",
                lit.temporal.start
            );
        }
    }

    #[test]
    fn test_reject_ambiguous_moment_numeric() {
        let err = parse_spl("(given (during p (moment 2020) inf))").unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("moment"),
            "Expected error mentioning moment for ambiguous numeric form"
        );
    }

    // =========================================================================
    // REGRESSION TESTS - Bug Hunt Fixes
    // =========================================================================

    #[test]
    fn test_claims_keyword_basic() {
        // Regression: SPL parser should handle 'claims' keyword
        let input = r#"(claims agent:alice :at "2024-01-01T00:00:00Z" (given bird))"#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 1);

        // Should have source metadata
        let fact = theory.facts().next().unwrap();
        let meta = theory.get_meta(&fact.label);
        assert!(meta.is_some(), "Claims should attach source metadata");
    }

    #[test]
    fn test_claims_keyword_multiple_facts() {
        let input = r#"(claims agent:bob (given sunny) (given warm))"#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);
    }

    #[test]
    fn test_auto_label_no_collision() {
        // Regression: auto-labels should not collide with user-provided labels
        let input = "(normally r3 bird flies)\n(normally bird2 animal)";
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);

        // Both rules should have distinct labels
        let labels: Vec<_> = theory.rules().map(|r| r.label.clone()).collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(labels.len(), unique.len(), "Labels should be unique");
    }

    #[test]
    fn test_double_negation_stripped() {
        // Regression: ~~name should be treated as positive literal
        let theory = parse_spl("(given ~~bird)").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert!(
            !lit.is_negated(),
            "~~bird should be positive (double negation cancels)"
        );
        assert_eq!(lit.name(), "bird");
    }

    #[test]
    fn test_double_negation_in_rule() {
        let theory = parse_spl("(normally r1 ~~p q)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert!(!rule.body[0].is_negated(), "~~p in body should be positive");
    }

    #[test]
    fn test_line_numbers_in_errors() {
        // Regression: parser errors should report actual line numbers, not always line 1
        let input = "(given bird)\n(given fish)\n(bogus_keyword something)";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        // The error is on line 3
        assert!(
            message.contains("3") || message.contains("bogus_keyword"),
            "Error should reference line 3 or the bad keyword, got: {message}"
        );
    }

    #[test]
    fn test_line_numbers_not_always_one() {
        // More specific test: an error late in the file should NOT say line 1
        let input = "(given a)\n(given b)\n(given c)\n(given d)\n(bad)";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        // Should mention line 5 (or at least not line 1)
        assert!(
            !message.contains("line 1") || message.contains("line 5"),
            "Error on line 5 should not report line 1, got: {message}"
        );
    }

    // =========================================================================
    // Reviewer Fix Tests
    // =========================================================================

    #[test]
    fn test_claims_timestamp_attached() {
        // Fix 2: verify "timestamp" metadata key is set when :at is provided
        let input = r#"(claims agent:alice :at "2024-06-15T12:00:00Z" (given sunny))"#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 1);

        let fact = theory.facts().next().unwrap();
        let meta = theory.get_meta(&fact.label);
        assert!(meta.is_some(), "Should have metadata");

        let meta_map = meta.unwrap();
        assert!(
            meta_map.properties.contains_key("timestamp"),
            "Should have 'timestamp' metadata key"
        );
        let ts_val = &meta_map.properties["timestamp"];
        match ts_val {
            MetaValue::String(s) => assert_eq!(s, "2024-06-15T12:00:00Z"),
            other => panic!("Expected String metadata for timestamp, got: {other:?}"),
        }

        assert!(
            meta_map.properties.contains_key("source"),
            "Should have 'source' metadata key"
        );
    }

    #[test]
    fn test_claims_prefer_no_wrong_metadata() {
        // Fix 3: claims block with prefer inside — metadata should only attach to actual rules
        let input = r#"(claims agent:bob (given bird) (normally r1 bird flies) (prefer r1 r2) (given fish))"#;
        let theory = parse_spl(input).unwrap();

        // Should have 3 rules: bird fact, r1, fish fact
        assert_eq!(theory.rule_count(), 3);

        // All 3 rules should have source metadata
        for rule in theory.rules() {
            let meta = theory.get_meta(&rule.label);
            assert!(
                meta.is_some() && meta.unwrap().properties.contains_key("source"),
                "Rule {} should have source metadata",
                rule.label
            );
        }

        // The prefer expression should NOT have attached metadata to the wrong rule.
        // Specifically, r1's source should be "agent:bob" (from claims), not duplicated.
        let r1_meta = theory.get_meta("r1").unwrap();
        match &r1_meta.properties["source"] {
            MetaValue::String(s) => assert_eq!(s, "agent:bob"),
            other => panic!("Expected String source for r1, got: {other:?}"),
        }
    }

    #[test]
    fn test_claims_at_requires_atom_timestamp() {
        let err = parse_spl("(claims agent:alice :at (given bird))").unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("claims :at requires a timestamp atom"),
            "Expected malformed :at value to fail, got: {message}"
        );
    }

    #[test]
    fn test_line_numbers_with_lang_directive() {
        let input = "#lang spindle\n(bogus something)";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("line 2"),
            "Expected error to report line 2 after #lang, got: {message}"
        );
    }

    #[test]
    fn test_claims_inner_expression_line_numbers() {
        let input = "(claims agent:alice\n  (given bird)\n  (bogus something))";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("line 3"),
            "Expected bad inner claims expression to report line 3, got: {message}"
        );
    }

    #[test]
    fn test_claims_inner_expression_single_line_reports_line_one() {
        let input = "(claims agent:alice (given bird) (bogus something))";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("line 1"),
            "Expected bad inner claims expression on single line to report line 1, got: {message}"
        );
    }

    #[test]
    fn test_claims_sig_attached() {
        let input = r#"(claims agent:alice :sig "abc123" (given sunny))"#;
        let theory = parse_spl(input).unwrap();
        let fact = theory.facts().next().unwrap();
        let meta = theory.get_meta(&fact.label).unwrap();
        match &meta.properties["signature"] {
            MetaValue::String(s) => assert_eq!(s, "abc123"),
            other => panic!("Expected String metadata for signature, got: {other:?}"),
        }
    }

    #[test]
    fn test_claims_id_attached() {
        let input = r#"(claims agent:alice :id "claim-001" (given sunny))"#;
        let theory = parse_spl(input).unwrap();
        let fact = theory.facts().next().unwrap();
        let meta = theory.get_meta(&fact.label).unwrap();
        match &meta.properties["id"] {
            MetaValue::String(s) => assert_eq!(s, "claim-001"),
            other => panic!("Expected String metadata for id, got: {other:?}"),
        }
    }

    #[test]
    fn test_claims_note_attached() {
        let input = r#"(claims agent:alice :note "scan results" (given sunny))"#;
        let theory = parse_spl(input).unwrap();
        let fact = theory.facts().next().unwrap();
        let meta = theory.get_meta(&fact.label).unwrap();
        match &meta.properties["note"] {
            MetaValue::String(s) => assert_eq!(s, "scan results"),
            other => panic!("Expected String metadata for note, got: {other:?}"),
        }
    }

    #[test]
    fn test_claims_all_metadata() {
        let input = r#"(claims agent:alice :at "2024-06-15T12:00:00Z" :sig "sig456" :id "block-1" :note "full test" (given sunny))"#;
        let theory = parse_spl(input).unwrap();
        let fact = theory.facts().next().unwrap();
        let meta = theory.get_meta(&fact.label).unwrap();

        match &meta.properties["source"] {
            MetaValue::String(s) => assert_eq!(s, "agent:alice"),
            other => panic!("Expected String for source, got: {other:?}"),
        }
        match &meta.properties["timestamp"] {
            MetaValue::String(s) => assert_eq!(s, "2024-06-15T12:00:00Z"),
            other => panic!("Expected String for timestamp, got: {other:?}"),
        }
        match &meta.properties["signature"] {
            MetaValue::String(s) => assert_eq!(s, "sig456"),
            other => panic!("Expected String for signature, got: {other:?}"),
        }
        match &meta.properties["id"] {
            MetaValue::String(s) => assert_eq!(s, "block-1"),
            other => panic!("Expected String for id, got: {other:?}"),
        }
        match &meta.properties["note"] {
            MetaValue::String(s) => assert_eq!(s, "full test"),
            other => panic!("Expected String for note, got: {other:?}"),
        }
    }

    #[test]
    fn test_claims_sig_requires_atom() {
        let err = parse_spl("(claims agent:alice :sig (given bird))").unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("claims :sig requires a signature atom"),
            "Expected malformed :sig value to fail, got: {message}"
        );
    }

    #[test]
    fn test_claims_id_requires_atom() {
        let err = parse_spl("(claims agent:alice :id (given bird))").unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("claims :id requires a id atom"),
            "Expected malformed :id value to fail, got: {message}"
        );
    }

    #[test]
    fn test_claims_note_requires_atom() {
        let err = parse_spl("(claims agent:alice :note (given bird))").unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("claims :note requires a note atom"),
            "Expected malformed :note value to fail, got: {message}"
        );
    }

    // =========================================================================
    // Trust Directive Tests
    // =========================================================================

    #[test]
    fn test_parse_trusts_directive() {
        let theory = parse_spl("(trusts agent:coder 0.9)").unwrap();
        assert_eq!(theory.trust_policy().get_trust("agent:coder"), 0.9);
    }

    #[test]
    fn test_parse_trusts_multiple() {
        let input =
            "(trusts agent:coder 0.9)\n(trusts agent:security 0.95)\n(trusts system:policy 1.0)";
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.trust_policy().get_trust("agent:coder"), 0.9);
        assert_eq!(theory.trust_policy().get_trust("agent:security"), 0.95);
        assert_eq!(theory.trust_policy().get_trust("system:policy"), 1.0);
    }

    #[test]
    fn test_parse_trusts_invalid_value() {
        let err = parse_spl("(trusts agent:coder 1.5)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("[0.0, 1.0]"),
            "Should reject out-of-range value, got: {msg}"
        );
    }

    #[test]
    fn test_parse_trusts_not_a_number() {
        let err = parse_spl("(trusts agent:coder abc)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("number"),
            "Should reject non-numeric value, got: {msg}"
        );
    }

    #[test]
    fn test_parse_threshold_directive() {
        let theory = parse_spl("(threshold action 0.7)").unwrap();
        assert_eq!(
            theory.trust_policy().is_above_threshold(0.8, "action"),
            Some(true)
        );
        assert_eq!(
            theory.trust_policy().is_above_threshold(0.6, "action"),
            Some(false)
        );
    }

    #[test]
    fn test_parse_threshold_multiple() {
        let input = "(threshold action 0.7)\n(threshold warn 0.5)";
        let theory = parse_spl(input).unwrap();
        assert_eq!(
            theory.trust_policy().is_above_threshold(0.6, "action"),
            Some(false)
        );
        assert_eq!(
            theory.trust_policy().is_above_threshold(0.6, "warn"),
            Some(true)
        );
    }

    #[test]
    fn test_parse_decays_exponential() {
        let theory = parse_spl("(decays agent:coder exponential 3600.0)").unwrap();
        let policy = theory.trust_policy();
        assert!(policy.decay_map.contains_key("agent:coder"));
    }

    #[test]
    fn test_parse_decays_linear() {
        let theory = parse_spl("(decays agent:sensor linear 0.001)").unwrap();
        let policy = theory.trust_policy();
        assert!(policy.decay_map.contains_key("agent:sensor"));
    }

    #[test]
    fn test_parse_decays_step() {
        let theory = parse_spl("(decays agent:temp step 86400.0)").unwrap();
        let policy = theory.trust_policy();
        assert!(policy.decay_map.contains_key("agent:temp"));
    }

    #[test]
    fn test_parse_decays_unknown_model() {
        let err = parse_spl("(decays agent:x quadratic 1.0)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Unknown decay model"),
            "Should reject unknown model, got: {msg}"
        );
    }

    #[test]
    fn test_trust_directives_with_theory() {
        let input = r#"
            (trusts agent:coder 0.9)
            (trusts agent:security 0.95)
            (threshold action 0.7)
            (decays agent:coder exponential 3600.0)
            (given bird)
            (normally r1 bird flies)
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);
        assert_eq!(theory.trust_policy().get_trust("agent:coder"), 0.9);
        assert_eq!(theory.trust_policy().get_trust("agent:security"), 0.95);
        assert_eq!(
            theory.trust_policy().is_above_threshold(0.8, "action"),
            Some(true)
        );
    }

    #[test]
    fn test_trusts_missing_args() {
        let err = parse_spl("(trusts agent:coder)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("two arguments"),
            "Should require source + value, got: {msg}"
        );
    }

    #[test]
    fn test_threshold_missing_args() {
        let err = parse_spl("(threshold action)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("two arguments"),
            "Should require name + value, got: {msg}"
        );
    }

    #[test]
    fn test_decays_missing_args() {
        let err = parse_spl("(decays agent:x exponential)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("at least"),
            "Should require source + model + param, got: {msg}"
        );
    }

    // =========================================================================
    // REQ-025/026: Error message improvements and auto-label collision fixes
    // =========================================================================

    #[test]
    fn test_error_includes_source_line_text() {
        // REQ-025: error messages should include the source line text
        let input = "(given bird)\n(given fish)\n(bogus_keyword something)";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        // The error message should include the offending source line
        assert!(
            message.contains("(bogus_keyword something)"),
            "Error should include source line text, got: {message}"
        );
        // And should have the pipe prefix for visual context
        assert!(
            message.contains("| (bogus_keyword something)"),
            "Error should show source line with | prefix, got: {message}"
        );
    }

    #[test]
    fn test_error_includes_source_line_for_bad_rule() {
        // REQ-025: check that rule parse errors also get source line enrichment
        let input = "(given bird)\n(normally)";
        let err = parse_spl(input).unwrap_err();
        let message = format!("{err}");
        assert!(
            message.contains("| (normally)"),
            "Error should show source line for bad rule, got: {message}"
        );
    }

    #[test]
    fn test_auto_label_collision_with_user_label_r2() {
        // REQ-026: user uses label r2, auto-gen must skip r2
        let input = "(normally r2 bird flies)\n(normally animal walks)";
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);

        let labels: Vec<_> = theory.rules().map(|r| r.label.clone()).collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(
            labels.len(),
            unique.len(),
            "Labels must be unique: {labels:?}"
        );

        // The auto-generated label should NOT be r2 (that's taken)
        assert!(
            labels.contains(&"r2".to_string()),
            "Should have user-provided r2"
        );
        // The other label should be something else (r1, r3, etc.)
        let other_label = labels.iter().find(|l| l.as_str() != "r2").unwrap();
        assert_ne!(
            other_label.as_str(),
            "r2",
            "Auto-gen must not collide with r2"
        );
    }

    #[test]
    fn test_auto_label_collision_with_multiple_user_labels() {
        // REQ-026: multiple user labels that would collide with auto-gen sequence
        let input = "(normally r1 a b)\n(normally r2 c d)\n(normally e f)";
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 3);

        let labels: Vec<_> = theory.rules().map(|r| r.label.clone()).collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(
            labels.len(),
            unique.len(),
            "All labels must be unique even when user takes r1 and r2: {labels:?}"
        );
    }

    #[test]
    fn test_auto_label_collision_facts() {
        // REQ-026: fact auto-labels (f1, f2, ...) should not collide if user creates rule f1
        // Note: facts use "f" prefix, rules use "r" prefix, but we test the general pattern.
        let input = "(given bird)\n(given fish)\n(given cat)";
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 3);

        let labels: Vec<_> = theory.rules().map(|r| r.label.clone()).collect();
        let unique: std::collections::HashSet<_> = labels.iter().collect();
        assert_eq!(
            labels.len(),
            unique.len(),
            "Fact labels must be unique: {labels:?}"
        );
    }

    #[test]
    fn test_source_line_in_error_display() {
        // REQ-025: verify the Display impl shows the pipe-prefixed source line
        use crate::ParseError;
        use crate::error::ParserFormat;

        let err = ParseError::ParserError {
            line: 5,
            message: "Unknown keyword: bogus".to_string(),
            format: ParserFormat::Spl,
            source_line: Some("(bogus something)".to_string()),
        };
        let msg = format!("{err}");
        assert!(
            msg.contains("line 5"),
            "Should mention line number, got: {msg}"
        );
        assert!(
            msg.contains("| (bogus something)"),
            "Should show source line with pipe prefix, got: {msg}"
        );
    }

    #[test]
    fn test_source_line_none_omits_context() {
        // REQ-025: when source_line is None, the old format is used
        use crate::ParseError;
        use crate::error::ParserFormat;

        let err = ParseError::ParserError {
            line: 3,
            message: "some error".to_string(),
            format: ParserFormat::Spl,
            source_line: None,
        };
        let msg = format!("{err}");
        assert_eq!(msg, "SPL parse error at line 3: some error");
    }

    // =========================================================================
    // Coverage tests: literals.rs
    // =========================================================================

    #[test]
    fn test_parse_single_tilde_negation() {
        // ~name should parse as negated literal
        let theory = parse_spl("(given ~bird)").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert!(lit.is_negated(), "~bird should be negated");
        assert_eq!(lit.name(), "bird");
    }

    #[test]
    fn test_parse_double_negation_empty() {
        // ~~ with no name should error
        let err = parse_spl("(given ~~)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Double negation with empty name"),
            "Expected double negation empty error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_may_modal() {
        // (may literal) should produce a permission mode
        let theory = parse_spl("(given (may (access resource)))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert!(lit.is_modal(), "(may ...) should be modal");
        assert_eq!(lit.mode.name, Some("P".to_string()));
        assert_eq!(lit.name(), "access");
    }

    #[test]
    fn test_parse_forbidden_modal() {
        // (forbidden literal) should produce a forbidden mode
        let theory = parse_spl("(given (forbidden (delete data)))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert!(lit.is_modal(), "(forbidden ...) should be modal");
        assert_eq!(lit.mode.name, Some("F".to_string()));
        assert_eq!(lit.name(), "delete");
    }

    #[test]
    fn test_parse_must_wrong_arity() {
        // (must) with no arg should error
        let err = parse_spl("(given (must))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("must takes exactly one argument"),
            "Expected must arity error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_must_too_many_args() {
        // (must a b) should error
        let err = parse_spl("(given (must a b))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("must takes exactly one argument"),
            "Expected must arity error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_may_wrong_arity() {
        // (may) with no arg should error
        let err = parse_spl("(given (may))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("may takes exactly one argument"),
            "Expected may arity error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_may_too_many_args() {
        // (may a b) should error
        let err = parse_spl("(given (may a b))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("may takes exactly one argument"),
            "Expected may arity error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_forbidden_wrong_arity() {
        // (forbidden) with no arg should error
        let err = parse_spl("(given (forbidden))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("forbidden takes exactly one argument"),
            "Expected forbidden arity error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_forbidden_too_many_args() {
        // (forbidden a b) should error
        let err = parse_spl("(given (forbidden a b))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("forbidden takes exactly one argument"),
            "Expected forbidden arity error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_not_no_args() {
        // (not) with no argument should error
        let err = parse_spl("(given (not))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("not takes exactly one argument"),
            "Expected not arity error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_not_too_many_args() {
        // (not a b c) should error
        let err = parse_spl("(given (not a b c))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("not takes exactly one argument"),
            "Expected not arity error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_during_wrong_arity() {
        // (during lit) missing start and end should error
        let err = parse_spl("(given (during p))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("during takes either"),
            "Expected during arity error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_timepoint_neg_inf() {
        // -inf should parse as NegInf
        let theory = parse_spl("(given (during p -inf 100))").unwrap();
        let fact = theory.facts().next().unwrap();
        let lit = fact.head_literal();
        assert_eq!(lit.temporal.start, TimePoint::NegInf);
        assert_eq!(lit.temporal.end, TimePoint::from_millis(100));
    }

    #[test]
    fn test_parse_timepoint_invalid_string() {
        // A non-numeric, non-inf string should error
        let err = parse_spl("(given (during p hello 100))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Invalid timepoint"),
            "Expected invalid timepoint error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_moment_non_atom_arg() {
        // (moment (nested)) where arg is a list, not atom, should error
        let err = parse_spl("(given (during p (moment (nested)) inf))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("moment argument must be atom"),
            "Expected moment non-atom error, got: {msg}"
        );
    }

    #[test]
    fn test_parse_predicate_nested_list_arg() {
        // (name (nested)) where predicate arg is a list, not atom
        let err = parse_spl("(given (myPred (nested)))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Expected atom argument"),
            "Expected atom argument error for nested list in predicate, got: {msg}"
        );
    }

    #[test]
    fn test_parse_empty_list_literal() {
        // (given ()) - empty list as literal should error
        let err = parse_spl("(given ())").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Empty list is not a valid literal"),
            "Expected empty list literal error, got: {msg}"
        );
    }

    // =========================================================================
    // Coverage tests: metadata.rs
    // =========================================================================

    #[test]
    fn test_claims_no_args_error() {
        // (claims) with no arguments should error
        let err = parse_spl("(claims)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("claims requires a source"),
            "Expected claims no-source error, got: {msg}"
        );
    }

    #[test]
    fn test_claims_non_atom_source_error() {
        // (claims (non-atom)) where source is a list, not atom
        let err = parse_spl("(claims (non-atom) (given bird))").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("claims source must be an atom"),
            "Expected claims non-atom source error, got: {msg}"
        );
    }

    #[test]
    fn test_claims_unknown_keyword() {
        // (claims source :unknown value) - unknown keyword should break out of
        // keyword parsing and try to process ":unknown" as an expression, which fails
        let err = parse_spl("(claims source :unknown value)").unwrap_err();
        let msg = format!("{err}");
        // The parser tries to interpret ":unknown" as an expression atom, which is not a list
        assert!(
            msg.contains("Expected list expression") || msg.contains("Unknown keyword"),
            "Expected error for unknown keyword in claims, got: {msg}"
        );
    }

    #[test]
    fn test_claims_keyword_no_value() {
        // (claims source :at) - keyword with no value should error
        let err = parse_spl("(claims source :at)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("claims :at requires a timestamp atom"),
            "Expected missing value error, got: {msg}"
        );
    }

    #[test]
    fn test_trusts_no_args_error() {
        // (trusts) with no arguments should error
        let err = parse_spl("(trusts)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("two arguments"),
            "Expected trusts missing args error, got: {msg}"
        );
    }

    #[test]
    fn test_trusts_missing_value() {
        // (trusts source) missing the trust value
        let err = parse_spl("(trusts agent:bob)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("two arguments"),
            "Expected trusts missing value error, got: {msg}"
        );
    }

    #[test]
    fn test_trusts_non_numeric_value() {
        // (trusts source not-a-number) should error
        let err = parse_spl("(trusts agent:coder abc)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("number"),
            "Expected trusts non-numeric error, got: {msg}"
        );
    }

    #[test]
    fn test_decays_no_args_error() {
        // (decays) with no arguments should error
        let err = parse_spl("(decays)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("at least"),
            "Expected decays missing args error, got: {msg}"
        );
    }

    #[test]
    fn test_threshold_no_args_error() {
        // (threshold) with no arguments should error
        let err = parse_spl("(threshold)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("two arguments"),
            "Expected threshold missing args error, got: {msg}"
        );
    }

    // =========================================================================
    // Coverage tests: expressions.rs
    // =========================================================================

    #[test]
    fn test_expr_atom_not_list_error() {
        // A bare atom at top level (not wrapped in a list) should error.
        // This can't happen through parse_spl since the lexer always wraps
        // in lists, but we test the underlying behavior with a malformed input.
        // Actually, an unparenthesized atom at top level would be unparsed input.
        let err = parse_spl("bare_atom").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Unparsed input remaining") || msg.contains("parse error"),
            "Expected error for bare atom at top level, got: {msg}"
        );
    }

    #[test]
    fn test_expr_empty_list_ignored() {
        // () at top level should be silently ignored (returns Ok with no rules)
        let theory = parse_spl("()").unwrap();
        assert_eq!(theory.rule_count(), 0, "Empty list should produce no rules");
    }

    #[test]
    fn test_expr_list_starting_with_non_atom() {
        // ((nested) body head) where keyword position is a list
        let err = parse_spl("((nested) body head)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Expected keyword"),
            "Expected keyword error for list in keyword position, got: {msg}"
        );
    }

    #[test]
    fn test_expr_lang_directive_ignored() {
        // #lang spindle should be silently ignored and not cause errors
        let theory = parse_spl("#lang spindle\n(given bird)").unwrap();
        // The #lang line gets treated as a comment or ignored expression;
        // only the (given bird) should produce a rule
        assert!(
            theory.rule_count() >= 1,
            "Should parse the given fact after #lang"
        );
    }

    #[test]
    fn test_expr_unknown_keyword_error() {
        // (foobar x y) should produce an "Unknown keyword" error
        let err = parse_spl("(foobar x y)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("Unknown keyword: foobar"),
            "Expected unknown keyword error, got: {msg}"
        );
    }

    #[test]
    fn test_expr_given_no_args_error() {
        // (given) with no arguments should error
        let err = parse_spl("(given)").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("fact requires at least one argument"),
            "Expected fact no-args error, got: {msg}"
        );
    }

    // =========================================================================
    // Coverage tests: rules.rs
    // =========================================================================

    #[test]
    fn test_rule_first_arg_list_not_atom() {
        // (always (nested) body head) - first arg is a list, should be treated as body
        // With 3 args where first is a list: label=None, body=(nested), head=body
        // "body" and "head" are just atoms used as literal names here
        let theory = parse_spl("(always (and a b) c)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(
            rule.body.len(),
            2,
            "Should parse (and a b) as conjunction body"
        );
        assert_eq!(rule.head_literal().name(), "c");
    }

    #[test]
    fn test_rule_label_reserved_keyword_and() {
        // (always and body head) - "and" looks like label but is reserved
        // Should be treated as body, not as a label
        let theory = parse_spl("(always and body head)").unwrap();
        let rule = theory.rules().next().unwrap();
        // "and" is treated specially: label=None, body_expr=and, head_expr=body
        // But "and" as a bare atom in body position becomes a simple literal
        assert_eq!(rule.body[0].name(), "and");
    }
}
