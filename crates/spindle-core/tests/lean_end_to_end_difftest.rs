//! End-to-end differential testing: Rust pipeline vs Lean oracle.
//!
//! Takes non-ground theories (rules with variables + domain), runs the full
//! pipeline on both sides:
//!   Lean:  ground (Herbrand enumeration) → semi-naive forward chaining
//!   Rust:  prepare() (grounding) → reason_prepared() (forward chaining)
//!
//! Compares the set of derived facts (positive +D conclusions) between
//! the two implementations.
//!
//! Run with: `cargo test --test lean_end_to_end_difftest`
//! Requires: `lake build EndToEndOracle` in the `lean/` directory first.

use std::collections::BTreeSet;
use std::process::Command;

use proptest::prelude::*;
use proptest::strategy::ValueTree;
use serde_json::{Value as JValue, json};

use spindle_core::conclusion::ConclusionType;
use spindle_core::grounding;
use spindle_core::intern::resolve;
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::pipeline::PrepareOptions;
use spindle_core::reason;
use spindle_core::rule::Rule;
use spindle_core::temporal::Temporal;
use spindle_core::term::Term;
use spindle_core::theory::Theory;

// ---------------------------------------------------------------------------
// Normalized types for comparison
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NormalizedLiteral {
    name: String,
    negation: bool,
    args: Vec<String>,
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
    let negation = j
        .get("negation")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
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
// Oracle invocation
// ---------------------------------------------------------------------------

fn find_oracle_binary() -> Option<std::path::PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let lean_dir = std::path::Path::new(manifest_dir)
        .parent()? // crates/
        .parent()? // project root
        .join("lean");

    let oracle_path = lean_dir.join(".lake/build/bin/EndToEndOracle");
    if oracle_path.exists() {
        return Some(oracle_path);
    }

    // Try building it
    let status = Command::new("lake")
        .arg("build")
        .arg("EndToEndOracle")
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
        eprintln!("Oracle stderr: {}", stderr);
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&stdout).ok()
}

fn call_oracle_batch(
    cases: &[JValue],
    oracle_path: &std::path::Path,
) -> Option<Vec<JValue>> {
    let batch = json!({"cases": cases});
    let input = serde_json::to_string(&batch).ok()?;
    let output = call_oracle(&input, oracle_path)?;
    let results = output.get("results")?.as_array()?.clone();
    Some(results)
}

// ---------------------------------------------------------------------------
// Rust-side pipeline: ground + reason
// ---------------------------------------------------------------------------

/// Run the full Rust pipeline on a theory: prepare (ground) + reason.
/// Returns the set of positive +D conclusions as NormalizedLiterals.
fn rust_pipeline_derive(theory: &Theory) -> BTreeSet<NormalizedLiteral> {
    let opts = PrepareOptions {
        grounding: spindle_core::pipeline::GroundingOptions {
            enabled: true,
            max_iterations: 100,
            max_instances: 10_000,
        },
        ..Default::default()
    };

    let prepared = spindle_core::pipeline::prepare(theory, opts)
        .expect("Rust pipeline prepare() failed");

    let conclusions = reason::reason_prepared(&prepared.theory)
        .expect("Rust reason_prepared() failed");

    // Only take +D conclusions (definite, positive) since Lean's semi-naive
    // is monotone forward chaining without defeasibility.
    conclusions
        .into_iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
        .map(|c| normalize_rust_literal(&c.literal))
        .collect()
}

// ---------------------------------------------------------------------------
// Theory builders (construct both Rust Theory and Lean JSON)
// ---------------------------------------------------------------------------

struct TestTheory {
    rust_theory: Theory,
    lean_json: JValue,
}

/// Serialize args to JSON term array.
fn args_to_json(args: &[String]) -> Vec<JValue> {
    args.iter()
        .map(|a| {
            if grounding::is_variable(a) {
                json!({"variable": a})
            } else {
                json!({"symbol": a})
            }
        })
        .collect()
}

