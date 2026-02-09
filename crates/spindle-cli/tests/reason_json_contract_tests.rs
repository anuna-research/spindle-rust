use assert_cmd::cargo::cargo_bin_cmd;
use regex::Regex;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

fn setup_theory_file(content: &str, extension: &str) -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join(format!("theory.{extension}"));
    fs::write(&file_path, content).unwrap();
    (dir, file_path)
}

fn run_reason_json(content: &str, extension: &str) -> Value {
    let (_dir, path) = setup_theory_file(content, extension);
    let output = cargo_bin_cmd!("spindle")
        .arg("reason")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("Failed to run spindle reason --json");

    assert!(output.status.success(), "spindle reason --json failed");

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    serde_json::from_str(&stdout).expect("reason output should be valid JSON")
}

#[test]
fn test_reason_json_has_required_top_level_fields() {
    let content = r#"
f1: >> bird
r1: bird => flies
"#;

    let value = run_reason_json(content, "dfl");

    let obj = value
        .as_object()
        .expect("reason output should be a JSON object");
    assert!(obj.contains_key("schema_version"));
    assert!(obj.contains_key("grounding"));
    assert!(obj.contains_key("conclusions"));
    assert!(obj.contains_key("stats"));
}

#[test]
fn test_reason_json_literal_struct_has_stable_semantic_fields() {
    let content = r#"
(given (p a))
"#;

    let value = run_reason_json(content, "spl");
    let conclusions = value
        .get("conclusions")
        .and_then(Value::as_array)
        .expect("conclusions should be an array");

    let conclusion = conclusions
        .iter()
        .find(|c| c.get("literal_spl").is_some())
        .expect("expected at least one conclusion");

    let literal_struct = conclusion
        .get("literal_struct")
        .and_then(Value::as_object)
        .expect("literal_struct should be an object");

    assert!(literal_struct.contains_key("functor"));
    assert!(literal_struct.contains_key("args"));
    assert!(literal_struct.contains_key("negated"));
    assert!(literal_struct.contains_key("mode"));
    assert!(literal_struct.contains_key("temporal"));

    assert!(
        !literal_struct.contains_key("name_id"),
        "literal_struct should not expose internal name_id"
    );
    assert!(
        !literal_struct.contains_key("predicate_ids"),
        "literal_struct should not expose internal predicate_ids"
    );
}

#[test]
fn test_reason_json_literal_spl_is_canonical_format() {
    let content = r#"
(given (p a))
"#;

    let value = run_reason_json(content, "spl");
    let conclusions = value
        .get("conclusions")
        .and_then(Value::as_array)
        .expect("conclusions should be an array");

    let literal_spl = conclusions
        .iter()
        .filter_map(|c| c.get("literal_spl"))
        .filter_map(Value::as_str)
        .find(|s| s.contains("p"))
        .expect("expected literal_spl for p(a)");

    assert_eq!(
        literal_spl, "(p a)",
        "literal_spl should use SPL s-expression format"
    );
}

#[test]
fn test_reason_json_literal_spl_quotes_atoms_with_spaces() {
    let content = r#"
(given (p "doc 1"))
"#;

    let value = run_reason_json(content, "spl");
    let conclusions = value
        .get("conclusions")
        .and_then(Value::as_array)
        .expect("conclusions should be an array");

    let literal_spl = conclusions
        .iter()
        .filter_map(|c| c.get("literal_spl"))
        .filter_map(Value::as_str)
        .find(|s| s.contains("p"))
        .expect("expected literal_spl for p(\"doc 1\")");

    assert_eq!(
        literal_spl, "(p \"doc 1\")",
        "literal_spl should quote atoms containing spaces"
    );
}

// =============================================================================
// NEW TESTS FOR SPEC ALIGNMENT (RFC §3.5, §5)
// =============================================================================

/// Test that evaluated_at uses RFC3339 format when --at is provided (spec §5 Milestone 5)
#[test]
fn test_reason_json_evaluated_at_is_rfc3339_format() {
    let content = r#"
(given bird)
(normally r1 bird flies)
"#;

    let (_dir, path) = setup_theory_file(content, "spl");
    let output = cargo_bin_cmd!("spindle")
        .arg("reason")
        .arg(&path)
        .arg("--json")
        .arg("--at")
        .arg("2024-02-06T12:00:00Z")
        .output()
        .expect("Failed to run spindle reason --json --at");

    assert!(
        output.status.success(),
        "spindle reason --json --at failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    let value: Value = serde_json::from_str(&stdout).expect("reason output should be valid JSON");

    let evaluated_at = value
        .get("evaluated_at")
        .and_then(Value::as_str)
        .expect("evaluated_at should be present when --at is provided");

    // RFC3339 format: YYYY-MM-DDTHH:MM:SS.sssZ or similar
    let rfc3339_regex = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$").unwrap();
    assert!(
        rfc3339_regex.is_match(evaluated_at),
        "evaluated_at should be RFC3339 format, got: {evaluated_at}"
    );

    // Verify the date matches what we passed in
    assert!(
        evaluated_at.starts_with("2024-02-06"),
        "evaluated_at should contain the date we passed in: {evaluated_at}"
    );
}

/// Test that conclusions are sorted deterministically (spec §3.5)
#[test]
fn test_reason_json_conclusions_are_sorted_deterministically() {
    let content = r#"
(given zebra)
(given aardvark)
(given monkey)
(normally r1 zebra z_derived)
(normally r2 aardvark a_derived)
(normally r3 monkey m_derived)
"#;

    // Run multiple times to verify determinism
    for _ in 0..3 {
        let value = run_reason_json(content, "spl");
        let conclusions = value
            .get("conclusions")
            .and_then(Value::as_array)
            .expect("conclusions should be an array");

        // Extract literal_spl values
        let spls: Vec<&str> = conclusions
            .iter()
            .filter_map(|c| c.get("literal_spl").and_then(Value::as_str))
            .collect();

        // Check that they are sorted
        let mut sorted_spls = spls.clone();
        sorted_spls.sort();

        assert_eq!(
            spls, sorted_spls,
            "conclusions should be sorted by literal_spl"
        );
    }
}

/// Test that conclusions with same literal_spl are sorted by conclusion_type
#[test]
fn test_reason_json_conclusions_sorted_by_literal_spl_then_conclusion_type() {
    let content = r#"
(given (bird))
"#;

    let value = run_reason_json(content, "spl");
    let conclusions = value
        .get("conclusions")
        .and_then(Value::as_array)
        .expect("conclusions should be an array");

    // Find all conclusions for "bird"
    let bird_conclusions: Vec<(&str, &str)> = conclusions
        .iter()
        .filter_map(|c| {
            let spl = c.get("literal_spl").and_then(Value::as_str)?;
            let ct = c.get("conclusion_type").and_then(Value::as_str)?;
            if spl == "(bird)" {
                Some((spl, ct))
            } else {
                None
            }
        })
        .collect();

    // Should have both +D and +d for bird (as a fact)
    assert!(
        bird_conclusions.len() >= 2,
        "expected multiple conclusion types for bird, got {bird_conclusions:?}"
    );

    // Verify conclusion types are sorted
    let types: Vec<&str> = bird_conclusions.iter().map(|(_, ct)| *ct).collect();
    let mut sorted_types = types.clone();
    sorted_types.sort();

    assert_eq!(
        types, sorted_types,
        "conclusion_types for same literal should be sorted"
    );
}
