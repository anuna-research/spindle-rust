//! Format Equivalence Tests
//!
//! Tests that equivalent .dfl and .spl theories produce identical conclusions.
//! Ported from spindle-racket/tests/test-format-equivalence.rkt

use crate::{parse_dfl, parse_spl};
use spindle_core::reason::reason;
use spindle_core::{Conclusion, ConclusionType};

/// Sort conclusions by their string representation for comparison
fn sort_conclusions(conclusions: &[Conclusion]) -> Vec<String> {
    let mut sorted: Vec<String> = conclusions
        .iter()
        .map(|c| format!("{:?}:{}", c.conclusion_type, c.literal.canonical_name()))
        .collect();
    sorted.sort();
    sorted
}

/// Compare two conclusion sets for equivalence
fn conclusions_equivalent(c1: &[Conclusion], c2: &[Conclusion]) -> bool {
    sort_conclusions(c1) == sort_conclusions(c2)
}

/// Test a DFL/SPL equivalence pair
fn test_equivalence(name: &str, dfl_str: &str, spl_str: &str) {
    let dfl_theory = parse_dfl(dfl_str).expect(&format!("DFL parse failed for: {}", name));
    let spl_theory = parse_spl(spl_str).expect(&format!("SPL parse failed for: {}", name));

    let dfl_conclusions = reason(&dfl_theory);
    let spl_conclusions = reason(&spl_theory);

    // Check both produced conclusions
    assert!(
        !dfl_conclusions.is_empty(),
        "DFL should produce conclusions for: {}",
        name
    );
    assert!(
        !spl_conclusions.is_empty(),
        "SPL should produce conclusions for: {}",
        name
    );

    // Check conclusions are equivalent
    assert!(
        conclusions_equivalent(&dfl_conclusions, &spl_conclusions),
        "Conclusions should match for: {}\nDFL: {:?}\nSPL: {:?}",
        name,
        sort_conclusions(&dfl_conclusions),
        sort_conclusions(&spl_conclusions)
    );
}

#[test]
fn test_equiv_simple_fact() {
    test_equivalence("simple fact", "f1: >> bird", "(given bird)");
}

#[test]
fn test_equiv_multiple_facts() {
    test_equivalence(
        "multiple facts",
        "f1: >> bird\nf2: >> penguin",
        "(given bird)\n(given penguin)",
    );
}

#[test]
fn test_equiv_strict_rule() {
    test_equivalence(
        "strict rule",
        "f1: >> bird\nr1: bird -> animal",
        "(given bird)\n(always r1 bird animal)",
    );
}

#[test]
fn test_equiv_defeasible_rule() {
    test_equivalence(
        "defeasible rule",
        "f1: >> bird\nr1: bird => flies",
        "(given bird)\n(normally r1 bird flies)",
    );
}

#[test]
fn test_equiv_negated_conclusion() {
    test_equivalence(
        "negated conclusion",
        "f1: >> penguin\nr1: penguin => -flies",
        "(given penguin)\n(normally r1 penguin (not flies))",
    );
}

#[test]
fn test_equiv_superiority() {
    test_equivalence(
        "superiority",
        "f1: >> bird\nf2: >> penguin\nr1: bird => flies\nr2: penguin => -flies\nr2 > r1",
        "(given bird)\n(given penguin)\n(normally r1 bird flies)\n(normally r2 penguin (not flies))\n(prefer r2 r1)",
    );
}

#[test]
fn test_equiv_defeater() {
    test_equivalence(
        "defeater",
        "f1: >> bird\nf2: >> broken\nr1: bird => flies\nd1: broken ~> -flies",
        "(given bird)\n(given broken)\n(normally r1 bird flies)\n(except d1 broken (not flies))",
    );
}

#[test]
fn test_equiv_rule_chain() {
    test_equivalence(
        "rule chain",
        "f1: >> tweety\nr1: tweety -> bird\nr2: bird => flies",
        "(given tweety)\n(always r1 tweety bird)\n(normally r2 bird flies)",
    );
}

#[test]
fn test_equiv_multiple_superiorities() {
    test_equivalence(
        "multiple superiorities",
        "f1: >> a\nf2: >> b\nf3: >> c\nr1: a => x\nr2: b => -x\nr3: c => x\nr2 > r1\nr3 > r2",
        "(given a)\n(given b)\n(given c)\n(normally r1 a x)\n(normally r2 b (not x))\n(normally r3 c x)\n(prefer r2 r1)\n(prefer r3 r2)",
    );
}

#[test]
fn test_equiv_penguin_example() {
    test_equivalence(
        "penguin example",
        "f1: >> bird\nf2: >> penguin\nr1: bird => flies\nr2: penguin => -flies\nr2 > r1",
        "(given bird)\n(given penguin)\n(normally r1 bird flies)\n(normally r2 penguin (not flies))\n(prefer r2 r1)",
    );
}

/// Additional test: verify conclusion types match
#[test]
fn test_conclusion_types_match() {
    let dfl = "f1: >> bird\nf2: >> penguin\nr1: bird => flies\nr2: penguin => -flies\nr2 > r1";
    let spl = "(given bird)\n(given penguin)\n(normally r1 bird flies)\n(normally r2 penguin (not flies))\n(prefer r2 r1)";

    let dfl_theory = parse_dfl(dfl).unwrap();
    let spl_theory = parse_spl(spl).unwrap();

    let dfl_conclusions = reason(&dfl_theory);
    let spl_conclusions = reason(&spl_theory);

    // Check that the same conclusion types are present
    let dfl_definite: Vec<_> = dfl_conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
        .map(|c| c.literal.canonical_name())
        .collect();
    let spl_definite: Vec<_> = spl_conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefinitelyProvable)
        .map(|c| c.literal.canonical_name())
        .collect();

    let mut dfl_sorted = dfl_definite.clone();
    let mut spl_sorted = spl_definite.clone();
    dfl_sorted.sort();
    spl_sorted.sort();

    assert_eq!(
        dfl_sorted, spl_sorted,
        "Definite conclusions should match"
    );

    let dfl_defeasible: Vec<_> = dfl_conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
        .map(|c| c.literal.canonical_name())
        .collect();
    let spl_defeasible: Vec<_> = spl_conclusions
        .iter()
        .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
        .map(|c| c.literal.canonical_name())
        .collect();

    let mut dfl_sorted = dfl_defeasible.clone();
    let mut spl_sorted = spl_defeasible.clone();
    dfl_sorted.sort();
    spl_sorted.sort();

    assert_eq!(
        dfl_sorted, spl_sorted,
        "Defeasible conclusions should match"
    );
}
