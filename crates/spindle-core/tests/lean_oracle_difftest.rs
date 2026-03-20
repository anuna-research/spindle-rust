//! Differential testing: Rust `reason()` vs Lean oracle (`spindlelean_oracle`).
//!
//! Generates random theories via proptest, serializes to JSON, shells out to the
//! Lean oracle executable, parses the JSON response, and compares conclusion sets.
//! Reports the first mismatch with a full theory dump for debugging.
//!
//! # Running
//!
//! The Lean oracle must be built first:
//! ```sh
//! cd lean && lake build spindlelean_oracle
//! ```
//!
//! Run with:
//! ```sh
//! cargo test --test lean_oracle_difftest
//! ```
//!
//! Set `LEAN_ORACLE_BIN` to override the oracle binary path.

mod proptest_helpers;

use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Command, Stdio};

use proptest::prelude::*;
use serde_json::{Value, json};

use spindle_core::reason::reason;
use spindle_core::rule::RuleType;
use spindle_core::theory::Theory;

use proptest_helpers::arb_theory;

// =============================================================================
// JSON serialization: Rust Theory → Lean oracle JSON format
// =============================================================================

/// A normalized conclusion for comparison: (conclusion_type_symbol, literal_name, polarity).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NormalizedConclusion {
    conclusion_type: String,
    name: String,
    polarity: bool,
}

impl std::fmt::Display for NormalizedConclusion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let neg = if self.polarity { "" } else { "~" };
        write!(f, "{} {}{}", self.conclusion_type, neg, self.name)
    }
}

/// Serialize a Rust `Theory` to the JSON format expected by the Lean oracle.
///
/// Format:
/// ```json
/// {
///   "rules": [
///     {"label": "r1", "rule_type": "strict", "body": [{"name": "a", "polarity": true}], "head": {"name": "b", "polarity": true}}
///   ],
///   "superiority": [["r1", "r2"]]
/// }
/// ```
fn theory_to_json(theory: &Theory) -> Value {
    let mut rules = Vec::new();

    for rule in theory.rules() {
        // Facts are encoded as rules with empty body in the Lean oracle.
        // The Lean oracle only knows strict/defeasible/defeater — facts are
        // strict rules with empty body.
        let rule_type_str = match rule.rule_type {
            RuleType::Fact => "strict", // facts are strict rules with empty body
            RuleType::Strict => "strict",
            RuleType::Defeasible => "defeasible",
            RuleType::Defeater => "defeater",
        };

        let body_json: Vec<Value> = rule
            .body
            .iter()
            .filter_map(|bl| {
                // Only serialize logic literals (skip arithmetic constraints)
                match bl {
                    spindle_core::body::BodyLiteral::Logic(lit) => Some(json!({
                        "name": lit.name(),
                        "polarity": !lit.negation,
                    })),
                    _ => None,
                }
            })
            .collect();

        let head = rule.head_literal();
        let head_json = json!({
            "name": head.name(),
            "polarity": !head.negation,
        });

        rules.push(json!({
            "label": rule.label,
            "rule_type": rule_type_str,
            "body": body_json,
            "head": head_json,
        }));
    }

    let superiority: Vec<Value> = theory
        .superiorities()
        .iter()
        .map(|s| json!([s.superior, s.inferior]))
        .collect();

    json!({
        "rules": rules,
        "superiority": superiority,
    })
}

/// Parse the Lean oracle JSON output into a set of normalized conclusions.
fn parse_oracle_conclusions(output: &str) -> Result<BTreeSet<NormalizedConclusion>, String> {
    let v: Value = serde_json::from_str(output).map_err(|e| format!("JSON parse error: {e}"))?;
    let conclusions = v
        .get("conclusions")
        .and_then(|c| c.as_array())
        .ok_or_else(|| "missing 'conclusions' array in oracle output".to_string())?;

    let mut set = BTreeSet::new();
    for c in conclusions {
        let ct = c
            .get("conclusion_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing conclusion_type".to_string())?;
        let lit = c
            .get("literal")
            .ok_or_else(|| "missing literal".to_string())?;
        let name = lit
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "missing literal name".to_string())?;
        let polarity = lit
            .get("polarity")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| "missing literal polarity".to_string())?;

        set.insert(NormalizedConclusion {
            conclusion_type: ct.to_string(),
            name: name.to_string(),
            polarity,
        });
    }
    Ok(set)
}