/// Build a test theory with facts (as empty-body rules) and strict rules
/// with variables. Returns both the Rust Theory and the Lean JSON input.
fn build_theory(
    facts: &[(String, Vec<String>)],          // (name, args)
    rules: &[(String, Vec<String>, Vec<(String, Vec<String>)>)], // (head_name, head_args, body[(name, args)])
    domain: &[String],
) -> TestTheory {
    let mut theory = Theory::new();
    let mut lean_rules: Vec<JValue> = Vec::new();
    let mut label_counter = 0usize;

    // Add facts as strict rules with empty body
    for (name, args) in facts {
        label_counter += 1;
        let label = format!("f{label_counter}");
        let lit = Literal::new(
            name.as_str(),
            false,
            Mode::empty(),
            Temporal::empty(),
            args.clone(),
        );
        let rule = Rule::fact(label, lit);
        lean_rules.push(json!({
            "head": {
                "name": name,
                "negation": false,
                "args": args_to_json(args)
            },
            "body": []
        }));
        theory.add_rule(rule);
    }

    // Add strict rules (may have variables)
    for (head_name, head_args, body_parts) in rules {
        label_counter += 1;
        let label = format!("s{label_counter}");
        let head_lit = Literal::new(
            head_name.as_str(),
            false,
            Mode::empty(),
            Temporal::empty(),
            head_args.clone(),
        );
        let body_lits: Vec<Literal> = body_parts
            .iter()
            .map(|(bname, bargs)| {
                Literal::new(
                    bname.as_str(),
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    bargs.clone(),
                )
            })
            .collect();

        let lean_body: Vec<JValue> = body_parts
            .iter()
            .map(|(bname, bargs)| {
                json!({
                    "name": bname,
                    "negation": false,
                    "args": args_to_json(bargs)
                })
            })
            .collect();

        lean_rules.push(json!({
            "head": {
                "name": head_name,
                "negation": false,
                "args": args_to_json(head_args)
            },
            "body": lean_body
        }));

        let rule = Rule::strict(label, body_lits, head_lit);
        theory.add_rule(rule);
    }

    let lean_domain: Vec<JValue> = domain
        .iter()
        .map(|d| json!({"symbol": d}))
        .collect();

    let lean_json = json!({
        "rules": lean_rules,
        "domain": lean_domain,
        "fuel": 100
    });

    TestTheory {
        rust_theory: theory,
        lean_json,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Smoke test: simple chain with one variable.
/// bird(alice). bird(bob). flies(?x) :- bird(?x).
/// Expected derived: bird(alice), bird(bob), flies(alice), flies(bob)
#[test]
fn smoke_test_end_to_end() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "Skipping: Lean EndToEndOracle not built. \
                 Run `lake build EndToEndOracle` in lean/"
            );
            return;
        }
    };

    let tt = build_theory(
        &[
            ("bird".into(), vec!["alice".into()]),
            ("bird".into(), vec!["bob".into()]),
        ],
        &[(
            "flies".into(),
            vec!["?x".into()],
            vec![("bird".into(), vec!["?x".into()])],
        )],
        &["alice".into(), "bob".into()],
    );

    // Lean oracle
    let input = serde_json::to_string(&tt.lean_json).unwrap();
    let lean_result = call_oracle(&input, &oracle_path)
        .expect("Lean oracle call failed");

    let lean_facts: BTreeSet<NormalizedLiteral> = lean_result
        .get("derived_facts")
        .and_then(|v| v.as_array())
        .expect("No derived_facts in oracle response")
        .iter()
        .map(normalize_literal_json)
        .collect();

    // Rust pipeline
    let rust_facts = rust_pipeline_derive(&tt.rust_theory);

    assert_eq!(
        rust_facts, lean_facts,
        "End-to-end mismatch!\n  Rust: {:?}\n  Lean: {:?}",
        rust_facts, lean_facts
    );

    // Verify expected facts
    assert!(
        lean_facts.iter().any(|f| f.name == "flies" && f.args == vec!["alice"]),
        "Expected flies(alice) in derived facts"
    );
    assert!(
        lean_facts.iter().any(|f| f.name == "flies" && f.args == vec!["bob"]),
        "Expected flies(bob) in derived facts"
    );
}

/// Two-hop chain: fact → rule1 → rule2.
/// parent(alice, bob). parent(bob, carol).
/// ancestor(?x, ?y) :- parent(?x, ?y).
/// ancestor(?x, ?z) :- parent(?x, ?y), ancestor(?y, ?z).
#[test]
fn test_transitive_chain() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: EndToEndOracle not built");
            return;
        }
    };

    let tt = build_theory(
        &[
            ("parent".into(), vec!["alice".into(), "bob".into()]),
            ("parent".into(), vec!["bob".into(), "carol".into()]),
        ],
        &[
            // ancestor(?x, ?y) :- parent(?x, ?y)
            (
                "ancestor".into(),
                vec!["?x".into(), "?y".into()],
                vec![("parent".into(), vec!["?x".into(), "?y".into()])],
            ),
            // ancestor(?x, ?z) :- parent(?x, ?y), ancestor(?y, ?z)
            (
                "ancestor".into(),
                vec!["?x".into(), "?z".into()],
                vec![
                    ("parent".into(), vec!["?x".into(), "?y".into()]),
                    ("ancestor".into(), vec!["?y".into(), "?z".into()]),
                ],
            ),
        ],
        &["alice".into(), "bob".into(), "carol".into()],
    );

    let input = serde_json::to_string(&tt.lean_json).unwrap();
    let lean_result = call_oracle(&input, &oracle_path)
        .expect("Lean oracle call failed");

    let lean_facts: BTreeSet<NormalizedLiteral> = lean_result
        .get("derived_facts")
        .and_then(|v| v.as_array())
        .expect("No derived_facts")
        .iter()
        .map(normalize_literal_json)
        .collect();

    let rust_facts = rust_pipeline_derive(&tt.rust_theory);

    assert_eq!(
        rust_facts, lean_facts,
        "Transitive chain mismatch!\n  Rust: {:?}\n  Lean: {:?}",
        rust_facts, lean_facts
    );

    // Must derive ancestor(alice, carol) via transitive closure
    assert!(
        lean_facts.iter().any(|f| {
            f.name == "ancestor" && f.args == vec!["alice", "carol"]
        }),
        "Expected ancestor(alice, carol) via transitive closure"
    );
}

