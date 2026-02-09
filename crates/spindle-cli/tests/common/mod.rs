//! Test helpers module for spindle-cli contract tests

use assert_cmd::Command;
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// Get a Command for the spindle binary using non-deprecated cargo_bin_cmd!
pub fn spindle_cmd() -> Command {
    cargo_bin_cmd!("spindle")
}

/// Run a command with optional stdin input
pub fn run_with_stdin(args: &[&str], stdin: Option<&str>) -> (i32, String, String) {
    let mut cmd = spindle_cmd();
    cmd.args(args);

    if let Some(input) = stdin {
        cmd.write_stdin(input);
    }

    let output = cmd.output().expect("Failed to execute command");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

// =============================================================================
// SCHEMA REGISTRY AND VALIDATION
// =============================================================================

lazy_static::lazy_static! {
    static ref SCHEMA_CACHE: Mutex<Option<HashMap<String, Value>>> = Mutex::new(None);
}

/// Build a comprehensive schema registry from all schema files
fn build_schema_registry() -> HashMap<String, Value> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let schema_dir = manifest_dir.join("../../contracts/spindle/v1/schemas");

    let mut registry = HashMap::new();

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
        if !path.exists() {
            panic!("Schema file not found: {:?}", path);
        }

        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("Failed to read schema {}: {}", path.display(), e));
        let schema: Value = serde_json::from_str(&content)
            .unwrap_or_else(|e| panic!("Failed to parse schema {}: {}", path.display(), e));
        registry.insert(name.to_string(), schema);
    }

    registry
}

/// Get or initialize the schema registry
fn get_schema_registry() -> HashMap<String, Value> {
    let mut cache = SCHEMA_CACHE.lock().unwrap();
    if cache.is_none() {
        *cache = Some(build_schema_registry());
    }
    cache.as_ref().unwrap().clone()
}

