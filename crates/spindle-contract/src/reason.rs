//! Reason output transport types — schema `spindle.reason.v1`

use serde::{Deserialize, Serialize};

use crate::diagnostic::DiagnosticEntry;
use crate::literal::LiteralStructJson;

/// Structured reason output — schema `spindle.reason.v1`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasonOutput {
    pub schema_version: String,
    pub evaluated_at: Option<String>,
    pub grounding: GroundingStats,
    pub conclusions: Vec<ConclusionEntry>,
    pub diagnostics: Vec<DiagnosticEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<TheoryStats>,
}

/// Grounding statistics from the preparation pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundingStats {
    pub performed: bool,
    pub had_variables: bool,
    pub instances: usize,
    pub limit_hit: bool,
}

/// A single conclusion entry for structured output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConclusionEntry {
    pub conclusion_type: String,
    pub literal_spl: String,
    pub literal_struct: LiteralStructJson,
    pub positive: bool,
}

/// Theory statistics (rule and fact counts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TheoryStats {
    pub rule_count: usize,
    pub fact_count: usize,
}