/// Normalize Rust conclusions into the same representation used for oracle output.
fn normalize_rust_conclusions(
    conclusions: &[spindle_core::conclusion::Conclusion],
) -> BTreeSet<NormalizedConclusion> {
    conclusions
        .iter()
        .map(|c| NormalizedConclusion {
            conclusion_type: c.conclusion_type.symbol().to_string(),
            name: c.literal.name().to_string(),
            polarity: !c.literal.negation,
        })
        .collect()
}

/// Find the Lean oracle binary, checking `LEAN_ORACLE_BIN` env var first,
/// then the standard Lake build output path.
fn find_oracle_bin() -> Option<String> {
    if let Ok(path) = std::env::var("LEAN_ORACLE_BIN") {
        if std::path::Path::new(&path).exists() {
            return Some(path);
        }
    }

    // Standard Lake build output path (relative to workspace root)
    let candidates = [
        "lean/.lake/build/bin/spindlelean_oracle",
        "../../../lean/.lake/build/bin/spindlelean_oracle",
    ];
    for candidate in &candidates {
        let path = std::path::Path::new(candidate);
        if path.exists() {
            return Some(path.to_string_lossy().to_string());
        }
    }
    None
}

/// Run the Lean oracle on a JSON-encoded theory, returning the raw stdout.
fn run_oracle(oracle_bin: &str, theory_json: &str) -> Result<String, String> {
    let mut child = Command::new(oracle_bin)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to spawn oracle: {e}"))?;

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(theory_json.as_bytes())
        .map_err(|e| format!("failed to write to oracle stdin: {e}"))?;

    let output = child
        .wait_with_output()
        .map_err(|e| format!("failed to wait for oracle: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "oracle exited with status {}: {}",
            output.status, stderr
        ));
    }

    String::from_utf8(output.stdout).map_err(|e| format!("oracle stdout not UTF-8: {e}"))
}

/// Format a mismatch report for debugging.
fn mismatch_report(
    theory_json: &str,
    rust_set: &BTreeSet<NormalizedConclusion>,
    lean_set: &BTreeSet<NormalizedConclusion>,
) -> String {
    let only_rust: Vec<_> = rust_set.difference(lean_set).collect();
    let only_lean: Vec<_> = lean_set.difference(rust_set).collect();

    let mut report = String::new();
    report.push_str("=== MISMATCH DETECTED ===\n\n");

    if !only_rust.is_empty() {
        report.push_str("Conclusions only in Rust:\n");
        for c in &only_rust {
            report.push_str(&format!("  {c}\n"));
        }
    }
    if !only_lean.is_empty() {
        report.push_str("Conclusions only in Lean:\n");
        for c in &only_lean {
            report.push_str(&format!("  {c}\n"));
        }
    }

    report.push_str(&format!(
        "\nRust conclusions ({}):\n",
        rust_set.len()
    ));
    for c in rust_set {
        report.push_str(&format!("  {c}\n"));
    }

    report.push_str(&format!(
        "\nLean conclusions ({}):\n",
        lean_set.len()
    ));
    for c in lean_set {
        report.push_str(&format!("  {c}\n"));
    }

    report.push_str("\nTheory JSON:\n");
    // Pretty-print the JSON for readability
    if let Ok(v) = serde_json::from_str::<Value>(theory_json) {
        report.push_str(&serde_json::to_string_pretty(&v).unwrap_or_else(|_| theory_json.to_string()));
    } else {
        report.push_str(theory_json);
    }
    report.push('\n');

    report
}

