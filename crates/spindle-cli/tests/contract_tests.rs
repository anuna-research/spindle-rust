//! Contract validation tests
//!
//! These tests verify that spindle CLI outputs conform to the v1 contract.

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn spindle() -> Command {
    cargo_bin_cmd!("spindle")
}

fn setup_theory_file(content: &str, extension: &str) -> (TempDir, std::path::PathBuf) {
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

    let output = spindle()
        .arg("reason")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    // Verify schema version
    assert_eq!(json["schema_version"], "spindle.reason.v1");

    // Verify required fields
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

    // Test provable
    let output = spindle()
        .arg("query")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

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

    let output = spindle()
        .arg("query")
        .arg(&path)
        .arg("unknown_literal")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    assert_eq!(json["status"], "unknown");
    assert!(json["conclusion_type"].is_null());
}

#[test]
fn test_requires_output_validates_against_schema() {
    let content = r#"
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle()
        .arg("requires")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

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

    let output = spindle()
        .arg("requires")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    // When satisfied=true, solutions must be empty per contract
    assert_eq!(json["satisfied"], true);
    assert_eq!(json["solutions"].as_array().unwrap().len(), 0);
}

#[test]
fn test_requires_unsatisfied_has_non_empty_solutions() {
    let content = r#"
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle()
        .arg("requires")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    // When satisfied=false, solutions must be non-empty per contract
    assert_eq!(json["satisfied"], false);
    assert!(json["solutions"].as_array().unwrap().len() > 0);

    // Verify solutions have score field (not confidence)
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

    let output = spindle()
        .arg("explain")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

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

    let output = spindle()
        .arg("explain")
        .arg(&path)
        .arg("swims")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    // Not provable should have status "unknown" and proof_tree null
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

    let output = spindle()
        .arg("why-not")
        .arg(&path)
        .arg("swims")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    assert_eq!(json["schema_version"], "spindle.why_not.v1");
    assert!(json["literal_spl"].is_string());
    assert!(json["literal_struct"].is_object());
    assert!(json["blocked_by"].is_array());
    assert!(json["trust"].is_null());
    assert!(json["diagnostics"].is_array());
}

#[test]
fn test_capabilities_output_validates_against_schema() {
    let output = spindle()
        .arg("capabilities")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

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

    let output = spindle()
        .arg("query")
        .arg(&path)
        .arg("unknown_literal")
        .output()
        .expect("Failed to execute command");

    // Per contract §8.2: query returning unknown is exit code 0
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

    let output = spindle()
        .arg("requires")
        .arg(&path)
        .arg("flies")
        .output()
        .expect("Failed to execute command");

    // Per contract §8.2: requires returning unsatisfied is exit code 0
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

    let output = spindle()
        .arg("explain")
        .arg(&path)
        .arg("swims")
        .output()
        .expect("Failed to execute command");

    // Per contract §8.2: explain with no proof tree is exit code 0
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

    // Test all three status values
    let cases = vec![("flies", "provable"), ("unknown_literal", "unknown")];

    for (literal, expected_status) in cases {
        let output = spindle()
            .arg("query")
            .arg(&path)
            .arg(literal)
            .arg("--json")
            .output()
            .expect("Failed to execute command");

        let json: serde_json::Value =
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

    let output = spindle()
        .arg("query")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json_str = String::from_utf8(output.stdout).unwrap();

    // Ensure no legacy status strings appear
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

    // When literal is refuted (complement is provable), why-not should show "refuted"
    let output = spindle()
        .arg("why-not")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

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

    // --max 0 should be rejected as validation error (exit code 2)
    let output = spindle()
        .arg("requires")
        .arg(&path)
        .arg("flies")
        .arg("--max")
        .arg("0")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    // Should fail with exit code 2 (validation error)
    assert_eq!(
        output.status.code(),
        Some(2),
        "--max 0 should be rejected with exit code 2"
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    assert!(json["error"].is_object());
    assert_eq!(json["error"]["code"], "INVALID_ARGUMENT");
}

#[test]
fn test_capabilities_stdin_false() {
    let output = spindle()
        .arg("capabilities")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    // stdin feature should be false since --stdin is not implemented
    assert_eq!(
        json["features"]["stdin"], false,
        "capabilities should truthfully report stdin as false"
    );
}
