//! Error types for Spindle

use thiserror::Error;

/// Error category for taxonomy mapping (SPEC-010 §10.1).
///
/// Maps error variants to exit codes and default titles. Used by
/// presentation crates to convert library errors into `ProblemDetails`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCategory {
    /// Input parsing failures (SPL lexer or parser errors). Exit code 2.
    ParseError,
    /// Structural or semantic validation failures. Exit code 2.
    ValidationError,
    /// Reasoning or query execution failures. Exit code 3.
    ExecutionError,
    /// Timeouts, size limits, or truncation errors. Exit code 4.
    ResourceLimit,
    /// Unexpected failures with no safe recovery path. Exit code 3.
    InternalError,
}

impl ErrorCategory {
    /// Returns the CLI exit code for this category.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::ParseError | Self::ValidationError => 2,
            Self::ExecutionError | Self::InternalError => 3,
            Self::ResourceLimit => 4,
        }
    }

    /// Returns the default title for this category.
    pub fn default_title(&self) -> &'static str {
        match self {
            Self::ParseError => "Parse Error",
            Self::ValidationError => "Validation Error",
            Self::ExecutionError => "Execution Error",
            Self::ResourceLimit => "Resource Limit Exceeded",
            Self::InternalError => "Internal Error",
        }
    }
}

impl std::fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.default_title())
    }
}

/// Spindle error type
#[derive(Error, Debug)]
#[non_exhaustive]
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

    /// Validation error
    #[error("validation error: {message}")]
    Validation {
        /// Human-readable validation error message
        message: String,
    },

    /// Wraps a parser error via composition (ADR-107).
    ///
    /// The `code` field preserves the parser error's stable code.
    /// Constructed via `From<ParseError>` in `spindle-parser`.
    #[error("{message}")]
    Parse {
        /// Stable error code from the original `ParseError`
        code: &'static str,
        /// Human-readable error message
        message: String,
        /// Source line number where the parse error occurred
        line: Option<usize>,
    },
}

impl SpindleError {
    /// Returns the stable error code for this variant (SPEC-010 §10.2).
    pub fn code(&self) -> &'static str {
        match self {
            Self::RuleNotFound(_) => "RULE_NOT_FOUND",
            Self::InvalidLiteral(_) => "INVALID_LITERAL",
            Self::TheoryError(_) => "THEORY_ERROR",
            Self::ReasoningError(_) => "REASONING_ERROR",
            Self::Validation { .. } => "VALIDATION_ERROR",
            Self::Parse { code, .. } => code,
        }
    }

    /// Returns the source line number, if available.
    pub fn line(&self) -> Option<usize> {
        match self {
            Self::Parse { line, .. } => *line,
            _ => None,
        }
    }

    /// Returns the error category for this variant (SPEC-010 §10.2).
    pub fn category(&self) -> ErrorCategory {
        match self {
            Self::RuleNotFound(_) => ErrorCategory::ExecutionError,
            Self::InvalidLiteral(_) => ErrorCategory::ValidationError,
            Self::TheoryError(_) => ErrorCategory::ValidationError,
            Self::ReasoningError(_) => ErrorCategory::ExecutionError,
            Self::Validation { .. } => ErrorCategory::ValidationError,
            Self::Parse { .. } => ErrorCategory::ParseError,
        }
    }
}

/// Result type alias for Spindle operations
pub type Result<T> = std::result::Result<T, SpindleError>;
