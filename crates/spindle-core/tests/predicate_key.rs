use std::collections::{BTreeSet, HashSet};

use spindle_core::prelude::*;

#[test]
fn identity_separates_functors_and_arities_in_collections() {
    let keys = [
        PredicateKey::new(intern("ci-green"), 0),
        PredicateKey::new(intern("ci-green"), 1),
        PredicateKey::new(intern("ci-green"), 2),
        PredicateKey::new(intern("other"), 1),
        PredicateKey::new(intern("ci-green"), 1),
    ];
    assert_eq!(keys.into_iter().collect::<HashSet<_>>().len(), 4);
    assert_eq!(keys.into_iter().collect::<BTreeSet<_>>().len(), 4);
    assert_eq!(keys[1].functor_id(), intern("ci-green"));
    assert_eq!(keys[1].functor(), "ci-green");
    assert_eq!(keys[1].arity(), 1);
    assert_eq!(keys[1].to_string(), "ci-green/1");
    assert_eq!(Literal::simple("ci-green").predicate_key(), keys[0]);
}

#[test]
fn occurrence_details_do_not_change_identity() {
    let plain = Literal::new(
        "p",
        false,
        Mode::empty(),
        Temporal::empty(),
        vec!["a".into()],
    );
    let variants = [
        plain.complement(),
        Literal::new(
            "p",
            false,
            Mode::obligation(),
            Temporal::empty(),
            vec!["a".into()],
        ),
        Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(2)),
            vec!["a".into()],
        ),
        Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["b".into()],
        ),
    ];
    for variant in variants {
        assert_eq!(plain.predicate_key(), variant.predicate_key());
    }
}

#[test]
fn body_identity_counts_arithmetic_positions() {
    let body = BodyLogicLiteral::new(
        "p",
        true,
        Mode::permission(),
        Temporal::new(TimePoint::Moment(3), TimePoint::Moment(4)),
        vec![
            BodyArg::Term(Term::Symbol(intern("a"))),
            BodyArg::Arith(ArithExpr::Var(intern("?x"))),
        ],
    );
    let head = Literal::new(
        "p",
        false,
        Mode::empty(),
        Temporal::empty(),
        vec!["b".into(), "c".into()],
    );
    assert_eq!(body.predicate_key(), head.predicate_key());
    assert_eq!(body.predicate_key().arity(), 2);
}

#[test]
fn key_accepts_full_arity_range_and_all_literal_names() {
    assert_eq!(
        PredicateKey::new(intern("p"), usize::MAX).arity(),
        usize::MAX
    );
    for name in ["", "a\nb", "rate/limit"] {
        assert_eq!(Literal::simple(name).predicate_key().functor(), name);
    }
}
