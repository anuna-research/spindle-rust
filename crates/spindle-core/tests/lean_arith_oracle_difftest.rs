//! Differential testing: Rust arith.rs vs Lean arithmetic oracle.
//!
//! Generates random arithmetic expressions (without variables, to avoid
//! substitution format mismatches), evaluates them in both Rust and Lean,
//! and asserts the results agree.
//!
//! The Lean oracle is built from `lean/Spindle/DiffTest/ArithOracle.lean`
//! and communicates via JSON over stdin/stdout.
//!
//! Run with: `cargo test --test lean_arith_oracle_difftest`
//! Requires: `lake build ArithOracle` in the `lean/` directory first.

use std::process::Command;

use proptest::prelude::*;
use proptest::strategy::ValueTree;
use rust_decimal::Decimal;
use serde_json::{Value as JValue, json};

use spindle_core::arith::{ArithExpr, BinArithOp, CmpOp, NaryArithOp, UnaryArithOp};
use spindle_core::grounding::Substitution;
use spindle_core::term::NumericValue;

// ---------------------------------------------------------------------------
// JSON serialization (Rust ArithExpr → Lean JSON format)
// ---------------------------------------------------------------------------

fn numeric_value_to_json(v: &NumericValue) -> JValue {
    match v {
        NumericValue::Integer(n) => json!({"int": *n}),
        NumericValue::Decimal(d) => {
            // Convert rust_decimal to (n, scale) representation matching Lean's Value.decimal
            let scale = d.scale();
            // The mantissa is the unscaled value: d = mantissa / 10^scale
            let mantissa = d.mantissa();
            // Lean uses Int, so we need i128 → we'll pass as string for large values
            json!({"decimal": {"n": mantissa, "scale": scale}})
        }
        NumericValue::Float(f) => json!({"float": f.to_string()}),
    }
}

fn nary_op_to_lean(op: &NaryArithOp) -> Option<&'static str> {
    match op {
        NaryArithOp::Add => Some("sum"),
        NaryArithOp::Mul => Some("product"),
        NaryArithOp::Min => Some("min"),
        NaryArithOp::Max => Some("max"),
        // Sub and Div have different semantics in Lean (binary only)
        NaryArithOp::Sub => None,
        NaryArithOp::Div => None,
    }
}

fn bin_op_to_lean(op: &BinArithOp) -> &'static str {
    match op {
        BinArithOp::IDiv => "div",
        BinArithOp::Rem => "mod",
        BinArithOp::Pow => "pow",
    }
}

fn cmp_op_to_lean(op: &CmpOp) -> &'static str {
    match op {
        CmpOp::Eq => "eq",
        CmpOp::Ne => "ne",
        CmpOp::Lt => "lt",
        CmpOp::Gt => "gt",
        CmpOp::Le => "le",
        CmpOp::Ge => "ge",
    }
}

/// Serialize a Rust ArithExpr to the Lean oracle JSON format.
/// Returns None if the expression uses constructs that don't map directly to Lean.
fn expr_to_json(expr: &ArithExpr) -> Option<JValue> {
    match expr {
        ArithExpr::Lit(v) => Some(json!({"lit": numeric_value_to_json(v)})),
        ArithExpr::Var(_) => None, // Variables require substitution mapping
        ArithExpr::NaryOp { op, args } => {
            // Handle Sub/Div specially: map 2-arg Sub to Lean binOp sub
            match op {
                NaryArithOp::Sub if args.len() == 2 => {
                    let lhs = expr_to_json(&args[0])?;
                    let rhs = expr_to_json(&args[1])?;
                    Some(json!({"binOp": {"op": "sub", "lhs": lhs, "rhs": rhs}}))
                }
                NaryArithOp::Sub if args.len() == 1 => {
                    let arg = expr_to_json(&args[0])?;
                    Some(json!({"unaryOp": {"op": "neg", "arg": arg}}))
                }
                NaryArithOp::Div if args.len() == 2 => {
                    // Rust int/int division → Decimal; Lean int/int → Int
                    // This is a known divergence — skip for now
                    None
                }
                _ => {
                    let lean_op = nary_op_to_lean(op)?;
                    let json_args: Option<Vec<JValue>> = args.iter().map(expr_to_json).collect();
                    Some(json!({"naryOp": {"op": lean_op, "args": json_args?}}))
                }
            }
        }
        ArithExpr::BinOp { op, lhs, rhs } => {
            // IDiv and Rem require integer operands in both Rust and Lean
            let l = expr_to_json(lhs)?;
            let r = expr_to_json(rhs)?;
            Some(json!({"binOp": {"op": bin_op_to_lean(op), "lhs": l, "rhs": r}}))
        }
        ArithExpr::UnaryOp { op, expr } => {
            let arg = expr_to_json(expr)?;
            let lean_op = match op {
                UnaryArithOp::Abs => "abs",
            };
            Some(json!({"unaryOp": {"op": lean_op, "arg": arg}}))
        }
    }
}

