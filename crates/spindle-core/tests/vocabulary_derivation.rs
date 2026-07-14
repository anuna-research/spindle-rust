//! SPEC-024 integration tests for theory-signature, vocabulary, shape, and
//! reasoning-parity behaviors (TEST-009, TEST-010, TEST-011, TEST-013,
//! TEST-016, TEST-019).

use spindle_core::arith::{ArithConstraint, ArithExpr, CmpOp};
use spindle_core::body::{BodyArg, BodyLiteral, BodyLogicLiteral};
use spindle_core::intern::intern;
use spindle_core::mode::Mode;
use spindle_core::rule::{RuleBody, RuleHead};
use spindle_core::term::Term;
use spindle_core::vocabulary::{
    ArgumentDecl, DeclarationOrigin, DeclarationState, MetaTarget, OccurrenceRole,
    PredicateDeclaration, PredicateSignature, PredicateSymbol, PrimitiveSort, TheorySignature,
    Vocabulary, VocabularyDiagnostic,
};
use spindle_core::{Literal, MetaValue, NumericValue, Rule, RuleType, Theory, reason};

fn sym(name: &str, arity: usize) -> PredicateSymbol {
    PredicateSymbol::try_new(name.into(), arity).unwrap()
}

fn lit(name: &str, negated: bool, args: &[&str]) -> Literal {
    Literal::new(
        name,
        negated,
        Mode::empty(),
        Default::default(),
        args.iter().map(|s| s.to_string()).collect(),
    )
}

fn body(elems: Vec<BodyLiteral>) -> RuleBody {
    elems.into_iter().collect()
}

fn head(l: Literal) -> RuleHead {
    std::iter::once(l).collect()
}

fn signature(name: &str, args: &[(&str, PrimitiveSort)]) -> PredicateSignature {
    PredicateSignature::try_new(
        sym(name, args.len()),
        args.iter()
            .map(|(n, s)| ArgumentDecl::new(*n, *s))
            .collect(),
    )
    .unwrap()
}

// ---------------------------------------------------------------------------
// TEST-009: Complete TheorySignature
// ---------------------------------------------------------------------------

#[test]
fn test_009_symbols_from_declarations_heads_and_bodies() {
    let mut theory = Theory::new();

    // r1: body [ request(?t), available(?a), (> ?x 1) ] head assign-to(?t, ?a)
    let arith = ArithConstraint::Compare {
        op: CmpOp::Gt,
        lhs: ArithExpr::Var(intern("?x")),
        rhs: ArithExpr::Lit(NumericValue::Integer(1)),
    };
    let r1 = Rule::new(
        "r1",
        RuleType::Defeasible,
        body(vec![
            BodyLiteral::Logic(BodyLogicLiteral::new(
                "request",
                false,
                Mode::empty(),
                Default::default(),
                vec![BodyArg::Term(Term::Symbol(intern("?t")))],
            )),
            BodyLiteral::Logic(BodyLogicLiteral::new(
                "available",
                false,
                Mode::empty(),
                Default::default(),
                vec![BodyArg::Term(Term::Symbol(intern("?a")))],
            )),
            BodyLiteral::Arithmetic(arith),
        ]),
        head(lit("assign-to", false, &["?t", "?a"])),
    );
    theory.add_rule(r1);

    // r2: body [ cost(?item, (+ ?a ?b)) ] head ~flies  (arith arg counts toward arity)
    let cost = BodyLogicLiteral::new(
        "cost",
        false,
        Mode::empty(),
        Default::default(),
        vec![
            BodyArg::Term(Term::Symbol(intern("?item"))),
            BodyArg::Arith(ArithExpr::Var(intern("?total"))),
        ],
    );
    let r2 = Rule::new(
        "r2",
        RuleType::Defeasible,
        body(vec![BodyLiteral::Logic(cost)]),
        head(lit("flies", true, &[])),
    );
    theory.add_rule(r2);

    // r3: two heads collapsing on polarity: p(a) and ~p(b) both p/1
    let r3 = Rule::new(
        "r3",
        RuleType::Defeasible,
        body(vec![BodyLiteral::simple("trigger")]),
        {
            let mut h: RuleHead = head(lit("p", false, &["a"]));
            h.push(lit("p", true, &["b"]));
            h
        },
    );
    theory.add_rule(r3);

    // Unobserved declaration.
    theory.add_predicate_declaration(PredicateDeclaration::new(
        signature("unused", &[("x", PrimitiveSort::Symbol)]),
        DeclarationOrigin::Programmatic,
    ));

    let ts = TheorySignature::derive(&theory);

    // Arithmetic constraint contributes no symbol; arithmetic arg keeps arity 2.
    let expected: Vec<PredicateSymbol> = vec![
        sym("assign-to", 2),
        sym("available", 1),
        sym("cost", 2),
        sym("flies", 0),
        sym("p", 1),
        sym("request", 1),
        sym("trigger", 0),
        sym("unused", 1),
    ];
    for s in &expected {
        assert!(ts.symbols.contains(s), "missing {}", s.indicator());
    }
    assert_eq!(ts.symbols.len(), expected.len());
    // Polarity collapse: only one p/1.
    assert_eq!(
        ts.symbols
            .iter()
            .filter(|s| s.functor().resolve() == "p")
            .count(),
        1
    );
}

