//! Supplemental contract tests not covered by the matrix harness.

use assert_cmd::cargo::cargo_bin_cmd;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_theory_file(content: &str, extension: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join(format!("theory.{extension}"));
    fs::write(&file_path, content).unwrap();
    (dir, file_path)
}

#[test]
fn test_exit_code_2_for_invalid_file_path() {
    let output = cargo_bin_cmd!("spindle")
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

#[test]
fn test_query_unknown_exit_code_0_without_json() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = cargo_bin_cmd!("spindle")
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
fn test_requires_unsatisfied_exit_code_0_without_json() {
    let content = r#"
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = cargo_bin_cmd!("spindle")
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
fn test_explain_not_provable_exit_code_0_without_json() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = cargo_bin_cmd!("spindle")
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