// ---------------------------------------------------------------------------
// Parse Lean oracle JSON output
// ---------------------------------------------------------------------------

#[derive(Debug)]
#[allow(dead_code)]
enum LeanResult {
    Ok(LeanValue),
    Error(String),
}

#[derive(Debug)]
enum LeanValue {
    Int(i128),
    Decimal { n: i128, scale: u64 },
    Float(String),
}

fn parse_lean_value(j: &JValue) -> Option<LeanValue> {
    if let Some(n) = j.get("int") {
        return Some(LeanValue::Int(n.as_i64()? as i128));
    }
    if let Some(d) = j.get("decimal") {
        let n = d.get("n")?.as_i64()? as i128;
        let scale = d.get("scale")?.as_u64()?;
        return Some(LeanValue::Decimal { n, scale });
    }
    if let Some(f) = j.get("float") {
        return Some(LeanValue::Float(f.as_str()?.to_string()));
    }
    None
}

fn parse_lean_result(j: &JValue) -> Option<LeanResult> {
    if let Some(v) = j.get("ok") {
        return Some(LeanResult::Ok(parse_lean_value(v)?));
    }
    if let Some(e) = j.get("error") {
        return Some(LeanResult::Error(e.as_str()?.to_string()));
    }
    None
}

// ---------------------------------------------------------------------------
// Result comparison
// ---------------------------------------------------------------------------

/// Compare Rust evaluation result with Lean oracle result.
/// Returns true if they agree (both error, or both produce the same value).
fn results_agree(
    rust_result: &Result<NumericValue, spindle_core::arith::ArithError>,
    lean_result: &LeanResult,
) -> bool {
    match (rust_result, lean_result) {
        (Err(_), LeanResult::Error(_)) => true,
        (Ok(rv), LeanResult::Ok(lv)) => rust_value_matches_lean(rv, lv),
        // One succeeded, other failed — disagreement
        _ => false,
    }
}

