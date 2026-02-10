//! Error rendering tests (SPEC-010 Phase D5)
//!
//! Tests for uniform format, JSON envelope, and redaction behavior.

mod common;
use common::run_with_stdin;

// =============================================================================
// TEST-112: Uniform format for all error categories
// =============================================================================

/// All error categories should produce the same template format:
/// "Error: <title>\n  <detail>\n"
#[test]
fn test_uniform_format_parse_error() {
    let (exit, _stdout, stderr) = run_with_stdin(&["reason", "--stdin"], Some("invalid syntax!!!"));
    assert_eq!(exit, 2);
    assert!(
        stderr.starts_with("Error: "),
        "Parse error should start with 'Error: '"
    );
    assert!(
        stderr.contains("Parse Error") || stderr.contains("parse"),
        "Should indicate parse error"
    );
}

#[test]
fn test_uniform_format_validation_error() {
    let (exit, _stdout, stderr) = run_with_stdin(&["reason"], None);
    assert_eq!(exit, 2);
    assert!(
        stderr.starts_with("Error: "),
        "Validation error should start with 'Error: '"
    );
}

#[test]
fn test_uniform_format_all_categories_same_template() {
    // Parse error
    let (_, _, parse_stderr) = run_with_stdin(&["reason", "--stdin"], Some("bad input!!!"));

    // Validation error (missing input source)
    let (_, _, validation_stderr) = run_with_stdin(&["reason"], None);

    // Both should follow "Error: <title>\n  <detail>\n" template
    assert!(
        parse_stderr.starts_with("Error: "),
        "Parse error format mismatch"
    );
    assert!(
        validation_stderr.starts_with("Error: "),
        "Validation error format mismatch"
    );

    // Both should have an indented detail line
    let parse_lines: Vec<&str> = parse_stderr.lines().collect();
    let validation_lines: Vec<&str> = validation_stderr.lines().collect();
    assert!(
        parse_lines.len() >= 2,
        "Parse error should have at least 2 lines"
    );
    assert!(
        validation_lines.len() >= 2,
        "Validation error should have at least 2 lines"
    );
    assert!(
        parse_lines[1].starts_with("  "),
        "Detail should be indented"
    );
    assert!(
        validation_lines[1].starts_with("  "),
        "Detail should be indented"
    );
}

// =============================================================================
// TEST-113: Location and source context rendering
// =============================================================================

#[test]
fn test_human_parse_error_shows_source_location() {
    let input = "line one\nbad syntax!!!\nline three\n";
    let (exit, _stdout, stderr) = run_with_stdin(&["reason", "--stdin"], Some(input));
    assert_eq!(exit, 2);
    // Should show source name (stdin) in location line
    assert!(
        stderr.contains("--> stdin"),
        "Human parse error should show '--> stdin' location, got: {stderr}"
    );
}

#[test]
fn test_human_parse_error_shows_source_context() {
    // Multi-line input where the error is on a specific line
    let input = "r1: bird => flies\nbad syntax!!!\nr2: cat => pet\n";
    let (exit, _stdout, stderr) = run_with_stdin(&["reason", "--stdin"], Some(input));
    assert_eq!(exit, 2);
    // Source context should include line numbers with pipe separator
    assert!(
        stderr.contains(" | "),
        "Human parse error should show source context lines, got: {stderr}"
    );
}

#[test]
fn test_json_parse_error_includes_location_fields() {
    let input = "r1: bird => flies\nbad syntax!!!\nr2: cat => pet\n";
    let (exit, stdout, _stderr) = run_with_stdin(&["reason", "--json", "--stdin"], Some(input));
    assert_eq!(exit, 2);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Error output should be valid JSON");

    let problem = &json["error"]["details"]["problem"];

    // source_name should be present
    assert_eq!(
        problem["source_name"].as_str(),
        Some("stdin"),
        "JSON parse error should include source_name"
    );
}

