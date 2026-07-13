//! Differential testing: Rust grounding vs Lean grounding oracle.
//!
//! Generates random rules with variables and a domain, computes ground
//! instances in both Rust (direct Herbrand enumeration) and Lean
//! (via the GroundingOracle), and asserts the results agree.
//!
//! The Lean oracle is built from `lean/Spindle/DiffTest/GroundingOracle.lean`
//! and communicates via JSON over stdin/stdout.
//!
//! Run with: `cargo test --test lean_grounding_oracle_difftest`
//! Requires: `lake build GroundingOracle` in the `lean/` directory first.

use std::collections::BTreeSet;
use std::process::Command;

use proptest::prelude::*;
use proptest::strategy::ValueTree;
use serde_json::{Value as JValue, json};

use spindle_core::grounding;
use spindle_core::intern::{intern, resolve};
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::rule::Rule;
use spindle_core::temporal::Temporal;
use spindle_core::term::Term;

// ---------------------------------------------------------------------------
// JSON serialization (Rust → Lean wire format)
// ---------------------------------------------------------------------------

/// Serialize a Rust Term to the Lean oracle JSON format.
/// In Rust, variables are Term::Symbol whose resolved name starts with '?'.
fn term_to_json(t: &Term) -> JValue {
    match t {
        Term::Symbol(id) => {
            let name = resolve(*id);
            if grounding::is_variable(name) {
                json!({"variable": name})
            } else {
                json!({"symbol": name})
            }
        }
        Term::Integer(n) => json!({"integer": *n}),
        Term::Decimal(d) => {
            let scale = d.scale();
            let mantissa = d.mantissa();
            json!({"decimal": {"n": mantissa, "scale": scale}})
        }
        Term::Float(f) => json!({"float": f.to_string()}),
    }
}

/// Serialize a Rust Literal to the Lean oracle JSON format.
fn literal_to_json(lit: &Literal) -> JValue {
    let args: Vec<JValue> = lit.predicate_args().iter().map(term_to_json).collect();
    json!({
        "name": lit.name(),
        "negation": lit.negation,
        "args": args
    })
}

/// Serialize a Rust Rule to the Lean oracle JSON format.
/// Only uses head and logic body literals (no arithmetic constraints).
fn rule_to_json(rule: &Rule) -> JValue {
    let body: Vec<JValue> = rule
        .body
        .iter()
        .filter_map(|bl| bl.as_logic())
        .map(|logic| literal_to_json(&logic.to_literal()))
        .collect();
    json!({
        "head": literal_to_json(&rule.head[0]),
        "body": body
    })
}

// ---------------------------------------------------------------------------
// JSON deserialization (Lean response → Rust comparison types)
// ---------------------------------------------------------------------------

/// A normalized ground literal for comparison (name, negation, args as strings).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NormalizedLiteral {
    name: String,
    negation: bool,
    args: Vec<String>,
}

/// A normalized ground rule for set comparison.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NormalizedRule {
    head: NormalizedLiteral,
    body: Vec<NormalizedLiteral>,
}

fn normalize_term_json(t: &JValue) -> String {
    if let Some(s) = t.get("symbol").and_then(|v| v.as_str()) {
        s.to_string()
    } else if let Some(n) = t.get("integer").and_then(|v| v.as_i64()) {
        n.to_string()
    } else if let Some(d) = t.get("decimal") {
        let n = d.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
        let scale = d.get("scale").and_then(|v| v.as_u64()).unwrap_or(0);
        if scale == 0 {
            format!("{n}")
        } else {
            format!("decimal({n},{scale})")
        }
    } else if let Some(f) = t.get("float").and_then(|v| v.as_str()) {
        format!("float({f})")
    } else if let Some(v) = t.get("variable").and_then(|v| v.as_str()) {
        // Variables should not appear in ground instances
        format!("?{v}")
    } else {
        format!("unknown({t})")
    }
}

