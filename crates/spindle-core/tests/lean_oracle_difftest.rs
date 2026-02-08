//! Differential testing harness: Lean oracle vs Rust scalable
//!
//! Generates random theories, serializes to JSON, runs both the Lean oracle
//! executable and Rust's reason_scalable(), and compares conclusion sets.
//!
//! Requires: Lean oracle built (`lake build` in the `lean/` directory).
//! The test is `#[ignore]`d by default since it requires the Lean toolchain.
//! Run with: `cargo test --test lean_oracle_difftest -- --ignored`

use std::collections::BTreeSet;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use spindle_core::conclusion::ConclusionType;
use spindle_core::index::IndexedTheory;
use spindle_core::literal::Literal;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::rule::{Rule, RuleType};
use spindle_core::scalable::reason_scalable;
use spindle_core::theory::Theory;

// ═══════════════════════════════════════════════════════════════
// JSON serialization for Lean oracle input
// ═══════════════════════════════════════════════════════════════

fn literal_to_json(lit: &Literal) -> String {
    format!(
        r#"{{"name":"{}","negated":{}}}"#,
        lit.name(),
        lit.negation,
    )
}

fn rule_to_json(rule: &Rule) -> String {
    let rule_type = match rule.rule_type {
        RuleType::Fact => "fact",
        RuleType::Strict => "strict",
        RuleType::Defeasible => "defeasible",
        RuleType::Defeater => "defeater",
    };
    let body: Vec<String> = rule.body.iter().map(|l| literal_to_json(l)).collect();
    let head = literal_to_json(rule.head_literal());
    format!(
        r#"{{"label":"{}","type":"{}","body":[{}],"head":{}}}"#,
        rule.label,
        rule_type,
        body.join(","),
        head,
    )
}

