//! Exhaustive small-scope differential test: Rust temporal-family
//! semantics vs the verified Lean family model (`SpindleLean.Family`).
//!
//! Enumerates every ground temporal theory over a small scope — one atom,
//! windows {none, [1,10], [20,30]}, bodies of at most 1 literal, up to
//! 2 rules with all superiority orientations (plus 3-rule theories without
//! superiority at `SPINDLE_EXHAUSTIVE=full`) — and compares all four
//! conclusion tags per exact literal.
//!
//! The family semantics under test (established by
//! `tests/family_probe.rs`, mirrored in `lean/SpindleLean/Family.lean`):
//! exact-identity conflict (same window required), family support for
//! atemporal bodies only, applied uniformly across definite/defeasible
//! phases and defeater bodies.
//!
//! Run with:
//!   cargo test --test lean_family_exhaustive_difftest -- --ignored --nocapture
//! Requires `lake build spindlelean` in `lean/` first.

use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value as JValue;

use spindle_core::conclusion::ConclusionType;
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::reason;
use spindle_core::rule::Rule;
use spindle_core::temporal::{Temporal, TimePoint};
use spindle_core::theory::Theory;

const ATOMS: &[&str] = &["p", "q"];
const WINDOWS: &[Option<(i64, i64)>] = &[None, Some((1, 10)), Some((20, 30))];

/// (atom index, negated, window index into WINDOWS)
type Lit = (u8, bool, u8);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum RType {
    Fact,
    Strict,
    Defeasible,
    Defeater,
}

impl RType {
    fn as_str(self) -> &'static str {
        match self {
            RType::Fact => "fact",
            RType::Strict => "strict",
            RType::Defeasible => "defeasible",
            RType::Defeater => "defeater",
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct Shape {
    rtype: RType,
    head: Lit,
    body: Vec<Lit>,
}

fn shapes_for(lits: &[Lit]) -> Vec<Shape> {
    let mut shapes = Vec::new();
    for &h in lits {
        shapes.push(Shape {
            rtype: RType::Fact,
            head: h,
            body: vec![],
        });
    }
    for rt in [RType::Strict, RType::Defeasible, RType::Defeater] {
        for &h in lits {
            shapes.push(Shape {
                rtype: rt,
                head: h,
                body: vec![],
            });
            for &b in lits {
                shapes.push(Shape {
                    rtype: rt,
                    head: h,
                    body: vec![b],
                });
            }
        }
    }
    shapes
}

/// Temporal universe: 1 atom x negation x 3 windows.
fn temporal_lits() -> Vec<Lit> {
    (0..WINDOWS.len() as u8)
        .flat_map(|w| [(0, false, w), (0, true, w)])
        .collect()
}

/// Propositional universe: 2 atoms x negation, windowless.
fn prop_lits() -> Vec<Lit> {
    (0..ATOMS.len() as u8)
        .flat_map(|a| [(a, false, 0), (a, true, 0)])
        .collect()
}

#[derive(Clone, Debug)]
struct Case {
    rules: Vec<Shape>,
    superiority: Vec<(usize, usize)>,
}

fn enumerate_cases(level: &str) -> Vec<Case> {
    let shapes = shapes_for(&temporal_lits());
    let n = shapes.len();
    let mut cases = Vec::new();

    for shape in &shapes {
        cases.push(Case {
            rules: vec![shape.clone()],
            superiority: vec![],
        });
    }
    for i in 0..n {
        for j in (i + 1)..n {
            let rules = vec![shapes[i].clone(), shapes[j].clone()];
            for sup in [vec![], vec![(0, 1)], vec![(1, 0)]] {
                cases.push(Case {
                    rules: rules.clone(),
                    superiority: sup,
                });
            }
        }
    }

    // 4-rule propositional tier: covers the constructive defeat-discard
    // class (minimal witness: => p, => ~p, p ~> ~q, => q), which has no
    // witness below 4 rules. Default uses a curated shape set (facts +
    // defeasible/defeater with bodies <= 1); full uses all 64 shapes.
    let prop_shapes_all = shapes_for(&prop_lits());
    let prop_shapes: Vec<Shape> = if level == "full" {
        prop_shapes_all.clone()
    } else {
        prop_shapes_all
            .iter()
            .filter(|s| matches!(s.rtype, RType::Fact | RType::Defeasible | RType::Defeater))
            .cloned()
            .collect()
    };
    let pn = prop_shapes.len();
    for i in 0..pn {
        for j in (i + 1)..pn {
            for k in (j + 1)..pn {
                for m in (k + 1)..pn {
                    cases.push(Case {
                        rules: vec![
                            prop_shapes[i].clone(),
                            prop_shapes[j].clone(),
                            prop_shapes[k].clone(),
                            prop_shapes[m].clone(),
                        ],
                        superiority: vec![],
                    });
                }
            }
        }
    }

    if level == "full" {
        for i in 0..n {
            for j in (i + 1)..n {
                for k in (j + 1)..n {
                    cases.push(Case {
                        rules: vec![shapes[i].clone(), shapes[j].clone(), shapes[k].clone()],
                        superiority: vec![],
                    });
                }
            }
        }
    }

    cases
}

// ---------------------------------------------------------------------------
// JSON construction (Lean.Data.Json parser on the oracle side is
// field-order tolerant)
// ---------------------------------------------------------------------------

fn lit_json(l: Lit) -> String {
    let name = ATOMS[l.0 as usize];
    match WINDOWS[l.2 as usize] {
        None => format!("{{\"name\":\"{name}\",\"negated\":{}}}", l.1),
        Some((s, e)) => format!(
            "{{\"name\":\"{name}\",\"negated\":{},\"window\":[{s},{e}]}}",
            l.1
        ),
    }
}

fn case_to_lean_json(case: &Case) -> String {
    let rules: Vec<String> = case
        .rules
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let body: Vec<String> = s.body.iter().map(|&b| lit_json(b)).collect();
            format!(
                "{{\"label\":\"r{}\",\"type\":\"{}\",\"body\":[{}],\"head\":{}}}",
                i,
                s.rtype.as_str(),
                body.join(","),
                lit_json(s.head)
            )
        })
        .collect();
    let sup: Vec<String> = case
        .superiority
        .iter()
        .map(|&(w, l)| format!("[\"r{w}\",\"r{l}\"]"))
        .collect();
    format!(
        "{{\"rules\":[{}],\"superiority\":[{}]}}",
        rules.join(","),
        sup.join(",")
    )
}

