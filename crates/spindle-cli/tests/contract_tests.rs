//! Contract validation tests
//!
//! These tests verify that spindle CLI outputs conform to the v1 contract.
//! Uses shared helpers from common/mod.rs for schema validation.

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

mod common;
use common::{spindle_cmd, validate_against_schema, validate_error_envelope};

fn setup_theory_file(content: &str, extension: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join(format!("theory.{extension}"));
    fs::write(&file_path, content).unwrap();
    (dir, file_path)
}

#[test]
fn test_reason_output_validates_against_schema() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("reason")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(
        &json,
        "reason",
        "test_reason_output_validates_against_schema",
    );

    assert_eq!(json["schema_version"], "spindle.reason.v1");
    assert!(json["evaluated_at"].is_null() || json["evaluated_at"].is_string());
    assert!(json["grounding"].is_object());
    assert!(json["conclusions"].is_array());
    assert!(json["diagnostics"].is_array());
}

#[test]
fn test_query_output_validates_against_schema() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("query")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(&json, "query", "test_query_output_validates_against_schema");

    assert_eq!(json["schema_version"], "spindle.query.v1");
    assert_eq!(json["status"], "provable");
    assert!(json["literal_spl"].is_string());
    assert!(json["literal_struct"].is_object());
    assert!(json["trust"].is_null());
    assert!(json["evaluated_at"].is_null() || json["evaluated_at"].is_string());
}

#[test]
fn test_query_unknown_validates_against_schema() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("query")
        .arg("unknown_literal")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(
        &json,
        "query",
        "test_query_unknown_validates_against_schema",
    );

    assert_eq!(json["status"], "unknown");
    assert!(json["conclusion_type"].is_null());
}

#[test]
fn test_requires_output_validates_against_schema() {
    let content = r#"
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("requires")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(
        &json,
        "requires",
        "test_requires_output_validates_against_schema",
    );

    assert_eq!(json["schema_version"], "spindle.requires.v1");
    assert!(json["goal_spl"].is_string());
    assert!(json["goal_struct"].is_object());
    assert!(json["satisfied"].is_boolean());
    assert!(json["solutions"].is_array());
    assert!(json["trust"].is_null());
}

#[test]
fn test_requires_satisfied_has_empty_solutions() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("requires")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(
        &json,
        "requires",
        "test_requires_satisfied_has_empty_solutions",
    );

    assert_eq!(json["satisfied"], true);
    assert_eq!(json["solutions"].as_array().unwrap().len(), 0);
}

#[test]
fn test_requires_unsatisfied_has_non_empty_solutions() {
    let content = r#"
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("requires")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(
        &json,
        "requires",
        "test_requires_unsatisfied_has_non_empty_solutions",
    );

    assert_eq!(json["satisfied"], false);
    assert!(json["solutions"].as_array().unwrap().len() > 0);
    let solution = &json["solutions"][0];
    assert!(solution["score"].is_number());
}

#[test]
fn test_explain_output_validates_against_schema() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("explain")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(
        &json,
        "explain",
        "test_explain_output_validates_against_schema",
    );

    assert_eq!(json["schema_version"], "spindle.explain.v1");
    assert_eq!(json["status"], "provable");
    assert!(json["literal_spl"].is_string());
    assert!(json["literal_struct"].is_object());
    assert!(json["trust"].is_null());
    assert!(json["diagnostics"].is_array());
}

#[test]
fn test_explain_not_provable_validates_against_schema() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("explain")
        .arg("swims")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(
        &json,
        "explain",
        "test_explain_not_provable_validates_against_schema",
    );

    assert_eq!(json["status"], "unknown");
    assert!(json["proof_tree"].is_null());
    assert!(json["diagnostics"].as_array().unwrap().len() > 0);
}

