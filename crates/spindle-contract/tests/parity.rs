//! DTO parity tests (TEST-203)
//!
//! Verify that identical theory outcomes produce identical JSON when serialized
//! through the shared contract types. Since both CLI and WASM now use the same
//! contract types, this validates the shared serialization path.

use serde_json::{Value, json};

use spindle_contract::diagnostic::DiagnosticEntry;
use spindle_contract::error::{Diagnostic as ErrorDiagnostic, ErrorReport, ProblemDetails};
use spindle_contract::literal::{LiteralStructJson, ModeJson, TemporalJson};
use spindle_contract::reason::{ConclusionEntry, GroundingStats, ReasonOutput, TheoryStats};
use spindle_core::literal::Literal;

/// Build a ReasonOutput from a set of conclusions, mimicking what both CLI and WASM do.
fn build_reason_output(literals: &[(&str, &str, bool)]) -> ReasonOutput {
    let mut conclusions: Vec<ConclusionEntry> = literals
        .iter()
        .map(|(name, conclusion_type, positive)| {
            let lit = if *positive {
                Literal::simple(*name)
            } else {
                Literal::negated(*name)
            };
            ConclusionEntry {
                conclusion_type: conclusion_type.to_string(),
                literal_spl: lit.to_spl(),
                literal_struct: LiteralStructJson::from(&lit),
                positive: *positive,
            }
        })
        .collect();

    // Sort for deterministic output (same sort as both CLI and WASM)
    conclusions.sort_by(|a, b| {
        a.literal_spl
            .cmp(&b.literal_spl)
            .then_with(|| a.conclusion_type.cmp(&b.conclusion_type))
    });

    ReasonOutput {
        schema_version: "spindle.reason.v1".to_string(),
        evaluated_at: None,
        grounding: GroundingStats {
            performed: false,
            had_variables: false,
            instances: 0,
            limit_hit: false,
        },
        conclusions,
        diagnostics: vec![],
        stats: Some(TheoryStats {
            rule_count: 2,
            fact_count: 1,
        }),
    }
}

#[test]
fn test_reason_output_round_trip() {
    let output = build_reason_output(&[("bird", "+D", true), ("flies", "+d", true)]);

    let json = serde_json::to_string_pretty(&output).expect("serialize");
    let deserialized: ReasonOutput = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(deserialized.schema_version, "spindle.reason.v1");
    assert_eq!(deserialized.conclusions.len(), 2);
    assert_eq!(deserialized.diagnostics.len(), 0);
    assert_eq!(deserialized.stats.as_ref().unwrap().rule_count, 2);
}

#[test]
fn test_reason_output_deterministic_ordering() {
    // Build the same output twice — must produce identical JSON
    let output1 = build_reason_output(&[
        ("bird", "+D", true),
        ("flies", "+d", true),
        ("flies", "-d", false),
    ]);
    let output2 = build_reason_output(&[
        ("bird", "+D", true),
        ("flies", "+d", true),
        ("flies", "-d", false),
    ]);

    let json1 = serde_json::to_string(&output1).unwrap();
    let json2 = serde_json::to_string(&output2).unwrap();
    assert_eq!(json1, json2, "Identical inputs must produce identical JSON");
}

#[test]
fn test_reason_output_schema_fields_present() {
    let output = build_reason_output(&[("bird", "+D", true)]);
    let value: Value = serde_json::to_value(&output).unwrap();
    let obj = value.as_object().unwrap();

    // All required top-level fields must be present
    assert!(obj.contains_key("schema_version"));
    assert!(obj.contains_key("evaluated_at"));
    assert!(obj.contains_key("grounding"));
    assert!(obj.contains_key("conclusions"));
    assert!(obj.contains_key("diagnostics"));
    assert!(obj.contains_key("stats"));
}

#[test]
fn test_conclusion_entry_fields() {
    let lit = Literal::simple("bird");
    let entry = ConclusionEntry {
        conclusion_type: "+D".to_string(),
        literal_spl: lit.to_spl(),
        literal_struct: LiteralStructJson::from(&lit),
        positive: true,
    };

    let value: Value = serde_json::to_value(&entry).unwrap();
    let obj = value.as_object().unwrap();

    assert!(obj.contains_key("conclusion_type"));
    assert!(obj.contains_key("literal_spl"));
    assert!(obj.contains_key("literal_struct"));
    assert!(obj.contains_key("positive"));

    // Verify literal_struct nested fields
    let lit_obj = obj["literal_struct"].as_object().unwrap();
    assert!(lit_obj.contains_key("mode"));
    assert!(lit_obj.contains_key("negated"));
    assert!(lit_obj.contains_key("functor"));
    assert!(lit_obj.contains_key("args"));
    assert!(lit_obj.contains_key("temporal"));
}