fn case_pretty(case: &Case) -> String {
    let mut out = String::new();
    let lit_str = |l: &Lit| {
        let w = match WINDOWS[l.2 as usize] {
            None => String::new(),
            Some((s, e)) => format!("[{s},{e}]"),
        };
        format!("{}{}{w}", if l.1 { "~" } else { "" }, ATOMS[l.0 as usize])
    };
    for (i, s) in case.rules.iter().enumerate() {
        let body: Vec<String> = s.body.iter().map(&lit_str).collect();
        let arrow = match s.rtype {
            RType::Fact => ">>",
            RType::Strict => "->",
            RType::Defeasible => "=>",
            RType::Defeater => "~>",
        };
        out.push_str(&format!(
            "  r{i}: {} {arrow} {}\n",
            body.join(", "),
            lit_str(&s.head)
        ));
    }
    for &(w, l) in &case.superiority {
        out.push_str(&format!("  r{w} > r{l}\n"));
    }
    out
}

// ---------------------------------------------------------------------------
// Rust side
// ---------------------------------------------------------------------------

fn lit_rust(l: Lit) -> Literal {
    let temporal = match WINDOWS[l.2 as usize] {
        Some((s, e)) => Temporal::from_bounds(s, e),
        None => Temporal::empty(),
    };
    Literal::new(
        ATOMS[l.0 as usize],
        l.1,
        Mode::empty(),
        temporal,
        Vec::<String>::new(),
    )
}

fn case_to_rust_theory(case: &Case) -> Theory {
    let mut theory = Theory::new();
    for (i, s) in case.rules.iter().enumerate() {
        let label = format!("r{i}");
        let head = lit_rust(s.head);
        let body: Vec<Literal> = s.body.iter().map(|&b| lit_rust(b)).collect();
        let rule = match s.rtype {
            RType::Fact => Rule::fact(label, head),
            RType::Strict => Rule::strict(label, body, head),
            RType::Defeasible => Rule::defeasible(label, body, head),
            RType::Defeater => Rule::defeater(label, body, head),
        };
        theory.add_rule(rule);
    }
    for &(w, l) in &case.superiority {
        theory.add_superiority(&format!("r{w}"), &format!("r{l}"));
    }
    theory
}

/// Comparison key: (name, negated, window as "s:e" or "").
type Key = (String, bool, String);

