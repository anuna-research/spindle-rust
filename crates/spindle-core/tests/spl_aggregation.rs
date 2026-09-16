use spindle_core::{
    pipeline::{PrepareOptions, prepare},
    reason::{reason, reason_prepared},
};
use spindle_parser::parse_spl;

fn positive(source: &str) -> Vec<String> {
    reason(&parse_spl(source).unwrap())
        .unwrap()
        .into_iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| format!("{} {}", c.conclusion_type, c.literal.to_spl()))
        .collect()
}

#[test]
fn completed_snapshot_and_strict_evidence() {
    let source = "(aggregate-domain 0 10 20 30)
        (given (amount 10)) (given (amount 20)) (given (amount 10))
        (always total (bind ?s (fold + ?v :from (amount ?v) :initial 0)) (total ?s))";
    let output = positive(source);
    assert!(output.contains(&"+d (total 30)".into()), "{output:?}");
    assert!(!output.contains(&"+D (total 30)".into()));
    assert!(!output.iter().any(|s| s.contains("snapshot")));
    let raw = parse_spl(source).unwrap();
    assert!(reason_prepared(&raw).is_err());
    let prepared = prepare(&raw, PrepareOptions::default()).unwrap();
    assert!(
        prepared
            .theory
            .rules()
            .any(|r| r.template_label() == "total")
    );
    assert_eq!(
        reason_prepared(&prepared.theory).unwrap().len(),
        reason(&raw).unwrap().len()
    );
}

#[test]
fn grouping_and_equal_contributions() {
    let output = positive(
        "(aggregate-domain alice bob 0 1 2 10 20)
        (given (person alice)) (given (person bob))
        (given (payment alice 1 10)) (given (payment alice 2 10))
        (normally totals (and (person ?p)
          (bind ?n (fold + ?cost :from (payment ?p ?id ?cost) :initial 0))) (total ?p ?n))",
    );
    assert!(output.contains(&"+d (total alice 20)".into()), "{output:?}");
    assert!(output.contains(&"+d (total bob 0)".into()), "{output:?}");
}

#[test]
fn defeated_rows_and_ordinary_dependency_chain() {
    let output = positive(
        "(aggregate-domain 0 10)
        (normally yes () (amount 10)) (normally no () (not (amount 10))) (prefer no yes)
        (normally first (bind ?s (fold + ?v :from (amount ?v) :initial 0)) (total ?s))
        (always forward (total ?s) (copy ?s))
        (normally second (bind ?s (fold + ?v :from (copy ?v) :initial 0)) (final ?s))",
    );
    assert!(output.contains(&"+d (final 0)".into()), "{output:?}");
    assert!(
        !output
            .iter()
            .any(|s| s == "+d (total 10)" || s == "+d (final 10)")
    );
}

#[test]
fn empty_input_policies_and_count() {
    let output = positive(
        "(aggregate-domain 0 1 2)
        (given (row 1)) (given (row 2))
        (normally count-rows (bind ?n (fold + 1 :from (row ?v) :initial 0)) (size ?n))
        (normally minimum (bind ?n (fold min ?v :from (missing ?v) :require-nonempty)) (least ?n))",
    );
    assert!(output.contains(&"+d (size 2)".into()));
    assert!(!output.iter().any(|s| s.contains("least")));
}

#[test]
fn rejects_invalid_semantics() {
    for (source, message) in [
        (
            "(normally r (agg ?n sum ?v (p ?v)) (n ?missing ?n))",
            "unsafe head",
        ),
        (
            "(aggregate-domain 0 1) (normally r (bind ?n (fold + 1 :from (p ?v) :initial 0)) (p ?n))",
            "cycle",
        ),
        (
            "(normally r (and (> ?x 0) (agg ?n sum ?v (p ?v))) (n ?n))",
            "unsafe variable",
        ),
        (
            "(aggregate-domain 0) (normally r (bind ?n (+ 1 (fold + 1 :from p :initial 0))) (n ?n))",
            "direct expression",
        ),
        (
            "(aggregate-domain 0) (normally r (bind ?n (fold + 1 :from (must p) :initial 0)) (n ?n))",
            "modal",
        ),
    ] {
        let error = reason(&parse_spl(source).unwrap()).unwrap_err().to_string();
        assert!(error.contains(message), "{error}; expected {message}");
    }
}

