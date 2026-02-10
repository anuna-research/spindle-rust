//! Error quality tests (SPEC-010 Phase D5)
//!
//! Tests ProblemDetails construction, source context, hint quality,
//! tone, and title/detail separation.

use spindle_contract::error::{ProblemDetails, SourceContext};
use spindle_core::error::SpindleError;

// =============================================================================
// TEST-101/107: ProblemDetails construction from each SpindleError variant
// =============================================================================

#[test]
fn test_construction_rule_not_found() {
    let err = SpindleError::RuleNotFound("missing_rule".into());
    let pd = ProblemDetails::from(&err);

    assert_eq!(
        pd.problem_type,
        "tag:spindle.dev,2026:error:RULE_NOT_FOUND"
    );
    assert_eq!(pd.title, "Execution Error");
    assert!(pd.detail.is_some());
    assert!(pd.detail.as_ref().unwrap().contains("missing_rule"));
    assert_eq!(pd.extensions.exit_code, 3);
}

#[test]
fn test_construction_invalid_literal() {
    let err = SpindleError::InvalidLiteral("bad@lit".into());
    let pd = ProblemDetails::from(&err);

    assert_eq!(
        pd.problem_type,
        "tag:spindle.dev,2026:error:INVALID_LITERAL"
    );
    assert_eq!(pd.title, "Validation Error");
    assert!(pd.detail.as_ref().unwrap().contains("bad@lit"));
    assert_eq!(pd.extensions.exit_code, 2);
}

#[test]
fn test_construction_theory_error() {
    let err = SpindleError::TheoryError("circular dependency".into());
    let pd = ProblemDetails::from(&err);

    assert_eq!(
        pd.problem_type,
        "tag:spindle.dev,2026:error:THEORY_ERROR"
    );
    assert_eq!(pd.title, "Validation Error");
    assert!(pd.detail.as_ref().unwrap().contains("circular dependency"));
    assert_eq!(pd.extensions.exit_code, 2);
}

#[test]
fn test_construction_reasoning_error() {
    let err = SpindleError::ReasoningError("ambiguity cycle".into());
    let pd = ProblemDetails::from(&err);

    assert_eq!(
        pd.problem_type,
        "tag:spindle.dev,2026:error:REASONING_ERROR"
    );
    assert_eq!(pd.title, "Execution Error");
    assert!(pd.detail.as_ref().unwrap().contains("ambiguity cycle"));
    assert_eq!(pd.extensions.exit_code, 3);
}

#[test]
fn test_construction_validation_error() {
    let err = SpindleError::Validation {
        message: "empty theory".into(),
    };
    let pd = ProblemDetails::from(&err);

    assert_eq!(
        pd.problem_type,
        "tag:spindle.dev,2026:error:VALIDATION_ERROR"
    );
    assert_eq!(pd.title, "Validation Error");
    assert!(pd.detail.as_ref().unwrap().contains("empty theory"));
    assert_eq!(pd.extensions.exit_code, 2);
}

#[test]
fn test_construction_parse_error() {
    let err = SpindleError::Parse {
        code: "LEXER_ERROR",
        message: "unexpected character '@'".into(),
    };
    let pd = ProblemDetails::from(&err);

    assert_eq!(
        pd.problem_type,
        "tag:spindle.dev,2026:error:LEXER_ERROR"
    );
    assert_eq!(pd.title, "Parse Error");
    assert!(pd.detail.as_ref().unwrap().contains("unexpected character"));
    assert_eq!(pd.extensions.exit_code, 2);
}

#[test]
fn test_construction_type_uri_format() {
    // All ProblemDetails must use the standard tag URI scheme
    let variants: Vec<SpindleError> = vec![
        SpindleError::RuleNotFound("r".into()),
        SpindleError::InvalidLiteral("l".into()),
        SpindleError::TheoryError("t".into()),
        SpindleError::ReasoningError("r".into()),
        SpindleError::Validation {
            message: "v".into(),
        },
        SpindleError::Parse {
            code: "PARSE_ERROR",
            message: "p".into(),
        },
    ];

    for err in &variants {
        let pd = ProblemDetails::from(err);
        assert!(
            pd.problem_type
                .starts_with("tag:spindle.dev,2026:error:"),
            "Type URI must use tag scheme, got: {}",
            pd.problem_type
        );
    }
}

#[test]
fn test_construction_required_fields_present() {
    // RFC 9457: problem_type and title are required
    let err = SpindleError::RuleNotFound("test".into());
    let pd = ProblemDetails::from(&err);

    assert!(!pd.problem_type.is_empty());
    assert!(!pd.title.is_empty());
    // exit_code is always set (Spindle extension)
    assert!(pd.extensions.exit_code > 0);
}

