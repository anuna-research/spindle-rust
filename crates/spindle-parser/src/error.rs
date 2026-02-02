//! Parser error types

use thiserror::Error;

/// Parse error type
#[derive(Error, Debug)]
pub enum ParseError {
    /// Lexer error
    #[error("lexer error at position {position}: {message}")]
    LexerError { position: usize, message: String },

    /// Parser error
    #[error("parse error at line {line}: {message}")]
    ParserError { line: usize, message: String },

    /// Unexpected token
    #[error("unexpected token: expected {expected}, found {found}")]
    UnexpectedToken { expected: String, found: String },

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
