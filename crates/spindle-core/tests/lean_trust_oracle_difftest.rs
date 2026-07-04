//! Differential test: Rust trust module vs the verified Lean model.
//!
//! The Lean model (`lean/Spindle/Trust/`) formalizes the diminishment
//! operator, weakest-link propagation, linear/step decay, and thresholds
//! over exact rationals, with the operator properties proven
//! (see `lean/PROOFS.md`). This test drives the `TrustOracle` executable
//! over exhaustive grids and random structures and compares against
//! `spindle_core::trust` (f64) within a small tolerance.
//!
//! Exponential decay is excluded here: it is irrational (0.5^(age/h)) and
//! covered by the abstract `DecayLaw` interface in Lean plus the Rust unit
//! tests.
//!
//! Run with:
//!   cargo test --test lean_trust_oracle_difftest -- --ignored --nocapture
//! Requires `lake build TrustOracle` in `lean/` first.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value as JValue, json};

use spindle_core::trust::{DecayModel, DiminisherInfo, TrustDerivationNode, TrustPolicy};

const TOLERANCE: f64 = 1e-9;

// ---------------------------------------------------------------------------
// Oracle plumbing
// ---------------------------------------------------------------------------

fn find_trust_oracle() -> Option<std::path::PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let lean_dir = std::path::Path::new(manifest_dir)
        .parent()?
        .parent()?
        .join("lean");
    let oracle = lean_dir.join(".lake/build/bin/TrustOracle");
    if oracle.exists() {
        return Some(oracle);
    }
    let status = Command::new("lake")
        .arg("build")
        .arg("TrustOracle")
        .current_dir(&lean_dir)
        .status()
        .ok()?;
    if status.success() && oracle.exists() {
        Some(oracle)
    } else {
        None
    }
}

fn oracle_batch(oracle: &std::path::Path, lines: &[String]) -> Vec<JValue> {
    let mut child = Command::new(oracle)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn TrustOracle");

    let input = lines.join("\n");
    let mut stdin = child.stdin.take().expect("no stdin");
    let writer = std::thread::spawn(move || {
        stdin.write_all(input.as_bytes()).expect("stdin write");
        stdin.write_all(b"\n").expect("stdin write");
    });

    let output = child.wait_with_output().expect("oracle wait");
    writer.join().expect("stdin writer thread");
    assert!(
        output.status.success(),
        "oracle exited nonzero: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("oracle stdout utf8");
    let results: Vec<JValue> = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("oracle output line is JSON"))
        .collect();
    assert_eq!(results.len(), lines.len(), "oracle result count mismatch");
    results
}

/// Parse an exact-rational oracle response into f64.
fn rat_to_f64(v: &JValue) -> f64 {
    if let Some(err) = v.get("error") {
        panic!("oracle error: {err}");
    }
    let num = v["num"].as_f64().expect("num");
    let den = v["den"].as_f64().expect("den");
    num / den
}

fn assert_close(rust: f64, lean: f64, context: &str) {
    assert!(
        (rust - lean).abs() <= TOLERANCE,
        "{context}: Rust {rust} vs Lean {lean} (diff {})",
        (rust - lean).abs()
    );
}

// ---------------------------------------------------------------------------
// Grids
// ---------------------------------------------------------------------------

/// Unit-interval grid: i/16 for i in 0..=16 (exactly representable in f64).
fn unit_grid() -> Vec<f64> {
    (0..=16).map(|i| i as f64 / 16.0).collect()
}

fn rust_diminish(c: f64, d: f64) -> f64 {
    DiminisherInfo::new("x", d, c).resulting_degree()
}