#[test]
fn test_why_not_output_validates_against_schema() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("why-not")
        .arg("swims")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(
        &json,
        "why_not",
        "test_why_not_output_validates_against_schema",
    );

    assert_eq!(json["schema_version"], "spindle.why_not.v1");
    assert!(json["literal_spl"].is_string());
    assert!(json["literal_struct"].is_object());
    assert!(json["blocked_by"].is_array());
    assert!(json["trust"].is_null());
    assert!(json["diagnostics"].is_array());
}

#[test]
fn test_capabilities_output_validates_against_schema() {
    let output = spindle_cmd()
        .arg("capabilities")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(
        &json,
        "capabilities",
        "test_capabilities_output_validates_against_schema",
    );

    assert_eq!(json["schema_version"], "spindle.capabilities.v1");
    assert!(json["commands"].is_array());
    assert!(json["features"].is_object());
    assert!(json["schemas"].is_object());
}

#[test]
fn test_query_exit_code_0_for_unknown() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("query")
        .arg("unknown_literal")
        .arg(&path)
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "query unknown should return exit code 0"
    );
}

#[test]
fn test_requires_exit_code_0_for_unsatisfied() {
    let content = r#"
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("requires")
        .arg("flies")
        .arg(&path)
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "requires unsatisfied should return exit code 0"
    );
}

#[test]
fn test_explain_exit_code_0_for_not_provable() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("explain")
        .arg("swims")
        .arg(&path)
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "explain not provable should return exit code 0"
    );
}

#[test]
fn test_status_values_normalized() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let cases = vec![("flies", "provable"), ("unknown_literal", "unknown")];

    for (literal, expected_status) in cases {
        let output = spindle_cmd()
            .arg("query")
            .arg(literal)
            .arg(&path)
            .arg("--json")
            .output()
            .expect("Failed to execute command");

        let json: Value =
            serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

        assert_eq!(
            json["status"], expected_status,
            "Status should be normalized to exactly provable|refuted|unknown"
        );
    }
}

#[test]
fn test_no_legacy_status_strings() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("query")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json_str = String::from_utf8(output.stdout).unwrap();

    assert!(
        !json_str.contains("\"proven\""),
        "Should not contain 'proven'"
    );
    assert!(
        !json_str.contains("\"true\""),
        "Should not contain 'true' as status"
    );
    assert!(
        !json_str.contains("\"disproven\""),
        "Should not contain 'disproven'"
    );
}

#[test]
fn test_why_not_refuted_status() {
    let content = r#"
f1: >> ~flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("why-not")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    assert_eq!(
        json["status"], "refuted",
        "why-not should return 'refuted' for refuted literals"
    );
}

#[test]
fn test_requires_max_zero_rejected() {
    let content = r#"
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("requires")
        .arg("flies")
        .arg(&path)
        .arg("--max")
        .arg("0")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    assert_eq!(
        output.status.code(),
        Some(2),
        "--max 0 should be rejected with exit code 2"
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_error_envelope(&json, "test_requires_max_zero_rejected");
    assert_eq!(json["error"]["code"], "INVALID_ARGUMENT");
}

/// Determinism check - byte identical output for same input
#[test]
fn test_determinism_byte_identical_output() {
    let content = r#"
f1: >> eagle
f2: >> sparrow
f3: >> robin
r1: eagle => flies
r2: sparrow => flies
r3: robin => flies
s1: penguin -> -flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    // Run 5 times and collect outputs
    let mut outputs: Vec<Vec<u8>> = Vec::new();
    for _ in 0..5 {
        let output = spindle_cmd()
            .arg("reason")
            .arg(&path)
            .arg("--json")
            .output()
            .expect("Failed to execute command");
        outputs.push(output.stdout);
    }

    // All outputs should be byte-identical
    let first = &outputs[0];
    for (i, output) in outputs.iter().enumerate().skip(1) {
        assert_eq!(
            output, first,
            "Output {} differs from first output - determinism failed",
            i
        );
    }
}

#[test]
fn test_query_at_populates_evaluated_at() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("query")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .arg("--at")
        .arg("2024-06-15T12:00:00Z")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(&json, "query", "test_query_at_populates_evaluated_at");

    // evaluated_at should be non-null and contain the provided timestamp
    assert!(
        json["evaluated_at"].is_string(),
        "evaluated_at should be a string when --at is provided"
    );
    let evaluated_at = json["evaluated_at"].as_str().unwrap();
    assert!(
        evaluated_at.contains("2024-06-15"),
        "evaluated_at should contain the provided date: {}",
        evaluated_at
    );
}

