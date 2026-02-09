//! Contract Matrix Tests - Table-driven comprehensive validation
//!
//! Data-driven contract harness that validates real JSON Schema semantics
//! for every schema-bearing success response and every JSON error envelope.

use serde_json::Value;
use tempfile::NamedTempFile;

mod common;
use common::{
    run_matrix_case, run_with_stdin, validate_error_envelope, ExpectedOutput, MatrixCase,
};

/// Single source of truth for all matrix test cases
fn get_matrix_cases() -> Vec<MatrixCase> {
    vec![
        // ============================================================================
        // Success cases (exit 0, strict schema-validated)
        // ============================================================================

        // reason --json --stdin (basic success)
        MatrixCase {
            name: "reason --json success",
            args: &["reason", "--json", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "reason",
                require_diagnostics: true,
                expected_schema_version: Some("spindle.reason.v1"),
                custom_check: None,
            },
        },
        // query <unknown> --json --stdin (status = unknown)
        MatrixCase {
            name: "query unknown literal",
            args: &["query", "(unknown_literal)", "--json", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "query",
                require_diagnostics: true,
                expected_schema_version: Some("spindle.query.v1"),
                custom_check: Some(|json| {
                    assert_eq!(
                        json["status"], "unknown",
                        "Unknown literal should have status=unknown"
                    );
                    assert!(
                        json["conclusion_type"].is_null(),
                        "Unknown literal should have null conclusion_type"
                    );
                }),
            },
        },
        // requires <unsatisfied> --json --stdin (satisfied=false, solutions non-empty)
        MatrixCase {
            name: "requires unsatisfied",
            args: &["requires", "(missing)", "--json", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "requires",
                require_diagnostics: true,
                expected_schema_version: Some("spindle.requires.v1"),
                custom_check: Some(|json| {
                    assert_eq!(
                        json["satisfied"], false,
                        "Unsatisfied goal should have satisfied=false"
                    );
                    let solutions = json["solutions"]
                        .as_array()
                        .expect("solutions should be array");
                    assert!(
                        !solutions.is_empty(),
                        "Unsatisfied goal should have non-empty solutions"
                    );
                }),
            },
        },
        // explain <unprovable> --json --stdin (proof_tree=null, diagnostics warning)
        MatrixCase {
            name: "explain unprovable",
            args: &["explain", "(unprovable)", "--json", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "explain",
                require_diagnostics: true,
                expected_schema_version: Some("spindle.explain.v1"),
                custom_check: Some(|json| {
                    assert_eq!(
                        json["status"], "unknown",
                        "Unprovable literal should have status=unknown"
                    );
                    assert!(
                        json["proof_tree"].is_null(),
                        "Unprovable literal should have null proof_tree"
                    );
                    let diagnostics = json["diagnostics"]
                        .as_array()
                        .expect("diagnostics should be array");
                    assert!(
                        !diagnostics.is_empty(),
                        "Unprovable literal should have warnings in diagnostics"
                    );
                }),
            },
        },
        // why-not <literal> --json --stdin (blocked_by array present)
        MatrixCase {
            name: "why-not literal",
            args: &["why-not", "(swims)", "--json", "--stdin"],
            stdin: Some("(given bird)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "why_not",
                require_diagnostics: true,
                expected_schema_version: Some("spindle.why_not.v1"),
                custom_check: Some(|json| {
                    assert!(
                        json["blocked_by"].is_array(),
                        "why-not should have blocked_by array"
                    );
                }),
            },
        },
        // capabilities --json (no diagnostics required)
        MatrixCase {
            name: "capabilities --json",
            args: &["capabilities", "--json"],
            stdin: None,
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "capabilities",
                require_diagnostics: false, // capabilities intentionally has no diagnostics
                expected_schema_version: Some("spindle.capabilities.v1"),
                custom_check: Some(|json| {
                    assert!(
                        json["commands"].is_array(),
                        "capabilities should have commands array"
                    );
                    assert!(
                        json["features"].is_object(),
                        "capabilities should have features object"
                    );
                    assert!(
                        json["schemas"].is_object(),
                        "capabilities should have schemas object"
                    );
                }),
            },
        },
        // ============================================================================
        // Error cases (non-zero, JSON envelope invariants)
        // ============================================================================

        // reason --json --stdin with invalid input (parse error, exit 2)
        // Schema commands should have schema_version even on errors
        MatrixCase {
            name: "reason parse error",
            args: &["reason", "--json", "--stdin"],
            stdin: Some("invalid syntax here!!!"),
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                schema: None,                // Don't validate against schema for parse errors
                expect_schema_version: true, // Schema command errors should have schema_version
                expected_error_code: None,
                custom_check: Some(|json| {
                    assert!(
                        json["error"]["code"].as_str().unwrap().contains("PARSE"),
                        "Parse error should have PARSE in error code"
                    );
                }),
            },
        },
        // reason --json with missing source (exit 2)
        MatrixCase {
            name: "reason missing source",
            args: &["reason", "--json"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                schema: None,
                expect_schema_version: true, // Schema command
                expected_error_code: Some("MISSING_INPUT_SOURCE"),
                custom_check: None,
            },
        },
        // reason --json --stdin <file> conflicting sources (exit 2)
        // Note: This test requires a temp file, handled separately

        // requires ... --json --max 0 --stdin (exit 2, error.details has argument metadata)
        MatrixCase {
            name: "requires max 0",
            args: &["requires", "(given a)", "--json", "--max", "0", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                schema: None,
                expect_schema_version: true, // Schema command
                expected_error_code: Some("INVALID_ARGUMENT"),
                custom_check: Some(|json| {
                    let details = &json["error"]["details"];
                    assert_eq!(details["argument"], "--max");
                    assert_eq!(details["provided"], 0);
                    assert_eq!(details["minimum"], 1);
                }),
            },
        },
        // reason --json --at invalid-time --stdin (exit 2)
        MatrixCase {
            name: "reason invalid time format",
            args: &["reason", "--json", "--at", "invalid-time", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                schema: None,
                expect_schema_version: true, // Schema command
                expected_error_code: Some("INVALID_TIME_FORMAT"),
                custom_check: None,
            },
        },
        // ============================================================================
        // Non-schema command error cases (validate, stats)
        // ============================================================================

        // Non-schema commands should not have schema_version even with --json
        MatrixCase {
            name: "validate --json with missing source",
            args: &["--json", "validate"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                schema: None,
                expect_schema_version: false, // Non-schema command
                expected_error_code: Some("MISSING_INPUT_SOURCE"),
                custom_check: None,
            },
        },
        MatrixCase {
            name: "stats --json with missing source",
            args: &["--json", "stats"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                schema: None,
                expect_schema_version: false, // Non-schema command
                expected_error_code: Some("MISSING_INPUT_SOURCE"),
                custom_check: None,
            },
        },
        // ============================================================================
        // Text output cases
        // ============================================================================
        MatrixCase {
            name: "capabilities text",
            args: &["capabilities"],
            stdin: None,
            expected_exit: 0,
            expected_output: ExpectedOutput::Text {
                contains: &["Commands:", "Features:", "Schema versions:"],
            },
        },
    ]
}

