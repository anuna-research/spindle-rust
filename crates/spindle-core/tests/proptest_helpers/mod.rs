//! Shared proptest strategy generators for spindle-core types.
//!
//! Provides reusable `Strategy` implementations for generating random
//! `Literal`, `Rule`, `Theory`, `TimePoint`, `Temporal`, `Mode`, and
//! `AllenRelation` values.
//!
//! # Usage
//!
//! ```ignore
//! mod proptest_helpers;
//! use proptest_helpers::*;
//! ```

use proptest::prelude::*;

use spindle_core::conclusion::ConclusionType;
use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::rule::RuleType;
use spindle_core::temporal::{AllenRelation, Temporal, TimePoint};
use spindle_core::theory::Theory;

/// Atom names used in generated theories.
pub const ATOMS: &[&str] = &[
    "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
];

/// Predicate argument names used in generated literals.
const PRED_ARGS: &[&str] = &["alice", "bob", "carol", "dave", "eve"];

// =============================================================================
// TimePoint / Temporal generators
// =============================================================================

/// Generate an arbitrary `TimePoint`.
pub fn arb_timepoint() -> impl Strategy<Value = TimePoint> {
    prop_oneof![
        2 => Just(TimePoint::NegInf),
        2 => Just(TimePoint::PosInf),
        // Use small values to keep intervals meaningful
        96 => (-1000i64..=1000).prop_map(TimePoint::Moment),
    ]
}

/// Generate a finite `TimePoint` (always `Moment`).
pub fn arb_finite_timepoint() -> impl Strategy<Value = TimePoint> {
    (-1000i64..=1000).prop_map(TimePoint::Moment)
}

/// Generate a valid `Temporal` interval (start <= end).
pub fn arb_temporal() -> impl Strategy<Value = Temporal> {
    prop_oneof![
        3 => Just(Temporal::empty()),
        7 => (arb_finite_timepoint(), arb_finite_timepoint()).prop_map(|(a, b)| {
            Temporal::new(a, b) // auto-swaps if inverted
        }),
    ]
}

/// Generate a non-empty `Temporal` interval with finite bounds.
pub fn arb_finite_temporal() -> impl Strategy<Value = Temporal> {
    (arb_finite_timepoint(), arb_finite_timepoint()).prop_map(|(a, b)| Temporal::new(a, b))
}

// =============================================================================
// AllenRelation generator
// =============================================================================

/// All 13 Allen interval relations.
const ALL_ALLEN_RELATIONS: [AllenRelation; 13] = [
    AllenRelation::Before,
    AllenRelation::After,
    AllenRelation::Meets,
    AllenRelation::MetBy,
    AllenRelation::Overlaps,
    AllenRelation::OverlappedBy,
    AllenRelation::Starts,
    AllenRelation::StartedBy,
    AllenRelation::During,
    AllenRelation::Contains,
    AllenRelation::Finishes,
    AllenRelation::FinishedBy,
    AllenRelation::Equals,
];

/// Generate an arbitrary Allen interval relation.
pub fn arb_allen_relation() -> impl Strategy<Value = AllenRelation> {
    proptest::sample::select(&ALL_ALLEN_RELATIONS)
}

// =============================================================================
// Mode generator
// =============================================================================

/// Generate an arbitrary `Mode`.
pub fn arb_mode() -> impl Strategy<Value = Mode> {
    prop_oneof![
        7 => Just(Mode::empty()),
        1 => Just(Mode::obligation()),
        1 => Just(Mode::permission()),
        1 => Just(Mode::forbidden()),
    ]
}

// =============================================================================
// Literal generator
// =============================================================================

/// Generate a simple `Literal` (name + optional negation).
pub fn arb_simple_literal() -> impl Strategy<Value = Literal> {
    (proptest::sample::select(ATOMS), proptest::bool::ANY).prop_map(|(name, negated)| {
        if negated {
            Literal::negated(name)
        } else {
            Literal::simple(name)
        }
    })
}

/// Generate a `Literal` with optional mode and predicates (but no temporal).
pub fn arb_literal() -> impl Strategy<Value = Literal> {
    (
        proptest::sample::select(ATOMS),
        proptest::bool::ANY,
        arb_mode(),
        proptest::collection::vec(
            proptest::sample::select(PRED_ARGS).prop_map(String::from),
            0..=2,
        ),
    )
        .prop_map(|(name, negated, mode, preds)| {
            Literal::new(name, negated, mode, Temporal::empty(), preds)
        })
}

// =============================================================================
// Rule specification generator
// =============================================================================

/// Generate a rule specification: (body atoms, head atom, negated head).
pub fn arb_rule_spec() -> impl Strategy<Value = (Vec<String>, String, bool)> {
    (
        proptest::collection::vec(
            proptest::sample::select(ATOMS).prop_map(String::from),
            1..=3,
        ),
        proptest::sample::select(ATOMS).prop_map(String::from),
        proptest::bool::ANY,
    )
}

// =============================================================================
// Theory generator
// =============================================================================

/// Generate a random theory with facts, rules, defeaters, and superiorities.
///
/// This produces ground theories (no variables) suitable for reasoning tests.
pub fn arb_theory() -> impl Strategy<Value = Theory> {
    (1_usize..=6, 0_usize..=3, 0_usize..=5, 0_usize..=2)
        .prop_flat_map(|(n_facts, n_strict, n_defeasible, n_defeaters)| {
            let fact_atoms =
                proptest::collection::vec(proptest::sample::select(ATOMS), n_facts..=n_facts);
            let strict_rules = proptest::collection::vec(arb_rule_spec(), n_strict..=n_strict);
            let defeasible_rules =
                proptest::collection::vec(arb_rule_spec(), n_defeasible..=n_defeasible);
            let defeater_rules =
                proptest::collection::vec(arb_rule_spec(), n_defeaters..=n_defeaters);
            (fact_atoms, strict_rules, defeasible_rules, defeater_rules)
        })
        .prop_map(|(facts, strict, defeasible, defeaters)| {
            build_theory(&facts, &strict, &defeasible, &defeaters)
        })
}

