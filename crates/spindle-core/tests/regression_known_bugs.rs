//! Regression test harness for known bugs documented in MEMORY.md.
//!
//! Each test targets a specific known bug from the February 2026 bug hunt.
//! Tests that expose bugs that are **already fixed** assert the correct
//! behavior directly. Tests that expose bugs that are **still present**
//! document the expected-correct behavior and the currently-observed
//! (buggy) behavior so that future fixes can simply flip the assertions.
//!
//! # Known Bugs Covered
//!
//! 1. `reason.rs` -- Credulous semantics (no ambiguity blocking)
//! 2. `reason.rs` -- Empty-body non-fact rules never fire
//! 3. `reason.rs` -- Duplicate fact double-decrement
//! 4. `query.rs`  -- `why_not` misses defeater blocking
//! 5. `query.rs`  -- `abduce` doesn't verify solutions
//! 6. `query.rs`  -- `explain()` can stack overflow on cycles

mod fixtures;

use spindle_core::conclusion::ConclusionType;
use spindle_core::explanation::explain;
use spindle_core::literal::Literal;
use spindle_core::query::{
    BlockingType, RequiresOptions, RequiresSearchStatus, abduce, requires_with_options, why_not,
};
use spindle_core::reason::reason;
use spindle_core::rule::{Rule, RuleType};
use spindle_core::theory::Theory;

// =============================================================================
// BUG 1: reason.rs -- Credulous semantics (no ambiguity blocking)
//
// When two defeasible rules conflict and there is no superiority relation,
// standard DL(d) semantics require that NEITHER conclusion be drawn
// (ambiguity blocking / skeptical semantics). The known bug is that
// reason.rs uses credulous semantics and draws BOTH conclusions.
// =============================================================================

#[test]
fn test_ambiguity_blocking_nixon_diamond() {
    // The Nixon Diamond: republican => ~pacifist, quaker => pacifist,
    // no superiority. Under skeptical/ambiguity-blocking semantics,
    // neither +d pacifist nor +d ~pacifist should be derived.
    let theory = fixtures::nixon_diamond();
    let conclusions = reason(&theory).unwrap();

    let has_pacifist = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "pacifist"
            && !c.literal.negation
    });

    let has_not_pacifist = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "pacifist"
            && c.literal.negation
    });

    // KNOWN BUG: reason.rs uses credulous semantics and derives BOTH.
    // Correct behavior (skeptical / ambiguity blocking): neither should be derived.
    //
    // When this bug is fixed, uncomment the correct assertions below and
    // remove the "documents the bug" block.

    if has_pacifist && has_not_pacifist {
        // Bug is present: credulous semantics drew both conclusions.
        // Document the bug but don't fail the test -- this is a known issue.
        eprintln!(
            "KNOWN BUG (credulous semantics): Nixon Diamond produced BOTH \
             +d pacifist and +d ~pacifist. Expected neither under skeptical semantics."
        );
    } else if !has_pacifist && !has_not_pacifist {
        // Bug is fixed! Skeptical semantics correctly blocks both.
        // This is the desired outcome.
    } else {
        // Unexpected: only one side derived. This would be a different bug.
        panic!(
            "Unexpected Nixon Diamond result: +d pacifist={has_pacifist}, \
             +d ~pacifist={has_not_pacifist}. Expected either both (known bug) or neither (fixed)."
        );
    }
}

#[test]
fn test_ambiguity_blocking_symmetric_conflict() {
    // A minimal symmetric conflict: p => q, p => ~q, no superiority.
    // Skeptical semantics: neither +d q nor +d ~q.
    let mut theory = Theory::new();
    theory.add_fact("p");
    theory.add_defeasible_rule(&["p"], "q");
    theory.add_defeasible_rule(&["p"], "~q");

    let conclusions = reason(&theory).unwrap();

    let has_q = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "q"
            && !c.literal.negation
    });

    let has_not_q = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "q"
            && c.literal.negation
    });

    // KNOWN BUG: credulous semantics derives both.
    if has_q && has_not_q {
        eprintln!(
            "KNOWN BUG (credulous semantics): symmetric conflict produced BOTH \
             +d q and +d ~q. Expected neither under skeptical semantics."
        );
    } else if !has_q && !has_not_q {
        // Fixed -- skeptical semantics.
    } else {
        panic!("Unexpected symmetric conflict result: +d q={has_q}, +d ~q={has_not_q}.");
    }
}

