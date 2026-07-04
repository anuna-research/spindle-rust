//! Exhaustive small-scope differential test: Rust SDL reasoner vs the
//! formally verified Lean model (`SpindleLean`).
//!
//! Enumerates *every* propositional theory over a small scope — 2 atoms,
//! rule bodies of at most 1 literal, up to 3 rules, superiority pairs — and
//! checks that the Rust engine (`spindle_core::reason::reason`) and the Lean
//! oracle (`spindlelean --oracle-batch`, whose algorithm carries the
//! faithfulness/soundness/completeness proofs in
//! `lean/SpindleLean/Properties/`) classify every literal identically:
//! +D / -D / +d / -d.
//!
//! Rationale (small-scope hypothesis): disagreements between two defeasible
//! logic implementations almost always have a witness with very few atoms
//! and rules. Exhaustive coverage of the small scope is therefore stronger
//! evidence than random sampling of a larger one.
//!
//! Comparison contract:
//! - `+D` and `+d` sets must match exactly (positives always live in the
//!   theory's mentioned-literal universe on both sides).
//! - `-D` and `-d` are compared restricted to the Lean universe
//!   (`Theory.allLiterals` = heads ++ bodies, deduped). The Rust engine
//!   additionally interns complement literals and may emit negatives for
//!   them; those extras are outside the verified model's universe and are
//!   not compared.
//!
//! Scope control via env var `SPINDLE_EXHAUSTIVE`:
//! - `pairs`   — all 1- and 2-rule theories with all superiority options
//! - (default) — `pairs` + all 3-rule theories without superiority
//! - `full`    — default + all 3-rule theories with every single
//!               superiority pair (6 orientations)
//!
//! Two documented defeasible-level divergence classes between the engine
//! and the model are tolerated (counted, never fatal) by default — see
//! `lean/DIVERGENCES.md`. Set `SPINDLE_STRICT=1` to make them fatal too.
//! The test always fails on any divergence outside those two classes.
//!
//! Run with:
//!   cargo test --test lean_sdl_exhaustive_difftest -- --ignored --nocapture
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
use spindle_core::temporal::Temporal;
use spindle_core::theory::Theory;

// ---------------------------------------------------------------------------
// Small-scope enumeration
// ---------------------------------------------------------------------------

const ATOMS: &[&str] = &["p", "q"];

/// Propositional literal: (atom index, negated).
type Lit = (u8, bool);

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

