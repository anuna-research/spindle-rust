//! Reason output transport types — schemas `spindle.reason.v1` and `spindle.reason.v2`

use serde::{Deserialize, Serialize};

use crate::diagnostic::DiagnosticEntry;
use crate::literal::{LiteralStructJson, LiteralStructJsonV2};

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
    /// Trust degree (weakest-link across derivation chain). Present when `--trust` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_degree: Option<f64>,
    /// Source IDs contributing to this conclusion. Present when `--trust` is used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_sources: Option<Vec<String>>,
}

/// Theory statistics (rule and fact counts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TheoryStats {
    pub rule_count: usize,
    pub fact_count: usize,
}

// ---------------------------------------------------------------------------
// V2 types — typed term arguments
// ---------------------------------------------------------------------------

/// Schema version constants.
pub const SCHEMA_V1: &str = "spindle.reason.v1";
/// V2 schema version string.
pub const SCHEMA_V2: &str = "spindle.reason.v2";

/// Structured reason output — schema `spindle.reason.v2`.
///
/// Identical to [`ReasonOutput`] (v1) except `conclusions` use
/// [`ConclusionEntryV2`] with typed term arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasonOutputV2 {
    pub schema_version: String,
    pub evaluated_at: Option<String>,
    pub grounding: GroundingStats,
    pub conclusions: Vec<ConclusionEntryV2>,
    pub diagnostics: Vec<DiagnosticEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stats: Option<TheoryStats>,
}

/// A v2 conclusion entry with typed literal arguments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConclusionEntryV2 {
    pub conclusion_type: String,
    pub literal_spl: String,
    pub literal_struct: LiteralStructJsonV2,
    pub positive: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_degree: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trust_sources: Option<Vec<String>>,
}
