//! CLI error types and rendering
//!
//! Structured error and diagnostic types for contract-compliant output.
//! `CliError` carries a `ProblemDetails` for RFC 9457 error rendering.
//! `render_human()` and `render_json()` convert `ProblemDetails` into
//! CLI output for stderr and stdout respectively.

use spindle_contract::error::{ErrorReport, ProblemDetails};

/// Structured CLI error with contract-compliant exit codes.
///
/// Each error carries a `ProblemDetails` for RFC 9457-compliant output.
/// Per contract §8.1: exit code 2=user, 3=execution, 4=resource.
#[derive(Debug)]
pub(crate) struct CliError {
    pub(crate) exit_code: i32,
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) problem: Box<ProblemDetails>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    /// Structured metadata from `with_details` call sites, merged into JSON output.
    pub(crate) details_metadata: serde_json::Map<String, serde_json::Value>,
}

impl CliError {
    /// Exit code 2: validation/user input error (per contract §8.1)
    pub(crate) fn validation(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        let problem = ProblemDetails::new(&code, "Validation Error", 2).with_detail(&message);
        Self {
            exit_code: 2,
            code: code.clone(),
            message: message.clone(),
            problem: Box::new(problem),
            diagnostics: vec![Diagnostic::error(&code, &message)],
            details_metadata: serde_json::Map::new(),
        }
    }

    /// Exit code 2: parse error (per contract §8.1)
    pub(crate) fn parse(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        let problem = ProblemDetails::new(&code, "Parse Error", 2).with_detail(&message);
        Self {
            exit_code: 2,
            code: code.clone(),
            message: message.clone(),
            problem: Box::new(problem),
            diagnostics: vec![Diagnostic::error(&code, &message)],
            details_metadata: serde_json::Map::new(),
        }
    }

    /// Exit code 3: execution/internal error (per contract §8.1)
    pub(crate) fn execution(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        let problem = ProblemDetails::new(&code, "Execution Error", 3).with_detail(&message);
        Self {
            exit_code: 3,
            code: code.clone(),
            message: message.clone(),
            problem: Box::new(problem),
            diagnostics: vec![Diagnostic::error(&code, &message)],
            details_metadata: serde_json::Map::new(),
        }
    }

    /// Exit code 4: resource/limit error (per contract §8.1)
    #[allow(dead_code)]
    pub(crate) fn resource(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        let message = message.into();
        let problem =
            ProblemDetails::new(&code, "Resource Limit Exceeded", 4).with_detail(&message);
        Self {
            exit_code: 4,
            code: code.clone(),
            message: message.clone(),
            problem: Box::new(problem),
            diagnostics: vec![Diagnostic::error(&code, &message)],
            details_metadata: serde_json::Map::new(),
        }
    }

    /// Add details to the error.
    ///
    /// Extracts `hint` into ProblemDetails and stores the full details map
    /// for inclusion in JSON output. Per contract §8.3.
    pub(crate) fn with_details(mut self, details: serde_json::Value) -> Self {
        if let Some(obj) = details.as_object() {
            if let Some(hint) = obj.get("hint").and_then(|v| v.as_str()) {
                self.problem.extensions.hint = Some(hint.to_string());
            }
            // Store all fields (except hint, which is already on ProblemDetails)
            // for merging into ErrorReport.error.details
            for (key, value) in obj {
                if key != "hint" {
                    self.details_metadata.insert(key.clone(), value.clone());
                }
            }
        }
        self
    }

    /// Attach the source file name or "stdin" to the error.
    pub(crate) fn with_source_name(mut self, name: impl Into<String>) -> Self {
        self.problem.extensions.source_name = Some(name.into());
        self
    }

    /// Attach source location (line, optional column) to the error.
    pub(crate) fn with_location(mut self, line: usize, column: Option<usize>) -> Self {
        self.problem.extensions.line = Some(line);
        self.problem.extensions.column = column;
        self
    }

    /// Attach source context lines around the error location.
    pub(crate) fn with_source_context(
        mut self,
        ctx: spindle_contract::error::SourceContext,
    ) -> Self {
        self.problem.extensions.source_context = Some(ctx);
        self
    }

    /// Build an `ErrorReport` from this error for JSON output.
    pub(crate) fn to_error_report(&self, schema_version: Option<&'static str>) -> ErrorReport {
        let contract_diagnostics: Vec<spindle_contract::error::Diagnostic> = self
            .diagnostics
            .iter()
            .map(|d| spindle_contract::error::Diagnostic {
                severity: d.severity.clone(),
                code: d.code.clone(),
                message: d.message.clone(),
                details: d.details.clone(),
            })
            .collect();

        ErrorReport::new(
            schema_version,
            &self.code,
            &self.message,
            (*self.problem).clone(),
        )
        .with_details_metadata(self.details_metadata.clone())
        .with_diagnostics(contract_diagnostics)
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

/// Render a ProblemDetails as human-readable stderr output.
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
pub(crate) fn render_human(pd: &ProblemDetails, debug: bool) -> String {
    use super::redact::{redact_detail, redact_source_name};

    let mut out = String::new();

    out.push_str(&format!("Error: {}\n", pd.title));

    if let Some(detail) = &pd.detail {
        let detail = redact_detail(detail, debug);
        out.push_str(&format!("  {detail}\n"));
    }

    // Show source location if available (always, not just in debug mode)
    if let Some(name) = &pd.extensions.source_name {
        let name = redact_source_name(name, debug);
        let loc = match (pd.extensions.line, pd.extensions.column) {
            (Some(l), Some(c)) => format!("  --> {name}:{l}:{c}\n"),
            (Some(l), None) => format!("  --> {name}:{l}\n"),
            _ => format!("  --> {name}\n"),
        };
        out.push_str(&loc);
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

    // In debug mode, show the error type URI and exit code
    if debug {
        out.push_str(&format!(
            "Code: {} (exit {})\n",
            pd.problem_type, pd.extensions.exit_code
        ));
    }

    if let Some(hint) = &pd.extensions.hint {
        out.push_str(&format!("Hint: {hint}\n"));
    }

    out
}

/// Render an ErrorReport as pretty-printed JSON for stdout.
pub(crate) fn render_json(report: &ErrorReport) -> Result<String, CliError> {
    serde_json::to_string_pretty(report).map_err(|e| {
        CliError::execution(
            "JSON_SERIALIZATION_ERROR",
            format!("Failed to serialize error report: {e}"),
        )
    })
}
