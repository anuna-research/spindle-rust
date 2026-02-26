//! Contract Matrix Tests - Table-driven comprehensive validation
//!
//! Data-driven contract harness that validates real JSON Schema semantics
//! for every schema-bearing success response and every JSON error envelope.

use tempfile::NamedTempFile;

mod common;
use common::{ExpectedOutput, MatrixCase, MatrixRuntime, run_matrix_case};

fn setup_reason_conflicting_sources_case() -> MatrixRuntime {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    std::fs::write(&temp_file, "(given a)").expect("Failed to write temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();

    MatrixRuntime {
        args: vec![
            "reason".to_string(),
            "--json".to_string(),
            "--stdin".to_string(),
            file_path,
        ],
        stdin: Some("(given a)".to_string()),
        _temp_files: vec![temp_file],
    }
}

fn setup_validate_conflicting_sources_case() -> MatrixRuntime {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    std::fs::write(&temp_file, "(given a)").expect("Failed to write temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();

    MatrixRuntime {
        args: vec![
            "--json".to_string(),
            "validate".to_string(),
            file_path,
            "--stdin".to_string(),
        ],
        stdin: Some("(given b)".to_string()),
        _temp_files: vec![temp_file],
    }
}

fn setup_stats_conflicting_sources_case() -> MatrixRuntime {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    std::fs::write(&temp_file, "(given a)").expect("Failed to write temp file");
    let file_path = temp_file.path().to_str().unwrap().to_string();

    MatrixRuntime {
        args: vec![
            "--json".to_string(),
            "stats".to_string(),
            file_path,
            "--stdin".to_string(),
        ],
        stdin: Some("(given b)".to_string()),
        _temp_files: vec![temp_file],
    }
}

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
                custom_check: None,
            },
            setup: None,
        },
        // query <unknown> --json --stdin (status = unknown)
        MatrixCase {
            name: "query unknown literal",
            args: &["query", "(unknown_literal)", "--json", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "query",
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
            setup: None,
        },
        // query refuted literal
        MatrixCase {
            name: "query refuted literal",
            args: &["query", "flies", "--json", "--stdin"],
            stdin: Some("(given penguin)\n(normally r1 penguin (not flies))\n"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "query",
                custom_check: Some(|json| {
                    assert_eq!(
                        json["status"], "refuted",
                        "Complement provable should produce status=refuted"
                    );
                    assert!(
                        json["conclusion_type"].is_null(),
                        "Refuted literal should have null conclusion_type"
                    );
                }),
            },
            setup: None,
        },
        // query with --at should populate evaluated_at
        MatrixCase {
            name: "query with --at",
            args: &[
                "query",
                "flies",
                "--json",
                "--stdin",
                "--at",
                "2024-06-15T12:00:00Z",
            ],
            stdin: Some("(given bird)\n(normally r1 bird flies)\n"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "query",
                custom_check: Some(|json| {
                    assert!(
                        json["evaluated_at"].is_string(),
                        "--at should populate evaluated_at"
                    );
                    let evaluated_at = json["evaluated_at"]
                        .as_str()
                        .expect("evaluated_at should be a string");
                    assert!(
                        evaluated_at.contains("2024-06-15"),
                        "evaluated_at should contain provided date"
                    );
                }),
            },
            setup: None,
        },
        // global --stdin before subcommand should be accepted
        MatrixCase {
            name: "global --stdin before reason",
            args: &["--stdin", "reason", "--json"],
            stdin: Some("(given bird)\n(normally r1 bird flies)\n"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "reason",
                custom_check: None,
            },
            setup: None,
        },
        // requires <unsatisfied> --json --stdin (satisfied=false, solutions non-empty)
        MatrixCase {
            name: "requires unsatisfied",
            args: &["requires", "(missing)", "--json", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "requires_v2",
                custom_check: Some(|json| {
                    assert_eq!(
                        json["satisfied"], false,
                        "Unsatisfied goal should have satisfied=false"
                    );
                    assert_eq!(
                        json["verification_mode"], "verified",
                        "Requires v2 must report verification_mode=verified"
                    );
                    assert!(
                        json["verification"]["raw_examined"].is_number(),
                        "Requires v2 must include verification counters"
                    );
                    assert!(
                        json["search_status"].is_string(),
                        "Requires v2 must include search_status"
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
            setup: None,
        },
        // explain <unprovable> --json --stdin (proof_tree=null, diagnostics warning)
        MatrixCase {
            name: "explain unprovable",
            args: &["explain", "(unprovable)", "--json", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "explain",
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
            setup: None,
        },
        // why-not <literal> --json --stdin (blocked_by array present)
        MatrixCase {
            name: "why-not literal",
            args: &["why-not", "(swims)", "--json", "--stdin"],
            stdin: Some("(given bird)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "why_not",
                custom_check: None,
            },
            setup: None,
        },
        // why-not for a refuted literal should report status=refuted
        MatrixCase {
            name: "why-not refuted literal",
            args: &["why-not", "flies", "--json", "--stdin"],
            stdin: Some("(given (not flies))\n"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "why_not",
                custom_check: Some(|json| {
                    assert_eq!(
                        json["status"], "refuted",
                        "why-not should return refuted status when complement is provable"
                    );
                }),
            },
            setup: None,
        },
        // capabilities --json (no diagnostics required)
        MatrixCase {
            name: "capabilities --json",
            args: &["capabilities", "--json"],
            stdin: None,
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccess {
                schema: "capabilities",
                custom_check: Some(|json| {
                    assert_eq!(json["features"]["stdin"], true);
                }),
            },
            setup: None,
        },
        // validate --json success (non-schema utility command)
        MatrixCase {
            name: "validate --json success",
            args: &["--json", "validate", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccessNoSchema {
                required_fields: &["valid", "diagnostics"],
                custom_check: Some(|json| {
                    assert_eq!(json["valid"], true);
                }),
            },
            setup: None,
        },
        // stats --json success (non-schema utility command)
        MatrixCase {
            name: "stats --json success",
            args: &["--json", "stats", "--stdin"],
            stdin: Some("(given bird)\n(normally r1 bird flies)\n"),
            expected_exit: 0,
            expected_output: ExpectedOutput::JsonSuccessNoSchema {
                required_fields: &["stats", "diagnostics"],
                custom_check: Some(|json| {
                    let stats = json["stats"]
                        .as_object()
                        .expect("stats should be an object");
                    for key in [
                        "total_rules",
                        "facts",
                        "strict",
                        "defeasible",
                        "defeaters",
                        "superiorities",
                    ] {
                        assert!(stats.contains_key(key), "stats missing key '{}'", key);
                        assert!(
                            stats[key].is_number(),
                            "stats key '{}' should have numeric value",
                            key
                        );
                    }
                }),
            },
            setup: None,
        },
        // ============================================================================
        // Error cases (non-zero, JSON envelope invariants)
        // ============================================================================

        // clap parse errors with --json should still produce JSON envelopes
        MatrixCase {
            name: "--json missing subcommand parse error",
            args: &["--json"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: false,
                expected_error_code: Some("CLI_PARSE_ERROR"),
                custom_check: Some(|json| {
                    // ProblemDetails should be present in the error envelope
                    let problem = &json["error"]["details"]["problem"];
                    assert!(
                        problem["type"].is_string(),
                        "error.details.problem.type should be present"
                    );
                    assert!(
                        problem["title"].is_string(),
                        "error.details.problem.title should be present"
                    );
                }),
            },
            setup: None,
        },
        MatrixCase {
            name: "--json query missing literal parse error",
            args: &["--json", "query"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: false,
                expected_error_code: Some("CLI_PARSE_ERROR"),
                custom_check: Some(|json| {
                    let problem = &json["error"]["details"]["problem"];
                    assert!(
                        problem["type"].is_string(),
                        "error.details.problem.type should be present"
                    );
                }),
            },
            setup: None,
        },
        MatrixCase {
            name: "--json unknown subcommand parse error",
            args: &["--json", "frobnicate"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: false,
                expected_error_code: Some("CLI_PARSE_ERROR"),
                custom_check: Some(|json| {
                    let problem = &json["error"]["details"]["problem"];
                    assert!(
                        problem["type"].is_string(),
                        "error.details.problem.type should be present"
                    );
                }),
            },
            setup: None,
        },
        // reason --json --stdin with invalid input (parse error, exit 2)
        // Schema commands should have schema_version even on errors
        MatrixCase {
            name: "reason parse error",
            args: &["reason", "--json", "--stdin"],
            stdin: Some("invalid syntax here!!!"),
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: true, // Schema command errors should have schema_version
                expected_error_code: None,
                custom_check: Some(|json| {
                    assert!(
                        json["error"]["code"].as_str().unwrap().contains("PARSE"),
                        "Parse error should have PARSE in error code"
                    );
                }),
            },
            setup: None,
        },
        // reason --json with missing source (exit 2)
        MatrixCase {
            name: "reason missing source",
            args: &["reason", "--json"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: true, // Schema command
                expected_error_code: Some("MISSING_INPUT_SOURCE"),
                custom_check: None,
            },
            setup: None,
        },
        // reason --json --stdin <file> conflicting sources (exit 2)
        MatrixCase {
            name: "reason conflicting sources",
            args: &[],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: true,
                expected_error_code: Some("CONFLICTING_INPUT_SOURCES"),
                custom_check: None,
            },
            setup: Some(setup_reason_conflicting_sources_case),
        },
        // requires ... --json --max 0 --stdin (exit 2, error.details has argument metadata)
        MatrixCase {
            name: "requires max 0",
            args: &["requires", "(given a)", "--json", "--max", "0", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: true, // Schema command
                expected_error_code: Some("INVALID_ARGUMENT"),
                custom_check: Some(|json| {
                    assert_eq!(
                        json["schema_version"], "spindle.requires.v2",
                        "Requires error envelope should use v2 schema version"
                    );
                    // ProblemDetails should contain the error details
                    let problem = &json["error"]["details"]["problem"];
                    assert_eq!(
                        problem["title"].as_str(),
                        Some("Validation Error"),
                        "Should be a validation error"
                    );
                    let detail = problem["detail"]
                        .as_str()
                        .expect("problem.detail should be present");
                    assert!(
                        detail.contains("--max"),
                        "Detail should mention --max argument"
                    );
                }),
            },
            setup: None,
        },
        // reason --json --at invalid-time --stdin (exit 2)
        MatrixCase {
            name: "reason invalid time format",
            args: &["reason", "--json", "--at", "invalid-time", "--stdin"],
            stdin: Some("(given a)"),
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: true, // Schema command
                expected_error_code: Some("INVALID_TIME_FORMAT"),
                custom_check: None,
            },
            setup: None,
        },
        // ============================================================================
        // Non-schema command error cases (validate, stats)
        // ============================================================================

        // validate conflicting sources (non-schema command)
        MatrixCase {
            name: "validate conflicting sources",
            args: &[],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: false,
                expected_error_code: Some("CONFLICTING_INPUT_SOURCES"),
                custom_check: None,
            },
            setup: Some(setup_validate_conflicting_sources_case),
        },
        // stats conflicting sources (non-schema command)
        MatrixCase {
            name: "stats conflicting sources",
            args: &[],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: false,
                expected_error_code: Some("CONFLICTING_INPUT_SOURCES"),
                custom_check: None,
            },
            setup: Some(setup_stats_conflicting_sources_case),
        },
        // Non-schema commands should not have schema_version even with --json
        MatrixCase {
            name: "validate --json with missing source",
            args: &["--json", "validate"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: false, // Non-schema command
                expected_error_code: Some("MISSING_INPUT_SOURCE"),
                custom_check: None,
            },
            setup: None,
        },
        MatrixCase {
            name: "stats --json with missing source",
            args: &["--json", "stats"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::JsonError {
                expect_schema_version: false, // Non-schema command
                expected_error_code: Some("MISSING_INPUT_SOURCE"),
                custom_check: None,
            },
            setup: None,
        },
        // ============================================================================
        // Exit-code only cases (non-JSON domain outcomes)
        // ============================================================================
        MatrixCase {
            name: "reason invalid file path exits 2",
            args: &["reason", "/nonexistent/path/that/does/not/exist.spl"],
            stdin: None,
            expected_exit: 2,
            expected_output: ExpectedOutput::ExitOnly {
                stdout_contains: None,
                stderr_contains: None,
            },
            setup: None,
        },
        MatrixCase {
            name: "query unknown exits 0 without json",
            args: &["query", "unknown_literal", "--stdin"],
            stdin: Some("(given bird)\n(normally r1 bird flies)\n"),
            expected_exit: 0,
            expected_output: ExpectedOutput::ExitOnly {
                stdout_contains: None,
                stderr_contains: None,
            },
            setup: None,
        },
        MatrixCase {
            name: "requires unsatisfied exits 0 without json",
            args: &["requires", "(missing)", "--stdin"],
            stdin: Some("(given a)\n"),
            expected_exit: 0,
            expected_output: ExpectedOutput::ExitOnly {
                stdout_contains: None,
                stderr_contains: None,
            },
            setup: None,
        },
        MatrixCase {
            name: "explain unprovable exits 0 without json",
            args: &["explain", "(missing)", "--stdin"],
            stdin: Some("(given a)\n"),
            expected_exit: 0,
            expected_output: ExpectedOutput::ExitOnly {
                stdout_contains: None,
                stderr_contains: None,
            },
            setup: None,
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
            setup: None,
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