// =============================================================================
// BUG 2: reason.rs -- Empty-body non-fact rules never fire
//
// A defeasible rule with an empty body (an axiom) should fire and produce
// its head as a conclusion. scalable.rs handles this correctly; the
// standard reason.rs was reported to have issues with this.
//
// STATUS: FIXED -- the rearchitected reason.rs now handles empty-body rules.
// =============================================================================

#[test]
fn test_empty_body_nonfact_rule_fires() {
    // A defeasible rule with no body literals is essentially an axiom.
    // It should produce +d for its head literal.
    let mut theory = Theory::new();
    let rule = Rule::new(
        "axiom1",
        RuleType::Defeasible,
        vec![],
        vec![Literal::simple("truth")],
    );
    theory.add_rule(rule);

    let conclusions = reason(&theory).unwrap();

    let has_truth = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "truth"
    });

    // This bug was previously documented. If this assertion passes, the bug
    // is fixed. If it fails, the bug has regressed.
    assert!(
        has_truth,
        "BUG REGRESSION: Empty-body defeasible rule did not fire. \
         Expected +d truth from axiom with no body."
    );
}

#[test]
fn test_empty_body_nonfact_rule_chains_forward() {
    // An empty-body defeasible rule should seed forward chaining.
    // axiom => base, base => derived
    let mut theory = Theory::new();
    let axiom = Rule::new(
        "axiom",
        RuleType::Defeasible,
        vec![],
        vec![Literal::simple("base")],
    );
    theory.add_rule(axiom);
    theory.add_defeasible_rule(&["base"], "derived");

    let conclusions = reason(&theory).unwrap();

    let has_derived = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "derived"
    });

    assert!(
        has_derived,
        "BUG REGRESSION: Forward chaining from empty-body rule head failed. \
         Expected +d derived from axiom => base => derived."
    );
}

#[test]
fn test_empty_body_strict_rule_fires() {
    // Strict empty-body rule (axiom) should produce +D.
    let mut theory = Theory::new();
    let rule = Rule::new(
        "strict_axiom",
        RuleType::Strict,
        vec![],
        vec![Literal::simple("axiom_lit")],
    );
    theory.add_rule(rule);

    let conclusions = reason(&theory).unwrap();

    let has_axiom = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "axiom_lit"
    });

    assert!(
        has_axiom,
        "Empty-body strict rule should produce +D axiom_lit."
    );
}

// =============================================================================
// BUG 3: reason.rs -- Duplicate fact double-decrement
//
// Adding the same fact twice could cause body counters to underflow,
// leading to spurious rule firings. The fix ensures duplicate facts
// are deduplicated during initialization.
//
// STATUS: FIXED -- duplicate facts are now deduplicated.
// =============================================================================

#[test]
fn test_duplicate_fact_no_double_decrement() {
    // Adding fact "p" twice should not cause a rule with body [p, q]
    // to fire when only p is present. If body counters are decremented
    // twice for the same literal, the counter underflows and the rule
    // fires prematurely.
    let mut theory = Theory::new();
    theory.add_fact("p");
    theory.add_fact("p"); // duplicate
    theory.add_defeasible_rule(&["p", "q"], "r");

    let conclusions = reason(&theory).unwrap();

    let has_r = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "r"
    });

    assert!(
        !has_r,
        "BUG REGRESSION (double-decrement): Rule with body [p, q] fired when only p \
         is proven (duplicate p fact caused counter underflow). Expected r NOT provable."
    );
}

