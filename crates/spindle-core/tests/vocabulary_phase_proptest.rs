//! SPEC-024 TEST-004 (negative-output): property test that literal-phase
//! classification is total and exclusive, and that the phase invariant holds —
//! a `Ground` result has no variable symbol; a `Pattern` result has at least
//! one variable, interval variable, or unresolved temporal endpoint.

use proptest::prelude::*;
use spindle_core::Literal;
use spindle_core::intern::intern;
use spindle_core::mode::Mode;
use spindle_core::term::Term;
use spindle_core::vocabulary::{ClassifiedLiteral, is_variable_symbol};

/// Build a literal from a functor spelling, argument spellings, and optional
/// interval-variable marker.
fn make_literal(functor: &str, args: &[String], interval_var: Option<&str>) -> Literal {
    let mut lit = Literal::from_ids(
        intern(functor),
        false,
        Mode::empty(),
        Default::default(),
        args.iter().map(|s| Term::Symbol(intern(s))).collect(),
    );
    lit.interval_var = interval_var.map(intern);
    lit
}

fn has_any_variable(lit: &Literal) -> bool {
    is_variable_symbol(lit.name_id())
        || lit
            .predicate_args()
            .iter()
            .any(|t| matches!(t, Term::Symbol(id) if is_variable_symbol(*id)))
        || lit.interval_var.is_some()
        || lit.temporal_expr.is_some()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(3000))]

    #[test]
    fn classify_is_total_and_matches_variable_content(
        functor in "[?a-z][a-z0-9]{0,4}",
        args in prop::collection::vec("[?a-z][a-z0-9]{0,4}", 0..4),
        interval in prop::option::of("\\?[A-Z]"),
    ) {
        let lit = make_literal(&functor, &args, interval.as_deref());
        let had_variable = has_any_variable(&lit);

        match lit.classify() {
            ClassifiedLiteral::Ground(g) => {
                // A ground result must contain no variable content.
                prop_assert!(!has_any_variable(g.as_literal()));
            }
            ClassifiedLiteral::Pattern(p) => {
                // A pattern must contain at least one variable/unresolved marker.
                prop_assert!(has_any_variable(p.as_literal()));
            }
        }

        // The classification agrees with the structural variable check.
        let is_pattern = matches!(
            make_literal(&functor, &args, interval.as_deref()).classify(),
            ClassifiedLiteral::Pattern(_)
        );
        prop_assert_eq!(is_pattern, had_variable);
    }
}
