//! Integration tests for the spindle CLI

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
use predicates::prelude::*;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

/// Helper to create a temp directory with a theory file
fn setup_theory_file(content: &str, extension: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join(format!("theory.{extension}"));
    fs::write(&file_path, content).unwrap();
    (dir, file_path)
}

/// Get the spindle command
fn spindle() -> Command {
    cargo_bin_cmd!("spindle")
}

// ============================================================================
// No arguments / Help
// ============================================================================

#[test]
fn test_no_args_shows_usage() {
    spindle()
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"))
        .stderr(predicate::str::contains("spindle"));
}

#[test]
fn test_help_flag() {
    spindle()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("spindle"));
}

#[test]
fn test_version_flag() {
    spindle()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("spindle"));
}

// ============================================================================
// Reason command
// ============================================================================

#[test]
fn test_reason_spl_file_from_text_fixture() {
    let content = r#"
; Facts
(given bird)

; Defeasible rules
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Conclusions:"))
        .stdout(predicate::str::contains("+d flies"));
}

#[test]
fn test_reason_spl_file() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Conclusions:"))
        .stdout(predicate::str::contains("flies"));
}

#[test]
fn test_reason_with_positive_flag() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .arg("--positive")
        .assert()
        .success()
        .stdout(predicate::str::contains("Conclusions:"));
}

#[test]
fn test_reason_direct_file_arg() {
    // Direct invocation without subcommand is rejected.
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg(&path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:"))
        .stderr(predicate::str::contains("<COMMAND>"));
}

#[test]
fn test_reason_with_strict_rules() {
    let content = r#"
(given human)
(always s1 human mortal)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("+D mortal"));
}

#[test]
fn test_reason_with_superiority() {
    let content = r#"
(given bird)
(given penguin)
(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("+d ~flies"));
}

// ============================================================================
// Validate command
// ============================================================================

#[test]
fn test_validate_rejects_dfl_file() {
    let content = "f1: >> bird\nr1: bird => flies\n";
    let (_dir, path) = setup_theory_file(content, "dfl");

    spindle()
        .arg("validate")
        .arg(&path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Validation Error"))
        .stderr(predicate::str::contains(
            "DFL format is no longer supported",
        ));
}

#[test]
fn test_validate_valid_spl() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("validate")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Valid theory file"));
}

#[test]
fn test_validate_invalid_spl() {
    let content = r#"
this is not valid syntax !!!
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("validate")
        .arg(&path)
        .assert()
        .failure()
        .stderr(predicate::str::contains("parse error"));
}

#[test]
fn test_validate_nonexistent_file() {
    spindle()
        .arg("validate")
        .arg("/nonexistent/path/to/file.spl")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error reading file"));
}

#[test]
fn test_validate_json_success_from_stdin() {
    let output = spindle()
        .arg("--json")
        .arg("validate")
        .arg("--stdin")
        .write_stdin("(given a)\n")
        .output()
        .expect("Failed to execute validate --json --stdin");

    assert!(output.status.success(), "validate --json should succeed");
    let json: Value =
        serde_json::from_slice(&output.stdout).expect("validate --json should emit valid JSON");
    assert_eq!(json["valid"], true);
    assert!(
        json["diagnostics"].is_array(),
        "validate --json should include diagnostics array"
    );
    assert!(
        json.get("schema_version").is_none(),
        "validate --json should not include schema_version"
    );
}

// ============================================================================
// Stats command
// ============================================================================

#[test]
fn test_stats_command() {
    let content = r#"
(given bird)
(given penguin)
(normally r1 bird flies)
(normally r2 penguin (not flies))
(always s1 human mortal)
(prefer r2 r1)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("stats")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Theory Statistics:"))
        .stdout(predicate::str::contains("Total rules:"))
        .stdout(predicate::str::contains("Facts:"))
        .stdout(predicate::str::contains("Strict:"))
        .stdout(predicate::str::contains("Defeasible:"))
        .stdout(predicate::str::contains("Superiorities:"));
}

#[test]
fn test_stats_json_success_from_stdin() {
    let output = spindle()
        .arg("--json")
        .arg("stats")
        .arg("--stdin")
        .write_stdin("(given bird)\n(normally r1 bird flies)\n")
        .output()
        .expect("Failed to execute stats --json --stdin");

    assert!(output.status.success(), "stats --json should succeed");
    let json: Value =
        serde_json::from_slice(&output.stdout).expect("stats --json should emit valid JSON");
    assert!(
        json["stats"].is_object(),
        "stats --json should include stats object"
    );
    assert!(
        json["diagnostics"].is_array(),
        "stats --json should include diagnostics array"
    );
    assert!(
        json.get("schema_version").is_none(),
        "stats --json should not include schema_version"
    );
}

