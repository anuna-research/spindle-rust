use serde_json::Value;
use spindle_core::literal::{Literal, LiteralStruct};
use spindle_wasm::{JsConclusionStruct, JsGroundingStats, JsReasonOutput, JsTheoryStats};

#[test]
fn test_reason_output_serializes_with_literal_struct() {
    let output = JsReasonOutput {
        schema_version: "spindle.reason.v1".to_string(),
        evaluated_at: None,
        grounding: JsGroundingStats {
            performed: false,
            had_variables: false,
            instances: 0,
            limit_hit: false,
        },
        conclusions: vec![JsConclusionStruct {
            conclusion_type: "+d".to_string(),
            literal_spl: "(bird)".to_string(),
            literal_struct: LiteralStruct::from(Literal::simple("bird")),
            positive: true,
        }],
        stats: JsTheoryStats {
            rule_count: 1,
            fact_count: 1,
        },
    };

    let value = serde_json::to_value(&output).expect("JsReasonOutput should serialize");

    match value {
        Value::Object(map) => {
            assert!(map.contains_key("conclusions"));
        }
        _ => panic!("Expected JSON object for JsReasonOutput"),
    }
}
