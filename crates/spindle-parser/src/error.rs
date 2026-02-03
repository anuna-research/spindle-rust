//! Parser error types

use thiserror::Error;

/// Parse error type
#[derive(Error, Debug)]
pub enum ParseError {
    /// Lexer error
    #[error("lexer error at position {position}: {message}")]
    LexerError {
        /// Position in the input string
        position: usize,
        /// Error message
        message: String,
    },

    /// Parser error
    #[error("parse error at line {line}: {message}")]
    ParserError {
        /// Line number
        line: usize,
        /// Error message
        message: String,
    },

    /// Unexpected token
    #[error("unexpected token: expected {expected}, found {found}")]
    UnexpectedToken {
        /// Expected token description
        expected: String,
        /// Found token description
        found: String,
    },

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