fn rust_value_matches_lean(rv: &NumericValue, lv: &LeanValue) -> bool {
    match (rv, lv) {
        (NumericValue::Integer(ri), LeanValue::Int(li)) => *ri as i128 == *li,
        // Integer result can match Lean decimal with scale 0
        (NumericValue::Integer(ri), LeanValue::Decimal { n, scale }) => {
            *scale == 0 && *ri as i128 == *n
        }
        (NumericValue::Decimal(rd), LeanValue::Decimal { n, scale }) => {
            // Compare numeric value, not raw mantissa/scale, because
            // Rust and Lean may normalize scales differently.
            // Align to the larger scale for comparison.
            let rs = rd.scale() as u64;
            let rm = rd.mantissa();
            if rs == *scale {
                rm == *n
            } else if rs < *scale {
                // Scale up Rust mantissa
                let factor = 10i128.pow((*scale - rs) as u32);
                rm.checked_mul(factor).is_some_and(|aligned| aligned == *n)
            } else {
                // Scale up Lean mantissa
                let factor = 10i128.pow((rs - *scale) as u32);
                n.checked_mul(factor).is_some_and(|aligned| aligned == rm)
            }
        }
        // Decimal matching integer (Lean returned int, Rust returned decimal)
        (NumericValue::Decimal(rd), LeanValue::Int(li)) => {
            // Check if the decimal is actually an integer
            rd.fract().is_zero() && rd.mantissa() / 10i128.pow(rd.scale()) == *li
        }
        (NumericValue::Float(_rf), LeanValue::Float(_lf)) => {
            // Float comparison is approximate — skip for now
            true
        }
        _ => false,
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

    // Try the lake build output path
    let oracle_path = lean_dir.join(".lake/build/bin/ArithOracle");
    if oracle_path.exists() {
        return Some(oracle_path);
    }

    // Try building it
    let status = Command::new("lake")
        .arg("build")
        .arg("ArithOracle")
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
// Proptest generators (literal-only, no variables)
// ---------------------------------------------------------------------------

fn arb_int_value() -> impl Strategy<Value = NumericValue> {
    (-10_000i64..10_000).prop_map(NumericValue::Integer)
}

fn arb_decimal_value() -> impl Strategy<Value = NumericValue> {
    // Use small mantissas and scales to avoid overflow in Lean
    (-99_999i64..99_999, 0u32..4).prop_map(|(n, s)| NumericValue::Decimal(Decimal::new(n, s)))
}

fn arb_numeric_value_no_float() -> impl Strategy<Value = NumericValue> {
    prop_oneof![
        6 => arb_int_value(),
        4 => arb_decimal_value(),
    ]
}

fn arb_lean_compatible_expr(max_depth: u32) -> impl Strategy<Value = ArithExpr> {
    let leaf = arb_numeric_value_no_float().prop_map(ArithExpr::Lit);
    leaf.prop_recursive(max_depth, 32, 3, |inner| {
        prop_oneof![
            // N-ary sum with 1-3 args
            proptest::collection::vec(inner.clone(), 1..=3).prop_map(|args| ArithExpr::NaryOp {
                op: NaryArithOp::Add,
                args,
            }),
            // N-ary product with 1-3 args
            proptest::collection::vec(inner.clone(), 1..=3).prop_map(|args| ArithExpr::NaryOp {
                op: NaryArithOp::Mul,
                args,
            }),
            // N-ary min/max with 1-3 args
            (
                prop_oneof![Just(NaryArithOp::Min), Just(NaryArithOp::Max)],
                proptest::collection::vec(inner.clone(), 1..=3)
            )
                .prop_map(|(op, args)| ArithExpr::NaryOp { op, args }),
            // Binary sub (2 args mapped to Lean binOp sub)
            (inner.clone(), inner.clone()).prop_map(|(a, b)| ArithExpr::NaryOp {
                op: NaryArithOp::Sub,
                args: vec![a, b],
            }),
            // Unary negation (1-arg sub)
            inner.clone().prop_map(|a| ArithExpr::NaryOp {
                op: NaryArithOp::Sub,
                args: vec![a],
            }),
            // Unary abs
            inner.clone().prop_map(|e| ArithExpr::UnaryOp {
                op: UnaryArithOp::Abs,
                expr: Box::new(e),
            }),
            // BinOp pow (integer only, small exponents)
            (
                arb_int_value().prop_map(ArithExpr::Lit),
                (0i64..5).prop_map(|n| ArithExpr::Lit(NumericValue::Integer(n)))
            )
                .prop_map(|(base, exp)| ArithExpr::BinOp {
                    op: BinArithOp::Pow,
                    lhs: Box::new(base),
                    rhs: Box::new(exp),
                }),
        ]
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Smoke test: evaluate a simple integer addition in both Rust and Lean.
#[test]
#[ignore] // requires `lake build ArithOracle` in lean/
fn smoke_test_lean_arith_oracle() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!(
                "Skipping: Lean ArithOracle not built. Run `lake build ArithOracle` in lean/"
            );
            return;
        }
    };

    // Test: 2 + 3 = 5
    let input = json!({
        "expr": {"naryOp": {"op": "sum", "args": [
            {"lit": {"int": 2}},
            {"lit": {"int": 3}}
        ]}},
        "env": []
    });

    let result = call_oracle(&serde_json::to_string(&input).unwrap(), &oracle_path);
    let result = result.expect("oracle should produce output");
    let lean_result = parse_lean_result(&result).expect("should parse result");

    match lean_result {
        LeanResult::Ok(LeanValue::Int(n)) => assert_eq!(n, 5),
        other => panic!("expected Ok(Int(5)), got {other:?}"),
    }
}

/// Smoke test: constraint evaluation.
#[test]
#[ignore] // requires `lake build ArithOracle` in lean/
fn smoke_test_lean_constraint_oracle() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: Lean ArithOracle not built.");
            return;
        }
    };

    // Test: compare 3 < 5
    let input = json!({
        "constraint": {"compare": {
            "op": "lt",
            "lhs": {"lit": {"int": 3}},
            "rhs": {"lit": {"int": 5}}
        }},
        "env": []
    });

    let result = call_oracle(&serde_json::to_string(&input).unwrap(), &oracle_path);
    let result = result.expect("oracle should produce output");
    assert!(
        result.get("satisfied").is_some(),
        "expected satisfied, got: {result}"
    );
}