fn window_key_rust(lit: &Literal) -> String {
    let t = &lit.temporal;
    if t.is_empty() {
        String::new()
    } else {
        match (t.start, t.end) {
            (TimePoint::Moment(s), TimePoint::Moment(e)) => format!("{s}:{e}"),
            _ => format!("{:?}:{:?}", t.start, t.end),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Classified {
    plus_d_upper: BTreeSet<Key>,
    minus_d_upper: BTreeSet<Key>,
    plus_d_lower: BTreeSet<Key>,
    minus_d_lower: BTreeSet<Key>,
}

fn classify_rust(theory: &Theory) -> Result<Classified, String> {
    let conclusions = reason::reason(theory).map_err(|e| e.to_string())?;
    let mut c = Classified::default();
    for conc in conclusions {
        let key = (
            conc.literal.name().to_string(),
            conc.literal.negation,
            window_key_rust(&conc.literal),
        );
        match conc.conclusion_type {
            ConclusionType::DefinitelyProvable => {
                c.plus_d_upper.insert(key);
            }
            ConclusionType::DefinitelyNotProvable => {
                c.minus_d_upper.insert(key);
            }
            ConclusionType::DefeasiblyProvable => {
                c.plus_d_lower.insert(key);
            }
            ConclusionType::DefeasiblyNotProvable => {
                c.minus_d_lower.insert(key);
            }
        }
    }
    Ok(c)
}

fn classify_lean(result: &JValue) -> Classified {
    let mut c = Classified::default();
    let conclusions = result
        .get("conclusions")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("oracle result missing conclusions: {result}"));
    for conc in conclusions {
        let lit = &conc["literal"];
        let window = match lit.get("window") {
            Some(JValue::Array(arr)) if arr.len() == 2 => {
                format!(
                    "{}:{}",
                    arr[0].as_i64().unwrap_or(0),
                    arr[1].as_i64().unwrap_or(0)
                )
            }
            _ => String::new(),
        };
        let key = (
            lit["name"].as_str().unwrap_or("").to_string(),
            lit["negated"].as_bool().unwrap_or(false),
            window,
        );
        match conc["type"].as_str().unwrap_or("") {
            "+D" => {
                c.plus_d_upper.insert(key);
            }
            "-D" => {
                c.minus_d_upper.insert(key);
            }
            "+d" => {
                c.plus_d_lower.insert(key);
            }
            "-d" => {
                c.minus_d_lower.insert(key);
            }
            other => panic!("unknown conclusion type from oracle: {other}"),
        }
    }
    c
}

// ---------------------------------------------------------------------------
// Oracle plumbing
// ---------------------------------------------------------------------------

fn find_oracle() -> Option<std::path::PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let lean_dir = std::path::Path::new(manifest_dir)
        .parent()?
        .parent()?
        .join("lean");
    let oracle = lean_dir.join(".lake/build/bin/spindlelean");
    if oracle.exists() { Some(oracle) } else { None }
}

fn oracle_batch(oracle: &std::path::Path, lines: &[String]) -> Vec<JValue> {
    let mut child = Command::new(oracle)
        .arg("--oracle-family-batch")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn family oracle");

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

// ---------------------------------------------------------------------------
// The exhaustive test
// ---------------------------------------------------------------------------

const BATCH_SIZE: usize = 5000;
const MAX_REPORTED: usize = 20;

#[test]
#[ignore] // requires Lean oracle binary; run with -- --ignored
fn exhaustive_family_difftest() {
    let oracle = match find_oracle() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: spindlelean not built. Run `lake build spindlelean` in lean/");
            return;
        }
    };

    let level = std::env::var("SPINDLE_EXHAUSTIVE").unwrap_or_default();
    let cases = enumerate_cases(&level);
    eprintln!(
        "Enumerated {} temporal-family theories (level: {})",
        cases.len(),
        if level.is_empty() { "default" } else { &level }
    );

    let mut mismatches = 0usize;
    let mut checked = 0usize;

    for chunk in cases.chunks(BATCH_SIZE) {
        let lines: Vec<String> = chunk.iter().map(case_to_lean_json).collect();
        let lean_results = oracle_batch(&oracle, &lines);

        for (case, lean_result) in chunk.iter().zip(lean_results.iter()) {
            checked += 1;
            let lean = classify_lean(lean_result);
            let theory = case_to_rust_theory(case);
            let rust = match classify_rust(&theory) {
                Ok(c) => c,
                Err(e) => {
                    mismatches += 1;
                    if mismatches <= MAX_REPORTED {
                        eprintln!(
                            "MISMATCH (Rust error):\n{}  error: {e}\n",
                            case_pretty(case)
                        );
                    }
                    continue;
                }
            };

            // Universe = literals the Lean model classified.
            let universe: BTreeSet<Key> = lean
                .plus_d_upper
                .iter()
                .chain(lean.minus_d_upper.iter())
                .chain(lean.plus_d_lower.iter())
                .chain(lean.minus_d_lower.iter())
                .cloned()
                .collect();
            let restrict = |s: &BTreeSet<Key>| -> BTreeSet<Key> {
                s.intersection(&universe).cloned().collect()
            };

            let diffs: Vec<(&str, BTreeSet<Key>, BTreeSet<Key>)> = [
                (
                    "+D",
                    restrict(&rust.plus_d_upper),
                    lean.plus_d_upper.clone(),
                ),
                (
                    "-D",
                    restrict(&rust.minus_d_upper),
                    lean.minus_d_upper.clone(),
                ),
                (
                    "+d",
                    restrict(&rust.plus_d_lower),
                    lean.plus_d_lower.clone(),
                ),
                (
                    "-d",
                    restrict(&rust.minus_d_lower),
                    lean.minus_d_lower.clone(),
                ),
            ]
            .into_iter()
            .filter(|(_, r, l)| r != l)
            .collect();

            if !diffs.is_empty() {
                mismatches += 1;
                if mismatches <= MAX_REPORTED {
                    eprintln!("MISMATCH on theory:\n{}", case_pretty(case));
                    for (kind, r, l) in &diffs {
                        eprintln!("  {kind}: Rust {r:?} vs Lean {l:?}");
                    }
                    eprintln!();
                }
            }
        }
    }

    eprintln!("Checked {checked} temporal-family theories: {mismatches} mismatches");
    assert_eq!(
        mismatches, 0,
        "{mismatches} of {checked} temporal-family theories diverge \
         between the Rust engine and the verified Lean family model"
    );
}