/// Ground-only theory (no variables): facts only.
#[test]
fn test_ground_facts_only() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: EndToEndOracle not built");
            return;
        }
    };

    let tt = build_theory(
        &[
            ("bird".into(), vec!["tweety".into()]),
            ("mammal".into(), vec!["fido".into()]),
        ],
        &[],
        &["tweety".into(), "fido".into()],
    );

    let input = serde_json::to_string(&tt.lean_json).unwrap();
    let lean_result = call_oracle(&input, &oracle_path)
        .expect("Lean oracle call failed");

    let lean_facts: BTreeSet<NormalizedLiteral> = lean_result
        .get("derived_facts")
        .and_then(|v| v.as_array())
        .expect("No derived_facts")
        .iter()
        .map(normalize_literal_json)
        .collect();

    let rust_facts = rust_pipeline_derive(&tt.rust_theory);

    assert_eq!(rust_facts, lean_facts);
    assert_eq!(lean_facts.len(), 2);
}

/// Ground facts with predicates (no variables).
#[test]
fn test_ground_strict_chain() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: EndToEndOracle not built");
            return;
        }
    };

    // a(alice). b(?x) :- a(?x). c(?x) :- b(?x).
    let tt = build_theory(
        &[("a".into(), vec!["alice".into()])],
        &[
            (
                "b".into(),
                vec!["?x".into()],
                vec![("a".into(), vec!["?x".into()])],
            ),
            (
                "c".into(),
                vec!["?x".into()],
                vec![("b".into(), vec!["?x".into()])],
            ),
        ],
        &["alice".into(), "bob".into()],
    );

    let input = serde_json::to_string(&tt.lean_json).unwrap();
    let lean_result = call_oracle(&input, &oracle_path)
        .expect("Lean oracle call failed");

    let lean_facts: BTreeSet<NormalizedLiteral> = lean_result
        .get("derived_facts")
        .and_then(|v| v.as_array())
        .expect("No derived_facts")
        .iter()
        .map(normalize_literal_json)
        .collect();

    let rust_facts = rust_pipeline_derive(&tt.rust_theory);

    assert_eq!(
        rust_facts, lean_facts,
        "Ground strict chain mismatch!\n  Rust: {:?}\n  Lean: {:?}",
        rust_facts, lean_facts
    );

    // c(alice) should be derived through the chain
    assert!(
        lean_facts.iter().any(|f| f.name == "c" && f.args == vec!["alice"]),
        "Expected c(alice)"
    );
}

/// Multi-predicate join: head depends on two different body predicates.
#[test]
fn test_join_rule() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: EndToEndOracle not built");
            return;
        }
    };

    // human(alice). human(bob). mortal(?x) :- human(?x).
    // wise(alice). scholar(?x) :- human(?x), wise(?x).
    let tt = build_theory(
        &[
            ("human".into(), vec!["alice".into()]),
            ("human".into(), vec!["bob".into()]),
            ("wise".into(), vec!["alice".into()]),
        ],
        &[
            (
                "mortal".into(),
                vec!["?x".into()],
                vec![("human".into(), vec!["?x".into()])],
            ),
            (
                "scholar".into(),
                vec!["?x".into()],
                vec![
                    ("human".into(), vec!["?x".into()]),
                    ("wise".into(), vec!["?x".into()]),
                ],
            ),
        ],
        &["alice".into(), "bob".into()],
    );

    let input = serde_json::to_string(&tt.lean_json).unwrap();
    let lean_result = call_oracle(&input, &oracle_path)
        .expect("Lean oracle call failed");

    let lean_facts: BTreeSet<NormalizedLiteral> = lean_result
        .get("derived_facts")
        .and_then(|v| v.as_array())
        .expect("No derived_facts")
        .iter()
        .map(normalize_literal_json)
        .collect();

    let rust_facts = rust_pipeline_derive(&tt.rust_theory);

    assert_eq!(
        rust_facts, lean_facts,
        "Join rule mismatch!\n  Rust: {:?}\n  Lean: {:?}",
        rust_facts, lean_facts
    );

    // scholar(alice) should be derived (human + wise), not scholar(bob) (no wise(bob))
    assert!(
        lean_facts.iter().any(|f| f.name == "scholar" && f.args == vec!["alice"]),
        "Expected scholar(alice)"
    );
    assert!(
        !lean_facts.iter().any(|f| f.name == "scholar" && f.args == vec!["bob"]),
        "Unexpected scholar(bob)"
    );
}