#[test]
fn malformed_syntax_is_rejected() {
    for fold in [
        "(fold - ?v :from (p ?v) :initial 0)",
        "(fold + ?v :from (p ?v))",
        "(fold + ?v :from (p ?v) :initial 0 :require-nonempty)",
        "(fold + ?v :initial 0 :initial 1)",
    ] {
        assert!(parse_spl(&format!("(normally r (bind ?n {fold}) (n ?n))")).is_err());
    }
    assert!(parse_spl("(aggregate-domain ?x)").is_err());
    assert!(parse_spl("(aggregate-domain 0) (aggregate-domain 1)").is_err());
}

#[test]
fn explicit_grounding_limit_is_enforced() {
    let source = parse_spl("(aggregate-domain 0 1 2) (normally r (bind ?n (fold + 1 :from (p ?a ?b) :initial 0)) (n ?n))").unwrap();
    let mut opts = PrepareOptions::default();
    opts.grounding.max_instances = 1;
    assert!(
        prepare(&source, opts)
            .err()
            .unwrap()
            .to_string()
            .contains("limit")
    );
}

#[test]
fn builtin_extraction_and_comparison() {
    let output = positive(
        "(aggregate-domain 0 1 2 6) (given (p 1)) (given (p 2))
        (normally r (and (bind ?n (fold + (* 2 ?v) :from (p ?v) :initial 0)) (> ?n 0)) (n ?n))",
    );
    assert!(output.contains(&"+d (n 6)".into()), "{output:?}");
}

#[test]
fn registered_extension_extracts_each_row_and_returns_symbols_in_bind() {
    use spindle_core::{
        function_registry::{
            Arity, EvalError, ExtensionFunction, FunctionRegistry, FunctionSignature,
        },
        intern::intern,
        term::Term,
    };
    struct Identity(FunctionSignature);
    impl ExtensionFunction for Identity {
        fn signature(&self) -> &FunctionSignature {
            &self.0
        }
        fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
            Ok(args[0].clone())
        }
    }
    let mut registry = FunctionRegistry::new();
    registry.register(Box::new(Identity(FunctionSignature {
        name: intern("identity"),
        arity: Arity::Fixed(1),
        description: "identity",
    })));
    for source in [
        "(normally r (bind ?s (identity (identity monday))) (day ?s))",
        "(aggregate-domain monday 0 1 2) (given (p 2))
          (normally r (and (bind ?s (identity monday)) (bind ?n (fold + (identity ?v) :from (p ?v) :initial 0))) (day ?s))",
    ] {
        let opts = PrepareOptions { function_registry: Some(registry.clone()), ..Default::default() };
        let output = spindle_core::reason::reason_with_options(&parse_spl(source).unwrap(), opts).unwrap();
        assert!(output.iter().any(|c| c.is_positive() && c.literal.to_spl() == "(day monday)"), "{output:?}");
    }
}

#[test]
fn internal_guard_cannot_collide_with_user_predicate() {
    let output = positive(
        "(aggregate-domain 0) (given (not __aggregate_snapshot_0))
      (normally r (bind ?n (fold + 1 :from absent :initial 0)) (n ?n))",
    );
    assert!(output.contains(&"+d (n 0)".into()), "{output:?}");
    assert!(
        output
            .iter()
            .any(|s| s.contains("(not (__aggregate_snapshot_0))"))
    );
}

#[test]
fn float_rounding_rejects_two_to_the_sixty_three() {
    use spindle_core::{
        function_registry::FunctionRegistry,
        intern::intern,
        term::{FiniteFloat, Term},
    };
    let registry = FunctionRegistry::with_prelude();
    for name in ["floor", "ceil"] {
        let f = registry.get(intern(name)).unwrap();
        assert!(
            f.eval(&[Term::Float(
                FiniteFloat::new(9223372036854775808.0).unwrap()
            )])
            .is_err()
        );
        assert_eq!(
            f.eval(&[Term::Float(
                FiniteFloat::new(-9223372036854775808.0).unwrap()
            )])
            .unwrap(),
            Term::Integer(i64::MIN)
        );
    }
}

