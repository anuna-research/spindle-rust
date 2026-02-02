//! Error types for Spindle

use thiserror::Error;

/// Spindle error type
#[derive(Error, Debug)]
pub enum SpindleError {
    /// Rule not found in theory
    #[error("rule not found: {0}")]
    RuleNotFound(String),

    /// Literal parsing error
    #[error("invalid literal: {0}")]
    InvalidLiteral(String),

    /// Theory construction error
    #[error("theory error: {0}")]
    TheoryError(String),

    /// Reasoning error
    #[error("reasoning error: {0}")]
    ReasoningError(String),
}

/// Result type alias for Spindle operations
pub type Result<T> = std::result::Result<T, SpindleError>;
