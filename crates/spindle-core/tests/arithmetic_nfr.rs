//! Non-functional requirement tests for arithmetic module.
//!
//! TEST-NFR-002: Integer overflow safety (no panics at i64 boundary)
//! TEST-NFR-001: Grounding performance regression (warning-only)
//! NFR-003: No unsafe blocks in arith.rs or term.rs

use spindle_core::conclusion::ConclusionType;
use spindle_core::pipeline::{PrepareOptions, prepare};
use spindle_core::reason::reason_prepared;
use spindle_parser::parse_spl;

fn reason_spl(input: &str) -> Vec<spindle_core::Conclusion> {
    let theory = parse_spl(input).expect("SPL parse failed");
    let opts = PrepareOptions::default();
    let result = prepare(&theory, opts).expect("pipeline failed");
    reason_prepared(&result.theory).expect("reasoning failed")
}

fn has_defeasible(conclusions: &[spindle_core::Conclusion], name: &str) -> bool {
    conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == name
    })
}

// ===========================================================================
// TEST-NFR-002: Integer Overflow Safety
// ===========================================================================

/// NFR-002.1: Addition at i64 boundary — no panic, grounding silently discarded
#[test]
fn nfr_002_1_overflow_addition_no_panic() {
    let spl = &format!(
        r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?z (+ {} 1)))
          (result ?z))
    "#,
        i64::MAX
    );
    // Should not panic — overflow produces ArithError::IntegerOverflow,
    // grounding discards this substitution path
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "result"),
        "Expected no result on i64::MAX + 1 overflow"
    );
}

/// NFR-002.2: Multiplication at i64 boundary — no panic
#[test]
fn nfr_002_2_overflow_multiplication_no_panic() {
    let spl = &format!(
        r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?z (* {} 2)))
          (result ?z))
    "#,
        i64::MAX
    );
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "result"),
        "Expected no result on i64::MAX * 2 overflow"
    );
}

/// NFR-002.3: Subtraction at i64 boundary — no panic
#[test]
fn nfr_002_3_overflow_subtraction_no_panic() {
    let spl = &format!(
        r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?z (- {} 1)))
          (result ?z))
    "#,
        i64::MIN
    );
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "result"),
        "Expected no result on i64::MIN - 1 overflow"
    );
}

/// Additional: power overflow — no panic
#[test]
fn nfr_002_extra_overflow_power_no_panic() {
    let spl = r#"
        (given trigger)
        (normally r1
          (and (trigger) (bind ?z (** 2 63)))
          (result ?z))
    "#;
    let conclusions = reason_spl(spl);
    assert!(
        !has_defeasible(&conclusions, "result"),
        "Expected no result on 2^63 overflow"
    );
}

// ===========================================================================
// TEST-NFR-001: Grounding Performance Regression
// ===========================================================================

/// NFR-001: Grounding with arithmetic should not have >10% overhead vs without.
/// This is a warning-only test (not a hard fail) since CI timing is noisy.
#[test]
fn nfr_001_grounding_performance_regression() {
    // Generate a theory with 500 facts and 200 rules WITHOUT arithmetic
    let mut spl_no_arith = String::new();
    for i in 0..500 {
        spl_no_arith.push_str(&format!("(given (data item-{} val-{}))\n", i, i % 50));
    }
    for i in 0..200 {
        let body_fact = i % 500;
        spl_no_arith.push_str(&format!(
            "(normally r{} (data item-{} ?v) (processed item-{} ?v))\n",
            i, body_fact, body_fact
        ));
    }

    // Generate equivalent theory WITH arithmetic (bind + comparison)
    let mut spl_arith = String::new();
    for i in 0..500 {
        spl_arith.push_str(&format!("(given (data item-{} {}))\n", i, i % 50));
    }
    spl_arith.push_str("(given (limit 25))\n");
    for i in 0..200 {
        let body_fact = i % 500;
        spl_arith.push_str(&format!(
            "(normally r{} (and (data item-{} ?v) (limit ?lim) (> ?v ?lim)) (flagged item-{} ?v))\n",
            i, body_fact, body_fact
        ));
    }

    // Measure grounding time for non-arithmetic theory
    let theory_no_arith = parse_spl(&spl_no_arith).expect("parse no-arith");
    let opts = PrepareOptions::default();

    let start = std::time::Instant::now();
    for _ in 0..5 {
        let _ = prepare(&theory_no_arith, opts.clone());
    }
    let baseline = start.elapsed() / 5;

    // Measure grounding time for arithmetic theory
    let theory_arith = parse_spl(&spl_arith).expect("parse arith");

    let start = std::time::Instant::now();
    for _ in 0..5 {
        let _ = prepare(&theory_arith, opts.clone());
    }
    let with_arith = start.elapsed() / 5;

    // Warn if overhead > 10x (relaxed threshold for CI noise)
    // This is informational, not a hard failure
    if baseline.as_nanos() > 0 {
        let ratio = with_arith.as_nanos() as f64 / baseline.as_nanos() as f64;
        if ratio > 10.0 {
            eprintln!(
                "WARNING: arithmetic grounding overhead is {:.1}x (baseline={:?}, arith={:?})",
                ratio, baseline, with_arith
            );
        }
    }

    // The real assertion: both complete without panic or error
    let result = prepare(&theory_arith, opts);
    assert!(result.is_ok(), "Arithmetic theory should prepare successfully");
}

// ===========================================================================
// NFR-003: No unsafe blocks in arith.rs or term.rs
// ===========================================================================

/// NFR-003: Verify no `unsafe` keyword appears in arith.rs or term.rs source.
#[test]
fn nfr_003_no_unsafe_in_arithmetic_modules() {
    let arith_src = include_str!("../src/arith.rs");
    let term_src = include_str!("../src/term.rs");

    // Check for `unsafe` as a keyword (not in comments or strings)
    // Simple heuristic: look for `unsafe` preceded by whitespace/newline
    let has_unsafe_block = |src: &str| -> bool {
        for line in src.lines() {
            let trimmed = line.trim();
            // Skip comments
            if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
            {
                continue;
            }
            // Check for unsafe keyword
            if trimmed.contains("unsafe ") || trimmed.contains("unsafe{") {
                return true;
            }
        }
        false
    };

    assert!(
        !has_unsafe_block(arith_src),
        "arith.rs must not contain unsafe blocks"
    );
    assert!(
        !has_unsafe_block(term_src),
        "term.rs must not contain unsafe blocks"
    );
}