#[test]
fn test_query_refuted_status() {
    let content = r#"
f1: >> penguin
r1: penguin => -flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("query")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(&json, "query", "test_query_refuted_status");

    assert_eq!(
        json["status"], "refuted",
        "query should return 'refuted' when complement is provable"
    );
    assert!(
        json["conclusion_type"].is_null(),
        "refuted literals should have null conclusion_type"
    );
}

#[test]
fn test_exit_code_2_for_invalid_file_path() {
    let output = spindle_cmd()
        .arg("reason")
        .arg("/nonexistent/path/that/does/not/exist.dfl")
        .output()
        .expect("Failed to execute command");

    assert_eq!(
        output.status.code(),
        Some(2),
        "Invalid file path should return exit code 2"
    );
}

// =============================================================================
// --stdin tests
// =============================================================================

#[test]
fn test_stdin_basic_reason() {
    let output = spindle_cmd()
        .arg("reason")
        .arg("--stdin")
        .arg("--json")
        .write_stdin("f1: >> bird\nr1: bird => flies\n")
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "reason --stdin should succeed: stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_against_schema(&json, "reason", "test_stdin_basic_reason");
    assert_eq!(json["schema_version"], "spindle.reason.v1");
    assert!(json["conclusions"].as_array().unwrap().len() > 0);
}

#[test]
fn test_stdin_and_file_reason_fails_validation() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle_cmd()
        .arg("reason")
        .arg(&path)
        .arg("--stdin")
        .arg("--json")
        .write_stdin("f1: >> bird\nr1: bird => flies\n")
        .output()
        .expect("Failed to execute command");

    assert_eq!(
        output.status.code(),
        Some(2),
        "reason with both file and --stdin should fail with exit code 2"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("CONFLICTING_INPUT_SOURCES") || stdout.contains("cannot specify both"),
        "Should report source conflict in JSON, got stdout: {}",
        stdout
    );

    let json: Value = serde_json::from_slice(&output.stdout)
        .expect("Error output should be valid JSON when --json is used");
    validate_error_envelope(&json, "test_stdin_and_file_reason_fails_validation");
    assert_eq!(json["error"]["code"], "CONFLICTING_INPUT_SOURCES");
}

#[test]
fn test_stdin_query_without_placeholder() {
    let output = spindle_cmd()
        .arg("query")
        .arg("flies")
        .arg("--stdin")
        .arg("--json")
        .write_stdin("f1: >> bird\nr1: bird => flies\n")
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "query --stdin should succeed without file placeholder: stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");
    validate_against_schema(&json, "query", "test_stdin_query_without_placeholder");
    assert_eq!(json["status"], "provable");
}

#[test]
fn test_global_stdin_before_reason_subcommand_works() {
    let output = spindle_cmd()
        .arg("--stdin")
        .arg("reason")
        .arg("--json")
        .write_stdin("f1: >> bird\nr1: bird => flies\n")
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "global --stdin before reason should succeed: stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");
    validate_against_schema(
        &json,
        "reason",
        "test_global_stdin_before_reason_subcommand_works",
    );
}

#[test]
fn test_stdin_no_file_no_stdin_fails() {
    let output = spindle_cmd()
        .arg("reason")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    assert!(
        !output.status.success(),
        "reason with neither file nor --stdin should fail"
    );
}

#[test]
fn test_capabilities_stdin_true() {
    let output = spindle_cmd()
        .arg("capabilities")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    assert_eq!(
        json["features"]["stdin"], true,
        "capabilities should report stdin as true"
    );
}
