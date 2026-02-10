use serde_json::Value;
use spindle_contract::literal::LiteralStructJson;
use spindle_contract::reason::{ConclusionEntry, GroundingStats, ReasonOutput, TheoryStats};
use spindle_core::literal::Literal;

#[test]
fn test_reason_output_serializes_with_literal_struct() {
    let output = ReasonOutput {
        schema_version: "spindle.reason.v1".to_string(),
        evaluated_at: None,
        grounding: GroundingStats {
            performed: false,
            had_variables: false,
            instances: 0,
            limit_hit: false,
        },
        conclusions: vec![ConclusionEntry {
            conclusion_type: "+d".to_string(),
            literal_spl: "(bird)".to_string(),
            literal_struct: LiteralStructJson::from(&Literal::simple("bird")),
            positive: true,
        }],
        diagnostics: vec![],
        stats: Some(TheoryStats {
            rule_count: 1,
            fact_count: 1,
        }),
    };

    let value = serde_json::to_value(&output).expect("ReasonOutput should serialize");

    match value {
        Value::Object(map) => {
            assert!(map.contains_key("conclusions"));
            assert!(map.contains_key("diagnostics"));
        }
        _ => panic!("Expected JSON object for ReasonOutput"),
    }
}
