//! SPEC-024 REQ-013 / NFR-003 anchored to the Lean oracle.
//!
//! `vocabulary_derivation.rs::test_013` already checks that adding declarations
//! and predicate metadata leaves the *Rust* conclusion set unchanged. This test
//! is stronger: it takes the formally verified Lean oracle's derived facts as
//! the reference, then asserts the Rust reasoner produces a byte-identical
//! conclusion set when the *same* argument-bearing theory is augmented with
//! (a) valid predicate declarations (one per observed symbol) plus descriptions,
//! and (b) an additional *conflicting* declaration — neither of which the
//! reasoner is permitted to consult.
//!
//! Run with: `cargo test --test lean_predicate_metadata_difftest -- --ignored`
//! Requires: `lake build EndToEndOracle` in the `lean/` directory first.

use std::collections::BTreeSet;
use std::process::Command;

use proptest::prelude::*;
use proptest::strategy::ValueTree;
use serde_json::{Value as JValue, json};

use spindle_core::MetaValue;
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
use spindle_core::vocabulary::{
    ArgumentDecl, DeclarationOrigin, MetaTarget, PredicateDeclaration, PredicateSignature,
    PrimitiveSort, TheorySignature,
};

// ---------------------------------------------------------------------------
// Normalized comparison types (shared shape with the end-to-end difftest)
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
        .parent()?
        .parent()?
        .join("lean");

    let oracle_path = lean_dir.join(".lake/build/bin/EndToEndOracle");
    if oracle_path.exists() {
        return Some(oracle_path);
    }
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
        eprintln!("Oracle stderr: {}", String::from_utf8_lossy(&output.stderr));
        return None;
    }
    serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).ok()
}

fn call_oracle_batch(cases: &[JValue], oracle_path: &std::path::Path) -> Option<Vec<JValue>> {
    let batch = json!({ "cases": cases });
    let output = call_oracle(&serde_json::to_string(&batch).ok()?, oracle_path)?;
    Some(output.get("results")?.as_array()?.clone())
}

fn lean_facts(result: &JValue) -> BTreeSet<NormalizedLiteral> {
    result
        .get("derived_facts")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().map(normalize_literal_json).collect())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Rust pipeline: ground + reason (definite, positive derived facts)
// ---------------------------------------------------------------------------

fn rust_pipeline_derive(theory: &Theory) -> BTreeSet<NormalizedLiteral> {
    let opts = PrepareOptions {
        grounding: spindle_core::pipeline::GroundingOptions {
            enabled: true,
            max_iterations: 100,
            max_instances: 10_000,
        },
        ..Default::default()
    };
    let prepared =
        spindle_core::pipeline::prepare(theory, opts).expect("Rust pipeline prepare() failed");
    let conclusions =
        reason::reason_prepared(&prepared.theory).expect("Rust reason_prepared() failed");
    conclusions
        .into_iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
        .map(|c| normalize_rust_literal(&c.literal))
        .collect()
}

// ---------------------------------------------------------------------------
// Theory builders
// ---------------------------------------------------------------------------

struct TestTheory {
    rust_theory: Theory,
    lean_json: JValue,
}

fn args_to_json(args: &[String]) -> Vec<JValue> {
    args.iter()
        .map(|a| {
            if grounding::is_variable(a) {
                json!({ "variable": a })
            } else {
                json!({ "symbol": a })
            }
        })
        .collect()
}

type FactSpec = (String, Vec<String>);
type RuleSpec = (String, Vec<String>, Vec<(String, Vec<String>)>);

