use spindle_core::{
    function_registry::{Arity, EvalError, ExtensionFunction, FunctionRegistry, FunctionSignature},
    intern::intern,
    pipeline::{PrepareOptions, prepare},
    reason::{reason_prepared, reason_with_options},
    term::{FiniteFloat, Term},
};
fn conclusions(s: &str) -> Vec<String> {
    reason_with_options(
        &spindle_parser::parse_spl(s).unwrap(),
        PrepareOptions::default(),
    )
    .unwrap()
    .into_iter()
    .map(|c| c.to_string())
    .collect()
}
const SIMPLE: &str = "(given (raw 10)) (normally derive (raw ?v) (pay ?v)) (normally total (fold ?t 0 + ?v (pay ?v)) (total ?t))";
#[test]
fn cli_path_matches_library() {
    let t = spindle_parser::parse_spl(SIMPLE).unwrap();
    let p = prepare(&t, PrepareOptions::default()).unwrap();
    let cli: Vec<_> = reason_prepared(&p.theory)
        .unwrap()
        .into_iter()
        .map(|c| c.to_string())
        .collect();
    let lib = conclusions(SIMPLE);
    println!("cli={cli:?}, lib={lib:?}");
    assert_eq!(cli, lib);
}
#[test]
fn preserves_defeasible_strength() {
    let c = conclusions(SIMPLE);
    assert!(!c.contains(&"+D pay(10)".to_string()), "{c:?}");
}
#[test]
fn rejects_transitive_cycle() {
    let t = spindle_parser::parse_spl(
        "(given (a 1)) (normally f (fold ?s 0 + ?x (a ?x)) (b ?s)) (normally r (b ?x) (a ?x))",
    )
    .unwrap();
    let result = prepare(&t, PrepareOptions::default());
    assert!(
        result.is_err(),
        "accepted strata: {:?}",
        result.unwrap().stratum_info
    );
}
#[test]
fn transitive_chain_total() {
    let c = conclusions(
        "(given (raw 10)) (normally r0 (raw ?v) (a ?v)) (normally fold-one (fold ?s 0 + ?x (a ?x)) (b ?s)) (normally r1 (b ?x) (c ?x)) (normally fold-two (fold ?s 0 + ?x (c ?x)) (total ?s))",
    );
    assert!(
        !c.iter()
            .any(|s| s.starts_with("+d total(0)") || s.starts_with("+D total(0)")),
        "{c:?}"
    );
    assert!(
        c.iter().any(|s| s == "+d total(10)" || s == "+D total(10)"),
        "{c:?}"
    );
}
#[test]
fn defeated_input_excluded() {
    let c = conclusions(
        "(given (raw 10)) (normally yes (raw ?v) (pay ?v)) (normally no (raw ?v) (not (pay ?v))) (prefer no yes) (normally total (fold ?t 0 + ?v (pay ?v)) (total ?t))",
    );
    assert!(c.contains(&"+d total(0)".to_string()), "{c:?}");
    assert!(!c.contains(&"+d total(10)".to_string()), "{c:?}");
}
#[test]
fn floor_ceil_overflow() {
    let reg = FunctionRegistry::with_prelude();
    for name in ["floor", "ceil"] {
        let result = reg.get(intern(name)).unwrap().eval(&[Term::Float(
            FiniteFloat::new(9223372036854775808.0).unwrap(),
        )]);
        assert!(result.is_err(), "{name} returned {result:?}");
    }
}
struct SymbolFn(FunctionSignature);
impl ExtensionFunction for SymbolFn {
    fn signature(&self) -> &FunctionSignature {
        &self.0
    }
    fn eval(&self, _: &[Term]) -> Result<Term, EvalError> {
        Ok(Term::Symbol(intern("monday")))
    }
}
#[test]
fn extension_can_bind_symbol() {
    let mut reg = FunctionRegistry::with_prelude();
    reg.register(Box::new(SymbolFn(FunctionSignature {
        name: intern("weekday"),
        arity: Arity::Fixed(0),
        description: "test",
    })));
    let t = spindle_parser::parse_spl("(normally r (bind ?d (weekday)) (day ?d))").unwrap();
    let c: Vec<_> = reason_with_options(
        &t,
        PrepareOptions {
            function_registry: Some(reg),
            ..Default::default()
        },
    )
    .unwrap()
    .into_iter()
    .map(|c| c.to_string())
    .collect();
    assert!(c.contains(&"+d day(monday)".to_string()), "{c:?}");
}

#[test]
fn reducer_must_be_order_independent() {
    let t = spindle_parser::parse_spl(
        "(given (v 10)) (given (v 3)) (normally total (fold ?s required - ?x (v ?x)) (total ?s))",
    )
    .unwrap();
    assert!(
        prepare(&t, PrepareOptions::default()).is_err(),
        "accepted order-dependent subtraction reducer"
    );
}
#[test]
fn multi_stratum_respects_reference_time() {
    use spindle_core::{
        literal::Literal,
        rule::Rule,
        temporal::{Temporal, TimePoint},
    };
    let mut t = spindle_parser::parse_spl(SIMPLE).unwrap();
    let mut lit = Literal::simple("expired");
    lit.temporal = Temporal::new(TimePoint::from_millis(100), TimePoint::from_millis(200));
    t.add_rule(Rule::fact("expired-fact", lit));
    let result = reason_with_options(
        &t,
        PrepareOptions {
            reference_time: Some(TimePoint::from_millis(300)),
            ..Default::default()
        },
    );
    if let Ok(cs) = result {
        assert!(
            !cs.iter()
                .any(|c| c.is_positive() && c.literal.name() == "expired"),
            "{:?}",
            cs.iter().map(|c| c.to_string()).collect::<Vec<_>>()
        );
    }
}
