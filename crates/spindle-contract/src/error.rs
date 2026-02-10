//! Error envelope and RFC 9457 Problem Details types (SPEC-010, DESIGN-001 §4.4)
//!
//! These types define the structured error output format used by CLI and WASM
//! presentation crates. `ProblemDetails` follows RFC 9457 without the HTTP
//! `status` field.

use serde::Serialize;

/// RFC 9457 Problem Details (without HTTP `status` field).
///
/// The `type` field uses the `tag:spindle.dev,2026:error:{CODE}` URI scheme.
#[derive(Debug, Clone, Serialize)]
pub struct ProblemDetails {
    /// Problem type URI (`tag:spindle.dev,2026:error:{CODE}`)
    #[serde(rename = "type")]
    pub problem_type: String,
    /// Short, human-readable summary of the problem
    pub title: String,
    /// Detailed human-readable explanation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// URI reference identifying the specific occurrence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// Spindle-specific extension fields
    #[serde(flatten)]
    pub extensions: ProblemExtensions,
}

/// Spindle-specific extensions to RFC 9457 Problem Details.
#[derive(Debug, Clone, Serialize)]
pub struct ProblemExtensions {
    /// CLI exit code for this error
    pub exit_code: u8,
    /// Source line number where the error occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// Source column number where the error occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// Actionable hint for the user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Name of the source file or input
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_name: Option<String>,
    /// Source code context around the error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_context: Option<SourceContext>,
}

/// Source code context around an error location.
#[derive(Debug, Clone, Serialize)]
pub struct SourceContext {
    /// Lines of source code around the error
    pub lines: Vec<SourceLine>,
    /// Index into `lines` identifying the error line
    pub highlight_index: usize,
}

/// A single line of source code with its line number.
#[derive(Debug, Clone, Serialize)]
pub struct SourceLine {
    /// 1-based line number in the source
    pub line_number: usize,
    /// The text content of the line
    pub text: String,
}

/// Top-level error report for JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorReport {
    /// Schema version for the error report
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<&'static str>,
    /// Diagnostic messages
    pub diagnostics: Vec<Diagnostic>,
    /// The error envelope
    pub error: ErrorEnvelope,
}

/// Error envelope wrapping problem details.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorEnvelope {
    /// Stable error code
    pub code: String,
    /// Human-readable error message
    pub message: String,
    /// Structured error details
    pub details: ErrorDetails,
}

/// Container for problem details within the error envelope.
#[derive(Debug, Clone, Serialize)]
pub struct ErrorDetails {
    /// RFC 9457 problem details
    pub problem: ProblemDetails,
}

/// A diagnostic message for error reports.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// Diagnostic code
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Additional detail
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ProblemDetails {
    /// Create a new ProblemDetails with the standard Spindle type URI.
    pub fn new(code: &str, title: impl Into<String>, exit_code: u8) -> Self {
        Self {
            problem_type: format!("tag:spindle.dev,2026:error:{code}"),
            title: title.into(),
            detail: None,
            instance: None,
            extensions: ProblemExtensions {
                exit_code,
                line: None,
                column: None,
                hint: None,
                source_name: None,
                source_context: None,
            },
        }
    }

    /// Set the detail message.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set the hint.
    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.extensions.hint = Some(hint.into());
        self
    }

    /// Set the source location.
    pub fn with_location(mut self, line: usize, column: Option<usize>) -> Self {
        self.extensions.line = Some(line);
        self.extensions.column = column;
        self
    }

    /// Set the source name.
    pub fn with_source_name(mut self, name: impl Into<String>) -> Self {
        self.extensions.source_name = Some(name.into());
        self
    }

    /// Set the source context.
    pub fn with_source_context(mut self, context: SourceContext) -> Self {
        self.extensions.source_context = Some(context);
        self
    }
}

impl SourceContext {
    /// Build a source context from source text around a given line.
    ///
    /// Extracts up to `context_lines` lines before and after `error_line` (1-based).
    pub fn from_source(source: &str, error_line: usize, context_lines: usize) -> Self {
        let all_lines: Vec<&str> = source.lines().collect();
        let start = error_line.saturating_sub(context_lines + 1);
        let end = (error_line + context_lines).min(all_lines.len());

        let lines: Vec<SourceLine> = (start..end)
            .map(|i| SourceLine {
                line_number: i + 1,
                text: all_lines[i].to_string(),
            })
            .collect();

        let highlight_index = error_line.saturating_sub(start + 1);

        Self {
            lines,
            highlight_index,
        }
    }
}

// =============================================================================
// From conversions: library errors → ProblemDetails (ADR-104)
// =============================================================================

impl From<&spindle_core::error::SpindleError> for ProblemDetails {
    fn from(err: &spindle_core::error::SpindleError) -> Self {
        use spindle_core::error::SpindleError;

        let code = err.code();
        let category = err.category();

        let mut pd = ProblemDetails::new(code, category.default_title(), category.exit_code())
            .with_detail(err.to_string());

        // Extract location info from Parse variant
        if let Some(line) = err.line() {
            pd = pd.with_location(line, None);
        }

        // Add category-specific hints
        match err {
            SpindleError::RuleNotFound(label) => {
                pd = pd.with_hint(format!(
                    "Check that rule '{label}' is defined in the theory."
                ));
            }
            SpindleError::InvalidLiteral(lit) => {
                pd = pd.with_hint(format!(
                    "Check the syntax of literal '{lit}'. Literals must be alphanumeric identifiers."
                ));
            }
            SpindleError::Parse { .. } => {
                pd = pd.with_hint(
                    "Check the input syntax. Use --stdin with DFL or SPL format.".to_string(),
                );
            }
            _ => {}
        }

        pd
    }
}

impl ErrorReport {
    /// Build an error report from a SpindleError.
    pub fn from_spindle_error(
        err: &spindle_core::error::SpindleError,
        schema_version: &'static str,
    ) -> Self {
        let problem = ProblemDetails::from(err);
        Self {
            schema_version: Some(schema_version),
            diagnostics: vec![],
            error: ErrorEnvelope {
                code: err.code().to_string(),
                message: err.to_string(),
                details: ErrorDetails { problem },
            },
        }
    }

    /// Build an error report from an error code, message, and ProblemDetails.
    pub fn new(
        schema_version: Option<&'static str>,
        code: impl Into<String>,
        message: impl Into<String>,
        problem: ProblemDetails,
    ) -> Self {
        Self {
            schema_version,
            diagnostics: vec![],
            error: ErrorEnvelope {
                code: code.into(),
                message: message.into(),
                details: ErrorDetails { problem },
            },
        }
    }

    /// Add diagnostics to the report.
    pub fn with_diagnostics(mut self, diagnostics: Vec<Diagnostic>) -> Self {
        self.diagnostics = diagnostics;
        self
    }
}

/// Convert a `ParseError` to `ProblemDetails` via its code/category methods.
///
/// This function is provided for presentation crates that have access to
/// `ParseError` directly (spindle-parser is not a dependency of this crate).
/// For indirect conversions, use `SpindleError::Parse` → `ProblemDetails`.
pub fn parse_error_to_problem_details(
    code: &str,
    message: &str,
    category: spindle_core::error::ErrorCategory,
    line: Option<usize>,
    hint: Option<String>,
) -> ProblemDetails {
    let mut pd = ProblemDetails::new(code, category.default_title(), category.exit_code())
        .with_detail(message.to_string());
    if let Some(l) = line {
        pd = pd.with_location(l, None);
    }
    if let Some(h) = hint {
        pd = pd.with_hint(h);
    }
    pd
}
