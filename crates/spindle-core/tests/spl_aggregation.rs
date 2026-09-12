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
            "(normally r (bind ?n (fold + 1 :from p :initial 0)) (n ?n))",
            "aggregate-domain",
        ),
        (
            "(aggregate-domain 0 1) (normally r (bind ?n (fold + 1 :from (p ?v) :initial 0)) (p ?n))",
            "cycle",
        ),
        (
            "(aggregate-domain 0 1) (given p) (normally r (bind ?n (fold + 2 :from p :initial 0)) (n ?n))",
            "outside domain",
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
    opts.grounding.max_instances = 2;
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
