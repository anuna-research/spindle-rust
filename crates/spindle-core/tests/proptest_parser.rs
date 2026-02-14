//! Property-based tests for SPL parser round-trip correctness.
//!
//! Tests:
//! - Parser never panics on arbitrary input (fuzz-like)
//! - Valid generated SPL for facts always parses
//! - Literal to_spl produces parseable output
//! - Rule to_spl produces parseable output

mod proptest_helpers;

use proptest::prelude::*;

use spindle_core::literal::Literal;
use spindle_core::rule::Rule;
use spindle_parser::parse_spl;

use proptest_helpers::ATOMS;

// =============================================================================
// Parser never panics on arbitrary strings
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// The SPL parser should never panic on arbitrary input, even garbage.
    /// It may return an error, but must not panic.
    #[test]
    fn parser_never_panics(input in "\\PC{0,200}") {
        // Just call parse_spl and discard the result - we only care about no panics
        let _ = parse_spl(&input);
    }
}

// =============================================================================
// Parser handles empty and whitespace input
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Empty and whitespace-only input should parse without panic.
    #[test]
    fn parser_handles_whitespace(ws in "[\\s]{0,50}") {
        let _ = parse_spl(&ws);
    }
}

// =============================================================================
// Valid fact SPL always parses
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Generated `(given atom)` SPL should always parse successfully.
    #[test]
    fn valid_fact_spl_parses(atom in proptest::sample::select(ATOMS)) {
        let spl = format!("(given {atom})");
        let result = parse_spl(&spl);
        prop_assert!(
            result.is_ok(),
            "Valid fact SPL should parse: '{}', error: {:?}",
            spl, result.err()
        );
    }
}

// =============================================================================
// Valid defeasible rule SPL always parses
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Generated `(normally label body head)` SPL should always parse.
    #[test]
    fn valid_defeasible_rule_spl_parses(
        label in "[a-z][a-z0-9-]{0,5}",
        body_atom in proptest::sample::select(ATOMS),
        head_atom in proptest::sample::select(ATOMS),
    ) {
        let spl = format!("(normally {label} ({body_atom}) ({head_atom}))");
        let result = parse_spl(&spl);
        prop_assert!(
            result.is_ok(),
            "Valid rule SPL should parse: '{}', error: {:?}",
            spl, result.err()
        );
    }
}

// =============================================================================
// Literal.to_spl() produces parseable output
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// The SPL string produced by Literal::to_spl() should be parseable
    /// when wrapped in a `(given ...)` form.
    #[test]
    fn literal_to_spl_is_parseable(
        name in proptest::sample::select(ATOMS),
        negated in proptest::bool::ANY,
    ) {
        let lit = if negated {
            Literal::negated(name)
        } else {
            Literal::simple(name)
        };

        let spl_form = format!("(given {})", lit.to_spl());
        let result = parse_spl(&spl_form);
        prop_assert!(
            result.is_ok(),
            "Literal.to_spl() should produce parseable SPL: '{}', error: {:?}",
            spl_form, result.err()
        );
    }
}

// =============================================================================
// Rule.to_spl() produces parseable output
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// The SPL string produced by Rule::to_spl() should parse successfully.
    #[test]
    fn rule_to_spl_is_parseable(
        body_atom in proptest::sample::select(ATOMS),
        head_atom in proptest::sample::select(ATOMS),
        negated_head in proptest::bool::ANY,
    ) {
        let body = vec![Literal::simple(body_atom)];
        let head = if negated_head {
            Literal::negated(head_atom)
        } else {
            Literal::simple(head_atom)
        };

        let rule = Rule::defeasible("r-test", body, head);
        let spl = rule.to_spl();
        let result = parse_spl(&spl);
        prop_assert!(
            result.is_ok(),
            "Rule.to_spl() should produce parseable SPL: '{}', error: {:?}",
            spl, result.err()
        );
    }
}

// =============================================================================
// Fact Rule.to_spl() produces parseable output
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Fact rules' to_spl() should produce parseable SPL.
    #[test]
    fn fact_to_spl_is_parseable(
        atom in proptest::sample::select(ATOMS),
        negated in proptest::bool::ANY,
    ) {
        let lit = if negated {
            Literal::negated(atom)
        } else {
            Literal::simple(atom)
        };
        let fact = Rule::fact("f-test", lit);
        let spl = fact.to_spl();
        let result = parse_spl(&spl);
        prop_assert!(
            result.is_ok(),
            "Fact to_spl() should produce parseable SPL: '{}', error: {:?}",
            spl, result.err()
        );
    }
}

// =============================================================================
// Multi-body Rule.to_spl() produces parseable output
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Rules with multiple body literals should produce parseable to_spl().
    #[test]
    fn multi_body_rule_to_spl_is_parseable(
        body_atoms in proptest::collection::vec(
            proptest::sample::select(ATOMS),
            2..=4,
        ),
        head_atom in proptest::sample::select(ATOMS),
    ) {
        let body: Vec<Literal> = body_atoms.iter().map(|&a| Literal::simple(a)).collect();
        let head = Literal::simple(head_atom);

        let rule = Rule::defeasible("r-multi", body, head);
        let spl = rule.to_spl();
        let result = parse_spl(&spl);
        prop_assert!(
            result.is_ok(),
            "Multi-body rule to_spl() should parse: '{}', error: {:?}",
            spl, result.err()
        );
    }
}

// =============================================================================
// Strict and defeater rule SPL round-trips
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Strict rules' to_spl() should parse.
    #[test]
    fn strict_rule_to_spl_is_parseable(
        body_atom in proptest::sample::select(ATOMS),
        head_atom in proptest::sample::select(ATOMS),
    ) {
        let rule = Rule::strict("s-test", vec![Literal::simple(body_atom)], Literal::simple(head_atom));
        let spl = rule.to_spl();
        let result = parse_spl(&spl);
        prop_assert!(
            result.is_ok(),
            "Strict rule to_spl() should parse: '{}', error: {:?}",
            spl, result.err()
        );
    }

    /// Defeater rules' to_spl() should parse.
    #[test]
    fn defeater_rule_to_spl_is_parseable(
        body_atom in proptest::sample::select(ATOMS),
        head_atom in proptest::sample::select(ATOMS),
    ) {
        let rule = Rule::defeater("d-test", vec![Literal::simple(body_atom)], Literal::simple(head_atom));
        let spl = rule.to_spl();
        let result = parse_spl(&spl);
        prop_assert!(
            result.is_ok(),
            "Defeater rule to_spl() should parse: '{}', error: {:?}",
            spl, result.err()
        );
    }
}