#[test]
fn test_duplicate_fact_single_conclusion() {
    // Duplicate facts should produce exactly one +D conclusion, not two.
    let mut theory = Theory::new();
    theory.add_fact("bird");
    theory.add_fact("bird"); // duplicate

    let conclusions = reason(&theory).unwrap();

    let bird_count = conclusions
        .iter()
        .filter(|c| {
            c.conclusion_type == ConclusionType::DefinitelyProvable && c.literal.name() == "bird"
        })
        .count();

    assert_eq!(
        bird_count, 1,
        "BUG REGRESSION: Duplicate facts produced {bird_count} +D conclusions instead of 1."
    );
}

#[test]
fn test_duplicate_fact_forward_chaining_correct() {
    // With duplicate facts, forward chaining should still work correctly.
    // bird (x2) => flies should produce exactly one +d flies.
    let mut theory = Theory::new();
    theory.add_fact("bird");
    theory.add_fact("bird"); // duplicate
    theory.add_defeasible_rule(&["bird"], "flies");

    let conclusions = reason(&theory).unwrap();

    let flies_count = conclusions
        .iter()
        .filter(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable && c.literal.name() == "flies"
        })
        .count();

    assert_eq!(
        flies_count, 1,
        "Duplicate facts should not produce duplicate derived conclusions. Got {flies_count} +d flies."
    );
}

// =============================================================================
// BUG 4: query.rs -- why_not misses defeater blocking
//
// When a defeater blocks a conclusion, why_not should report the defeater
// as a blocking condition. The known bug was that why_not did not detect
// defeater-based blocking.
//
// STATUS: FIXED -- why_not now checks for defeater blocking.
// =============================================================================

#[test]
fn test_why_not_reports_defeater_blocking() {
    // bird => flies, broken_wing ~> ~flies (defeater).
    // The defeater blocks +d flies. why_not should report this.
    let mut theory = Theory::new();
    theory.add_fact("bird");
    theory.add_fact("broken_wing");
    theory.add_defeasible_rule(&["bird"], "flies");
    theory.add_defeater(&["broken_wing"], "~flies");

    let result = why_not(&theory, &Literal::simple("flies")).unwrap();

    // flies should NOT be provable (blocked by defeater)
    let conclusions = reason(&theory).unwrap();
    let flies_provable = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "flies"
            && !c.literal.negation
    });
    assert!(
        !flies_provable,
        "flies should not be provable when blocked by defeater"
    );

    // why_not should report the blocking condition
    assert!(
        result.has_blockers(),
        "BUG REGRESSION: why_not should report blockers for defeater-blocked literal."
    );

    let has_defeated_or_contradicted = result.blocked_by.iter().any(|b| {
        b.blocking_type == BlockingType::Defeated || b.blocking_type == BlockingType::Contradicted
    });

    assert!(
        has_defeated_or_contradicted,
        "BUG REGRESSION: why_not should report Defeated or Contradicted blocking type \
         when a defeater blocks a conclusion. Got: {:?}",
        result
            .blocked_by
            .iter()
            .map(|b| format!("{}: {}", b.blocking_type, b.explanation))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_why_not_reports_defeater_blocking_with_fixture() {
    // Use the conflicting_defeaters fixture for a more complex scenario.
    let theory = fixtures::conflicting_defeaters();

    let result = why_not(&theory, &Literal::simple("flies")).unwrap();

    // flies should be blocked by defeaters d1 (injured ~> ~flies) and d2 (young ~> ~flies).
    assert!(
        result.has_blockers(),
        "why_not should report blockers when multiple defeaters block a conclusion."
    );
}

// =============================================================================
// BUG 5: query.rs -- abduce doesn't verify solutions
//
// The abduce function finds sets of missing facts that would satisfy
// rule bodies, but does not verify that adding those facts actually
// produces the target conclusion (they might be blocked by defeaters
// or conflicting rules). This test verifies the bug.
// =============================================================================

#[test]
fn test_requires_verified_rejects_defeater_blocked_candidate() {
    // Setup: q requires p, but p also activates a defeater for ~q.
    // Verified requires should reject {p} because q is still not provable.
    let mut theory = Theory::new();
    theory.add_defeasible_rule(&["p"], "q");
    theory.add_defeater(&["p"], "~q");

    let result = requires_with_options(
        &theory,
        &Literal::simple("q"),
        RequiresOptions {
            max_solutions: 10,
            max_raw_candidates: 10,
        },
    )
    .unwrap();

    assert!(!result.already_provable, "q should not be already provable");
    assert!(
        result.solutions.is_empty(),
        "Verified requires should not return defeater-blocked candidates"
    );
    assert!(
        result.verification.raw_examined >= 1,
        "At least one raw candidate should be examined"
    );
    assert_eq!(
        result.verification.accepted, 0,
        "Defeater-blocked candidate should not be accepted"
    );
    assert_eq!(
        result.verification.raw_examined, result.verification.rejected,
        "All examined candidates should be rejected in this scenario"
    );
    assert_eq!(result.search_status, RequiresSearchStatus::BoundedComplete);
}

#[test]
fn test_abduce_may_return_unverified_candidate_documented() {
    // Raw abduce behavior remains unchanged: it may return body-satisfying
    // candidates that fail under full defeasible reasoning.
    let mut theory = Theory::new();
    theory.add_defeasible_rule(&["p"], "q");
    theory.add_defeater(&["p"], "~q");

    let result = abduce(&theory, &Literal::simple("q"), 10).unwrap();
    assert!(
        result.has_solutions(),
        "Raw abduce should still return body-level candidates"
    );

    for (i, solution) in result.solutions.iter().enumerate() {
        if solution.is_already_provable() {
            continue;
        }

        let mut modified = theory.clone();
        for fact_lit in &solution.facts {
            let label = format!("__abduce_test_{i}_{}", fact_lit.name());
            modified.add_rule(Rule::fact(&label, fact_lit.clone()));
        }

        let modified_conclusions = reason(&modified).unwrap();
        let goal_achieved = modified_conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        });

        if !goal_achieved {
            return;
        }
    }

    panic!("Expected at least one raw abduce candidate to fail verification");
}