fn build_theory(facts: &[FactSpec], rules: &[RuleSpec], domain: &[String]) -> TestTheory {
    let mut theory = Theory::new();
    let mut lean_rules: Vec<JValue> = Vec::new();
    let mut label_counter = 0usize;

    for (name, args) in facts {
        label_counter += 1;
        let lit = Literal::new(
            name.as_str(),
            false,
            Mode::empty(),
            Temporal::empty(),
            args.clone(),
        );
        lean_rules.push(json!({
            "head": { "name": name, "negation": false, "args": args_to_json(args) },
            "body": []
        }));
        theory.add_rule(Rule::fact(format!("f{label_counter}"), lit));
    }

    for (head_name, head_args, body_parts) in rules {
        label_counter += 1;
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
                json!({ "name": bname, "negation": false, "args": args_to_json(bargs) })
            })
            .collect();
        lean_rules.push(json!({
            "head": { "name": head_name, "negation": false, "args": args_to_json(head_args) },
            "body": lean_body
        }));
        theory.add_rule(Rule::strict(
            format!("s{label_counter}"),
            body_lits,
            head_lit,
        ));
    }

    let lean_domain: Vec<JValue> = domain.iter().map(|d| json!({ "symbol": d })).collect();
    let lean_json = json!({ "rules": lean_rules, "domain": lean_domain, "fuel": 100 });
    TestTheory {
        rust_theory: theory,
        lean_json,
    }
}

// ---------------------------------------------------------------------------
// SPEC-024 augmentations (must not change any conclusion — REQ-013)
// ---------------------------------------------------------------------------

/// Clone `theory` and attach a valid declaration (all args `any`) plus a
/// description for every predicate symbol it uses.
fn augment_with_valid_declarations(theory: &Theory) -> Theory {
    let mut augmented = theory.clone();
    for symbol in &TheorySignature::derive(theory).symbols {
        let args: Vec<ArgumentDecl> = (0..symbol.arity())
            .map(|i| ArgumentDecl::new(format!("a{i}"), PrimitiveSort::Any))
            .collect();
        let signature = PredicateSignature::try_new(*symbol, args).unwrap();
        augmented.add_predicate_declaration(PredicateDeclaration::new(
            signature,
            DeclarationOrigin::Programmatic,
        ));
        augmented.add_meta_target(
            MetaTarget::Predicate(*symbol),
            "description",
            MetaValue::String(format!("doc for {}", symbol.indicator())),
        );
    }
    augmented
}

/// Start from the valid augmentation, then add a second, incompatible
/// declaration for one symbol so its declaration state is a `Conflict`.
fn augment_with_conflicting_declarations(theory: &Theory) -> Theory {
    let mut augmented = augment_with_valid_declarations(theory);
    if let Some(symbol) = TheorySignature::derive(theory).symbols.iter().next() {
        let args: Vec<ArgumentDecl> = (0..symbol.arity())
            .map(|i| ArgumentDecl::new(format!("z{i}"), PrimitiveSort::Integer))
            .collect();
        let signature = PredicateSignature::try_new(*symbol, args).unwrap();
        augmented.add_predicate_declaration(PredicateDeclaration::new(
            signature,
            DeclarationOrigin::Programmatic,
        ));
    }
    augmented
}

/// Assert: Lean reference == Rust(base) == Rust(+valid decls) == Rust(+conflict).
fn assert_augmentation_parity(reference: &BTreeSet<NormalizedLiteral>, base: &Theory) {
    let plain = rust_pipeline_derive(base);
    let valid = rust_pipeline_derive(&augment_with_valid_declarations(base));
    let conflict = rust_pipeline_derive(&augment_with_conflicting_declarations(base));

    assert_eq!(&plain, reference, "Rust base != Lean oracle reference");
    assert_eq!(
        &valid, reference,
        "adding valid declarations + metadata changed conclusions vs the Lean oracle"
    );
    assert_eq!(
        &conflict, reference,
        "adding a conflicting declaration changed conclusions vs the Lean oracle"
    );
}

