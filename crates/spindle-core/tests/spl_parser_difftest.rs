//! Differential test: `spindle-parser` vs the verified Lean SPL grammar
//! model (`lean/Spindle/Spl/Grammar.lean`).
//!
//! The Lean model defines the propositional-SDL fragment of the SPL
//! grammar with a machine-checked AST ↔ S-expression roundtrip theorem.
//! This test enumerates fragment ASTs, renders them with the canonical
//! printer (mirrored below), and checks that
//!   (a) the Lean model parses the text back to the original AST, and
//!   (b) `spindle_parser::parse_spl` produces the same normalized theory.
//!
//! Run with:
//!   cargo test --test spl_parser_difftest -- --ignored --nocapture
//! Requires `lake build spindlelean` in `lean/` first.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value as JValue;

use spindle_core::rule::RuleType;
use spindle_parser::parse_spl;

// ---------------------------------------------------------------------------
// Fragment AST (mirrors Spl.SplStmt)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Lit {
    name: &'static str,
    negated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RT {
    Strict,
    Defeasible,
    Defeater,
}

impl RT {
    fn keyword(self) -> &'static str {
        match self {
            RT::Strict => "always",
            RT::Defeasible => "normally",
            RT::Defeater => "except",
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            RT::Strict => "strict",
            RT::Defeasible => "defeasible",
            RT::Defeater => "defeater",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Stmt {
    Fact(Lit),
    Rule(RT, &'static str, Vec<Lit>, Lit),
    Prefer(&'static str, &'static str),
}

/// Canonical printer — mirrors `Spl.printTheory` exactly.
fn print_lit(l: &Lit) -> String {
    if l.negated {
        format!("(not {})", l.name)
    } else {
        l.name.to_string()
    }
}

fn print_stmt(s: &Stmt) -> String {
    match s {
        Stmt::Fact(l) => format!("(given {})", print_lit(l)),
        Stmt::Rule(t, label, body, head) => {
            let body_str = if body.len() == 1 {
                print_lit(&body[0])
            } else {
                format!(
                    "(and {})",
                    body.iter().map(print_lit).collect::<Vec<_>>().join(" ")
                )
            };
            format!(
                "({} {} {} {})",
                t.keyword(),
                label,
                body_str,
                print_lit(head)
            )
        }
        Stmt::Prefer(w, l) => format!("(prefer {w} {l})"),
    }
}

fn print_theory(t: &[Stmt]) -> String {
    t.iter().map(print_stmt).collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Normal form for comparison
// ---------------------------------------------------------------------------

/// (kind, type, label, body, head, winner, loser) — flattened normal form.
type Norm = (
    &'static str,
    String,
    String,
    Vec<(String, bool)>,
    Option<(String, bool)>,
    String,
    String,
);

fn norm_ast(t: &[Stmt]) -> Vec<Norm> {
    let mut out: Vec<Norm> = t
        .iter()
        .map(|s| match s {
            Stmt::Fact(l) => (
                "fact",
                String::new(),
                String::new(),
                vec![],
                Some((l.name.to_string(), l.negated)),
                String::new(),
                String::new(),
            ),
            Stmt::Rule(rt, label, body, head) => (
                "rule",
                rt.as_str().to_string(),
                label.to_string(),
                body.iter()
                    .map(|l| (l.name.to_string(), l.negated))
                    .collect(),
                Some((head.name.to_string(), head.negated)),
                String::new(),
                String::new(),
            ),
            Stmt::Prefer(w, l) => (
                "prefer",
                String::new(),
                String::new(),
                vec![],
                None,
                w.to_string(),
                l.to_string(),
            ),
        })
        .collect();
    out.sort();
    out
}

fn norm_lean(result: &JValue) -> Result<Vec<Norm>, String> {
    if let Some(e) = result.get("error") {
        return Err(format!("lean parse error: {e}"));
    }
    let stmts = result["stmts"].as_array().ok_or("missing stmts")?;
    let lit_of = |j: &JValue| -> (String, bool) {
        (
            j["name"].as_str().unwrap_or("").to_string(),
            j["negated"].as_bool().unwrap_or(false),
        )
    };
    let mut out: Vec<Norm> = stmts
        .iter()
        .map(|s| match s["kind"].as_str().unwrap_or("") {
            "fact" => (
                "fact",
                String::new(),
                String::new(),
                vec![],
                Some(lit_of(&s["head"])),
                String::new(),
                String::new(),
            ),
            "rule" => (
                "rule",
                s["type"].as_str().unwrap_or("").to_string(),
                s["label"].as_str().unwrap_or("").to_string(),
                s["body"]
                    .as_array()
                    .map(|a| a.iter().map(&lit_of).collect())
                    .unwrap_or_default(),
                Some(lit_of(&s["head"])),
                String::new(),
                String::new(),
            ),
            _ => (
                "prefer",
                String::new(),
                String::new(),
                vec![],
                None,
                s["winner"].as_str().unwrap_or("").to_string(),
                s["loser"].as_str().unwrap_or("").to_string(),
            ),
        })
        .collect();
    out.sort();
    Ok(out)
}

fn norm_rust(input: &str) -> Result<Vec<Norm>, String> {
    let theory = parse_spl(input).map_err(|e| format!("rust parse error: {e}"))?;
    let mut out: Vec<Norm> = Vec::new();
    for rule in theory.rules() {
        let head = rule.head_literal();
        let head_norm = Some((head.name().to_string(), head.negation));
        if rule.rule_type == RuleType::Fact {
            out.push((
                "fact",
                String::new(),
                String::new(),
                vec![],
                head_norm,
                String::new(),
                String::new(),
            ));
        } else {
            let ty = match rule.rule_type {
                RuleType::Strict => "strict",
                RuleType::Defeasible => "defeasible",
                RuleType::Defeater => "defeater",
                RuleType::Fact => unreachable!(),
            };
            let body: Vec<(String, bool)> = rule
                .body
                .iter()
                .filter_map(|bl| bl.as_logic())
                .map(|l| {
                    let lit = l.to_literal();
                    (lit.name().to_string(), lit.negation)
                })
                .collect();
            out.push((
                "rule",
                ty.to_string(),
                rule.label.clone(),
                body,
                head_norm,
                String::new(),
                String::new(),
            ));
        }
    }
    for sup in theory.superiorities() {
        out.push((
            "prefer",
            String::new(),
            String::new(),
            vec![],
            None,
            sup.superior.clone(),
            sup.inferior.clone(),
        ));
    }
    out.sort();
    Ok(out)
}

// ---------------------------------------------------------------------------
// Enumeration
// ---------------------------------------------------------------------------

fn all_lits() -> Vec<Lit> {
    ["p", "q"]
        .iter()
        .flat_map(|&name| {
            [
                Lit {
                    name,
                    negated: false,
                },
                Lit {
                    name,
                    negated: true,
                },
            ]
        })
        .collect()
}

fn enumerate_theories() -> Vec<Vec<Stmt>> {
    let lits = all_lits();
    let mut rules: Vec<Stmt> = Vec::new();
    for rt in [RT::Strict, RT::Defeasible, RT::Defeater] {
        for head in &lits {
            for b1 in &lits {
                rules.push(Stmt::Rule(rt, "r0", vec![b1.clone()], head.clone()));
                for b2 in &lits {
                    rules.push(Stmt::Rule(
                        rt,
                        "r0",
                        vec![b1.clone(), b2.clone()],
                        head.clone(),
                    ));
                }
            }
        }
    }

    let mut theories: Vec<Vec<Stmt>> = Vec::new();
    // Single facts.
    for l in &lits {
        theories.push(vec![Stmt::Fact(l.clone())]);
    }
    // Single rules.
    for r in &rules {
        theories.push(vec![r.clone()]);
    }
    // Fact + rule.
    for l in &lits {
        for r in &rules {
            theories.push(vec![Stmt::Fact(l.clone()), r.clone()]);
        }
    }
    // Rule + prefer.
    for r in &rules {
        theories.push(vec![r.clone(), Stmt::Prefer("r0", "r1")]);
        theories.push(vec![r.clone(), Stmt::Prefer("r1", "r0")]);
    }
    theories
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
        .arg("--parse-spl-batch")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn spl parse oracle");

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
// The difftest
// ---------------------------------------------------------------------------

#[test]
#[ignore] // requires Lean oracle binary; run with -- --ignored
fn spl_parser_difftest() {
    let oracle = match find_oracle() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: spindlelean not built. Run `lake build spindlelean` in lean/");
            return;
        }
    };

    let theories = enumerate_theories();
    eprintln!("Enumerated {} SPL fragment theories", theories.len());

    let lines: Vec<String> = theories.iter().map(|t| print_theory(t)).collect();
    let lean_results = oracle_batch(&oracle, &lines);

    let mut mismatches = 0usize;
    for ((ast, text), lean_result) in theories.iter().zip(lines.iter()).zip(lean_results.iter()) {
        let expected = norm_ast(ast);

        // (a) Lean model roundtrip: printed text parses back to the AST.
        match norm_lean(lean_result) {
            Ok(lean_norm) => {
                if lean_norm != expected {
                    mismatches += 1;
                    if mismatches <= 10 {
                        eprintln!(
                            "LEAN ROUNDTRIP MISMATCH on: {text}\n  expected {expected:?}\n  lean {lean_norm:?}\n"
                        );
                    }
                    continue;
                }
            }
            Err(e) => {
                mismatches += 1;
                if mismatches <= 10 {
                    eprintln!("LEAN PARSE FAILURE on: {text}\n  {e}\n");
                }
                continue;
            }
        }

        // (b) Rust parser agreement.
        match norm_rust(text) {
            Ok(rust_norm) => {
                if rust_norm != expected {
                    mismatches += 1;
                    if mismatches <= 10 {
                        eprintln!(
                            "RUST PARSER MISMATCH on: {text}\n  expected {expected:?}\n  rust {rust_norm:?}\n"
                        );
                    }
                }
            }
            Err(e) => {
                mismatches += 1;
                if mismatches <= 10 {
                    eprintln!("RUST PARSE FAILURE on: {text}\n  {e}\n");
                }
            }
        }
    }

    eprintln!(
        "Checked {} SPL fragment theories: {mismatches} mismatches",
        theories.len()
    );
    assert_eq!(mismatches, 0);
}