fn normalize_literal_json(j: &JValue) -> NormalizedLiteral {
    let name = j
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let negation = j.get("negation").and_then(|v| v.as_bool()).unwrap_or(false);
    let args: Vec<String> = j
        .get("args")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(normalize_term_json).collect())
        .unwrap_or_default();
    NormalizedLiteral {
        name,
        negation,
        args,
    }
}

fn normalize_rule_json(j: &JValue) -> NormalizedRule {
    let head = normalize_literal_json(j.get("head").unwrap_or(&JValue::Null));
    let body: Vec<NormalizedLiteral> = j
        .get("body")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(normalize_literal_json).collect())
        .unwrap_or_default();
    NormalizedRule { head, body }
}

/// Normalize a Rust ground literal for comparison.
fn normalize_rust_literal(lit: &Literal) -> NormalizedLiteral {
    let args: Vec<String> = lit
        .predicate_args()
        .iter()
        .map(|t| match t {
            Term::Symbol(id) => resolve(*id).to_string(),
            Term::Integer(n) => n.to_string(),
            Term::Decimal(d) => {
                if d.scale() == 0 {
                    format!("{}", d.mantissa())
                } else {
                    format!("decimal({},{})", d.mantissa(), d.scale())
                }
            }
            Term::Float(f) => format!("float({f})"),
        })
        .collect();
    NormalizedLiteral {
        name: lit.name().to_string(),
        negation: lit.negation,
        args,
    }
}

// ---------------------------------------------------------------------------
// Rust-side Herbrand grounding (mirrors Lean's Rule.groundInstances)
// ---------------------------------------------------------------------------

/// Compute all ground instances of a rule over a domain, using the same
/// algorithm as Lean's `Rule.groundInstances`:
/// 1. Collect unique variables from head + body
/// 2. Enumerate all substitutions mapping variables to domain terms
/// 3. Apply each substitution to the rule
fn rust_ground_instances(rule: &Rule, domain: &[Term]) -> Vec<NormalizedRule> {
    // Collect variables from head and body
    let mut vars: Vec<String> = Vec::new();
    collect_vars_from_literal(&rule.head[0], &mut vars);
    for bl in &rule.body {
        if let Some(logic) = bl.as_logic() {
            collect_vars_from_literal(&logic.to_literal(), &mut vars);
        }
    }
    // Deduplicate preserving order (like Lean's eraseDups)
    let mut seen = BTreeSet::new();
    vars.retain(|v| seen.insert(v.clone()));

    if vars.is_empty() {
        // Already ground, return as-is
        let head = normalize_rust_literal(&rule.head[0]);
        let body: Vec<NormalizedLiteral> = rule
            .body
            .iter()
            .filter_map(|bl| bl.as_logic())
            .map(|logic| normalize_rust_literal(&logic.to_literal()))
            .collect();
        return vec![NormalizedRule { head, body }];
    }

    // Generate all substitutions
    let substitutions = all_substitutions(&vars, domain);

    // Apply each substitution
    substitutions
        .into_iter()
        .map(|subst| {
            let head = apply_subst_literal(&rule.head[0], &subst);
            let body: Vec<NormalizedLiteral> = rule
                .body
                .iter()
                .filter_map(|bl| bl.as_logic())
                .map(|logic| apply_subst_literal(&logic.to_literal(), &subst))
                .collect();
            NormalizedRule { head, body }
        })
        .collect()
}

fn collect_vars_from_literal(lit: &Literal, vars: &mut Vec<String>) {
    for arg in lit.predicate_args() {
        if let Term::Symbol(id) = arg {
            let name = resolve(*id);
            if grounding::is_variable(name) {
                vars.push(name.to_string());
            }
        }
    }
}

/// Enumerate all substitutions from variables to domain terms.
fn all_substitutions(vars: &[String], domain: &[Term]) -> Vec<Vec<(String, Term)>> {
    if vars.is_empty() {
        return vec![vec![]];
    }
    let rest = all_substitutions(&vars[1..], domain);
    let mut result = Vec::new();
    for t in domain {
        for sigma in &rest {
            let mut new_sigma = vec![(vars[0].clone(), t.clone())];
            new_sigma.extend(sigma.iter().cloned());
            result.push(new_sigma);
        }
    }
    result
}

