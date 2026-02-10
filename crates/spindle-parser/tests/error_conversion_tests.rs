//! Tests for ParseError → SpindleError conversion and error taxonomy

use spindle_core::error::{ErrorCategory, SpindleError};
use spindle_parser::ParseError;

#[test]
fn test_parse_error_codes() {
    let e = ParseError::LexerError {
        position: 5,
        message: "bad char".into(),
    };
    assert_eq!(e.code(), "LEXER_ERROR");

    let e = ParseError::ParserError {
        line: 3,
        message: "unexpected".into(),
    };
    assert_eq!(e.code(), "PARSE_ERROR");

    let e = ParseError::UnexpectedToken {
        expected: "ident".into(),
        found: "number".into(),
    };
    assert_eq!(e.code(), "UNEXPECTED_TOKEN");
}

#[test]
fn test_parse_error_categories() {
    let e = ParseError::LexerError {
        position: 0,
        message: "err".into(),
    };
    assert_eq!(e.category(), ErrorCategory::ParseError);

    let e = ParseError::ParserError {
        line: 1,
        message: "err".into(),
    };
    assert_eq!(e.category(), ErrorCategory::ParseError);

    let e = ParseError::UnexpectedToken {
        expected: "a".into(),
        found: "b".into(),
    };
    assert_eq!(e.category(), ErrorCategory::ParseError);
}

#[test]
fn test_from_parse_error_for_spindle_error() {
    let parse_err = ParseError::LexerError {
        position: 42,
        message: "invalid character '@'".into(),
    };

    let spindle_err: SpindleError = parse_err.into();

    assert_eq!(spindle_err.code(), "LEXER_ERROR");
    assert_eq!(spindle_err.category(), ErrorCategory::ParseError);
    assert!(format!("{spindle_err}").contains("invalid character '@'"));
}

#[test]
fn test_from_parse_error_preserves_code() {
    let parse_err = ParseError::ParserError {
        line: 10,
        message: "expected rule body".into(),
    };

    let spindle_err: SpindleError = parse_err.into();
    assert_eq!(spindle_err.code(), "PARSE_ERROR");
}

#[test]
fn test_from_parse_error_via_question_mark() {
    fn fallible() -> Result<(), SpindleError> {
        let parse_err = ParseError::UnexpectedToken {
            expected: "=>".into(),
            found: "->".into(),
        };
        Err(parse_err)?
    }

    let err = fallible().unwrap_err();
    assert_eq!(err.code(), "UNEXPECTED_TOKEN");
    assert_eq!(err.category(), ErrorCategory::ParseError);
}
