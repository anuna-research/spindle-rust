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
        .success()
        .stdout(predicate::str::contains("Spindle"))
        .stdout(predicate::str::contains("Usage:"));
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
fn test_reason_dfl_file() {
    let content = r#"
# Facts
f1: >> bird

# Defeasible rules
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
fn test_reason_with_scalable_flag() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    spindle()
        .arg("reason")
        .arg(&path)
        .arg("--scalable")
        .assert()
        .success()
        .stdout(predicate::str::contains("Conclusions:"));
}

#[test]
fn test_reason_with_positive_flag() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
    // Test the direct file argument without subcommand
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    spindle()
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Conclusions:"));
}

#[test]
fn test_reason_with_strict_rules() {
    let content = r#"
f1: >> human
s1: human -> mortal
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
f2: >> penguin
r1: bird => flies
r2: penguin => -flies
r2 > r1
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
fn test_validate_valid_dfl() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    spindle()
        .arg("validate")
        .arg(&path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Valid theory file"));
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
fn test_validate_invalid_dfl() {
    let content = r#"
this is not valid dfl syntax !!!
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
        .arg("/nonexistent/path/to/file.dfl")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error reading file"));
}

// ============================================================================
// Stats command
// ============================================================================

#[test]
fn test_stats_command() {
    let content = r#"
f1: >> bird
f2: >> penguin
r1: bird => flies
r2: penguin => -flies
s1: human -> mortal
r2 > r1
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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

// ============================================================================
// Query command
// ============================================================================

#[test]
fn test_query_provable() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
f2: >> penguin
r1: bird => flies
r2: penguin => -flies
r2 > r1
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
r1: bird => flies
r2: plane => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
r1: bird => flies
r2: plane => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
    assert_eq!(json["satisfied"], false);
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
f1: >> bird
f2: >> penguin
f3: >> opus
r1: bird => flies
r2: penguin => -flies
r2 > r1
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
f2: >> wounded
r1: bird => flies
d1: wounded ~> -flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

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
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle()
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to execute direct --json");
    assert!(output.status.success(), "direct file --json should succeed");
    let _json: Value =
        serde_json::from_slice(&output.stdout).expect("direct --json should emit valid JSON");
}