// =============================================================================
// Property-based differential test
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// For every randomly generated theory, Rust reason() and the Lean oracle
    /// must produce identical conclusion sets.
    #[test]
    fn rust_matches_lean_oracle(theory in arb_theory()) {
        // Skip if oracle binary not found (graceful degradation in CI without Lean)
        let oracle_bin = match find_oracle_bin() {
            Some(bin) => bin,
            None => {
                // Not an error — just skip if Lean oracle isn't built
                return Ok(());
            }
        };

        // Serialize theory to JSON
        let theory_json = theory_to_json(&theory);
        let theory_json_str = theory_json.to_string();

        // Run Rust reasoner
        let rust_conclusions = reason(&theory)
            .map_err(|e| TestCaseError::Fail(format!("Rust reason() failed: {e}").into()))?;
        let rust_set = normalize_rust_conclusions(&rust_conclusions);

        // Run Lean oracle
        let oracle_output = run_oracle(&oracle_bin, &theory_json_str)
            .map_err(|e| TestCaseError::Fail(format!("Lean oracle failed: {e}").into()))?;
        let lean_set = parse_oracle_conclusions(&oracle_output)
            .map_err(|e| TestCaseError::Fail(format!("Failed to parse oracle output: {e}").into()))?;

        // Compare
        prop_assert_eq!(
            &rust_set,
            &lean_set,
            "{}",
            mismatch_report(&theory_json_str, &rust_set, &lean_set)
        );
    }
}

// =============================================================================
// Unit tests for serialization helpers
// =============================================================================

#[cfg(test)]
mod serialization_tests {
    use super::*;
    use spindle_core::theory::Theory;

    #[test]
    fn test_simple_theory_serialization() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");

        let json = theory_to_json(&theory);
        let rules = json["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 2);

        // Check that superiority is an array
        assert!(json["superiority"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_theory_with_superiority_serialization() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        let r1 = theory.add_defeasible_rule(&["a"], "b");
        let r2 = theory.add_defeasible_rule(&["a"], "~b");
        theory.add_superiority(&r1, &r2);

        let json = theory_to_json(&theory);
        let sup = json["superiority"].as_array().unwrap();
        assert_eq!(sup.len(), 1);
        assert_eq!(sup[0][0].as_str().unwrap(), r1);
        assert_eq!(sup[0][1].as_str().unwrap(), r2);
    }

    #[test]
    fn test_negated_literal_serialization() {
        let mut theory = Theory::new();
        theory.add_fact("~a");

        let json = theory_to_json(&theory);
        let rules = json["rules"].as_array().unwrap();
        let head = &rules[0]["head"];
        assert_eq!(head["name"].as_str().unwrap(), "a");
        assert_eq!(head["polarity"].as_bool().unwrap(), false);
    }

    #[test]
    fn test_parse_oracle_output() {
        let output = r#"{"conclusions":[{"conclusion_type":"+D","literal":{"name":"a","polarity":true}},{"conclusion_type":"+d","literal":{"name":"a","polarity":true}}]}"#;
        let set = parse_oracle_conclusions(output).unwrap();
        assert_eq!(set.len(), 2);
        assert!(set.contains(&NormalizedConclusion {
            conclusion_type: "+D".to_string(),
            name: "a".to_string(),
            polarity: true,
        }));
    }

    #[test]
    fn test_normalize_rust_conclusions() {
        use spindle_core::conclusion::Conclusion;
        use spindle_core::literal::Literal;

        let conclusions = vec![
            Conclusion::definitely_provable(Literal::simple("a")),
            Conclusion::defeasibly_provable(Literal::negated("b")),
        ];
        let set = normalize_rust_conclusions(&conclusions);
        assert_eq!(set.len(), 2);
        assert!(set.contains(&NormalizedConclusion {
            conclusion_type: "+D".to_string(),
            name: "a".to_string(),
            polarity: true,
        }));
        assert!(set.contains(&NormalizedConclusion {
            conclusion_type: "+d".to_string(),
            name: "b".to_string(),
            polarity: false,
        }));
    }
}
