//! CLI error types and rendering
//!
//! Structured error and diagnostic types for contract-compliant output.
//! Includes `render_human()` and `render_json()` for converting
//! `ProblemDetails` into CLI output.

/// Structured CLI error with contract-compliant exit codes
/// Per contract §8.1: 2=user, 3=execution, 4=resource
#[derive(Debug)]
pub(crate) struct CliError {
    pub(crate) exit_code: i32,
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) details: serde_json::Value,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl CliError {
    /// Exit code 2: validation/user input error (per contract §8.1)
    pub(crate) fn validation(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        Self {
            exit_code: 2,
            code: code.clone(),
            message: message.clone(),
            details: serde_json::json!({}),
            diagnostics: vec![Diagnostic::error(&code, &message)],
        }
    }

    /// Exit code 2: parse error (per contract §8.1)
    pub(crate) fn parse(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        Self {
            exit_code: 2,
            code: code.clone(),
            message: message.clone(),
            details: serde_json::json!({}),
            diagnostics: vec![Diagnostic::error(&code, &message)],
        }
    }

    /// Exit code 3: execution/internal error (per contract §8.1)
    pub(crate) fn execution(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        Self {
            exit_code: 3,
            code: code.clone(),
            message: message.clone(),
            details: serde_json::json!({}),
            diagnostics: vec![Diagnostic::error(&code, &message)],
        }
    }

    /// Exit code 4: resource/limit error (per contract §8.1)
    #[allow(dead_code)]
    pub(crate) fn resource(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        Self {
            exit_code: 4,
            code: code.clone(),
            message: message.clone(),
            details: serde_json::json!({}),
            diagnostics: vec![Diagnostic::error(&code, &message)],
        }
    }

    /// Add details to the error (must be an object per contract §8.3)
    pub(crate) fn with_details(mut self, details: serde_json::Value) -> Self {
        // Ensure details is an object
        if !details.is_object() {
            self.details = serde_json::json!({});
        } else {
            self.details = details;
        }
        self
    }
}

/// A diagnostic message for the output envelope (per contract §6.1)
#[derive(serde::Serialize, Debug, Clone)]
pub(crate) struct Diagnostic {
    pub(crate) severity: String,
    pub(crate) code: String,
    pub(crate) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) details: Option<serde_json::Value>,
}

impl Diagnostic {
    pub(crate) fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "warning".to_string(),
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub(crate) fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "error".to_string(),
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn info(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: "info".to_string(),
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }
}

// =============================================================================
// Error rendering (SPEC-010 §8.3)
// =============================================================================

use spindle_contract::error::{ErrorReport, ProblemDetails};

/// Render a ProblemDetails as human-readable stderr output.
///
/// Used by `replace-cli-error` task to wire ProblemDetails into the CLI output boundary.
///
/// Format:
/// ```text
/// Error: <title>
///   <detail>
///
///   3 | bad line here
///       ^^^^^^^^^^^^^
///
/// Hint: <hint>
/// ```
#[allow(dead_code)]
pub(crate) fn render_human(pd: &ProblemDetails) -> String {
    let mut out = String::new();

    out.push_str(&format!("Error: {}\n", pd.title));

    if let Some(detail) = &pd.detail {
        out.push_str(&format!("  {detail}\n"));
    }

    // Show source context if available
    if let Some(ctx) = &pd.extensions.source_context {
        out.push('\n');
        for (i, line) in ctx.lines.iter().enumerate() {
            let marker = if i == ctx.highlight_index { ">" } else { " " };
            out.push_str(&format!(
                "{marker} {:>4} | {}\n",
                line.line_number, line.text
            ));
        }
        out.push('\n');
    }

    if let Some(hint) = &pd.extensions.hint {
        out.push_str(&format!("Hint: {hint}\n"));
    }

    out
}

/// Render an ErrorReport as pretty-printed JSON for stdout.
#[allow(dead_code)]
pub(crate) fn render_json(report: &ErrorReport) -> Result<String, CliError> {
    serde_json::to_string_pretty(report).map_err(|e| {
        CliError::execution(
            "JSON_SERIALIZATION_ERROR",
            format!("Failed to serialize error report: {e}"),
        )
    })
}
