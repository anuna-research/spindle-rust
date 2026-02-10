//! Parser error types

use spindle_core::error::ErrorCategory;
use thiserror::Error;

/// Parse error type
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// Lexer error at a specific position
    #[error("lexer error at position {position}: {message}")]
    LexerError {
        /// Position in the input where the error occurred
        position: usize,
        /// Description of the lexer error
        message: String,
    },

    /// Parser error at a specific line
    #[error("parse error at line {line}: {message}")]
    ParserError {
        /// Line number where the error occurred
        line: usize,
        /// Description of the parse error
        message: String,
    },

    /// Unexpected token encountered
    #[error("unexpected token: expected {expected}, found {found}")]
    UnexpectedToken {
        /// Token that was expected
        expected: String,
        /// Token that was found
        found: String,
    },

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl ParseError {
    /// Returns the stable error code for this variant (SPEC-010 §10.2).
    pub fn code(&self) -> &'static str {
        match self {
            Self::LexerError { .. } => "LEXER_ERROR",
            Self::ParserError { .. } => "PARSE_ERROR",
            Self::UnexpectedToken { .. } => "UNEXPECTED_TOKEN",
            Self::IoError(_) => "IO_ERROR",
        }
    }

    /// Returns the error category for this variant (SPEC-010 §10.2).
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::LexerError { .. } | Self::ParserError { .. } | Self::UnexpectedToken { .. } => {
                ErrorCategory::ParseError
            }
            Self::IoError(_) => ErrorCategory::InternalError,
        }
    }
}