/// Apply a substitution to a literal, producing a normalized result.
fn apply_subst_literal(lit: &Literal, subst: &[(String, Term)]) -> NormalizedLiteral {
    let args: Vec<String> = lit
        .predicate_args()
        .iter()
        .map(|t| {
            if let Term::Symbol(id) = t {
                let name = resolve(*id);
                if grounding::is_variable(name) {
                    // Look up in substitution
                    for (var, val) in subst {
                        if var == name {
                            return match val {
                                Term::Symbol(sid) => resolve(*sid).to_string(),
                                Term::Integer(n) => n.to_string(),
                                Term::Decimal(d) => {
                                    if d.scale() == 0 {
                                        format!("{}", d.mantissa())
                                    } else {
                                        format!("decimal({},{})", d.mantissa(), d.scale())
                                    }
                                }
                                Term::Float(f) => format!("float({f})"),
                            };
                        }
                    }
                }
                name.to_string()
            } else {
                match t {
                    Term::Integer(n) => n.to_string(),
                    Term::Decimal(d) => {
                        if d.scale() == 0 {
                            format!("{}", d.mantissa())
                        } else {
                            format!("decimal({},{})", d.mantissa(), d.scale())
                        }
                    }
                    Term::Float(f) => format!("float({f})"),
                    _ => unreachable!(),
                }
            }
        })
        .collect();
    NormalizedLiteral {
        name: lit.name().to_string(),
        negation: lit.negation,
        args,
    }
}

// ---------------------------------------------------------------------------
// Oracle invocation
// ---------------------------------------------------------------------------

fn find_oracle_binary() -> Option<std::path::PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let lean_dir = std::path::Path::new(manifest_dir)
        .parent()? // crates/
        .parent()? // project root
        .join("lean");

    let oracle_path = lean_dir.join(".lake/build/bin/GroundingOracle");
    if oracle_path.exists() {
        return Some(oracle_path);
    }

    // Try building it
    let status = Command::new("lake")
        .arg("build")
        .arg("GroundingOracle")
        .current_dir(&lean_dir)
        .status()
        .ok()?;

    if status.success() && oracle_path.exists() {
        Some(oracle_path)
    } else {
        None
    }
}