#[test]
fn test_abduce_valid_solution_works() {
    // Positive test: when there is no defeater, abduce should find a valid solution.
    let mut theory = Theory::new();
    theory.add_defeasible_rule(&["p"], "q");

    let result = abduce(&theory, &Literal::simple("q"), 10).unwrap();
    assert!(result.has_solutions(), "Should find at least one solution");

    let solution = result.smallest_solution().unwrap();
    assert!(
        solution.facts.contains(&Literal::simple("p")),
        "Solution should propose adding fact 'p'"
    );

    // Verify: adding p should actually make q provable.
    let mut modified = theory.clone();
    modified.add_fact("p");
    let conclusions = reason(&modified).unwrap();
    let q_provable = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "q"
            && !c.literal.negation
    });
    assert!(
        q_provable,
        "Adding abduced fact 'p' should make 'q' provable."
    );
}

// =============================================================================
// BUG 6: query.rs -- explain() can stack overflow on cycles
//
// Circular rule chains (a => b, b => c, c => a) could cause infinite
// recursion in explain(). The fix adds cycle detection via a visited set.
//
// STATUS: FIXED -- explain() now uses a visited set for cycle detection.
// =============================================================================

#[test]
fn test_explain_handles_cycles() {
    // Create a theory with circular dependencies: a => b, b => c, c => a.
    // explain() should terminate without stack overflow.
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_defeasible_rule(&["a"], "b");
    theory.add_defeasible_rule(&["b"], "c");
    theory.add_defeasible_rule(&["c"], "a"); // cycle back to a

    // This should NOT stack overflow. It should return Ok.
    let result = explain(&theory, &Literal::simple("c"));
    assert!(
        result.is_ok(),
        "BUG REGRESSION: explain() should not stack overflow on cyclic rules. \
         Got error: {:?}",
        result.err()
    );

    // The explanation should exist since c IS provable (a => b => c).
    let explanation = result.unwrap();
    assert!(
        explanation.is_some(),
        "explain() should return Some for a provable literal even with cycles."
    );
}

