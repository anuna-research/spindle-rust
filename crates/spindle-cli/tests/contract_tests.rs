//! Contract validation tests
//!
//! These tests verify that spindle CLI outputs conform to the v1 contract.

use assert_cmd::cargo::cargo_bin_cmd;
use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn spindle() -> Command {
    cargo_bin_cmd!("spindle")
}

fn setup_theory_file(content: &str, extension: &str) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join(format!("theory.{extension}"));
    fs::write(&file_path, content).unwrap();
    (dir, file_path)
}

/// Build a comprehensive schema registry from all schema files
fn build_schema_registry() -> Value {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let schema_dir = manifest_dir.join("../../contracts/spindle/v1/schemas");
    let mut registry = serde_json::Map::new();

    // Load all schema files
    let schema_files = vec![
        ("common", "spindle.common.v1.schema.json"),
        ("reason", "spindle.reason.v1.schema.json"),
        ("query", "spindle.query.v1.schema.json"),
        ("requires", "spindle.requires.v1.schema.json"),
        ("explain", "spindle.explain.v1.schema.json"),
        ("why_not", "spindle.why_not.v1.schema.json"),
        ("capabilities", "spindle.capabilities.v1.schema.json"),
    ];

    for (name, filename) in schema_files {
        let path = schema_dir.join(filename);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read schema {}: {}", path.display(), e));
        let schema: Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse schema {}: {}", path.display(), e));
        registry.insert(name.to_string(), schema);
    }

    Value::Object(registry)
}

/// Flatten all schemas into a single schema with merged $defs
fn load_inlined_schema(schema_name: &str) -> Value {
    let registry = build_schema_registry();

    // Start with the requested schema
    let target = registry
        .get(schema_name)
        .cloned()
        .expect(&format!("Schema not found: {}", schema_name));

    // Create a merged $defs collection
    let mut all_defs = serde_json::Map::new();

    // Collect $defs from all schemas
    if let Some(schemas) = registry.as_object() {
        for (_, schema) in schemas {
            if let Some(defs) = schema.get("$defs").and_then(|d| d.as_object()) {
                for (key, value) in defs {
                    all_defs.insert(key.clone(), value.clone());
                }
            }
        }
    }

    // Build final schema with merged $defs
    let mut result = target.as_object().cloned().unwrap_or_default();
    if !all_defs.is_empty() {
        result.insert("$defs".to_string(), Value::Object(all_defs));
    }

    // Replace all remote $refs with local references
    let result = Value::Object(result);
    replace_remote_refs(result)
}

/// Replace remote schema references with local $ref references
fn replace_remote_refs(mut schema: Value) -> Value {
    match &mut schema {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if key == "$ref" {
                    if let Value::String(ref_str) = value {
                        // Replace remote refs like "https://.../spindle.X.v1.schema.json#/$defs/Y"
                        // with local refs like "#/$defs/Y"
                        if ref_str.contains("#/$defs/") {
                            let parts: Vec<&str> = ref_str.split("#/$defs/").collect();
                            if parts.len() == 2 {
                                *value = Value::String(format!("#/$defs/{}", parts[1]));
                            }
                        }
                    }
                } else {
                    *value = replace_remote_refs(value.clone());
                }
            }
        }
        Value::Array(arr) => {
            for i in 0..arr.len() {
                arr[i] = replace_remote_refs(arr[i].clone());
            }
        }
        _ => {}
    }
    schema
}

/// Validate JSON against a schema
fn validate_json(json: &Value, schema: &Value) -> Result<(), Vec<String>> {
    let compiled = jsonschema::JSONSchema::compile(schema)
        .map_err(|e| vec![format!("Failed to compile schema: {}", e)])?;

    let result = compiled.validate(json);
    match result {
        Ok(_) => Ok(()),
        Err(errors) => {
            let messages: Vec<String> = errors
                .map(|e| format!("{}: {}", e.instance_path, e))
                .collect();
            Err(messages)
        }
    }
}

/// Helper to validate command output against its schema
fn validate_command_output(json: &Value, schema_name: &str) {
    let schema = load_inlined_schema(schema_name);
    match validate_json(json, &schema) {
        Ok(_) => {}
        Err(errors) => {
            panic!(
                "Schema validation failed for {}:\n{}",
                schema_name,
                errors.join("\n")
            );
        }
    }
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

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "reason");

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

    let output = spindle()
        .arg("query")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "query");

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

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "query");

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

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "requires");

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

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "requires");

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

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "requires");

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

    let output = spindle()
        .arg("explain")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "explain");

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

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "explain");

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

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "why_not");

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

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "capabilities");

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
        let output = spindle()
            .arg("query")
            .arg(&path)
            .arg(literal)
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

    let output = spindle()
        .arg("query")
        .arg(&path)
        .arg("flies")
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

    let output = spindle()
        .arg("why-not")
        .arg(&path)
        .arg("flies")
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

    let output = spindle()
        .arg("requires")
        .arg(&path)
        .arg("flies")
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

    assert!(json["error"].is_object());
    assert_eq!(json["error"]["code"], "INVALID_ARGUMENT");
}

#[test]
fn test_determinism_byte_identical_output() {
    // Per SPINDLE-RUST-IMPLEMENTATION.md §6.1D: run command N times with identical input
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
        let output = spindle()
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
    // Test that --at flag populates evaluated_at field
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
        .arg("--at")
        .arg("2024-06-15T12:00:00Z")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "query");

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
    // Test that query returns "refuted" when complement is provable
    let content = r#"
f1: >> penguin
r1: penguin => -flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle()
        .arg("query")
        .arg(&path)
        .arg("flies")
        .arg("--json")
        .output()
        .expect("Failed to execute command");

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");

    validate_command_output(&json, "query");

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
    // Per SPINDLE-RUST-IMPLEMENTATION.md §6.1C: invalid file path should return exit code 2
    let output = spindle()
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
    // Test that --stdin works for reason command without a file argument
    let output = spindle()
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

    validate_command_output(&json, "reason");
    assert_eq!(json["schema_version"], "spindle.reason.v1");
    assert!(json["conclusions"].as_array().unwrap().len() > 0);
}

#[test]
fn test_stdin_and_file_reason_fails_validation() {
    // Per SPINDLE-CONTRACT.md §5.1: exactly one theory source per invocation.
    // If both are provided, command must fail with a validation error.
    let content = r#"
f1: >> bird
r1: bird => flies
"#;
    let (_dir, path) = setup_theory_file(content, "dfl");

    let output = spindle()
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

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot specify both file"),
        "Should report source conflict, got stderr: {}",
        stderr
    );
}

#[test]
fn test_stdin_query_with_placeholder() {
    // For two-positional subcommands, use "-" as file placeholder with --stdin
    let output = spindle()
        .arg("query")
        .arg("-")
        .arg("flies")
        .arg("--stdin")
        .arg("--json")
        .write_stdin("f1: >> bird\nr1: bird => flies\n")
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "query --stdin with '-' placeholder should succeed: stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("Failed to parse JSON output");
    validate_command_output(&json, "query");
    assert_eq!(json["status"], "provable");
}

#[test]
fn test_global_stdin_before_reason_subcommand_works() {
    // --stdin is global and should satisfy reason's missing file case.
    let output = spindle()
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
    validate_command_output(&json, "reason");
}

#[test]
fn test_stdin_no_file_no_stdin_fails() {
    // Per SPINDLE-CONTRACT.md §5.1: must specify either a file or --stdin
    let output = spindle()
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
    // Test that capabilities reports stdin: true after implementation
    let output = spindle()
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