fn call_oracle(input_json: &str, oracle_path: &std::path::Path) -> Option<JValue> {
    let output = Command::new(oracle_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()
        .and_then(|mut child| {
            use std::io::Write;
            child
                .stdin
                .as_mut()?
                .write_all(input_json.as_bytes())
                .ok()?;
            child.wait_with_output().ok()
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("Oracle stderr: {stderr}");
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).ok()
}

fn call_oracle_batch(cases: &[JValue], oracle_path: &std::path::Path) -> Option<Vec<JValue>> {
    let batch = json!({"cases": cases});
    let input = serde_json::to_string(&batch).ok()?;
    let output = call_oracle(&input, oracle_path)?;
    let results = output.get("results")?.as_array()?.clone();
    Some(results)
}

// ---------------------------------------------------------------------------
// Proptest generators
// ---------------------------------------------------------------------------

const PRED_NAMES: &[&str] = &["bird", "flies", "penguin", "parent", "mortal", "human"];
const CONSTANTS: &[&str] = &["alice", "bob", "carol", "dave"];
const VAR_NAMES: &[&str] = &["?x", "?y", "?z"];

fn arb_constant() -> impl Strategy<Value = String> {
    proptest::sample::select(CONSTANTS).prop_map(String::from)
}

fn arb_domain() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::hash_set(arb_constant(), 1..=4)
        .prop_map(|s| s.into_iter().collect::<Vec<_>>())
}

/// Generate a single rule with variables and a domain for testing.
fn arb_grounding_case() -> impl Strategy<Value = (Rule, Vec<String>)> {
    (
        // Head predicate name
        proptest::sample::select(PRED_NAMES).prop_map(String::from),
        // Body predicate name
        proptest::sample::select(PRED_NAMES).prop_map(String::from),
        // Variables used in head (1-2)
        proptest::collection::vec(
            proptest::sample::select(VAR_NAMES).prop_map(String::from),
            1..=2,
        ),
        // Variables used in body (1-2)
        proptest::collection::vec(
            proptest::sample::select(VAR_NAMES).prop_map(String::from),
            1..=2,
        ),
        // Domain
        arb_domain(),
        // Whether head is negated
        proptest::bool::ANY,
    )
        .prop_filter(
            "head and body pred names must differ",
            |(h, b, _, _, _, _)| h != b,
        )
        .prop_map(
            |(head_name, body_name, head_vars, body_vars, domain, head_neg)| {
                let head_lit = Literal::new(
                    head_name.as_str(),
                    head_neg,
                    Mode::empty(),
                    Temporal::empty(),
                    head_vars,
                );
                let body_lit = Literal::new(
                    body_name.as_str(),
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    body_vars,
                );
                let rule = Rule::defeasible("r_test", vec![body_lit], head_lit);
                (rule, domain)
            },
        )
}

/// Generate a ground-only rule (no variables) for edge case testing.
fn arb_ground_rule_case() -> impl Strategy<Value = (Rule, Vec<String>)> {
    (
        proptest::sample::select(PRED_NAMES).prop_map(String::from),
        proptest::sample::select(PRED_NAMES).prop_map(String::from),
        proptest::collection::vec(arb_constant(), 1..=2),
        proptest::collection::vec(arb_constant(), 1..=2),
        arb_domain(),
    )
        .prop_filter("head and body pred names must differ", |(h, b, _, _, _)| {
            h != b
        })
        .prop_map(|(head_name, body_name, head_args, body_args, domain)| {
            let head_lit = Literal::new(
                head_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                head_args,
            );
            let body_lit = Literal::new(
                body_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                body_args,
            );
            let rule = Rule::defeasible("r_ground", vec![body_lit], head_lit);
            (rule, domain)
        })
}

/// Generate a multi-body rule with shared variables (join pattern).
fn arb_join_rule_case() -> impl Strategy<Value = (Rule, Vec<String>)> {
    (
        proptest::sample::select(PRED_NAMES).prop_map(String::from),
        proptest::sample::select(PRED_NAMES).prop_map(String::from),
        proptest::sample::select(PRED_NAMES).prop_map(String::from),
        arb_domain(),
    )
        .prop_filter("all pred names must differ", |(h, b1, b2, _)| {
            h != b1 && h != b2 && b1 != b2
        })
        .prop_map(|(head_name, body1_name, body2_name, domain)| {
            // head(?x, ?z) :- body1(?x, ?y), body2(?y, ?z)
            let head_lit = Literal::new(
                head_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string(), "?z".to_string()],
            );
            let body1 = Literal::new(
                body1_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string(), "?y".to_string()],
            );
            let body2 = Literal::new(
                body2_name.as_str(),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?y".to_string(), "?z".to_string()],
            );
            let rule = Rule::defeasible("r_join", vec![body1, body2], head_lit);
            (rule, domain)
        })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Smoke test: ground a simple rule with one variable over a small domain.
