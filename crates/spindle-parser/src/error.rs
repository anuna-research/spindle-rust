//! Parser error types

use spindle_core::error::{ErrorCategory, SpindleError};
use thiserror::Error;

/// The input format that produced a parse error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParserFormat {
    /// Spindle Lisp format
    Spl,
}

impl std::fmt::Display for ParserFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spl => write!(f, "SPL"),
        }
    }
}

/// Parse error type
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ParseError {
    /// Lexer error at a specific position
    #[error("{format} lexer error at position {position}: {message}")]
    LexerError {
        /// Position in the input where the error occurred
        position: usize,
        /// Description of the lexer error
        message: String,
        /// Which parser format produced this error
        format: ParserFormat,
    },

    /// Parser error at a specific line
    #[error("{format} parse error at line {line}: {message}")]
    ParserError {
        /// Line number where the error occurred
        line: usize,
        /// Description of the parse error
        message: String,
        /// Which parser format produced this error
        format: ParserFormat,
    },

    /// Unexpected token encountered
    #[error("{format} unexpected token: expected {expected}, found {found}")]
    UnexpectedToken {
        /// Token that was expected
        expected: String,
        /// Token that was found
        found: String,
        /// Which parser format produced this error
        format: ParserFormat,
    },

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl ParseError {
    /// Returns the stable error code for this variant (SPEC-010 §10.2).
    ///
    /// Codes are prefixed with the parser format.
    pub fn code(&self) -> &'static str {
        match self {
            Self::LexerError {
                format: ParserFormat::Spl,
                ..
            } => "SPL_LEXER_ERROR",
            Self::ParserError {
                format: ParserFormat::Spl,
                ..
            } => "SPL_PARSE_ERROR",
            Self::UnexpectedToken {
                format: ParserFormat::Spl,
                ..
            } => "SPL_UNEXPECTED_TOKEN",
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

    /// Returns the parser format that produced this error, if applicable.
    pub fn format(&self) -> Option<ParserFormat> {
        match self {
            Self::LexerError { format, .. }
            | Self::ParserError { format, .. }
            | Self::UnexpectedToken { format, .. } => Some(*format),
            Self::IoError(_) => None,
        }
    }

    /// Returns the line number where the error occurred, if available.
    pub fn line(&self) -> Option<usize> {
        match self {
            Self::ParserError { line, .. } => Some(*line),
            _ => None,
        }
    }
}

impl From<ParseError> for SpindleError {
    fn from(e: ParseError) -> Self {
        let line = e.line();
        SpindleError::Parse {
            code: e.code(),
            message: e.to_string(),
            line,
        }
    }
}