// ============================================================================
// Query command
// ============================================================================

#[test]
fn test_query_provable() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("query")
        .arg("flies")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("flies"));
}

#[test]
fn test_query_unknown() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("query")
        .arg("swims")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Unknown"));
}

#[test]
fn test_query_with_json() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    let output = spindle()
        .arg("query")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute query --json");
    assert!(output.status.success(), "query --json should succeed");
    let json: Value =
        serde_json::from_slice(&output.stdout).expect("query --json should emit valid JSON");
    assert_eq!(json["status"], "provable");
}

#[test]
fn test_query_negated_literal() {
    let content = r#"
(given bird)
(given penguin)
(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("query")
        .arg("~flies")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("~flies"));
}

// ============================================================================
// Explain command
// ============================================================================

#[test]
fn test_explain_provable() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("explain")
        .arg("flies")
        .arg(&path)
        .assert()
        .success();
}

#[test]
fn test_explain_not_provable() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    // Per contract §8.2: explain with no proof tree returns exit code 0
    spindle()
        .arg("explain")
        .arg("swims")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("not provable"));
}

#[test]
fn test_explain_with_json() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("explain")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .assert()
        .success();
}

#[test]
fn test_explain_not_provable_json() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    // Per contract §8.2: explain with no proof tree returns exit code 0
    let output = spindle()
        .arg("explain")
        .arg("swims")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute explain --json");
    assert!(output.status.success(), "explain --json should succeed");
    let json: Value =
        serde_json::from_slice(&output.stdout).expect("explain --json should emit valid JSON");
    assert_eq!(json["status"], "unknown");
    assert!(json["proof_tree"].is_null());
}

// ============================================================================
// WhyNot command
// ============================================================================

#[test]
fn test_why_not() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("why-not")
        .arg("swims")
        .arg(&path)
        .assert()
        .success();
}

#[test]
fn test_why_not_json() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    let output = spindle()
        .arg("why-not")
        .arg("swims")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute why-not --json");
    assert!(output.status.success(), "why-not --json should succeed");
    let json: Value =
        serde_json::from_slice(&output.stdout).expect("why-not --json should emit valid JSON");
    assert_eq!(json["status"], "unknown");
    assert!(json["blocked_by"].is_array());
}

// ============================================================================
// Requires command
// ============================================================================

#[test]
fn test_requires() {
    let content = r#"
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("requires")
        .arg("flies")
        .arg(&path)
        .assert()
        .success();
}