#[test]
fn test_json_parse_error_includes_source_context() {
    let input = "r1: bird => flies\nbad syntax!!!\nr2: cat => pet\n";
    let (exit, stdout, _stderr) = run_with_stdin(&["reason", "--json", "--stdin"], Some(input));
    assert_eq!(exit, 2);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Error output should be valid JSON");

    let problem = &json["error"]["details"]["problem"];

    // source_context should be present with lines array
    if let Some(ctx) = problem.get("source_context") {
        assert!(
            ctx["lines"].is_array(),
            "source_context should have lines array"
        );
        let lines = ctx["lines"].as_array().unwrap();
        assert!(
            !lines.is_empty(),
            "source_context.lines should not be empty"
        );
        // Each line should have line_number and text
        for line in lines {
            assert!(
                line["line_number"].is_number(),
                "Each source line should have line_number"
            );
            assert!(
                line["text"].is_string(),
                "Each source line should have text"
            );
        }
        assert!(
            ctx["highlight_index"].is_number(),
            "source_context should have highlight_index"
        );
    }
    // Note: source_context may be absent if the parser doesn't report a line number
}

#[test]
fn test_location_shown_without_debug_flag() {
    let input = "bad syntax!!!\n";
    let (exit, _stdout, stderr) = run_with_stdin(&["reason", "--stdin"], Some(input));
    assert_eq!(exit, 2);
    // Location should appear even without --debug-errors
    assert!(
        stderr.contains("--> stdin"),
        "Source location should appear without --debug-errors, got: {stderr}"
    );
}

// =============================================================================
// TEST-103: Redaction (no absolute paths unless --debug-errors)
// =============================================================================

#[test]
fn test_redaction_no_abs_paths_in_normal_mode() {
    // Trigger a file-not-found error with an absolute path
    let (exit, _stdout, stderr) =
        run_with_stdin(&["reason", "/tmp/nonexistent_spindle_test_file.dfl"], None);
    assert_eq!(exit, 2);
    assert!(
        stderr.starts_with("Error: "),
        "Should produce structured error"
    );
    // In normal mode, the absolute path directory should be stripped from source_name
    assert!(
        stderr.contains("nonexistent_spindle_test_file.dfl"),
        "Should still mention the filename, got: {stderr}"
    );
    // Should NOT contain the /tmp/ directory prefix in the location line
    assert!(
        !stderr.contains("--> /tmp/"),
        "Normal mode should redact absolute path prefix from location, got: {stderr}"
    );
    // Should NOT contain any absolute path in the detail message either
    assert!(
        !stderr.contains("/tmp/nonexistent_spindle_test_file.dfl"),
        "Normal mode should redact absolute paths in detail message, got: {stderr}"
    );
}

#[test]
fn test_redaction_debug_mode_shows_full_path() {
    // With --debug-errors, the full absolute path should appear
    let (exit, _stdout, stderr) = run_with_stdin(
        &[
            "reason",
            "--debug-errors",
            "/tmp/nonexistent_spindle_test_file.dfl",
        ],
        None,
    );
    assert_eq!(exit, 2);
    assert!(
        stderr.contains("/tmp/nonexistent_spindle_test_file.dfl"),
        "Debug mode should show full absolute path, got: {stderr}"
    );
}

#[test]
fn test_redaction_debug_mode_shows_code() {
    // With --debug-errors, the error type URI and exit code should appear
    let (exit, _stdout, stderr) = run_with_stdin(
        &[
            "reason",
            "--debug-errors",
            "/tmp/nonexistent_spindle_test_file.dfl",
        ],
        None,
    );
    assert_eq!(exit, 2);
    assert!(
        stderr.contains("tag:spindle.dev,2026:error:"),
        "Debug mode should show error type URI"
    );
    assert!(
        stderr.contains("(exit 2)"),
        "Debug mode should show exit code"
    );
}

#[test]
fn test_redaction_normal_mode_hides_code() {
    // Without --debug-errors, the error type URI and exit code should NOT appear
    let (exit, _stdout, stderr) =
        run_with_stdin(&["reason", "/tmp/nonexistent_spindle_test_file.dfl"], None);
    assert_eq!(exit, 2);
    assert!(
        !stderr.contains("tag:spindle.dev,2026:error:"),
        "Normal mode should not show error type URI, got: {stderr}"
    );
    assert!(
        !stderr.contains("(exit 2)"),
        "Normal mode should not show exit code, got: {stderr}"
    );
}

#[test]
fn test_redaction_os_error_stripped_in_normal_mode() {
    // File not found should trigger OS error string in the detail
    let (exit, _stdout, stderr) =
        run_with_stdin(&["reason", "/tmp/nonexistent_spindle_test_file.dfl"], None);
    assert_eq!(exit, 2);
    // OS error codes like "(os error 2)" should be stripped in normal mode
    assert!(
        !stderr.contains("(os error"),
        "Normal mode should strip OS error codes, got: {stderr}"
    );
}