#[test]
#[ignore] // requires Lean oracle binary
fn smoke_test_lean_grounding_oracle() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "Skipping: Lean GroundingOracle not built. Run `lake build GroundingOracle` in lean/"
            );
            return;
        }
    };

    // Rule: flies(?x) :- bird(?x)
    // Domain: [alice, bob]
    let input = json!({
        "rules": [{
            "head": {"name": "flies", "negation": false, "args": [{"variable": "?x"}]},
            "body": [{"name": "bird", "negation": false, "args": [{"variable": "?x"}]}]
        }],
        "domain": [{"symbol": "alice"}, {"symbol": "bob"}]
    });

    let result = call_oracle(&serde_json::to_string(&input).unwrap(), &oracle_path);
    let result = result.expect("oracle should produce output");

    let results = result
        .get("results")
        .and_then(|v| v.as_array())
        .expect("should have results array");
    assert_eq!(results.len(), 1, "should have 1 rule result");

    let instances = results[0]
        .get("ground_instances")
        .and_then(|v| v.as_array())
        .expect("should have ground_instances");
    assert_eq!(
        instances.len(),
        2,
        "should have 2 ground instances for domain [alice, bob]"
    );

    // Verify the instances contain alice and bob
    let instance_set: BTreeSet<NormalizedRule> =
        instances.iter().map(normalize_rule_json).collect();

    let expected_alice = NormalizedRule {
        head: NormalizedLiteral {
            name: "flies".into(),
            negation: false,
            args: vec!["alice".into()],
        },
        body: vec![NormalizedLiteral {
            name: "bird".into(),
            negation: false,
            args: vec!["alice".into()],
        }],
    };
    let expected_bob = NormalizedRule {
        head: NormalizedLiteral {
            name: "flies".into(),
            negation: false,
            args: vec!["bob".into()],
        },
        body: vec![NormalizedLiteral {
            name: "bird".into(),
            negation: false,
            args: vec!["bob".into()],
        }],
    };

    assert!(
        instance_set.contains(&expected_alice),
        "should contain flies(alice) :- bird(alice)"
    );
    assert!(
        instance_set.contains(&expected_bob),
        "should contain flies(bob) :- bird(bob)"
    );
}

/// Smoke test: ground rule with no variables returns itself.
#[test]
#[ignore] // requires Lean oracle binary
fn smoke_test_ground_rule_unchanged() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: Lean GroundingOracle not built.");
            return;
        }
    };

    let input = json!({
        "rules": [{
            "head": {"name": "flies", "negation": false, "args": [{"symbol": "alice"}]},
            "body": [{"name": "bird", "negation": false, "args": [{"symbol": "alice"}]}]
        }],
        "domain": [{"symbol": "alice"}, {"symbol": "bob"}]
    });

    let result = call_oracle(&serde_json::to_string(&input).unwrap(), &oracle_path)
        .expect("oracle should produce output");

    let instances = result
        .get("results")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.get("ground_instances"))
        .and_then(|v| v.as_array())
        .expect("should have ground_instances");

    // Ground rule with no variables: single instance = itself
    assert_eq!(instances.len(), 1, "ground rule should produce 1 instance");
    let inst = normalize_rule_json(&instances[0]);
    assert_eq!(inst.head.name, "flies");
    assert_eq!(inst.head.args, vec!["alice"]);
}