// ---------------------------------------------------------------------------
// TEST-010: Declaration conflict handling
// ---------------------------------------------------------------------------

#[test]
fn test_010_identical_declarations_dedup_but_retain_origins() {
    let mut theory = Theory::new();
    let parsed = PredicateDeclaration::new(
        signature("q", &[("x", PrimitiveSort::Symbol)]),
        DeclarationOrigin::Parsed(spindle_core::vocabulary::SourceLocation::new(0, 1)),
    );
    let programmatic = PredicateDeclaration::new(
        signature("q", &[("x", PrimitiveSort::Symbol)]),
        DeclarationOrigin::Programmatic,
    );
    // Insert the programmatic one twice to prove exact-duplicate dedup.
    theory.add_predicate_declaration(parsed);
    theory.add_predicate_declaration(programmatic.clone());
    theory.add_predicate_declaration(programmatic);

    let ts = TheorySignature::derive(&theory);
    match ts.declarations.get(&sym("q", 1)).unwrap() {
        DeclarationState::Declared { origins, .. } => {
            assert_eq!(
                origins.len(),
                2,
                "distinct origins retained, duplicates removed"
            );
        }
        other => panic!("expected Declared, got {other:?}"),
    }

    // The merged origin order is canonical, not insertion order: inserting the
    // same two origins in the reverse order yields an identical state.
    let mut reversed = Theory::new();
    reversed.add_predicate_declaration(PredicateDeclaration::new(
        signature("q", &[("x", PrimitiveSort::Symbol)]),
        DeclarationOrigin::Programmatic,
    ));
    reversed.add_predicate_declaration(PredicateDeclaration::new(
        signature("q", &[("x", PrimitiveSort::Symbol)]),
        DeclarationOrigin::Parsed(spindle_core::vocabulary::SourceLocation::new(0, 1)),
    ));
    assert_eq!(
        TheorySignature::derive(&reversed)
            .declarations
            .get(&sym("q", 1)),
        ts.declarations.get(&sym("q", 1))
    );
}