/// Generate a theory that always contains at least one conflict
/// (two defeasible rules producing opposite conclusions from satisfied bodies).
pub fn arb_conflicting_theory() -> impl Strategy<Value = Theory> {
    (
        proptest::sample::select(ATOMS).prop_map(String::from),
        proptest::sample::select(ATOMS).prop_map(String::from),
    )
        .prop_map(|(fact_atom, head_atom)| {
            let mut theory = Theory::new();
            theory.add_fact(&fact_atom);
            theory.add_defeasible_rule(&[fact_atom.as_str()], &head_atom);
            theory.add_defeasible_rule(&[fact_atom.as_str()], &format!("~{head_atom}"));
            theory
        })
}

/// Generate a theory with a conflict and a superiority relation resolving it.
pub fn arb_resolved_conflict_theory() -> impl Strategy<Value = (Theory, String)> {
    (
        proptest::sample::select(ATOMS).prop_map(String::from),
        proptest::sample::select(ATOMS).prop_map(String::from),
    )
        .prop_map(|(fact_atom, head_atom)| {
            let mut theory = Theory::new();
            theory.add_fact(&fact_atom);
            let r1 = theory.add_defeasible_rule(&[fact_atom.as_str()], &head_atom);
            let r2 = theory.add_defeasible_rule(&[fact_atom.as_str()], &format!("~{head_atom}"));
            // r2 beats r1, so ~head should win
            theory.add_superiority(&r2, &r1);
            (theory, head_atom)
        })
}

/// Build a theory from component specs (factored out for reuse).
fn build_theory(
    facts: &[&str],
    strict: &[(Vec<String>, String, bool)],
    defeasible: &[(Vec<String>, String, bool)],
    defeaters: &[(Vec<String>, String, bool)],
) -> Theory {
    let mut theory = Theory::new();
    let mut rule_labels: Vec<String> = Vec::new();

    for &atom in facts {
        theory.add_fact(atom);
    }

    for (body_atoms, head, negated_head) in strict {
        let head_str = if *negated_head {
            format!("~{head}")
        } else {
            head.to_string()
        };
        let body_refs: Vec<&str> = body_atoms.iter().map(|s| s.as_str()).collect();
        let label = theory.add_strict_rule(&body_refs, &head_str);
        rule_labels.push(label);
    }

    for (body_atoms, head, negated_head) in defeasible {
        let head_str = if *negated_head {
            format!("~{head}")
        } else {
            head.to_string()
        };
        let body_refs: Vec<&str> = body_atoms.iter().map(|s| s.as_str()).collect();
        let label = theory.add_defeasible_rule(&body_refs, &head_str);
        rule_labels.push(label);
    }

    for (body_atoms, head, negated_head) in defeaters {
        let head_str = if *negated_head {
            format!("~{head}")
        } else {
            head.to_string()
        };
        let body_refs: Vec<&str> = body_atoms.iter().map(|s| s.as_str()).collect();
        theory.add_defeater(&body_refs, &head_str);
    }

    // Add some superiority relations between rules (avoiding cycles)
    if rule_labels.len() >= 2 {
        let pairs: Vec<(usize, usize)> = rule_labels
            .iter()
            .enumerate()
            .flat_map(|(i, _)| {
                rule_labels
                    .iter()
                    .enumerate()
                    .filter(move |(j, _)| i != *j)
                    .map(move |(j, _)| (i, j))
            })
            .take(2)
            .collect();
        for (i, j) in pairs {
            theory.add_superiority(&rule_labels[i], &rule_labels[j]);
        }
    }

    theory
}

/// Pick a random atom name to use as a query target.
pub fn arb_query_atom() -> impl Strategy<Value = String> {
    proptest::sample::select(ATOMS).prop_map(String::from)
}

// =============================================================================
// ConclusionType generator
// =============================================================================

/// All four conclusion types.
const ALL_CONCLUSION_TYPES: [ConclusionType; 4] = [
    ConclusionType::DefinitelyProvable,
    ConclusionType::DefinitelyNotProvable,
    ConclusionType::DefeasiblyProvable,
    ConclusionType::DefeasiblyNotProvable,
];

/// Generate an arbitrary `ConclusionType`.
pub fn arb_conclusion_type() -> impl Strategy<Value = ConclusionType> {
    proptest::sample::select(&ALL_CONCLUSION_TYPES)
}

// =============================================================================
// RuleType generator
// =============================================================================

/// All four rule types.
const ALL_RULE_TYPES: [RuleType; 4] = [
    RuleType::Fact,
    RuleType::Strict,
    RuleType::Defeasible,
    RuleType::Defeater,
];

/// Generate an arbitrary `RuleType`.
pub fn arb_rule_type() -> impl Strategy<Value = RuleType> {
    proptest::sample::select(&ALL_RULE_TYPES)
}

// =============================================================================
// Conclusion helpers
// =============================================================================

/// Helper: collect conclusions into sets for property comparison.
pub fn conclusion_set(
    conclusions: &[spindle_core::conclusion::Conclusion],
    ct: ConclusionType,
) -> std::collections::HashSet<String> {
    conclusions
        .iter()
        .filter(|c| c.conclusion_type == ct)
        .map(|c| c.literal.canonical_name())
        .collect()
}