#[test]
#[ignore] // requires Lean oracle binary; run with -- --ignored
fn exhaustive_trust_difftest() {
    let oracle = match find_trust_oracle() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: TrustOracle not built. Run `lake build TrustOracle` in lean/");
            return;
        }
    };

    let grid = unit_grid();
    let mut requests: Vec<String> = Vec::new();
    let mut expected: Vec<(f64, String)> = Vec::new();

    // --- diminish: full grid ---
    for &c in &grid {
        for &d in &grid {
            requests.push(json!({"op": "diminish", "c": c, "d": d}).to_string());
            expected.push((rust_diminish(c, d), format!("diminish({c},{d})")));
        }
    }

    // --- diminish_all: sequential folds over the grid ---
    for &c in &grid {
        for &d1 in &[0.0, 0.25, 0.5, 1.0] {
            for &d2 in &[0.125, 0.5, 0.9375] {
                let ds = vec![d1, d2];
                let folded = ds.iter().fold(c, |acc, &d| rust_diminish(acc, d));
                requests.push(json!({"op": "diminish_all", "c": c, "ds": ds}).to_string());
                expected.push((folded, format!("diminish_all({c},[{d1},{d2}])")));
            }
        }
    }

    // --- weakest_link: enumerated small trees ---
    for &t in &grid {
        for &c1 in &[0.0, 0.5, 1.0] {
            for &c2 in &[0.25, 0.9375] {
                let tree_rust =
                    TrustDerivationNode::new(spindle_core::literal::Literal::simple("g"), t)
                        .with_children(vec![
                            TrustDerivationNode::new(
                                spindle_core::literal::Literal::simple("a"),
                                c1,
                            ),
                            TrustDerivationNode::new(
                                spindle_core::literal::Literal::simple("b"),
                                c2,
                            )
                            .with_children(vec![
                                TrustDerivationNode::new(
                                    spindle_core::literal::Literal::simple("c"),
                                    0.75,
                                ),
                            ]),
                        ]);
                requests.push(
                    json!({"op": "weakest_link", "tree": {
                        "trust": t,
                        "children": [
                            {"trust": c1},
                            {"trust": c2, "children": [{"trust": 0.75}]}
                        ]
                    }})
                    .to_string(),
                );
                expected.push((
                    tree_rust.weakest_link_trust(),
                    format!("weakest_link(t={t},c1={c1},c2={c2})"),
                ));
            }
        }
    }

    // --- linear decay: rates x ages, including negative age and clamping ---
    let ages = [-10.0, 0.0, 1.0, 100.0, 10_000.0];
    for &rate in &[0.0, 0.0001, 0.001, 0.5] {
        for &age in &ages {
            let model = DecayModel::Linear { rate_per_sec: rate };
            requests.push(json!({"op": "linear_decay", "rate": rate, "age": age}).to_string());
            expected.push((model.apply(age), format!("linear_decay({rate},{age})")));
        }
    }

    // --- step decay ---
    for &cutoff in &[0.0, 100.0, 86_400.0] {
        for &age in &ages {
            let model = DecayModel::StepFunction {
                cutoff_secs: cutoff,
            };
            requests.push(json!({"op": "step_decay", "cutoff": cutoff, "age": age}).to_string());
            expected.push((model.apply(age), format!("step_decay({cutoff},{age})")));
        }
    }

    // --- effective trust through a policy (linear + step) ---
    for &base in &[0.0, 0.5, 0.9375, 1.0] {
        for &age in &ages {
            let policy = TrustPolicy::new(0.5).with_trust("s", base).with_decay(
                "s",
                DecayModel::Linear {
                    rate_per_sec: 0.001,
                },
            );
            requests.push(
                json!({"op": "effective_linear", "base": base, "rate": 0.001, "age": age})
                    .to_string(),
            );
            expected.push((
                policy.get_effective_trust("s", age),
                format!("effective_linear({base},{age})"),
            ));

            let policy2 = TrustPolicy::new(0.5)
                .with_trust("s", base)
                .with_decay("s", DecayModel::StepFunction { cutoff_secs: 100.0 });
            requests.push(
                json!({"op": "effective_step", "base": base, "cutoff": 100.0, "age": age})
                    .to_string(),
            );
            expected.push((
                policy2.get_effective_trust("s", age),
                format!("effective_step({base},{age})"),
            ));
        }
    }

    // --- run the numeric batch ---
    let results = oracle_batch(&oracle, &requests);
    for ((exp, ctx), result) in expected.iter().zip(results.iter()) {
        assert_close(*exp, rat_to_f64(result), ctx);
    }
    let numeric_cases = requests.len();

    // --- thresholds (boolean results) ---
    let mut trequests: Vec<String> = Vec::new();
    let mut texpected: Vec<(bool, String)> = Vec::new();
    for &v in &grid {
        for &t in &grid {
            let policy = TrustPolicy::new(0.5).with_threshold("th", t);
            trequests.push(json!({"op": "threshold", "v": v, "t": t}).to_string());
            texpected.push((
                policy.is_above_threshold(v, "th").unwrap(),
                format!("threshold({v},{t})"),
            ));
        }
    }
    let tresults = oracle_batch(&oracle, &trequests);
    for ((exp, ctx), result) in texpected.iter().zip(tresults.iter()) {
        let lean = result["bool"]
            .as_bool()
            .unwrap_or_else(|| panic!("bool response: {result}"));
        assert_eq!(*exp, lean, "{ctx}");
    }

    eprintln!(
        "Trust difftest: {} numeric + {} threshold cases, all matching within {TOLERANCE}",
        numeric_cases,
        trequests.len()
    );
}
