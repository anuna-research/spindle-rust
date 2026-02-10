//! Tests for error taxonomy: ErrorCategory, code(), category(), non-exhaustive, From<ParseError>

use spindle_core::error::{ErrorCategory, SpindleError};

#[test]
fn test_error_category_exit_codes() {
    assert_eq!(ErrorCategory::ParseError.exit_code(), 2);
    assert_eq!(ErrorCategory::ValidationError.exit_code(), 2);
    assert_eq!(ErrorCategory::ExecutionError.exit_code(), 3);
    assert_eq!(ErrorCategory::ResourceLimit.exit_code(), 4);
    assert_eq!(ErrorCategory::InternalError.exit_code(), 3);
}

#[test]
fn test_error_category_titles() {
    assert_eq!(ErrorCategory::ParseError.default_title(), "Parse Error");
    assert_eq!(
        ErrorCategory::ValidationError.default_title(),
        "Validation Error"
    );
    assert_eq!(
        ErrorCategory::ExecutionError.default_title(),
        "Execution Error"
    );
    assert_eq!(
        ErrorCategory::ResourceLimit.default_title(),
        "Resource Limit Exceeded"
    );
    assert_eq!(
        ErrorCategory::InternalError.default_title(),
        "Internal Error"
    );
}

#[test]
fn test_error_category_display() {
    assert_eq!(format!("{}", ErrorCategory::ParseError), "Parse Error");
}

#[test]
fn test_spindle_error_codes() {
    assert_eq!(
        SpindleError::RuleNotFound("r1".into()).code(),
        "RULE_NOT_FOUND"
    );
    assert_eq!(
        SpindleError::InvalidLiteral("bad".into()).code(),
        "INVALID_LITERAL"
    );
    assert_eq!(
        SpindleError::TheoryError("err".into()).code(),
        "THEORY_ERROR"
    );
    assert_eq!(
        SpindleError::ReasoningError("err".into()).code(),
        "REASONING_ERROR"
    );
    assert_eq!(
        SpindleError::Validation {
            message: "msg".into()
        }
        .code(),
        "VALIDATION_ERROR"
    );
    assert_eq!(
        SpindleError::Parse {
            code: "LEXER_ERROR",
            message: "bad token".into(),
            line: None,
        }
        .code(),
        "LEXER_ERROR"
    );
}

#[test]
fn test_spindle_error_categories() {
    assert_eq!(
        SpindleError::RuleNotFound("r1".into()).category(),
        ErrorCategory::ExecutionError
    );
    assert_eq!(
        SpindleError::InvalidLiteral("bad".into()).category(),
        ErrorCategory::ValidationError
    );
    assert_eq!(
        SpindleError::TheoryError("err".into()).category(),
        ErrorCategory::ValidationError
    );
    assert_eq!(
        SpindleError::ReasoningError("err".into()).category(),
        ErrorCategory::ExecutionError
    );
    assert_eq!(
        SpindleError::Validation {
            message: "msg".into()
        }
        .category(),
        ErrorCategory::ValidationError
    );
    assert_eq!(
        SpindleError::Parse {
            code: "PARSE_ERROR",
            message: "bad input".into(),
            line: None,
        }
        .category(),
        ErrorCategory::ParseError
    );
}

#[test]
fn test_spindle_error_display() {
    let err = SpindleError::RuleNotFound("r1".into());
    assert_eq!(format!("{err}"), "rule not found: r1");

    let err = SpindleError::Parse {
        code: "LEXER_ERROR",
        message: "unexpected character".into(),
        line: None,
    };
    assert_eq!(format!("{err}"), "unexpected character");
}

#[test]
fn test_spindle_error_is_std_error() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<SpindleError>();
}

#[test]
fn test_error_category_clone_copy_eq() {
    let cat = ErrorCategory::ParseError;
    let cat2 = cat;
    assert_eq!(cat, cat2);

    #[allow(clippy::clone_on_copy)]
    let cat3 = cat.clone();
    assert_eq!(cat, cat3);
}

// Compile-time assertions: error types must be Send + Sync + 'static (REQ-112)
#[allow(dead_code)]
const _: () = {
    fn assert_send_sync_static<T: Send + Sync + 'static>() {}
    fn assertions() {
        assert_send_sync_static::<SpindleError>();
        assert_send_sync_static::<ErrorCategory>();
    }
};