// ---------------------------------------------------------------------------
// Proptest: random theory generation
// ---------------------------------------------------------------------------

const PRED_NAMES: &[&str] = &["bird", "flies", "penguin", "parent", "mortal", "human"];
const CONSTANTS: &[&str] = &["alice", "bob", "carol"];
const VAR_NAMES: &[&str] = &["?x", "?y", "?z"];

fn arb_constant() -> impl Strategy<Value = String> {
    proptest::sample::select(CONSTANTS).prop_map(String::from)
}

fn arb_domain() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::hash_set(arb_constant(), 1..=3)
        .prop_map(|s| s.into_iter().collect::<Vec<_>>())
}

/// Generate a random end-to-end test case: facts + one rule with variables.
/// Ensures rule safety: all head variables must also appear in the body.
fn arb_e2e_case() -> impl Strategy<
    Value = (
        Vec<(String, Vec<String>)>,           // facts
        Vec<(String, Vec<String>, Vec<(String, Vec<String>)>)>, // rules
        Vec<String>,                          // domain
    ),
> {
    (
        // Facts: 1-3 facts with 1 constant arg
        proptest::collection::vec(
            (
                proptest::sample::select(&PRED_NAMES[..3]).prop_map(String::from),
                proptest::collection::vec(arb_constant(), 1..=1),
            ),
            1..=3,
        ),
        // Shared variables used in both head and body (safety condition)
        proptest::collection::vec(
            proptest::sample::select(VAR_NAMES).prop_map(String::from),
            1..=2,
        ),
        // Rule predicate names
        (
            proptest::sample::select(&PRED_NAMES[3..]).prop_map(String::from),
            proptest::sample::select(&PRED_NAMES[..3]).prop_map(String::from),
        ),
        arb_domain(),
    )
        .prop_map(|(facts, vars, (head_name, body_name), mut domain)| {
            // Head and body share the same variables (ensures safety)
            let rules = vec![(head_name, vars.clone(), vec![(body_name, vars)])];
            // Ensure domain contains all constants from facts, so Lean's
            // Herbrand enumeration can ground variables to those values.
            for (_, args) in &facts {
                for a in args {
                    if !domain.contains(a) {
                        domain.push(a.clone());
                    }
                }
            }
            (facts, rules, domain)
        })
}

/// Proptest: random theories produce matching results.
#[test]
fn proptest_end_to_end_match() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "Skipping: EndToEndOracle not built. \
                 Run `lake build EndToEndOracle` in lean/"
            );
            return;
        }
    };

    let mut runner = proptest::test_runner::TestRunner::new(
        proptest::test_runner::Config {
            cases: 50,
            ..Default::default()
        },
    );

    // Collect cases for batch mode
    let mut test_cases: Vec<(
        Theory,
        JValue,
    )> = Vec::new();

    for _ in 0..50 {
        let strategy = arb_e2e_case();
        let tree = strategy
            .new_tree(&mut runner)
            .expect("Failed to create value tree");
        let (facts, rules, domain) = tree.current();

        let tt = build_theory(&facts, &rules, &domain);
        test_cases.push((tt.rust_theory, tt.lean_json));
    }

    // Call oracle in batch mode
    let lean_inputs: Vec<JValue> = test_cases.iter().map(|(_, j)| j.clone()).collect();
    let lean_results = call_oracle_batch(&lean_inputs, &oracle_path)
        .expect("Batch oracle call failed");

    assert_eq!(lean_results.len(), test_cases.len());

    let mut mismatches = 0;
    for (i, ((theory, _), lean_result)) in
        test_cases.iter().zip(lean_results.iter()).enumerate()
    {
        let lean_facts: BTreeSet<NormalizedLiteral> = lean_result
            .get("derived_facts")
            .and_then(|v| v.as_array())
            .unwrap_or(&vec![])
            .iter()
            .map(normalize_literal_json)
            .collect();

        let rust_facts = rust_pipeline_derive(theory);

        if rust_facts != lean_facts {
            mismatches += 1;
            eprintln!(
                "Case {i}: MISMATCH\n  Rust ({} facts): {:?}\n  Lean ({} facts): {:?}",
                rust_facts.len(),
                rust_facts,
                lean_facts.len(),
                lean_facts
            );
        }
    }

    assert_eq!(
        mismatches, 0,
        "{mismatches} out of {} cases had mismatches",
        test_cases.len()
    );
}
