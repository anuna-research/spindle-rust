use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

#[test]
fn normal_cli_reasoning_returns_structured_aggregate_conclusions() {
    let file =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/aggregation.spl");
    let result = cargo_bin_cmd!("spindle")
        .args(["reason", file.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json: Value = serde_json::from_slice(&result).unwrap();
    let conclusions = json["conclusions"].as_array().unwrap();
    let text = serde_json::to_string(conclusions).unwrap();
    assert!(text.contains("(total-payment alice 20)"), "{json}");
    assert!(text.contains("(total-payment bob 0)"), "{json}");
    assert!(!text.contains("__aggregate_snapshot"));
}

#[test]
fn cli_query_uses_the_same_aggregate_preparation() {
    let file =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/aggregation.spl");
    cargo_bin_cmd!("spindle")
        .args(["query", "(total-payment alice 20)", file.to_str().unwrap()])
        .assert()
        .success();
}

#[test]
fn cli_reports_unsafe_aggregate_variable() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("invalid.spl");
    std::fs::write(&file, "(normally r (agg ?n sum ?v (p ?v)) (n ?missing ?n))").unwrap();
    cargo_bin_cmd!("spindle")
        .args(["reason", file.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unsafe head variable"));
}