#[test]
fn test_literal_struct_json_from_simple() {
    let lit = Literal::simple("bird");
    let json = LiteralStructJson::from(&lit);

    assert_eq!(json.functor, "bird");
    assert!(!json.negated);
    assert!(json.args.is_empty());
    assert!(!json.mode.negation);
    assert!(json.mode.name.is_none());
}

#[test]
fn test_literal_struct_json_from_negated() {
    let lit = Literal::negated("flies");
    let json = LiteralStructJson::from(&lit);

    assert_eq!(json.functor, "flies");
    assert!(json.negated);
}

#[test]
fn test_temporal_json_infinity_maps_to_null() {
    // Default temporal has NegInf start and PosInf end
    let lit = Literal::simple("test");
    let json = LiteralStructJson::from(&lit);

    // Infinities map to None → null in JSON
    assert!(json.temporal.start.is_none());
    assert!(json.temporal.end.is_none());

    // Verify JSON serialization
    let value: Value = serde_json::to_value(&json.temporal).unwrap();
    assert!(value["start"].is_null());
    assert!(value["end"].is_null());
}

#[test]
fn test_mode_json_from_empty_mode() {
    let mode = spindle_core::mode::Mode::empty();
    let json = ModeJson::from(&mode);

    assert!(json.name.is_none());
    assert!(!json.negation);
}

#[test]
fn test_temporal_json_with_moments() {
    use spindle_core::temporal::{Temporal, TimePoint};

    let temporal = Temporal {
        start: TimePoint::Moment(100),
        end: TimePoint::Moment(200),
    };
    let json = TemporalJson::from(&temporal);

    assert_eq!(json.start, Some(100));
    assert_eq!(json.end, Some(200));
}

#[test]
fn test_diagnostic_entry_round_trip() {
    let mut details = serde_json::Map::new();
    details.insert(
        "limit".to_string(),
        serde_json::Value::String("Max 1000 instances".to_string()),
    );
    let diag = DiagnosticEntry {
        severity: "warning".to_string(),
        code: "GROUNDING_LIMIT".to_string(),
        message: "Grounding limit reached".to_string(),
        details: Some(details.clone()),
    };

    let json = serde_json::to_string(&diag).unwrap();
    let deserialized: DiagnosticEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.severity, "warning");
    assert_eq!(deserialized.code, "GROUNDING_LIMIT");
    assert_eq!(deserialized.details, Some(details));
}

#[test]
fn test_diagnostic_entry_details_null_omitted() {
    let diag = DiagnosticEntry {
        severity: "info".to_string(),
        code: "INFO_CODE".to_string(),
        message: "Info message".to_string(),
        details: None,
    };

    let value: Value = serde_json::to_value(&diag).unwrap();
    // details should be omitted (skip_serializing_if = "Option::is_none")
    assert!(!value.as_object().unwrap().contains_key("details"));
}

#[test]
fn test_error_report_diagnostics_include_severity_and_details() {
    let problem = ProblemDetails::new("TEST_ERROR", "Test error", 3);
    let diagnostics = vec![ErrorDiagnostic {
        severity: "warning".to_string(),
        code: "TEST_DIAG".to_string(),
        message: "Diagnostic message".to_string(),
        details: Some(json!({"note": "extra"})),
    }];

    let report =
        ErrorReport::new(None, "TEST_ERROR", "Test error", problem).with_diagnostics(diagnostics);

    let value: Value = serde_json::to_value(&report).unwrap();
    let diag = &value["diagnostics"][0];

    assert_eq!(diag["severity"], "warning");
    assert_eq!(diag["code"], "TEST_DIAG");
    assert_eq!(diag["message"], "Diagnostic message");
    assert_eq!(diag["details"]["note"], "extra");
}

#[test]
fn test_stats_none_omitted() {
    let output = ReasonOutput {
        schema_version: "spindle.reason.v1".to_string(),
        evaluated_at: None,
        grounding: GroundingStats {
            performed: false,
            had_variables: false,
            instances: 0,
            limit_hit: false,
        },
        conclusions: vec![],
        diagnostics: vec![],
        stats: None,
    };

    let value: Value = serde_json::to_value(&output).unwrap();
    // stats should be omitted when None (skip_serializing_if)
    assert!(!value.as_object().unwrap().contains_key("stats"));
}