fn theory_to_json(theory: &Theory) -> String {
    let rules: Vec<String> = theory.rules().map(|r| rule_to_json(r)).collect();
    let sups: Vec<String> = theory
        .superiorities()
        .iter()
        .map(|s| format!(r#"["{}","{}"]"#, s.superior, s.inferior))
        .collect();
    format!(
        r#"{{"rules":[{}],"superiority":[{}]}}"#,
        rules.join(","),
        sups.join(","),
    )
}

// ═══════════════════════════════════════════════════════════════
// Lean oracle result parsing
// ═══════════════════════════════════════════════════════════════

/// Parsed conclusion from JSON
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct OracleConclusion {
    name: String,
    negated: bool,
    conclusion_type: String,
}

/// Parse oracle JSON output to extract conclusions
fn parse_oracle_conclusions(json: &str) -> Vec<OracleConclusion> {
    // Simple JSON parsing for our structured output
    let mut conclusions = Vec::new();

    // Find "conclusions" array
    if let Some(start) = json.find("\"conclusions\":[") {
        let rest = &json[start + 15..];
        // Parse each conclusion object
        let mut i = 0;
        while i < rest.len() {
            if let Some(lit_start) = rest[i..].find("\"literal\":{") {
                let base = i + lit_start + 11;
                // Extract name
                if let Some(name_start) = rest[base..].find("\"name\":\"") {
                    let ns = base + name_start + 8;
                    if let Some(name_end) = rest[ns..].find('"') {
                        let name = rest[ns..ns + name_end].to_string();
                        // Extract negated
                        if let Some(neg_start) = rest[ns..].find("\"negated\":") {
                            let nb = ns + neg_start + 10;
                            let negated = rest[nb..].starts_with("true");
                            // Extract type
                            if let Some(type_start) = rest[nb..].find("\"type\":\"") {
                                let ts = nb + type_start + 8;
                                if let Some(type_end) = rest[ts..].find('"') {
                                    let ct = rest[ts..ts + type_end].to_string();
                                    conclusions.push(OracleConclusion {
                                        name,
                                        negated,
                                        conclusion_type: ct,
                                    });
                                    i = ts + type_end;
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
            break;
        }
    }
    conclusions
}

// ═══════════════════════════════════════════════════════════════
// Oracle runner
// ═══════════════════════════════════════════════════════════════

fn lean_oracle_path() -> PathBuf {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    workspace.join("lean").join(".lake").join("build").join("bin").join("spindlelean")
}

fn run_lean_oracle(json: &str) -> Option<String> {
    let oracle = lean_oracle_path();
    if !oracle.exists() {
        return None;
    }

    let mut child = Command::new(&oracle)
        .arg("--oracle")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    child.stdin.as_mut()?.write_all(json.as_bytes()).ok()?;
    drop(child.stdin.take());

    let output = child.wait_with_output().ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════
// Test helpers
// ═══════════════════════════════════════════════════════════════

fn build_test_theory(
    facts: &[&str],
    defeasible: &[(&[&str], &str)],
    superiorities: &[(&str, &str)],
) -> Theory {
    let mut theory = Theory::new();
    for (i, name) in facts.iter().enumerate() {
        theory.add_rule(Rule::fact(format!("f{i}"), Literal::simple(name)));
    }
    for (i, (body, head)) in defeasible.iter().enumerate() {
        let body_lits: Vec<Literal> = body.iter().map(|s| {
            if let Some(n) = s.strip_prefix('~') {
                Literal::negated(n)
            } else {
                Literal::simple(s)
            }
        }).collect();
        let head_lit = if let Some(n) = head.strip_prefix('~') {
            Literal::negated(n)
        } else {
            Literal::simple(head)
        };
        theory.add_rule(Rule::defeasible(format!("r{i}"), body_lits, head_lit));
    }
    for (sup, inf) in superiorities {
        theory.add_superiority(sup, inf);
    }
    theory
}

// ═══════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════

#[test]
#[ignore] // Requires Lean oracle built
fn lean_oracle_tweety_triangle() {
    let theory = build_test_theory(
        &["bird_tweety", "penguin_tweety", "bird_eddie"],
        &[
            (&["bird_tweety"], "flies_tweety"),
            (&["penguin_tweety"], "~flies_tweety"),
            (&["bird_eddie"], "flies_eddie"),
        ],
        &[("r1", "r0")], // r1 (~flies) > r0 (flies)
    );

    let json = theory_to_json(&theory);
    let oracle_output = run_lean_oracle(&json)
        .expect("Lean oracle not built — run `lake build` in lean/");

    let oracle_conclusions = parse_oracle_conclusions(&oracle_output);

    // Check Lean oracle says eddie flies
    let eddie_flies = oracle_conclusions.iter().any(|c| {
        c.name == "flies_eddie" && !c.negated && c.conclusion_type == "+d"
    });
    assert!(eddie_flies, "Lean oracle should conclude +d flies_eddie");

    // Check Lean oracle says tweety doesn't fly
    let tweety_neg = oracle_conclusions.iter().any(|c| {
        c.name == "flies_tweety" && c.negated && c.conclusion_type == "+d"
    });
    assert!(tweety_neg, "Lean oracle should conclude +d ~flies_tweety");
}

#[test]
#[ignore] // Requires Lean oracle built
fn lean_vs_rust_scalable_simple_facts() {
    let theory = build_test_theory(
        &["p", "q", "r"],
        &[],
        &[],
    );

    let json = theory_to_json(&theory);
    let oracle_output = run_lean_oracle(&json)
        .expect("Lean oracle not built — run `lake build` in lean/");

    // Run Rust scalable
    let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
    let indexed = IndexedTheory::build(&prepared.theory);
    let rust_result = reason_scalable(&indexed);
    let rust_conclusions = rust_result.to_conclusions(&indexed);

    // Both should agree: p, q, r are +D
    let rust_definite: BTreeSet<_> = rust_conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
        .map(|c| (c.literal.name().to_string(), c.literal.negation))
        .collect();

    let oracle_definite: BTreeSet<_> = parse_oracle_conclusions(&oracle_output)
        .into_iter()
        .filter(|c| c.conclusion_type == "+D")
        .map(|c| (c.name, c.negated))
        .collect();

    assert_eq!(
        rust_definite, oracle_definite,
        "Rust and Lean disagree on +D for simple facts"
    );
}

#[test]
#[ignore] // Requires Lean oracle built
fn lean_vs_rust_scalable_defeasible_chain() {
    let theory = build_test_theory(
        &["a"],
        &[
            (&["a"], "b"),
            (&["b"], "c"),
        ],
        &[],
    );

    let json = theory_to_json(&theory);
    let oracle_output = run_lean_oracle(&json)
        .expect("Lean oracle not built — run `lake build` in lean/");

    // Run Rust scalable
    let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
    let indexed = IndexedTheory::build(&prepared.theory);
    let rust_result = reason_scalable(&indexed);
    let rust_conclusions = rust_result.to_conclusions(&indexed);

    // Both should derive +d b and +d c
    let rust_defeasible: BTreeSet<_> = rust_conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
        .map(|c| (c.literal.name().to_string(), c.literal.negation))
        .collect();

    let oracle_defeasible: BTreeSet<_> = parse_oracle_conclusions(&oracle_output)
        .into_iter()
        .filter(|c| c.conclusion_type == "+d")
        .map(|c| (c.name, c.negated))
        .collect();

    assert_eq!(
        rust_defeasible, oracle_defeasible,
        "Rust and Lean disagree on +d for defeasible chain.\nRust: {:?}\nLean: {:?}",
        rust_defeasible, oracle_defeasible,
    );
}