#[test]
fn test_010_conflicting_declarations_retain_all_distinct() {
    // Two insertion orders must produce the same conflict, with no winner.
    fn build(order: bool) -> Theory {
        let mut theory = Theory::new();
        let a = PredicateDeclaration::new(
            signature("r", &[("x", PrimitiveSort::Symbol)]),
            DeclarationOrigin::Programmatic,
        );
        let b = PredicateDeclaration::new(
            signature("r", &[("x", PrimitiveSort::Integer)]),
            DeclarationOrigin::Parsed(spindle_core::vocabulary::SourceLocation::new(5, 2)),
        );
        if order {
            theory.add_predicate_declaration(a);
            theory.add_predicate_declaration(b);
        } else {
            theory.add_predicate_declaration(b);
            theory.add_predicate_declaration(a);
        }
        theory
    }

    let forward = TheorySignature::derive(&build(true));
    let reverse = TheorySignature::derive(&build(false));

    for ts in [&forward, &reverse] {
        match ts.declarations.get(&sym("r", 1)).unwrap() {
            DeclarationState::Conflict(decls) => {
                assert_eq!(decls.len(), 2, "every distinct declaration retained");
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    // No winner is selected by insertion order: the two orders produce an
    // identical, canonically-ordered conflict (SPEC-024 NFR-001, REQ-010).
    assert_eq!(
        forward.declarations.get(&sym("r", 1)),
        reverse.declarations.get(&sym("r", 1))
    );
}

// ---------------------------------------------------------------------------
// TEST-011: Vocabulary provenance
// ---------------------------------------------------------------------------

#[test]
fn test_011_vocabulary_origins_and_description() {
    let mut theory = Theory::new();
    // r1: head p(a)
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        body(vec![BodyLiteral::simple("trigger")]),
        head(lit("p", false, &["a"])),
    ));
    // r2: body p(?x) head other
    theory.add_rule(Rule::new(
        "r2",
        RuleType::Defeasible,
        body(vec![BodyLiteral::Logic(BodyLogicLiteral::new(
            "p",
            false,
            Mode::empty(),
            Default::default(),
            vec![BodyArg::Term(Term::Symbol(intern("?x")))],
        ))]),
        head(lit("other", false, &[])),
    ));
    theory.add_meta_target(
        MetaTarget::Predicate(sym("p", 1)),
        "description",
        MetaValue::String("A p predicate.".to_string()),
    );

    let report = Vocabulary::derive(&theory);
    let entry = report
        .vocabulary
        .entries
        .iter()
        .find(|e| e.symbol == sym("p", 1))
        .unwrap();

    assert_eq!(entry.description.as_deref(), Some("A p predicate."));
    // Origins sorted: r1 head then r2 body.
    assert_eq!(entry.origins.len(), 2);
    assert_eq!(entry.origins[0].rule, "r1");
    assert_eq!(entry.origins[0].role, OccurrenceRole::Head { index: 0 });
    assert_eq!(entry.origins[1].rule, "r2");
    assert_eq!(entry.origins[1].role, OccurrenceRole::Body { index: 0 });
    // Profile: position 0 observed symbol (a) and variable (?x).
    assert_eq!(entry.profile.positions[0].len(), 2);
}

#[test]
fn test_011_unresolved_predicate_metadata_diagnostic() {
    let mut theory = Theory::new();
    theory.add_fact("bird");
    // Metadata for a symbol with neither a declaration nor an occurrence.
    theory.add_meta_target(
        MetaTarget::Predicate(sym("ghost", 3)),
        "description",
        MetaValue::String("nobody".to_string()),
    );

    let report = Vocabulary::derive(&theory);
    assert!(
        report
            .diagnostics
            .contains(&VocabularyDiagnostic::UnresolvedPredicateMetadata {
                symbol: sym("ghost", 3),
            })
    );
    // ghost/3 is not joined by name into any entry.
    assert!(
        !report
            .vocabulary
            .entries
            .iter()
            .any(|e| e.symbol == sym("ghost", 3))
    );
}

#[test]
fn malformed_functor_is_surfaced_not_silently_dropped() {
    // A functor an existing input can still create — here an empty functor via
    // add_fact("") — cannot form a predicate symbol. It must be counted and
    // reported, not silently erased from the derived model (SPEC-024 CON-001).
    let mut theory = Theory::new();
    theory.add_fact("");

    let report = Vocabulary::derive(&theory);
    assert_eq!(report.summary.observed_occurrences, 1);
    assert_eq!(report.summary.malformed_predicates, 1);
    assert!(report.diagnostics.iter().any(|d| matches!(
        d,
        VocabularyDiagnostic::MalformedPredicate { functor, arity }
            if functor.is_empty() && *arity == 0
    )));
    // The malformed occurrence produces no catalogue entry.
    assert!(report.vocabulary.entries.is_empty());
}

#[test]
fn repeated_malformed_functor_reported_once() {
    let mut theory = Theory::new();
    theory.add_fact("");
    theory.add_fact("");
    let report = Vocabulary::derive(&theory);
    // Both occurrences are counted, but the diagnostic is deduped.
    assert_eq!(report.summary.observed_occurrences, 2);
    assert_eq!(report.summary.malformed_predicates, 1);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|d| matches!(d, VocabularyDiagnostic::MalformedPredicate { .. }))
            .count(),
        1
    );
}

// ---------------------------------------------------------------------------
// TEST-013: Reasoning parity
// ---------------------------------------------------------------------------

#[test]
fn test_013_declarations_and_metadata_do_not_change_conclusions() {
    fn base() -> Theory {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");
        theory.add_defeasible_rule(&["penguin"], "~flies");
        theory
    }

    let plain = base();

    let mut with_valid = base();
    with_valid.add_predicate_declaration(PredicateDeclaration::new(
        signature("flies", &[("who", PrimitiveSort::Symbol)]),
        DeclarationOrigin::Programmatic,
    ));
    with_valid.add_meta_target(
        MetaTarget::Predicate(sym("flies", 0)),
        "description",
        MetaValue::String("can fly".to_string()),
    );

    let mut with_conflict = base();
    with_conflict.add_predicate_declaration(PredicateDeclaration::new(
        signature("flies", &[("a", PrimitiveSort::Symbol)]),
        DeclarationOrigin::Programmatic,
    ));
    with_conflict.add_predicate_declaration(PredicateDeclaration::new(
        signature("flies", &[("a", PrimitiveSort::Integer)]),
        DeclarationOrigin::Programmatic,
    ));

    let render = |t: &Theory| -> Vec<String> {
        let mut v: Vec<String> = reason(t)
            .unwrap()
            .iter()
            .map(|c| format!("{} {}", c.conclusion_type.symbol(), c.literal))
            .collect();
        v.sort();
        v
    };

    let expected = render(&plain);
    assert_eq!(render(&with_valid), expected);
    assert_eq!(render(&with_conflict), expected);
}

