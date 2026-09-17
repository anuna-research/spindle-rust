mod common;
use assert_cmd::cargo::cargo_bin_cmd;
use serde_json::Value;

fn run(args: &[&str], source: &str) -> Value {
    let out = cargo_bin_cmd!("spindle")
        .args(args)
        .args(["--stdin", "--json"])
        .write_stdin(source)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&out).unwrap()
}

#[test]
fn vocabulary_preserves_declared_and_observed_symbols() {
    let v = run(
        &["vocabulary"],
        "(predicate p ((n integer)) (description \"Count\"))\n(given (p 2))\n(given q)",
    );
    assert_eq!(v["schema"], "spindle.vocabulary/1");
    assert!(
        v["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["symbol"]["functor"] == "p"
                && e["symbol"]["arity"] == 1
                && e["description"] == "Count")
    );
    let dto: spindle_contract::vocabulary::VocabularyReportDto = serde_json::from_value(v).unwrap();
    dto.validate().unwrap();
}

#[test]
fn trust_reports_diminishers_and_thresholds_in_both_versions() {
    for extra in [vec!["reason", "--trust"], vec!["reason", "--trust", "--v2"]] {
        let v = run(
            &extra,
            include_str!("../../../examples/trust-diminishment.spl"),
        );
        if !extra.contains(&"--v2") {
            common::validate_against_schema(&v, "reason", "trust details");
        }
        let approved = v["conclusions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["literal_struct"]["functor"] == "approved" && c["conclusion_type"] == "+d")
            .unwrap();
        assert!((approved["trust_degree"].as_f64().unwrap() - 0.63).abs() < 1e-10);
        assert_eq!(
            approved["trust_details"]["diminished_by"][0]["defeater_label"],
            "challenge"
        );
        assert_eq!(
            approved["trust_details"]["above_threshold"]["action"],
            false
        );
    }
}

#[test]
fn hypothetical_facts_ground_new_bindings_and_candidates_are_unverified() {
    let source = "(normally r (p ?x) (q ?x))";
    let v = run(&["what-if", "(q 2)", "--given", "(p 2)"], source);
    assert_eq!(v["provable"], true);
    let v = run(&["abduce", "q"], "(normally r p q)");
    assert_eq!(v["solutions"][0]["facts"][0], "p");
}

#[test]
fn portable_extensions_work_in_reason_and_query() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/lookup-functions.json");
    let source = include_str!("../../../examples/lookup-functions.spl");
    let v = run(
        &["reason", "--v2", "--extensions", path.to_str().unwrap()],
        source,
    );
    assert!(
        v["conclusions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["positive"] == true && c["literal_spl"] == "(total-value 7)")
    );
    let v = run(
        &[
            "query",
            "(classification-of alice small)",
            "--extensions",
            path.to_str().unwrap(),
        ],
        source,
    );
    assert_eq!(v["status"], "provable");
}

#[test]
fn numeric_temporal_queries_and_invalid_literals() {
    let source = "(given (during (p 2) 100 200))";
    assert_eq!(
        run(&["query", "(during (p 2) 100 200)"], source)["status"],
        "provable"
    );
    let out = cargo_bin_cmd!("spindle")
        .args(["query", "(during (p 2) 110 190)", "--stdin", "--json"])
        .write_stdin(source)
        .output()
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap()["status"],
        "unknown"
    );
    cargo_bin_cmd!("spindle")
        .args(["query", "(p", "--stdin"])
        .write_stdin(source)
        .assert()
        .failure();
}