/// Load and inline a schema with merged $defs from all schemas
fn load_inlined_schema(schema_name: &str) -> Value {
    let registry = get_schema_registry();

    let target = registry
        .get(schema_name)
        .cloned()
        .unwrap_or_else(|| panic!("Schema not found: {}", schema_name));

    // Create a merged $defs collection
    let mut all_defs = serde_json::Map::new();

    // Collect $defs from all schemas
    for (_, schema) in &registry {
        if let Some(defs) = schema.get("$defs").and_then(|d| d.as_object()) {
            for (key, value) in defs {
                all_defs.insert(key.clone(), value.clone());
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

/// Validate JSON against a schema using strict JSON Schema validation
/// Fail fast if schema file is missing/unreadable
pub fn validate_against_schema(json: &Value, schema_name: &str, case_name: &str) {
    let schema = load_inlined_schema(schema_name);

    let compiled = match jsonschema::JSONSchema::compile(&schema) {
        Ok(c) => c,
        Err(e) => panic!(
            "{}: Failed to compile schema '{}': {}",
            case_name, schema_name, e
        ),
    };

    let result = compiled.validate(json);
    match result {
        Ok(_) => {}
        Err(errors) => {
            let messages: Vec<String> = errors
                .map(|e| format!("{}: {}", e.instance_path, e))
                .collect();
            panic!(
                "{}: Schema validation failed for schema '{}':\n{}",
                case_name,
                schema_name,
                messages.join("\n")
            );
        }
    }
}

/// Validate JSON error envelope invariants per contract §8.3
pub fn validate_error_envelope(json: &Value, case_name: &str) {
    assert!(
        json.get("diagnostics").is_some(),
        "{}: JSON error envelope missing 'diagnostics' field",
        case_name
    );
    assert!(
        json["diagnostics"].is_array(),
        "{}: 'diagnostics' must be an array",
        case_name
    );

    assert!(
        json.get("error").is_some(),
        "{}: JSON error envelope missing 'error' field",
        case_name
    );
    assert!(
        json["error"].is_object(),
        "{}: 'error' must be an object",
        case_name
    );

    let error = &json["error"];

    assert!(
        error.get("code").is_some(),
        "{}: error object missing 'code' field",
        case_name
    );
    assert!(
        error["code"].is_string(),
        "{}: error.code must be a string",
        case_name
    );

    assert!(
        error.get("message").is_some(),
        "{}: error object missing 'message' field",
        case_name
    );
    assert!(
        error["message"].is_string(),
        "{}: error.message must be a string",
        case_name
    );

    assert!(
        error.get("details").is_some(),
        "{}: error object missing 'details' field",
        case_name
    );
    assert!(
        error["details"].is_object(),
        "{}: error.details must be an object (even if empty)",
        case_name
    );
}

// =============================================================================
// MATRIX TEST MODEL
// =============================================================================

/// Expected output kind for matrix test cases
#[derive(Debug, Clone)]
pub enum ExpectedOutput {
    /// JSON success response with schema validation
    JsonSuccess {
        schema: &'static str,
        require_diagnostics: bool,
        expected_schema_version: Option<&'static str>,
        custom_check: Option<fn(&Value)>,
    },
    /// JSON error response with envelope validation
    JsonError {
        schema: Option<&'static str>,
        /// Whether schema_version should be present in error envelope
        /// Schema commands (reason, query, etc.) should have it, non-schema (validate, stats) should not
        expect_schema_version: bool,
        expected_error_code: Option<&'static str>,
        custom_check: Option<fn(&Value)>,
    },
    /// Plain text output
    Text { contains: &'static [&'static str] },
}

/// Matrix test case definition
#[derive(Debug, Clone)]
pub struct MatrixCase {
    pub name: &'static str,
    pub args: &'static [&'static str],
    pub stdin: Option<&'static str>,
    pub expected_exit: i32,
    pub expected_output: ExpectedOutput,
}

/// Run a single matrix test case
pub fn run_matrix_case(case: &MatrixCase) {
    let (exit_code, stdout, _stderr) = run_with_stdin(case.args, case.stdin);

    assert_eq!(
        exit_code, case.expected_exit,
        "{}: Expected exit code {}, got {}",
        case.name, case.expected_exit, exit_code
    );

    match &case.expected_output {
        ExpectedOutput::JsonSuccess {
            schema,
            require_diagnostics,
            expected_schema_version,
            custom_check,
        } => {
            let json: Value = serde_json::from_str(&stdout)
                .expect(&format!("{}: Output should be valid JSON", case.name));

            // Validate schema_version if expected
            if let Some(expected_version) = expected_schema_version {
                assert_eq!(
                    json["schema_version"].as_str(),
                    Some(*expected_version),
                    "{}: schema_version mismatch",
                    case.name
                );
            } else {
                assert!(
                    json.get("schema_version").is_some(),
                    "{}: Missing schema_version",
                    case.name
                );
            }

            // Validate diagnostics if required
            if *require_diagnostics {
                assert!(
                    json.get("diagnostics").is_some(),
                    "{}: Missing diagnostics",
                    case.name
                );
                assert!(
                    json["diagnostics"].is_array(),
                    "{}: diagnostics must be an array",
                    case.name
                );
            }

            // Success response should not contain error object
            assert!(
                json.get("error").is_none(),
                "{}: Success response should not contain error object",
                case.name
            );

            // Strict schema validation
            validate_against_schema(&json, schema, case.name);

            // Run custom check if provided
            if let Some(check) = custom_check {
                check(&json);
            }
        }
        ExpectedOutput::JsonError {
            schema,
            expect_schema_version,
            expected_error_code,
            custom_check,
        } => {
            let json: Value = serde_json::from_str(&stdout).expect(&format!(
                "{}: Output should be valid JSON even on error",
                case.name
            ));

            // Validate error envelope invariants
            validate_error_envelope(&json, case.name);

            // Check schema_version expectation
            if *expect_schema_version {
                assert!(
                    json.get("schema_version").is_some(),
                    "{}: Schema command error should include schema_version",
                    case.name
                );
            } else {
                assert!(
                    json.get("schema_version").is_none(),
                    "{}: Non-schema command error should not include schema_version",
                    case.name
                );
            }

            // Validate against schema if provided
            if let Some(s) = schema {
                validate_against_schema(&json, s, case.name);
            }

            // Check expected error code
            if let Some(code) = expected_error_code {
                assert_eq!(
                    json["error"]["code"].as_str(),
                    Some(*code),
                    "{}: Expected error code {}",
                    case.name,
                    code
                );
            }

            // Run custom check if provided
            if let Some(check) = custom_check {
                check(&json);
            }
        }
        ExpectedOutput::Text { contains } => {
            assert!(
                !stdout.is_empty(),
                "{}: Expected some text output",
                case.name
            );

            for expected in *contains {
                assert!(
                    stdout.contains(expected),
                    "{}: Expected output to contain '{}'",
                    case.name,
                    expected
                );
            }
        }
    }
}