#[test]
fn lowered_explanation_includes_defeasible_snapshot_evidence() {
    let raw = parse_spl(
        "(aggregate-domain 0) (always r (bind ?n (fold + 1 :from absent :initial 0)) (n ?n))",
    )
    .unwrap();
    let prepared = prepare(&raw, Default::default()).unwrap();
    let conclusions = reason_prepared(&prepared.theory).unwrap();
    let head = &conclusions
        .iter()
        .find(|c| c.is_positive() && c.literal.name() == "n")
        .unwrap()
        .literal;
    let proof = spindle_core::explanation::explain(&prepared.theory, head)
        .unwrap()
        .unwrap();
    let tree = proof.proof_tree.unwrap();
    let step = tree.proof_step.unwrap();
    assert_eq!(step.body_proofs.len(), 1);
    assert_eq!(
        step.body_proofs[0].derivation_type,
        spindle_core::explanation::DerivationType::Defeasible
    );
}

#[test]
fn named_aggregators_group_rows_and_define_empty_behavior() {
    let source = "(aggregate-domain alice bob first second 0 1 2 10 20)
        (given (person alice)) (given (person bob))
        (given (payment alice first 10)) (given (payment alice second 10))
        (normally totals (and (person ?p)
          (agg ?n sum ?cost (payment ?p ?id ?cost))) (total ?p ?n))
        (normally counts (and (person ?p)
          (agg ?n count ?id (payment ?p ?id ?cost))) (number ?p ?n))
        (normally minima (and (person ?p)
          (agg ?n min-of ?cost (payment ?p ?id ?cost))) (smallest ?p ?n))
        (normally maxima (and (person ?p)
          (agg ?n max-of ?cost (payment ?p ?id ?cost))) (largest ?p ?n))";
    let out = positive(source);
    for expected in [
        "+d (total alice 20)",
        "+d (total bob 0)",
        "+d (number alice 2)",
        "+d (number bob 0)",
        "+d (smallest alice 10)",
        "+d (largest alice 10)",
    ] {
        assert!(out.contains(&expected.to_string()), "{out:?}");
    }
    assert!(
        !out.iter()
            .any(|s| s.contains("(smallest bob") || s.contains("(largest bob"))
    );
}

#[test]
fn named_aggregate_is_a_binding_constraint_with_snapshot_evidence() {
    let source = "(aggregate-domain 0 10 20)
        (given (amount 10)) (given (expected 10)) (given (expected 20))
        (always total (and (expected ?n) (agg ?n sum ?v (amount ?v))) (total ?n))";
    let out = positive(source);
    assert!(out.contains(&"+d (total 10)".into()));
    assert!(!out.contains(&"+D (total 10)".into()));
    assert!(!out.contains(&"+d (total 20)".into()));
}

#[test]
fn unknown_aggregator_is_a_preparation_error() {
    let t =
        parse_spl("(aggregate-domain 0) (normally r (agg ?n median ?v (amount ?v)) (total ?n))")
            .unwrap();
    let err = prepare(&t, PrepareOptions::default())
        .err()
        .expect("unknown aggregator must fail")
        .to_string();
    assert!(err.contains("unknown aggregator 'median'"), "{err}");
}

#[test]
fn host_registered_aggregator_uses_its_combiner_and_identity() {
    use spindle_core::function_registry::{
        AggregatorDefinition, Arity, EvalError, ExtensionFunction, FunctionRegistry,
        FunctionSignature,
    };
    use spindle_core::{intern::intern, term::Term};
    struct BitOr(FunctionSignature);
    impl ExtensionFunction for BitOr {
        fn signature(&self) -> &FunctionSignature {
            &self.0
        }
        fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
            match args {
                [Term::Integer(a), Term::Integer(b)] => Ok(Term::Integer(a | b)),
                _ => Err(EvalError::TypeError("bit-or requires integers".into())),
            }
        }
    }
    let mut registry = FunctionRegistry::new();
    registry.register(Box::new(BitOr(FunctionSignature {
        name: intern("bit-or"),
        arity: Arity::Fixed(2),
        description: "bitwise union".into(),
    })));
    registry.register_aggregator(
        "bit-union",
        AggregatorDefinition {
            reducer: "bit-or".into(),
            identity: Some(0),
            count: false,
        },
    );
    let t = parse_spl(
        "(aggregate-domain 0 1 2 3)
        (given (bits 1)) (given (bits 2))
        (normally r (agg ?n bit-union ?b (bits ?b)) (result ?n))",
    )
    .unwrap();
    let prepared = prepare(
        &t,
        PrepareOptions {
            function_registry: Some(registry),
            ..Default::default()
        },
    )
    .unwrap();
    let out = reason_prepared(&prepared.theory).unwrap();
    assert!(
        out.iter()
            .any(|c| c.conclusion_type.is_positive() && c.literal.to_spl() == "(result 3)")
    );
}