/// Property test: Rust and Lean agree on arithmetic expression evaluation.
#[test]
#[ignore] // requires `lake build ArithOracle` in lean/
fn proptest_rust_lean_arith_agreement() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: Lean ArithOracle not built.");
            return;
        }
    };

    // Generate a batch of test cases, then evaluate them all at once
    let config = ProptestConfig::with_cases(100);
    let mut runner = proptest::test_runner::TestRunner::new(config);

    // Collect test cases
    let strategy = proptest::collection::vec(arb_lean_compatible_expr(3), 50);
    let exprs = strategy.new_tree(&mut runner).unwrap().current();

    let mut cases = Vec::new();
    let mut rust_results = Vec::new();

    for expr in &exprs {
        if let Some(json_expr) = expr_to_json(expr) {
            let case = json!({"expr": json_expr, "env": []});
            cases.push(case);

            let subst = Substitution::default();
            let rust_result = expr.eval(&subst);
            rust_results.push((expr.clone(), rust_result));
        }
    }

    if cases.is_empty() {
        return;
    }

    // Call oracle in batch
    let lean_results =
        call_oracle_batch(&cases, &oracle_path).expect("oracle batch should succeed");

    assert_eq!(
        rust_results.len(),
        lean_results.len(),
        "result count mismatch"
    );

    let mut agreements = 0;
    let mut disagreements = Vec::new();
    let mut skipped = 0;

    for (i, ((expr, rust_result), lean_json)) in
        rust_results.iter().zip(lean_results.iter()).enumerate()
    {
        let lean_result = match parse_lean_result(lean_json) {
            Some(r) => r,
            None => {
                skipped += 1;
                continue;
            }
        };

        if results_agree(rust_result, &lean_result) {
            agreements += 1;
        } else {
            disagreements.push(format!(
                "Case {i}: expr={expr}, rust={rust_result:?}, lean={lean_result:?}"
            ));
        }
    }

    eprintln!(
        "Arith oracle: {} agreements, {} disagreements, {} skipped out of {} cases",
        agreements,
        disagreements.len(),
        skipped,
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

/// Property test: comparison operators agree between Rust and Lean.
#[test]
#[ignore] // requires `lake build ArithOracle` in lean/
fn proptest_rust_lean_comparison_agreement() {
    let oracle_path = match find_oracle_binary() {
        Some(p) => p,
        None => {
            eprintln!("Skipping: Lean ArithOracle not built.");
            return;
        }
    };

    let config = ProptestConfig::with_cases(50);
    let mut runner = proptest::test_runner::TestRunner::new(config);

    let cmp_ops = [
        CmpOp::Eq,
        CmpOp::Ne,
        CmpOp::Lt,
        CmpOp::Le,
        CmpOp::Gt,
        CmpOp::Ge,
    ];

    let strategy = proptest::collection::vec((arb_int_value(), arb_int_value(), 0..6usize), 30);
    let triples = strategy.new_tree(&mut runner).unwrap().current();

    let mut cases = Vec::new();
    let mut rust_results = Vec::new();

    for (lv, rv, op_idx) in &triples {
        let op = &cmp_ops[*op_idx];
        let lhs = ArithExpr::Lit((*lv).clone());
        let rhs = ArithExpr::Lit((*rv).clone());
        let constraint = spindle_core::arith::ArithConstraint::Compare {
            op: op.clone(),
            lhs: lhs.clone(),
            rhs: rhs.clone(),
        };

        let lj = expr_to_json(&ArithExpr::Lit(lv.clone())).unwrap();
        let rj = expr_to_json(&ArithExpr::Lit(rv.clone())).unwrap();

        let case = json!({
            "constraint": {"compare": {
                "op": cmp_op_to_lean(op),
                "lhs": lj,
                "rhs": rj
            }},
            "env": []
        });
        cases.push(case);

        let mut subst = Substitution::default();
        let rust_result = constraint.eval(&mut subst);
        rust_results.push(rust_result);
    }

    let lean_results =
        call_oracle_batch(&cases, &oracle_path).expect("oracle batch should succeed");

    let mut disagreements = 0;
    for (i, (rust_result, lean_json)) in rust_results.iter().zip(lean_results.iter()).enumerate() {
        let rust_ok = rust_result.is_ok();
        let lean_satisfied = lean_json.get("satisfied").is_some();
        let lean_unsatisfied = lean_json.get("unsatisfied").is_some();

        let agree = (rust_ok && lean_satisfied)
            || (!rust_ok && (lean_unsatisfied || lean_json.get("error").is_some()));
        if !agree {
            eprintln!(
                "Comparison case {}: rust_ok={}, lean={}, input={}",
                i,
                rust_ok,
                lean_json,
                serde_json::to_string(&cases[i]).unwrap()
            );
            disagreements += 1;
        }
    }

    if disagreements > 0 {
        panic!("{disagreements} comparison disagreements found");
    }
}