/// Property test: Rust and Lean agree on ground instances for random rules.
#[test]
#[ignore] // requires Lean oracle binary
fn proptest_rust_lean_grounding_agreement() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: Lean GroundingOracle not built.");
            return;
        }
    };

    let config = ProptestConfig::with_cases(50);
    let mut runner = proptest::test_runner::TestRunner::new(config);

    let strategy = proptest::collection::vec(arb_grounding_case(), 20);
    let cases_vec = strategy.new_tree(&mut runner).unwrap().current();

    let mut oracle_cases = Vec::new();
    let mut rust_results = Vec::new();

    for (rule, domain) in &cases_vec {
        let domain_terms: Vec<Term> = domain.iter().map(|s| Term::Symbol(intern(s))).collect();

        // Compute Rust ground instances
        let rust_instances = rust_ground_instances(rule, &domain_terms);
        rust_results.push(rust_instances);

        // Build oracle input
        let domain_json: Vec<JValue> = domain.iter().map(|s| json!({"symbol": s})).collect();
        let case = json!({
            "rules": [rule_to_json(rule)],
            "domain": domain_json
        });
        oracle_cases.push(case);
    }

    // Call oracle in batch
    let lean_results =
        call_oracle_batch(&oracle_cases, &oracle_path).expect("oracle batch should succeed");

    assert_eq!(
        rust_results.len(),
        lean_results.len(),
        "result count mismatch"
    );

    let mut agreements = 0;
    let mut disagreements = Vec::new();

    for (i, (rust_instances, lean_json)) in rust_results.iter().zip(lean_results.iter()).enumerate()
    {
        // Parse Lean results
        let lean_instances_json = lean_json
            .get("results")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.get("ground_instances"))
            .and_then(|v| v.as_array());

        let lean_instances: BTreeSet<NormalizedRule> = match lean_instances_json {
            Some(arr) => arr.iter().map(normalize_rule_json).collect(),
            None => {
                disagreements.push(format!(
                    "Case {i}: Lean returned no ground_instances. Response: {lean_json}"
                ));
                continue;
            }
        };

        let rust_set: BTreeSet<NormalizedRule> = rust_instances.iter().cloned().collect();

        if rust_set == lean_set(&lean_instances) {
            agreements += 1;
        } else {
            let only_rust: Vec<_> = rust_set.difference(&lean_instances).collect();
            let only_lean: Vec<_> = lean_instances.difference(&rust_set).collect();
            disagreements.push(format!(
                "Case {}: rust={} lean={} only_rust={} only_lean={}",
                i,
                rust_set.len(),
                lean_instances.len(),
                only_rust.len(),
                only_lean.len(),
            ));
        }
    }

    eprintln!(
        "Grounding oracle: {} agreements, {} disagreements out of {} cases",
        agreements,
        disagreements.len(),
        rust_results.len()
    );

    if !disagreements.is_empty() {
        for d in &disagreements[..disagreements.len().min(10)] {
            eprintln!("  DISAGREE: {d}");
        }
        panic!(
            "{} disagreements found (showing first {})",
            disagreements.len(),
            disagreements.len().min(10)
        );
    }
}

// Helper to avoid type issues
fn lean_set(s: &BTreeSet<NormalizedRule>) -> BTreeSet<NormalizedRule> {
    s.clone()
}

/// Property test: ground rules produce exactly one instance (themselves).
#[test]
#[ignore] // requires Lean oracle binary
fn proptest_ground_rules_single_instance() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: Lean GroundingOracle not built.");
            return;
        }
    };

    let config = ProptestConfig::with_cases(30);
    let mut runner = proptest::test_runner::TestRunner::new(config);

    let strategy = proptest::collection::vec(arb_ground_rule_case(), 10);
    let cases_vec = strategy.new_tree(&mut runner).unwrap().current();

    let mut oracle_cases = Vec::new();

    for (rule, domain) in &cases_vec {
        let domain_json: Vec<JValue> = domain.iter().map(|s| json!({"symbol": s})).collect();
        let case = json!({
            "rules": [rule_to_json(rule)],
            "domain": domain_json
        });
        oracle_cases.push(case);
    }

    let lean_results =
        call_oracle_batch(&oracle_cases, &oracle_path).expect("oracle batch should succeed");

    for (i, lean_json) in lean_results.iter().enumerate() {
        let instances = lean_json
            .get("results")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.get("ground_instances"))
            .and_then(|v| v.as_array())
            .expect("should have ground_instances");

        assert_eq!(
            instances.len(),
            1,
            "Case {}: ground rule should produce exactly 1 instance, got {}",
            i,
            instances.len()
        );
    }
}