/// All rule shapes in scope: facts (empty body) plus strict/defeasible/
/// defeater rules with bodies of size <= 1. 4 + 3*4*5 = 64 shapes.
fn all_shapes() -> Vec<Shape> {
    let lits: Vec<Lit> = (0..ATOMS.len() as u8)
        .flat_map(|a| [(a, false), (a, true)])
        .collect();
    let mut shapes = Vec::new();
    for &h in &lits {
        shapes.push(Shape {
            rtype: RType::Fact,
            head: h,
            body: vec![],
        });
    }
    for rt in [RType::Strict, RType::Defeasible, RType::Defeater] {
        for &h in &lits {
            shapes.push(Shape {
                rtype: rt,
                head: h,
                body: vec![],
            });
            for &b in &lits {
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

/// A test case: an ordered list of rules (labeled r0, r1, ...) plus
/// superiority pairs as (winner index, loser index).
#[derive(Clone, Debug)]
struct Case {
    rules: Vec<Shape>,
    superiority: Vec<(usize, usize)>,
}

fn enumerate_cases(level: &str) -> Vec<Case> {
    let shapes = all_shapes();
    let n = shapes.len();
    let mut cases = Vec::new();

    // Singletons.
    for i in 0..n {
        cases.push(Case {
            rules: vec![shapes[i].clone()],
            superiority: vec![],
        });
    }

    // Pairs, with all superiority options.
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

    if level == "pairs" {
        return cases;
    }

    // Triples without superiority (default), optionally with every single
    // superiority pair (level = "full").
    let sup_options: Vec<Vec<(usize, usize)>> = if level == "full" {
        let mut opts = vec![vec![]];
        for w in 0..3usize {
            for l in 0..3usize {
                if w != l {
                    opts.push(vec![(w, l)]);
                }
            }
        }
        opts
    } else {
        vec![vec![]]
    };

    for i in 0..n {
        for j in (i + 1)..n {
            for k in (j + 1)..n {
                let rules = vec![shapes[i].clone(), shapes[j].clone(), shapes[k].clone()];
                for sup in &sup_options {
                    cases.push(Case {
                        rules: rules.clone(),
                        superiority: sup.clone(),
                    });
                }
            }
        }
    }

    cases
}

// ---------------------------------------------------------------------------
// Lean JSON construction (field order must match the oracle's parser:
// theory {rules, superiority}; rule {label, type, body, head};
// literal {name, negated})
// ---------------------------------------------------------------------------

fn lit_json(l: Lit) -> String {
    format!(
        "{{\"name\":\"{}\",\"negated\":{}}}",
        ATOMS[l.0 as usize], l.1
    )
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

/// Human-readable rendering for counterexample reports.
fn case_pretty(case: &Case) -> String {
    let mut out = String::new();
    for (i, s) in case.rules.iter().enumerate() {
        let lit_str = |l: &Lit| format!("{}{}", if l.1 { "~" } else { "" }, ATOMS[l.0 as usize]);
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
    Literal::new(
        ATOMS[l.0 as usize],
        l.1,
        Mode::empty(),
        Temporal::empty(),
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

/// Classification of a literal universe into the four conclusion sets.
#[derive(Debug, Default, PartialEq, Eq)]
struct Classified {
    plus_d_upper: BTreeSet<(String, bool)>,  // +D
    minus_d_upper: BTreeSet<(String, bool)>, // -D
    plus_d_lower: BTreeSet<(String, bool)>,  // +d
    minus_d_lower: BTreeSet<(String, bool)>, // -d
}

fn classify_rust(theory: &Theory) -> Result<Classified, String> {
    let conclusions = reason::reason(theory).map_err(|e| e.to_string())?;
    let mut c = Classified::default();
    for conc in conclusions {
        let key = (conc.literal.name().to_string(), conc.literal.negation);
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

// ---------------------------------------------------------------------------
// Lean oracle invocation (batched)
// ---------------------------------------------------------------------------

fn find_sdl_oracle() -> Option<std::path::PathBuf> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let lean_dir = std::path::Path::new(manifest_dir)
        .parent()? // crates/
        .parent()? // project root
        .join("lean");
    let oracle = lean_dir.join(".lake/build/bin/spindlelean");
    if oracle.exists() {
        return Some(oracle);
    }
    let status = Command::new("lake")
        .arg("build")
        .arg("spindlelean")
        .current_dir(&lean_dir)
        .status()
        .ok()?;
    if status.success() && oracle.exists() {
        Some(oracle)
    } else {
        None
    }
}

/// Run one oracle process over a chunk of JSONL cases; returns one parsed
/// JSON value per input line, in order.
fn oracle_batch(oracle: &std::path::Path, lines: &[String]) -> Vec<JValue> {
    let mut child = Command::new(oracle)
        .arg("--oracle-batch")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn spindlelean oracle");

    // Write stdin from a thread to avoid pipe deadlock on large batches.
    let input = lines.join("\n");
    let mut stdin = child.stdin.take().expect("no stdin");
    let writer = std::thread::spawn(move || {
        stdin.write_all(input.as_bytes()).expect("stdin write");
        stdin.write_all(b"\n").expect("stdin write");
        // stdin dropped here -> EOF
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
    assert_eq!(
        results.len(),
        lines.len(),
        "oracle returned {} results for {} inputs",
        results.len(),
        lines.len()
    );
    results
}

fn classify_lean(result: &JValue) -> Classified {
    let mut c = Classified::default();
    let conclusions = result
        .get("conclusions")
        .and_then(|v| v.as_array())
        .unwrap_or_else(|| panic!("oracle result missing conclusions: {result}"));
    for conc in conclusions {
        let lit = &conc["literal"];
        let key = (
            lit["name"].as_str().unwrap_or("").to_string(),
            lit["negated"].as_bool().unwrap_or(false),
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
// The exhaustive test
// ---------------------------------------------------------------------------

const BATCH_SIZE: usize = 5000;
const MAX_REPORTED: usize = 20;

#[test]
#[ignore] // requires Lean oracle binary; run with -- --ignored
fn exhaustive_sdl_difftest() {
    let oracle = match find_sdl_oracle() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: spindlelean not built. Run `lake build spindlelean` in lean/");
            return;
        }
    };

    let level = std::env::var("SPINDLE_EXHAUSTIVE").unwrap_or_default();
    let cases = enumerate_cases(&level);
    eprintln!(
        "Enumerated {} theories (level: {})",
        cases.len(),
        if level.is_empty() { "default" } else { &level }
    );

    // Two documented divergence classes exist between the Rust engine and
    // the Lean model (see lean/DIVERGENCES.md for minimal witnesses and
    // analysis). Both are defeasible-level only; +D/-D agree on every
    // theory in scope.
    //
    // Class 1 — inconsistent-delta subsumption: when +D l and +D ~l both
    // hold, Rust gates +D → +d on -D ~l (the spec's worked example 1);
    // Lean follows the spec's formal clause `+d q iff +D q OR (…)`
    // (standard Antoniou et al.), granting +d to both.
    //
    // Class 2 — circular-attacker discard: for never-provable attackers
    // with circular bodies (e.g. `p => p` attacking `=> ~p`), Lean's
    // lambda over-approximation discards the attacker and proves +d ~p;
    // Rust's constructive fixed point leaves the attacker undecided,
    // which blocks, yielding -d. Strictly one-directional: Lean's +d is
    // always a superset of Rust's within scope.
    //
    // By default both known classes are tolerated (counted, not fatal), so
    // this test guards against any NEW divergence. Set SPINDLE_STRICT=1 to
    // make every divergence fatal.
    let strict = std::env::var("SPINDLE_STRICT").is_ok();

    let mut mismatches = 0usize;
    let mut tolerated_class1 = 0usize;
    let mut tolerated_class2 = 0usize;
    let mut checked = 0usize;

    for chunk in cases.chunks(BATCH_SIZE) {
        let lines: Vec<String> = chunk.iter().map(case_to_lean_json).collect();
        let lean_results = oracle_batch(&oracle, &lines);

        for (case, lean_result) in chunk.iter().zip(lean_results.iter()) {
            checked += 1;
            let lean = classify_lean(lean_result);

            // Does the Lean delta contain a complementary pair?
            let delta_lits: BTreeSet<(String, bool)> = lean_result
                .get("delta")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|l| {
                            (
                                l["name"].as_str().unwrap_or("").to_string(),
                                l["negated"].as_bool().unwrap_or(false),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            let delta_inconsistent = delta_lits
                .iter()
                .any(|(n, neg)| delta_lits.contains(&(n.clone(), !neg)));

            let theory = case_to_rust_theory(case);
            let rust = match classify_rust(&theory) {
                Ok(c) => c,
                Err(e) => {
                    mismatches += 1;
                    if mismatches <= MAX_REPORTED {
                        eprintln!(
                            "MISMATCH (Rust error, Lean succeeded):\n{}  error: {e}\n",
                            case_pretty(case)
                        );
                    }
                    continue;
                }
            };

            // The Lean universe: every literal the oracle classified.
            let universe: BTreeSet<(String, bool)> = lean
                .plus_d_upper
                .iter()
                .chain(lean.minus_d_upper.iter())
                .chain(lean.plus_d_lower.iter())
                .chain(lean.minus_d_lower.iter())
                .cloned()
                .collect();

            let restrict = |s: &BTreeSet<(String, bool)>| -> BTreeSet<(String, bool)> {
                s.intersection(&universe).cloned().collect()
            };

            let diffs: Vec<(&str, BTreeSet<(String, bool)>, BTreeSet<(String, bool)>)> = [
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
                let only_defeasible_diffs = diffs
                    .iter()
                    .all(|(kind, _, _)| *kind == "+d" || *kind == "-d");
                let lean_superset = restrict(&rust.plus_d_lower).is_subset(&lean.plus_d_lower)
                    && lean.minus_d_lower.is_subset(&restrict(&rust.minus_d_lower));

                if !strict && only_defeasible_diffs && delta_inconsistent {
                    tolerated_class1 += 1;
                    continue;
                }
                if !strict && only_defeasible_diffs && lean_superset {
                    tolerated_class2 += 1;
                    continue;
                }

                mismatches += 1;
                if mismatches <= MAX_REPORTED {
                    eprintln!(
                        "MISMATCH on theory (delta_inconsistent={delta_inconsistent}):\n{}",
                        case_pretty(case)
                    );
                    for (kind, r, l) in &diffs {
                        eprintln!("  {kind}: Rust {r:?} vs Lean {l:?}");
                    }
                    eprintln!();
                }
            }
        }
    }

    eprintln!(
        "Checked {checked} theories: {mismatches} unexplained mismatches, \
         {tolerated_class1} class-1 (inconsistent-delta subsumption), \
         {tolerated_class2} class-2 (circular-attacker discard) — \
         see lean/DIVERGENCES.md"
    );
    assert_eq!(
        mismatches, 0,
        "{mismatches} of {checked} exhaustively enumerated theories diverge \
         between the Rust reasoner and the verified Lean model outside the \
         two documented divergence classes (lean/DIVERGENCES.md)"
    );
}
