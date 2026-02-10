//! Tests for ParseError → SpindleError conversion and error taxonomy

use spindle_core::error::{ErrorCategory, SpindleError};
use spindle_parser::{ParseError, ParserFormat};

#[test]
fn test_parse_error_codes() {
    let e = ParseError::LexerError {
        position: 5,
        message: "bad char".into(),
        format: ParserFormat::Dfl,
    };
    assert_eq!(e.code(), "DFL_LEXER_ERROR");

    let e = ParseError::LexerError {
        position: 5,
        message: "bad char".into(),
        format: ParserFormat::Spl,
    };
    assert_eq!(e.code(), "SPL_LEXER_ERROR");

    let e = ParseError::ParserError {
        line: 3,
        message: "unexpected".into(),
        format: ParserFormat::Dfl,
    };
    assert_eq!(e.code(), "DFL_PARSE_ERROR");

    let e = ParseError::ParserError {
        line: 3,
        message: "unexpected".into(),
        format: ParserFormat::Spl,
    };
    assert_eq!(e.code(), "SPL_PARSE_ERROR");

    let e = ParseError::UnexpectedToken {
        expected: "ident".into(),
        found: "number".into(),
        format: ParserFormat::Dfl,
    };
    assert_eq!(e.code(), "DFL_UNEXPECTED_TOKEN");

    let e = ParseError::UnexpectedToken {
        expected: "ident".into(),
        found: "number".into(),
        format: ParserFormat::Spl,
    };
    assert_eq!(e.code(), "SPL_UNEXPECTED_TOKEN");
}

#[test]
fn test_parse_error_categories() {
    let e = ParseError::LexerError {
        position: 0,
        message: "err".into(),
        format: ParserFormat::Dfl,
    };
    assert_eq!(e.category(), ErrorCategory::ParseError);

    let e = ParseError::ParserError {
        line: 1,
        message: "err".into(),
        format: ParserFormat::Spl,
    };
    assert_eq!(e.category(), ErrorCategory::ParseError);

    let e = ParseError::UnexpectedToken {
        expected: "a".into(),
        found: "b".into(),
        format: ParserFormat::Dfl,
    };
    assert_eq!(e.category(), ErrorCategory::ParseError);
}

#[test]
fn test_from_parse_error_for_spindle_error() {
    let parse_err = ParseError::LexerError {
        position: 42,
        message: "invalid character '@'".into(),
        format: ParserFormat::Dfl,
    };

    let spindle_err: SpindleError = parse_err.into();

    assert_eq!(spindle_err.code(), "DFL_LEXER_ERROR");
    assert_eq!(spindle_err.category(), ErrorCategory::ParseError);
    assert!(format!("{spindle_err}").contains("invalid character '@'"));
}

#[test]
fn test_from_parse_error_preserves_code() {
    let parse_err = ParseError::ParserError {
        line: 10,
        message: "expected rule body".into(),
        format: ParserFormat::Spl,
    };

    let spindle_err: SpindleError = parse_err.into();
    assert_eq!(spindle_err.code(), "SPL_PARSE_ERROR");
}

#[test]
fn test_from_parse_error_via_question_mark() {
    fn fallible() -> Result<(), SpindleError> {
        let parse_err = ParseError::UnexpectedToken {
            expected: "=>".into(),
            found: "->".into(),
            format: ParserFormat::Dfl,
        };
        Err(parse_err)?
    }

    let err = fallible().unwrap_err();
    assert_eq!(err.code(), "DFL_UNEXPECTED_TOKEN");
    assert_eq!(err.category(), ErrorCategory::ParseError);
}

#[test]
fn test_parse_error_format_accessor() {
    let e = ParseError::ParserError {
        line: 1,
        message: "err".into(),
        format: ParserFormat::Dfl,
    };
    assert_eq!(e.format(), Some(ParserFormat::Dfl));

    let e = ParseError::ParserError {
        line: 1,
        message: "err".into(),
        format: ParserFormat::Spl,
    };
    assert_eq!(e.format(), Some(ParserFormat::Spl));
}

#[test]
fn test_parse_error_line_accessor() {
    let e = ParseError::ParserError {
        line: 42,
        message: "err".into(),
        format: ParserFormat::Dfl,
    };
    assert_eq!(e.line(), Some(42));

    let e = ParseError::LexerError {
        position: 10,
        message: "err".into(),
        format: ParserFormat::Dfl,
    };
    assert_eq!(e.line(), None);
}

// Compile-time assertion: ParseError must be Send + Sync + 'static (REQ-112)
const _: () = {
    fn assert_send_sync_static<T: Send + Sync + 'static>() {}
    fn assertions() {
        assert_send_sync_static::<ParseError>();
    }
};