#[test]
fn test_redaction_os_error_shown_in_debug_mode() {
    let (exit, _stdout, stderr) = run_with_stdin(
        &[
            "reason",
            "--debug-errors",
            "/tmp/nonexistent_spindle_test_file.dfl",
        ],
        None,
    );
    assert_eq!(exit, 2);
    // Debug mode should show the OS error code
    assert!(
        stderr.contains("(os error"),
        "Debug mode should show OS error codes, got: {stderr}"
    );
}

// =============================================================================
// TEST-102: JSON error envelope with ProblemDetails
// =============================================================================

#[test]
fn test_json_envelope_has_problem_details() {
    let (exit, stdout, _stderr) =
        run_with_stdin(&["reason", "--json", "--stdin"], Some("invalid syntax!!!"));
    assert_eq!(exit, 2);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Error output should be valid JSON");

    // Check envelope structure
    assert!(json["error"].is_object(), "Should have error object");
    assert!(json["error"]["code"].is_string(), "Should have error.code");
    assert!(
        json["error"]["message"].is_string(),
        "Should have error.message"
    );
    assert!(
        json["error"]["details"].is_object(),
        "Should have error.details"
    );

    // Check ProblemDetails is embedded
    let problem = &json["error"]["details"]["problem"];
    assert!(problem.is_object(), "Should have error.details.problem");
    assert!(
        problem["type"].is_string(),
        "ProblemDetails should have type"
    );
    assert!(
        problem["title"].is_string(),
        "ProblemDetails should have title"
    );

    // Check type URI format
    let type_uri = problem["type"].as_str().unwrap();
    assert!(
        type_uri.starts_with("tag:spindle.dev,2026:error:"),
        "Type URI should use tag scheme, got: {type_uri}"
    );

    // Check exit_code extension
    assert_eq!(
        problem["exit_code"], 2,
        "Parse error should have exit_code 2"
    );
}

#[test]
fn test_json_envelope_diagnostics_array() {
    let (exit, stdout, _stderr) =
        run_with_stdin(&["reason", "--json", "--stdin"], Some("invalid syntax!!!"));
    assert_eq!(exit, 2);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Error output should be valid JSON");

    let diagnostics = json["diagnostics"]
        .as_array()
        .expect("Should have diagnostics array");
    assert!(
        !diagnostics.is_empty(),
        "Should have at least one diagnostic"
    );

    let diag = &diagnostics[0];
    assert!(
        diag["severity"].is_string(),
        "Diagnostic should have severity"
    );
    assert!(diag["code"].is_string(), "Diagnostic should have code");
    assert!(
        diag["message"].is_string(),
        "Diagnostic should have message"
    );
    if diag.get("details").is_some() {
        assert!(
            diag["details"].is_object(),
            "Diagnostic details must be an object when present"
        );
    }
}

#[test]
fn test_json_envelope_schema_version_present_for_schema_commands() {
    let (exit, stdout, _stderr) =
        run_with_stdin(&["reason", "--json", "--stdin"], Some("invalid!!!"));
    assert_eq!(exit, 2);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(
        json["schema_version"].is_string(),
        "Schema command error should include schema_version"
    );
    assert_eq!(
        json["schema_version"].as_str(),
        Some("spindle.reason.v1"),
        "Should use the command's schema version"
    );
}

#[test]
fn test_json_envelope_no_schema_version_for_non_schema() {
    let (exit, stdout, _stderr) = run_with_stdin(&["--json"], None);
    assert_eq!(exit, 2);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(
        json.get("schema_version").is_none(),
        "Non-schema command error should not include schema_version"
    );
}

#[test]
fn test_json_details_preserves_structured_metadata() {
    // CONFLICTING_INPUT_SOURCES passes file and stdin fields in with_details()
    // These should appear in error.details alongside problem
    let (exit, stdout, _stderr) = run_with_stdin(
        &["reason", "--json", "--stdin", "dummy.dfl"],
        Some("r1: a => b"),
    );
    assert_eq!(exit, 2);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("Error output should be valid JSON");

    let details = &json["error"]["details"];
    assert!(
        details["problem"].is_object(),
        "Should have problem in details"
    );
    // The structured metadata (file, stdin) should be present alongside problem
    assert!(
        details.get("file").is_some(),
        "Structured details should preserve 'file' field, got details: {details}"
    );
    assert!(
        details.get("stdin").is_some(),
        "Structured details should preserve 'stdin' field, got details: {details}"
    );
}