/// Test all matrix cases
#[test]
fn test_matrix_all_cases() {
    let cases = get_matrix_cases();

    for case in &cases {
        run_matrix_case(case);
    }
}

/// Test conflicting input sources with reason command
#[test]
fn test_matrix_reason_conflicting_sources() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    std::fs::write(&temp_file, "(given a)").expect("Failed to write temp file");

    let file_path = temp_file.path().to_str().unwrap();
    let args: Vec<String> = vec![
        "reason".to_string(),
        "--json".to_string(),
        "--stdin".to_string(),
        file_path.to_string(),
    ];
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let (exit_code, stdout, _) = run_with_stdin(&args_ref, Some("(given a)"));

    assert_eq!(
        exit_code, 2,
        "reason --json conflicting sources: Expected exit code 2"
    );

    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["error"]["code"], "CONFLICTING_INPUT_SOURCES");
    validate_error_envelope(&json, "reason --json conflicting sources");
}

/// Test conflicting input sources with validate command
#[test]
fn test_matrix_validate_conflicting_sources() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    std::fs::write(&temp_file, "(given a)").expect("Failed to write temp file");

    let file_path = temp_file.path().to_str().unwrap();
    let args: Vec<String> = vec![
        "--json".to_string(), // Global --json flag
        "validate".to_string(),
        file_path.to_string(),
        "--stdin".to_string(),
    ];
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let (exit_code, stdout, _) = run_with_stdin(&args_ref, Some("(given b)"));

    assert_eq!(
        exit_code, 2,
        "validate conflicting sources: Expected exit code 2"
    );

    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["error"]["code"], "CONFLICTING_INPUT_SOURCES");
    validate_error_envelope(&json, "validate conflicting sources");

    // Non-schema command errors should not have schema_version
    assert!(
        json.get("schema_version").is_none(),
        "Non-schema command error should not have schema_version"
    );
}

/// Test conflicting input sources with stats command
#[test]
fn test_matrix_stats_conflicting_sources() {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    std::fs::write(&temp_file, "(given a)").expect("Failed to write temp file");

    let file_path = temp_file.path().to_str().unwrap();
    let args: Vec<String> = vec![
        "--json".to_string(), // Global --json flag
        "stats".to_string(),
        file_path.to_string(),
        "--stdin".to_string(),
    ];
    let args_ref: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let (exit_code, stdout, _) = run_with_stdin(&args_ref, Some("(given b)"));

    assert_eq!(
        exit_code, 2,
        "stats conflicting sources: Expected exit code 2"
    );

    let json: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["error"]["code"], "CONFLICTING_INPUT_SOURCES");
    validate_error_envelope(&json, "stats conflicting sources");

    // Non-schema command errors should not have schema_version
    assert!(
        json.get("schema_version").is_none(),
        "Non-schema command error should not have schema_version"
    );
}

/// Test determinism - same input should produce same output
#[test]
fn test_matrix_determinism() {
    use common::run_with_stdin;

    let args = vec!["reason", "--json", "--stdin"];
    let stdin = Some("(given a)\n(given b)");

    let (_, stdout1, _) = run_with_stdin(&args, stdin);
    let (_, stdout2, _) = run_with_stdin(&args, stdin);

    assert_eq!(
        stdout1, stdout2,
        "Output should be deterministic (same input produces same output)"
    );
}

/// Verify that we're using the non-deprecated binary resolution
#[test]
fn test_matrix_uses_cargo_bin_cmd() {
    use common::spindle_cmd;

    let mut cmd = spindle_cmd();
    cmd.arg("--version");

    let output = cmd.output().expect("Should find binary via cargo_bin_cmd");
    assert!(output.status.success());

    let version = String::from_utf8_lossy(&output.stdout);
    assert!(version.contains("spindle"));
}