fn oracle_or_skip() -> Option<std::path::PathBuf> {
    match find_oracle_binary() {
        Some(p) => Some(p),
        None => {
            eprintln!(
                "Skipping: EndToEndOracle not built. Run `lake build EndToEndOracle` in lean/"
            );
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Deterministic cases
// ---------------------------------------------------------------------------

#[test]
#[ignore] // requires Lean oracle binary
fn augmentation_parity_transitive_chain() {
    let Some(oracle_path) = oracle_or_skip() else {
        return;
    };
    let tt = build_theory(
        &[
            ("parent".into(), vec!["alice".into(), "bob".into()]),
            ("parent".into(), vec!["bob".into(), "carol".into()]),
        ],
        &[
            (
                "ancestor".into(),
                vec!["?x".into(), "?y".into()],
                vec![("parent".into(), vec!["?x".into(), "?y".into()])],
            ),
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
    let reference = lean_facts(&call_oracle(&input, &oracle_path).expect("oracle call failed"));

    // Sanity: the reference is non-trivial and contains the transitive fact.
    assert!(
        reference
            .iter()
            .any(|f| f.name == "ancestor" && f.args == vec!["alice", "carol"])
    );
    assert_augmentation_parity(&reference, &tt.rust_theory);
}

#[test]
#[ignore] // requires Lean oracle binary
fn augmentation_parity_join_rule() {
    let Some(oracle_path) = oracle_or_skip() else {
        return;
    };
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
    let reference = lean_facts(&call_oracle(&input, &oracle_path).expect("oracle call failed"));
    assert!(
        reference
            .iter()
            .any(|f| f.name == "scholar" && f.args == vec!["alice"])
    );
    assert_augmentation_parity(&reference, &tt.rust_theory);
}

// ---------------------------------------------------------------------------
// Randomized cases (batch oracle)
// ---------------------------------------------------------------------------

const PRED_NAMES: &[&str] = &["bird", "flies", "penguin", "parent", "mortal", "human"];
const CONSTANTS: &[&str] = &["alice", "bob", "carol"];
const VAR_NAMES: &[&str] = &["?x", "?y", "?z"];

fn arb_constant() -> impl Strategy<Value = String> {
    proptest::sample::select(CONSTANTS).prop_map(String::from)
}

fn arb_domain() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::hash_set(arb_constant(), 1..=3).prop_map(|s| s.into_iter().collect())
}

fn arb_case() -> impl Strategy<Value = (Vec<FactSpec>, Vec<RuleSpec>, Vec<String>)> {
    (
        proptest::collection::vec(
            (
                proptest::sample::select(&PRED_NAMES[..3]).prop_map(String::from),
                proptest::collection::vec(arb_constant(), 1..=1),
            ),
            1..=3,
        ),
        proptest::collection::vec(
            proptest::sample::select(VAR_NAMES).prop_map(String::from),
            1..=2,
        ),
        (
            proptest::sample::select(&PRED_NAMES[3..]).prop_map(String::from),
            proptest::sample::select(&PRED_NAMES[..3]).prop_map(String::from),
        ),
        arb_domain(),
    )
        .prop_map(|(facts, vars, (head_name, body_name), mut domain)| {
            let rules = vec![(head_name, vars.clone(), vec![(body_name, vars)])];
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

#[test]
#[ignore] // requires Lean oracle binary
fn proptest_augmentation_parity() {
    let Some(oracle_path) = oracle_or_skip() else {
        return;
    };

    let mut runner = proptest::test_runner::TestRunner::new(proptest::test_runner::Config {
        cases: 50,
        ..Default::default()
    });

    let mut cases: Vec<Theory> = Vec::new();
    let mut lean_inputs: Vec<JValue> = Vec::new();
    for _ in 0..50 {
        let tree = arb_case().new_tree(&mut runner).expect("value tree");
        let (facts, rules, domain) = tree.current();
        let tt = build_theory(&facts, &rules, &domain);
        cases.push(tt.rust_theory);
        lean_inputs.push(tt.lean_json);
    }

    let results = call_oracle_batch(&lean_inputs, &oracle_path).expect("batch oracle call failed");
    assert_eq!(results.len(), cases.len());

    let mut mismatches = 0;
    for (i, (theory, result)) in cases.iter().zip(results.iter()).enumerate() {
        let reference = lean_facts(result);
        let plain = rust_pipeline_derive(theory);
        let valid = rust_pipeline_derive(&augment_with_valid_declarations(theory));
        let conflict = rust_pipeline_derive(&augment_with_conflicting_declarations(theory));

        if plain != reference || valid != reference || conflict != reference {
            mismatches += 1;
            eprintln!(
                "Case {i} MISMATCH\n  reference: {reference:?}\n  plain:    {plain:?}\n  valid:    {valid:?}\n  conflict: {conflict:?}"
            );
        }
    }
    assert_eq!(mismatches, 0, "{mismatches}/{} cases diverged", cases.len());
}
