//! CLI error types
//!
//! Structured error and diagnostic types for contract-compliant output.

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