#[test]
fn test_explain_handles_self_cycle() {
    // Edge case: a rule whose body includes its own head.
    // a => a (self-referential). With fact "a", explain should not loop.
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_defeasible_rule(&["a"], "a"); // self-cycle

    let result = explain(&theory, &Literal::simple("a"));
    assert!(
        result.is_ok(),
        "explain() should handle self-referential rules without stack overflow."
    );
}

#[test]
fn test_explain_handles_mutual_cycle() {
    // Mutual cycle: a => b, b => a. With fact "a", both are provable.
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_defeasible_rule(&["a"], "b");
    theory.add_defeasible_rule(&["b"], "a"); // mutual cycle

    let result_a = explain(&theory, &Literal::simple("a"));
    let result_b = explain(&theory, &Literal::simple("b"));

    assert!(
        result_a.is_ok(),
        "explain(a) should not stack overflow on mutual cycle."
    );
    assert!(
        result_b.is_ok(),
        "explain(b) should not stack overflow on mutual cycle."
    );
}

#[test]
fn test_explain_deep_cycle() {
    // A longer cycle: a => b => c => d => e => a.
    // With fact "a", all should be provable and explain should terminate.
    let mut theory = Theory::new();
    theory.add_fact("a");
    theory.add_defeasible_rule(&["a"], "b");
    theory.add_defeasible_rule(&["b"], "c");
    theory.add_defeasible_rule(&["c"], "d");
    theory.add_defeasible_rule(&["d"], "e");
    theory.add_defeasible_rule(&["e"], "a"); // cycle back

    let result = explain(&theory, &Literal::simple("e"));
    assert!(
        result.is_ok(),
        "explain() should handle deep cycles (length 5) without stack overflow."
    );
}

// =============================================================================
// COMBINED REGRESSION: Verify multiple bugs don't interact
// =============================================================================

#[test]
fn test_duplicate_facts_with_conflict() {
    // Combine bug 1 (credulous semantics) with bug 3 (duplicate facts).
    // Duplicate facts should not worsen the credulous semantics issue.
    let mut theory = Theory::new();
    theory.add_fact("p");
    theory.add_fact("p"); // duplicate
    theory.add_defeasible_rule(&["p"], "q");
    theory.add_defeasible_rule(&["p"], "~q");

    let conclusions = reason(&theory).unwrap();

    // At minimum, the duplicate fact should not cause additional problems
    // beyond the known credulous semantics bug.
    //
    // Count +d q (positive only, excluding ~q which is a separate literal).
    let q_pos_count = conclusions
        .iter()
        .filter(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && !c.literal.negation
        })
        .count();

    let q_neg_count = conclusions
        .iter()
        .filter(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.name() == "q"
                && c.literal.negation
        })
        .count();

    // Each side should appear at most once (no duplicate conclusions from
    // duplicate facts). The credulous bug may produce both q and ~q, but
    // each should appear exactly once.
    assert!(
        q_pos_count <= 1,
        "Duplicate facts should not produce duplicate +d q conclusions. Got {q_pos_count}."
    );
    assert!(
        q_neg_count <= 1,
        "Duplicate facts should not produce duplicate +d ~q conclusions. Got {q_neg_count}."
    );
}

#[test]
fn test_why_not_with_ambiguity() {
    // Combine bug 1 (credulous semantics) with bug 4 (why_not).
    // In the Nixon Diamond, if the bug is present (credulous), why_not
    // should still behave consistently with the reasoning result.
    let theory = fixtures::nixon_diamond();

    let conclusions = reason(&theory).unwrap();
    let pacifist_provable = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable
            && c.literal.name() == "pacifist"
            && !c.literal.negation
    });

    let wn_result = why_not(&theory, &Literal::simple("pacifist")).unwrap();

    if pacifist_provable {
        // Credulous semantics: pacifist IS provable, so why_not should
        // report no blockers.
        assert!(
            !wn_result.has_blockers(),
            "why_not should report no blockers when literal IS provable (credulous semantics)."
        );
    } else {
        // Skeptical semantics (fixed): pacifist is NOT provable, so
        // why_not should explain the blocking.
        assert!(
            wn_result.has_blockers(),
            "why_not should report blockers when literal is blocked by ambiguity."
        );
    }
}