#[test]
fn test_requires_with_max() {
    let content = r#"
(normally r1 bird flies)
(normally r2 plane flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("requires")
        .arg("flies")
        .arg(&path)
        .arg("--max")
        .arg("5")
        .assert()
        .success();
}

#[test]
fn test_requires_with_max_respected_in_text_output() {
    let content = r#"
(normally r1 bird flies)
(normally r2 plane flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("requires")
        .arg("flies")
        .arg(&path)
        .arg("--max")
        .arg("1")
        .assert()
        .success()
        .stdout(predicate::str::contains("  1."))
        .stdout(predicate::str::contains("\n  2.").not());
}

#[test]
fn test_requires_json() {
    let content = r#"
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    let output = spindle()
        .arg("requires")
        .arg("flies")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute requires --json");
    assert!(output.status.success(), "requires --json should succeed");
    let json: Value =
        serde_json::from_slice(&output.stdout).expect("requires --json should emit valid JSON");
    assert_eq!(json["schema_version"], "spindle.requires.v2");
    assert_eq!(json["satisfied"], false);
    assert_eq!(json["verification_mode"], "verified");
    assert!(
        json["verification"]["raw_examined"].is_number(),
        "requires --json should include verification counters"
    );
    assert!(
        json["solutions"].as_array().is_some_and(|s| !s.is_empty()),
        "requires unsatisfied should produce solutions"
    );
}

// ============================================================================
// Capabilities command
// ============================================================================

#[test]
fn test_capabilities_command() {
    spindle()
        .arg("capabilities")
        .assert()
        .success()
        .stdout(predicate::str::contains("Spindle Capabilities:"))
        .stdout(predicate::str::contains("Commands:"))
        .stdout(predicate::str::contains("Features:"));
}

#[test]
fn test_capabilities_json() {
    let output = spindle()
        .arg("capabilities")
        .arg("--json")
        .output()
        .expect("Failed to execute capabilities --json");
    assert!(
        output.status.success(),
        "capabilities --json should succeed"
    );
    let json: Value =
        serde_json::from_slice(&output.stdout).expect("capabilities --json should emit valid JSON");
    assert_eq!(json["features"]["stdin"], true);
    assert_eq!(json["schemas"]["requires"], "spindle.requires.v2");
}

// ============================================================================
// Error handling
// ============================================================================

#[test]
fn test_invalid_subcommand() {
    spindle().arg("invalid-command").assert().failure();
}

#[test]
fn test_reason_missing_file_arg() {
    spindle().arg("reason").assert().failure();
}

#[test]
fn test_query_missing_literal_arg() {
    spindle().arg("query").assert().failure();
}

#[test]
fn test_json_flag_parse_error_emits_json_envelope() {
    let output = spindle()
        .arg("--json")
        .output()
        .expect("Failed to execute --json parse error case");

    assert!(
        !output.status.success(),
        "--json with no subcommand should fail"
    );
    let json: Value = serde_json::from_slice(&output.stdout)
        .expect("parse failure with --json should emit JSON envelope");
    assert_eq!(json["error"]["code"], "CLI_PARSE_ERROR");
    assert!(json["diagnostics"].is_array());
}

#[test]
fn test_reason_rejects_dfl_file() {
    let content = "f1: >> bird\nr1: bird => flies\n";
    let (_dir, path) = setup_theory_file(content, "dfl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "DFL format is no longer supported",
        ));
}

#[test]
fn test_reason_rejects_dfl_stdin_json() {
    let output = spindle()
        .arg("--json")
        .arg("reason")
        .arg("--stdin")
        .write_stdin("f1: >> bird\nr1: bird => flies\n")
        .output()
        .expect("Failed to execute reason --json --stdin");

    assert!(!output.status.success(), "DFL stdin should be rejected");
    let json: Value = serde_json::from_slice(&output.stdout)
        .expect("reason --json with DFL stdin should emit JSON envelope");
    assert_eq!(json["error"]["code"], "UNSUPPORTED_INPUT_FORMAT");
    assert_eq!(json["error"]["details"]["input_format"], "dfl");
}

// ============================================================================
// SPL format detection
// ============================================================================

#[test]
fn test_spl_detection_by_extension() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Conclusions:"));
}

#[test]
fn test_spl_detection_by_lang_line() {
    let content = r#"#lang spindle
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "txt");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Conclusions:"));
}

#[test]
fn test_spl_detection_by_paren() {
    let content = r#"(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "txt");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Conclusions:"));
}

#[test]
fn test_spl_detection_by_comment() {
    let content = r#"; This is a comment
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "txt");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Conclusions:"));
}

// ============================================================================
// Complex theories
// ============================================================================

#[test]
fn test_reason_complex_theory() {
    let content = r#"
(given bird)
(given penguin)
(given opus)
(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("+d ~flies"));
}

#[test]
fn test_reason_defeaters() {
    let content = r#"
(given bird)
(given wounded)
(normally r1 bird flies)
(except d1 wounded (not flies))
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("-d flies"));
}

// ============================================================================
// SPL keyword tests
// ============================================================================

#[test]
fn test_spl_always_rule() {
    let content = r#"
(given human)
(always s1 human mortal)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("+D mortal"));
}

#[test]
fn test_spl_except_rule() {
    let content = r#"
(given bird)
(given wounded)
(normally r1 bird flies)
(except d1 wounded (not flies))
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    spindle()
        .arg("reason")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("-d flies"));
}

#[test]
fn test_reason_json_schema() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    let output = spindle()
        .arg("reason")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute reason --json");
    assert!(output.status.success(), "reason --json should succeed");
    let json: Value =
        serde_json::from_slice(&output.stdout).expect("reason --json should emit valid JSON");
    assert!(
        json["conclusions"]
            .as_array()
            .is_some_and(|c| !c.is_empty()),
        "reason --json should include at least one conclusion for this theory"
    );
}

#[test]
fn test_direct_json_flag() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;
    let (_dir, path) = setup_theory_file(content, "spl");

    let output = spindle()
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute direct --json");
    assert!(!output.status.success(), "direct file --json should fail");
    let json: Value = serde_json::from_slice(&output.stdout)
        .expect("direct parse failure with --json should emit JSON envelope");
    assert_eq!(json["error"]["code"], "CLI_PARSE_ERROR");
    assert!(json["diagnostics"].is_array());
}