// =============================================================================
// TEST-111: SourceContext construction
// =============================================================================

#[test]
fn test_source_context_from_source() {
    let source = "line one\nline two\nline three\nline four\nline five";
    let ctx = SourceContext::from_source(source, 3, 1);

    assert_eq!(ctx.lines.len(), 3);
    assert_eq!(ctx.lines[0].line_number, 2);
    assert_eq!(ctx.lines[0].text, "line two");
    assert_eq!(ctx.lines[1].line_number, 3);
    assert_eq!(ctx.lines[1].text, "line three");
    assert_eq!(ctx.lines[2].line_number, 4);
    assert_eq!(ctx.lines[2].text, "line four");
    assert_eq!(ctx.highlight_index, 1); // error line is at index 1
}

#[test]
fn test_source_context_at_start() {
    let source = "first\nsecond\nthird";
    let ctx = SourceContext::from_source(source, 1, 1);

    assert_eq!(ctx.lines[0].line_number, 1);
    assert_eq!(ctx.lines[0].text, "first");
    assert_eq!(ctx.highlight_index, 0);
}

#[test]
fn test_source_context_at_end() {
    let source = "first\nsecond\nthird";
    let ctx = SourceContext::from_source(source, 3, 1);

    let last = ctx.lines.last().unwrap();
    assert_eq!(last.line_number, 3);
    assert_eq!(last.text, "third");
}

#[test]
fn test_source_context_highlight_within_bounds() {
    let source = "a\nb\nc\nd\ne\nf\ng";
    for line in 1..=7 {
        let ctx = SourceContext::from_source(source, line, 2);
        assert!(
            ctx.highlight_index < ctx.lines.len(),
            "highlight_index {} out of bounds for line {} (len {})",
            ctx.highlight_index,
            line,
            ctx.lines.len()
        );
    }
}

#[test]
fn test_source_context_with_problem_details() {
    let source = "rule1: a => b\nrule2: bad syntax\nrule3: c => d";
    let ctx = SourceContext::from_source(source, 2, 1);

    let pd = ProblemDetails::new("PARSE_ERROR", "Parse Error", 2)
        .with_detail("Unexpected token in rule body")
        .with_source_context(ctx);

    assert!(pd.extensions.source_context.is_some());
    let sc = pd.extensions.source_context.unwrap();
    assert_eq!(sc.lines[sc.highlight_index].text, "rule2: bad syntax");
}

// =============================================================================
// TEST-106: source() chain preserved through From conversions
// =============================================================================

#[test]
fn test_source_chain_spindle_error_to_problem_details() {
    // SpindleError → ProblemDetails preserves the error message in detail
    let err = SpindleError::RuleNotFound("my_rule".into());
    let display = format!("{err}");
    let pd = ProblemDetails::from(&err);

    assert_eq!(pd.detail.as_deref(), Some(display.as_str()));
}

#[test]
fn test_source_chain_parse_variant_preserves_code() {
    let err = SpindleError::Parse {
        code: "UNEXPECTED_TOKEN",
        message: "expected '=>' but found '->'".into(),
    };
    let pd = ProblemDetails::from(&err);

    // Code is preserved in the type URI
    assert!(pd.problem_type.contains("UNEXPECTED_TOKEN"));
    // Message preserved in detail
    assert!(pd.detail.as_ref().unwrap().contains("expected"));
}

// =============================================================================
// TEST-113: Hint quality - actionable suggestions
// =============================================================================

#[test]
fn test_hint_rule_not_found_is_actionable() {
    let err = SpindleError::RuleNotFound("missing_rule".into());
    let pd = ProblemDetails::from(&err);

    let hint = pd.extensions.hint.as_ref().expect("should have hint");
    // Hint should suggest checking the rule definition
    assert!(
        hint.contains("defined") || hint.contains("Check"),
        "Hint should suggest actionable check, got: {hint}"
    );
    // Hint should not just restate the error
    assert!(
        !hint.starts_with("rule not found"),
        "Hint should not restate the error title"
    );
}

#[test]
fn test_hint_invalid_literal_is_actionable() {
    let err = SpindleError::InvalidLiteral("bad@lit".into());
    let pd = ProblemDetails::from(&err);

    let hint = pd.extensions.hint.as_ref().expect("should have hint");
    // Should suggest checking syntax
    assert!(
        hint.contains("syntax") || hint.contains("alphanumeric"),
        "Hint should suggest correct syntax, got: {hint}"
    );
}

