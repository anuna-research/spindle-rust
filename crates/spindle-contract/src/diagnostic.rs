//! Diagnostic entry type for output envelopes

use serde::{Deserialize, Serialize};

/// A diagnostic message for output envelopes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticEntry {
    pub severity: String,
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}