// ---------------------------------------------------------------------------
// TEST-016: Deterministic output
// ---------------------------------------------------------------------------

#[test]
fn test_016_insertion_order_independence() {
    fn build(reverse: bool) -> Theory {
        let mut theory = Theory::new();
        let mut facts = vec!["apple", "mango", "zebra"];
        if reverse {
            facts.reverse();
        }
        for f in facts {
            theory.add_fact(f);
        }
        // Exercise the previously-masked cases: a multi-origin `Declared` and a
        // multi-declaration `Conflict`, both order-sensitive if not canonicalized.
        let loc = spindle_core::vocabulary::SourceLocation::new(3, 1);
        let mut decls = vec![
            // apple/0 declared from two distinct origins (multi-origin merge).
            PredicateDeclaration::new(signature("apple", &[]), DeclarationOrigin::Programmatic),
            PredicateDeclaration::new(signature("apple", &[]), DeclarationOrigin::Parsed(loc)),
            // widget/1 declared with conflicting sorts (conflict payload).
            PredicateDeclaration::new(
                signature("widget", &[("x", PrimitiveSort::Symbol)]),
                DeclarationOrigin::Programmatic,
            ),
            PredicateDeclaration::new(
                signature("widget", &[("x", PrimitiveSort::Integer)]),
                DeclarationOrigin::Programmatic,
            ),
        ];
        if reverse {
            decls.reverse();
        }
        for d in decls {
            theory.add_predicate_declaration(d);
        }
        theory
    }

    // Facts get auto labels (f1, f2, ...) which differ by insertion order, so
    // compare the symbol set and declaration map (label-independent).
    let a = TheorySignature::derive(&build(false));
    let b = TheorySignature::derive(&build(true));
    assert_eq!(a.symbols, b.symbols);
    assert_eq!(a.declarations, b.declarations);
}

// ---------------------------------------------------------------------------
// TEST-019: Derivation summary
// ---------------------------------------------------------------------------

#[test]
fn test_019_summary_counts_match_hand_enumeration() {
    let mut theory = Theory::new();
    // Observed: bird/0 (fact head), flies/0 (rule head), bird/0 (rule body).
    theory.add_fact("bird");
    theory.add_defeasible_rule(&["bird"], "flies");
    // Declared + observed: flies/0.
    theory.add_predicate_declaration(PredicateDeclaration::new(
        signature("flies", &[]),
        DeclarationOrigin::Programmatic,
    ));
    // Conflicting declaration for a fresh symbol widget/0.
    theory.add_predicate_declaration(PredicateDeclaration::new(
        signature("widget", &[]),
        DeclarationOrigin::Programmatic,
    ));
    theory.add_predicate_declaration(PredicateDeclaration::new(
        PredicateSignature::try_new(
            sym("widget", 1),
            vec![ArgumentDecl::new("x", PrimitiveSort::Any)],
        )
        .unwrap(),
        DeclarationOrigin::Programmatic,
    ));
    // Unresolved predicate metadata for ghost/2.
    theory.add_meta_target(
        MetaTarget::Predicate(sym("ghost", 2)),
        "description",
        MetaValue::String("x".to_string()),
    );

    let report = Vocabulary::derive(&theory);
    let s = report.summary;

    // Distinct symbols: bird/0, flies/0, widget/0, widget/1  = 4.
    assert_eq!(s.distinct_symbols, 4);
    // Observed occurrences: fact head bird, rule head flies, rule body bird = 3.
    assert_eq!(s.observed_occurrences, 3);
    // Raw declarations stored = 3.
    assert_eq!(s.declarations, 3);
    // widget/0 vs widget/1 are different symbols; each is Declared, no conflict.
    assert_eq!(s.declaration_conflicts, 0);
    // Unresolved predicate metadata: ghost/2 = 1.
    assert_eq!(s.unresolved_predicate_metadata, 1);
    // Undeclared uses: bird/0 (observed, not declared) = 1.
    assert_eq!(s.undeclared_uses, 1);
}
