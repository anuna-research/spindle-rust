//! Parser error types

use thiserror::Error;

/// Parse error type
#[derive(Error, Debug)]
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