#[test]
fn test_hint_parse_error_is_actionable() {
    let err = SpindleError::Parse {
        code: "PARSE_ERROR",
        message: "unexpected token".into(),
    };
    let pd = ProblemDetails::from(&err);

    let hint = pd.extensions.hint.as_ref().expect("should have hint");
    // Should suggest checking input format
    assert!(
        hint.contains("syntax") || hint.contains("format") || hint.contains("DFL"),
        "Hint should reference input format, got: {hint}"
    );
}

// =============================================================================
// TEST-116: Tone - no blame language or anthropomorphism
// =============================================================================

const BLAME_WORDS: &[&str] = &[
    "you failed",
    "you did",
    "your mistake",
    "illegal",
    "I expected",
    "I found",
    "we expected",
    "stupid",
    "dumb",
    "wrong",
];

fn assert_no_blame(text: &str, context: &str) {
    let lower = text.to_lowercase();
    for word in BLAME_WORDS {
        assert!(
            !lower.contains(word),
            "Found blame language '{word}' in {context}: {text}"
        );
    }
}

#[test]
fn test_tone_no_blame_in_titles() {
    let variants: Vec<SpindleError> = vec![
        SpindleError::RuleNotFound("r".into()),
        SpindleError::InvalidLiteral("l".into()),
        SpindleError::TheoryError("t".into()),
        SpindleError::ReasoningError("r".into()),
        SpindleError::Validation {
            message: "v".into(),
        },
        SpindleError::Parse {
            code: "PARSE_ERROR",
            message: "p".into(),
        },
    ];

    for err in &variants {
        let pd = ProblemDetails::from(err);
        assert_no_blame(&pd.title, &format!("title for {}", err.code()));
    }
}

#[test]
fn test_tone_no_blame_in_hints() {
    let variants: Vec<SpindleError> = vec![
        SpindleError::RuleNotFound("r".into()),
        SpindleError::InvalidLiteral("l".into()),
        SpindleError::Parse {
            code: "PARSE_ERROR",
            message: "p".into(),
        },
    ];

    for err in &variants {
        let pd = ProblemDetails::from(err);
        if let Some(hint) = &pd.extensions.hint {
            assert_no_blame(hint, &format!("hint for {}", err.code()));
        }
    }
}

#[test]
fn test_tone_category_titles_neutral() {
    use spindle_core::error::ErrorCategory;
    let categories = [
        ErrorCategory::ParseError,
        ErrorCategory::ValidationError,
        ErrorCategory::ExecutionError,
        ErrorCategory::ResourceLimit,
        ErrorCategory::InternalError,
    ];
    for cat in &categories {
        let title = cat.default_title();
        assert_no_blame(title, &format!("category title for {cat:?}"));
    }
}

// =============================================================================
// TEST-117: Title and detail do not repeat
// =============================================================================

#[test]
fn test_title_detail_not_identical() {
    let variants: Vec<SpindleError> = vec![
        SpindleError::RuleNotFound("missing_rule".into()),
        SpindleError::InvalidLiteral("bad@lit".into()),
        SpindleError::TheoryError("circular dependency".into()),
        SpindleError::ReasoningError("ambiguity cycle".into()),
        SpindleError::Validation {
            message: "empty theory".into(),
        },
        SpindleError::Parse {
            code: "LEXER_ERROR",
            message: "unexpected char".into(),
        },
    ];

    for err in &variants {
        let pd = ProblemDetails::from(err);
        if let Some(detail) = &pd.detail {
            assert_ne!(
                pd.title, *detail,
                "Title and detail should differ for {}",
                err.code()
            );
        }
    }
}

#[test]
fn test_detail_provides_occurrence_info() {
    // Detail should contain the specific occurrence info (the actual error message),
    // not just the generic category title
    let err = SpindleError::RuleNotFound("alpha_rule".into());
    let pd = ProblemDetails::from(&err);

    let detail = pd.detail.as_ref().expect("should have detail");
    assert!(
        detail.contains("alpha_rule"),
        "Detail should contain occurrence-specific info, got: {detail}"
    );
}

#[test]
fn test_detail_differs_for_same_category() {
    // Two errors of the same category should produce different detail text
    let err1 = SpindleError::RuleNotFound("rule_a".into());
    let err2 = SpindleError::RuleNotFound("rule_b".into());

    let pd1 = ProblemDetails::from(&err1);
    let pd2 = ProblemDetails::from(&err2);

    // Titles should be the same (same category)
    assert_eq!(pd1.title, pd2.title);
    // Details should differ (different occurrences)
    assert_ne!(pd1.detail, pd2.detail);
}