#[test]
fn predicates_supply_derived_rows_and_new_values_flow_to_later_aggregates() {
    let out = positive(
        "(given (person alice)) (given (person bob))
      (given (purchase alice first 30)) (given (purchase alice second 45))
      (normally eligible (purchase ?p ?id ?cost) (payment ?p ?id ?cost))
      (always total (and (person ?p) (agg ?n sum ?v (payment ?p ?id ?v))) (total ?p ?n))
      (normally copy (total ?p ?n) (copied ?p ?n))
      (normally grand (agg ?n sum ?v (copied ?p ?v)) (grand ?n))",
    );
    for expected in [
        "+d (total alice 75)",
        "+d (total bob 0)",
        "+d (copied alice 75)",
        "+d (grand 75)",
    ] {
        assert!(out.contains(&expected.into()), "{out:?}");
    }
    assert!(!out.contains(&"+D (total alice 75)".into()));
}

#[test]
fn generated_bind_values_do_not_require_enumeration() {
    let out = positive(
        "(given (payment 30)) (given (payment 45))
      (normally total (agg ?n sum ?v (payment ?v)) (total ?n))
      (normally fee (and (total ?n) (bind ?m (+ ?n 1))) (with-fee ?m))",
    );
    assert!(out.contains(&"+d (with-fee 76)".into()), "{out:?}");
}

#[test]
fn predicate_grounding_retains_undecided_attackers() {
    let out = positive(
        "(given (person alice))
      (always loop (undecided ?x) (undecided ?x))
      (normally support (person ?x) (eligible ?x))
      (normally attack (and (person ?x) (undecided ?y)) (not (eligible ?x)))
      (normally count-eligible (agg ?n count ?x (eligible ?x)) (row-count ?n))",
    );
    assert!(!out.contains(&"+d (eligible alice)".into()), "{out:?}");
    assert!(out.contains(&"+d (row-count 0)".into()), "{out:?}");
}

#[test]
fn newly_computed_values_also_instantiate_earlier_cycles() {
    let out = positive(
        "(given (payment 30)) (given (payment 45))
      (always loop (undecided ?x) (undecided ?x))
      (normally total (agg ?n sum ?v (payment ?v)) (total ?n))
      (normally support (total ?n) (eligible ?n))
      (normally attack (and (total ?n) (undecided ?n)) (not (eligible ?n)))",
    );
    assert!(out.contains(&"+d (total 75)".into()), "{out:?}");
    assert!(!out.contains(&"+d (eligible 75)".into()), "{out:?}");
}

#[test]
fn obsolete_domain_does_not_constrain_predicates_or_outputs() {
    let out = positive(
        "(aggregate-domain 0)
      (given (payment 30)) (given (payment 45))
      (normally total (agg ?n sum ?v (payment ?v)) (total ?n))",
    );
    assert!(out.contains(&"+d (total 75)".into()), "{out:?}");
}

#[test]
fn value_generating_recursion_exhausts_the_grounding_budget() {
    let raw = parse_spl(
        "(given (p 1))
      (normally next (and (p ?x) (bind ?y (+ ?x 1))) (p ?y))
      (normally count-eligible (agg ?n count ?x (p ?x)) (row-count ?n))",
    )
    .unwrap();
    let mut opts = PrepareOptions::default();
    opts.grounding.max_instances = 100;
    let err = prepare(&raw, opts).err().unwrap().to_string();
    assert!(err.contains("limit"), "{err}");
}

#[test]
fn source_priority_survives_decoding_of_aggregate_instances() {
    let out = positive(
        "(given (payment 30)) (given (payment 45))
      (always loop (undecided ?x) (undecided ?x))
      (normally total (agg ?n sum ?v (payment ?v)) (total ?n))
      (normally support (total ?n) (eligible ?n))
      (always attack (and (total ?n) (undecided ?n)) (not (eligible ?n)))
      (prefer support attack)",
    );
    assert!(out.contains(&"+d (eligible 75)".into()), "{out:?}");
}