/// Property test: join rules with shared variables produce correct instance count.
#[test]
#[ignore] // requires Lean oracle binary
fn proptest_join_rule_grounding() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: Lean GroundingOracle not built.");
            return;
        }
    };

    let config = ProptestConfig::with_cases(30);
    let mut runner = proptest::test_runner::TestRunner::new(config);

    let strategy = proptest::collection::vec(arb_join_rule_case(), 10);
    let cases_vec = strategy.new_tree(&mut runner).unwrap().current();

    let mut oracle_cases = Vec::new();
    let mut rust_results = Vec::new();

    for (rule, domain) in &cases_vec {
        let domain_terms: Vec<Term> = domain.iter().map(|s| Term::Symbol(intern(s))).collect();

        let rust_instances = rust_ground_instances(rule, &domain_terms);
        rust_results.push(rust_instances);

        let domain_json: Vec<JValue> = domain.iter().map(|s| json!({"symbol": s})).collect();
        let case = json!({
            "rules": [rule_to_json(rule)],
            "domain": domain_json
        });
        oracle_cases.push(case);
    }

    let lean_results =
        call_oracle_batch(&oracle_cases, &oracle_path).expect("oracle batch should succeed");

    let mut disagreements = 0;
    for (i, (rust_instances, lean_json)) in rust_results.iter().zip(lean_results.iter()).enumerate()
    {
        let lean_instances = lean_json
            .get("results")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.get("ground_instances"))
            .and_then(|v| v.as_array())
            .expect("should have ground_instances");

        let rust_set: BTreeSet<NormalizedRule> = rust_instances.iter().cloned().collect();
        let lean_set: BTreeSet<NormalizedRule> =
            lean_instances.iter().map(normalize_rule_json).collect();

        if rust_set != lean_set {
            eprintln!(
                "Join case {}: rust={} lean={} (domain size={})",
                i,
                rust_set.len(),
                lean_set.len(),
                cases_vec[i].1.len(),
            );
            disagreements += 1;
        }
    }

    if disagreements > 0 {
        panic!("{disagreements} join grounding disagreements found");
    }
}

/// Property test: instance count equals domain^(num_unique_vars) for simple rules.
#[test]
#[ignore] // requires Lean oracle binary
fn proptest_instance_count_matches_domain_power() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: Lean GroundingOracle not built.");
            return;
        }
    };

    let config = ProptestConfig::with_cases(30);
    let mut runner = proptest::test_runner::TestRunner::new(config);

    // Generate rules with known variable counts
    let strategy = proptest::collection::vec(
        (
            proptest::sample::select(PRED_NAMES).prop_map(String::from),
            proptest::sample::select(PRED_NAMES).prop_map(String::from),
            // 1-3 unique variables
            proptest::collection::hash_set(
                proptest::sample::select(VAR_NAMES).prop_map(String::from),
                1..=3,
            ),
            arb_domain(),
        )
            .prop_filter("names differ", |(h, b, _, _)| h != b)
            .prop_map(|(head_name, body_name, vars_set, domain)| {
                let vars: Vec<String> = vars_set.into_iter().collect();
                let head_lit = Literal::new(
                    head_name.as_str(),
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    vars.clone(),
                );
                let body_lit = Literal::new(
                    body_name.as_str(),
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    vars.clone(),
                );
                let rule = Rule::defeasible("r_count", vec![body_lit], head_lit);
                let num_vars = vars.len();
                (rule, domain, num_vars)
            }),
        10,
    );

    let cases_vec = strategy.new_tree(&mut runner).unwrap().current();

    let mut oracle_cases = Vec::new();
    let mut expected_counts = Vec::new();

    for (rule, domain, num_vars) in &cases_vec {
        let expected = domain.len().pow(*num_vars as u32);
        expected_counts.push(expected);

        let domain_json: Vec<JValue> = domain.iter().map(|s| json!({"symbol": s})).collect();
        let case = json!({
            "rules": [rule_to_json(rule)],
            "domain": domain_json
        });
        oracle_cases.push(case);
    }

    let lean_results =
        call_oracle_batch(&oracle_cases, &oracle_path).expect("oracle batch should succeed");

    for (i, (expected, lean_json)) in expected_counts.iter().zip(lean_results.iter()).enumerate() {
        let instances = lean_json
            .get("results")
            .and_then(|v| v.as_array())
            .and_then(|a| a.first())
            .and_then(|v| v.get("ground_instances"))
            .and_then(|v| v.as_array())
            .expect("should have ground_instances");

        assert_eq!(
            instances.len(),
            *expected,
            "Case {}: expected {} instances (domain^vars = {}^{}), got {}",
            i,
            expected,
            cases_vec[i].1.len(),
            cases_vec[i].2,
            instances.len()
        );
    }
}
