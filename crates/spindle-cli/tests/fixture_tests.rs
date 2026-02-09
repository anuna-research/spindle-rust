//! Integration tests that verify SPL fixture files against expected conclusions.
//!
//! Each `.spl` file in `tests/fixtures/` contains `; EXPECT: <conclusion>` and
//! `; EXPECT-NOT: <conclusion>` comments that are verified against the scalable
//! reasoner output.

use assert_cmd::cargo::cargo_bin_cmd;
use std::path::Path;

/// Parse EXPECT / EXPECT-NOT annotations from an SPL fixture file.
fn parse_expectations(content: &str) -> (Vec<String>, Vec<String>) {
    let mut expect = Vec::new();
    let mut expect_not = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("; EXPECT-NOT:") {
            expect_not.push(rest.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("; EXPECT:") {
            expect.push(rest.trim().to_string());
        }
    }
    (expect, expect_not)
}

/// Run the scalable reasoner on a fixture file and return stdout.
fn run_reason(path: &Path) -> String {
    let output = cargo_bin_cmd!("spindle")
        .arg("reason")
        .arg(path)
        .arg("--scalable")
        .output()
        .expect("failed to execute spindle");
    assert!(
        output.status.success(),
        "spindle reason failed on {}: {}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invalid utf8 in stdout")
}

/// Parse conclusion lines from the reasoner output.
/// Each conclusion line looks like "  +D bird" or "  +d flies".
fn parse_conclusions(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| {
            l.starts_with("+D ")
                || l.starts_with("+d ")
                || l.starts_with("-D ")
                || l.starts_with("-d ")
        })
        .map(|l| l.to_string())
        .collect()
}

fn run_fixture(name: &str) {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name);
    assert!(
        fixture_path.exists(),
        "fixture not found: {}",
        fixture_path.display()
    );

    let content = std::fs::read_to_string(&fixture_path).expect("failed to read fixture");
    let (expects, expect_nots) = parse_expectations(&content);
    assert!(
        !expects.is_empty(),
        "fixture {name} has no EXPECT annotations"
    );

    let output = run_reason(&fixture_path);
    let conclusions = parse_conclusions(&output);

    let mut failures = Vec::new();

    for exp in &expects {
        if !conclusions.contains(exp) {
            failures.push(format!("MISSING expected conclusion: {exp}"));
        }
    }

    for not_exp in &expect_nots {
        if conclusions.contains(not_exp) {
            failures.push(format!("UNEXPECTED conclusion present: {not_exp}"));
        }
    }

    if !failures.is_empty() {
        panic!(
            "Fixture {name} failed:\n{}\n\nActual conclusions:\n{}",
            failures.join("\n"),
            conclusions.join("\n")
        );
    }
}

#[test]
fn fixture_01_basic_facts() {
    run_fixture("01_basic_facts.spl");
}

#[test]
fn fixture_02_strict_rules() {
    run_fixture("02_strict_rules.spl");
}

#[test]
fn fixture_03_defeasible_rules() {
    run_fixture("03_defeasible_rules.spl");
}

#[test]
fn fixture_04_defeaters() {
    run_fixture("04_defeaters.spl");
}

#[test]
fn fixture_05_superiority() {
    run_fixture("05_superiority.spl");
}

#[test]
fn fixture_06_ambiguity() {
    run_fixture("06_ambiguity.spl");
}

#[test]
fn fixture_07_negation() {
    run_fixture("07_negation.spl");
}

#[test]
fn fixture_08_conjunction() {
    run_fixture("08_conjunction.spl");
}

#[test]
fn fixture_09_variables() {
    run_fixture("09_variables.spl");
}

#[test]
fn fixture_10_predicates() {
    run_fixture("10_predicates.spl");
}

#[test]
fn fixture_11_strict_overrides_defeasible() {
    run_fixture("11_empty_body.spl");
}

#[test]
fn fixture_12_chains() {
    run_fixture("12_chains.spl");
}

#[test]
fn fixture_13_mixed_rules() {
    run_fixture("13_mixed_rules.spl");
}

#[test]
fn fixture_14_diamond() {
    run_fixture("14_diamond.spl");
}

#[test]
fn fixture_15_team_defeat() {
    run_fixture("15_team_defeat.spl");
}
