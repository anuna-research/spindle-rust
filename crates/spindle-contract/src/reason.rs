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
    /// Full diminishment and threshold results when trust is requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_details: Option<TrustDetails>,
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
    /// Full diminishment and threshold results when trust is requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_details: Option<TrustDetails>,
}

/// Trust provenance and named threshold outcomes after diminishment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustDetails {
    pub diminished_by: Vec<DiminisherEntry>,
    pub above_threshold: std::collections::BTreeMap<String, bool>,
}

/// One overruled defeater's effect, in application order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiminisherEntry {
    pub defeater_label: String,
    pub defeater_degree: f64,
    pub target_degree: f64,
    pub resulting_degree: f64,
    pub diminishment: f64,
    pub full_defeat: bool,
}

impl From<&spindle_core::trust::WeightedConclusion> for TrustDetails {
    fn from(w: &spindle_core::trust::WeightedConclusion) -> Self {
        Self {
            diminished_by: w
                .diminished_by
                .iter()
                .map(|d| DiminisherEntry {
                    defeater_label: d.defeater_label.clone(),
                    defeater_degree: d.defeater_degree,
                    target_degree: d.target_degree,
                    resulting_degree: d.resulting_degree(),
                    diminishment: d.diminishment,
                    full_defeat: d.full_defeat,
                })
                .collect(),
            above_threshold: w
                .above_threshold
                .iter()
                .map(|(k, v)| (k.clone(), *v))
                .collect(),
        }
    }
}

/// Project prepared conclusions into either public reason envelope.
/// No reasoning is performed here; callers supply the matching trust vector.
pub fn reason_output(
    prepared: &spindle_core::pipeline::PipelineResult,
    conclusions: &[spindle_core::Conclusion],
    weighted: Option<&[spindle_core::trust::WeightedConclusion]>,
    positive_only: bool,
    v2: bool,
) -> serde_json::Value {
    use serde_json::json;
    let mut entries: Vec<_> = conclusions.iter().enumerate()
        .filter(|(_, c)| !positive_only || c.is_positive())
        .map(|(i,c)| {
            let mut entry = json!({
                "conclusion_type":c.conclusion_type.symbol(),
                "literal_spl":c.literal.to_spl(),
                "literal_struct": if v2 { json!(LiteralStructJsonV2::from(&c.literal)) } else { json!(LiteralStructJson::from(&c.literal)) },
                "positive":c.is_positive()
            });
            if let Some(w) = weighted.and_then(|w| w.get(i)) {
                let mut sources: Vec<_> = w.sources.iter().map(|s| s.id.clone()).collect(); sources.sort();
                entry["trust_degree"] = json!(w.degree);
                if !sources.is_empty() { entry["trust_sources"] = json!(sources); }
                entry["trust_details"] = json!(TrustDetails::from(w));
            }
            entry
        }).collect();
    entries.sort_by(|a, b| {
        a["literal_spl"]
            .as_str()
            .cmp(&b["literal_spl"].as_str())
            .then_with(|| {
                a["conclusion_type"]
                    .as_str()
                    .cmp(&b["conclusion_type"].as_str())
            })
    });
    json!({
        "schema_version": if v2 { SCHEMA_V2 } else { SCHEMA_V1 },
        "evaluated_at":prepared.evaluated_at.and_then(|t| t.to_rfc3339()),
        "grounding": {"performed":prepared.grounding_report.performed,"had_variables":prepared.grounding_report.had_variables,"instances":prepared.grounding_report.instances,"limit_hit":prepared.grounding_report.limit_hit},
        "conclusions":entries,"diagnostics":[],
        "stats":{"rule_count":prepared.theory.rule_count(),"fact_count":prepared.theory.facts().count()}
    })
}
