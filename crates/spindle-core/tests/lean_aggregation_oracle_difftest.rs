//! Production Rust aggregate evaluator vs the verified Lean aggregate lowerer.
//! Build with `cd lean && lake build AggregationOracle`, then run this target
//! with `-- --ignored`. Missing binaries and malformed responses fail the test.
//! Compares all four tags on each evaluator's mentioned user-atom universe;
//! internal guards and Rust-only synthesized complements are not public outputs.

use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use spindle_core::aggregation::{
    self, Condition, Expression, Fold, Pattern, Program, SchemaRule, Term,
};
use spindle_core::{ConclusionType, RuleType};

fn int(n: i64) -> Term {
    Term::Integer(n)
}
fn var(v: &str) -> Term {
    Term::Variable(v.into())
}
fn expr(t: Term) -> Expression {
    Expression::Term(t)
}
fn atom(name: &str, args: Vec<Term>) -> Pattern {
    Pattern {
        name: name.into(),
        negation: false,
        args,
    }
}
fn rule(label: &str, kind: RuleType, body: Vec<Condition>, head: Pattern) -> SchemaRule {
    SchemaRule {
        label: label.into(),
        kind,
        body,
        heads: vec![head],
    }
}
fn total_fold() -> Fold {
    Fold {
        result_var: "n".into(),
        seed: Some(expr(int(0))),
        reducer: "+".into(),
        extract: expr(var("pay")),
        pattern: atom("shift", vec![var("id"), var("pay")]),
        grouping_vars: vec![],
    }
}
fn total(f: Fold) -> SchemaRule {
    rule(
        "total",
        RuleType::Strict,
        vec![Condition::Fold(f)],
        atom("total", vec![var("n")]),
    )
}
fn shift(label: &str, id: i64, pay: i64) -> SchemaRule {
    rule(
        label,
        RuleType::Defeasible,
        vec![],
        atom("shift", vec![int(id), int(pay)]),
    )
}
fn base() -> Program {
    Program {
        rules: vec![
            shift("one", 1, 25),
            shift("two", 2, 25),
            shift("dup", 1, 25),
            total(total_fold()),
        ],
        priorities: vec![],
    }
}
fn domain() -> Vec<Term> {
    [0, 1, 2, 7, 25, 32, 50, 57, 100]
        .into_iter()
        .map(int)
        .collect()
}

fn term_json(t: &Term) -> Value {
    match t {
        Term::Integer(n) => json!({"integer":n}),
        Term::Symbol(s) => json!({"symbol":s}),
        Term::Variable(v) => json!({"variable":v}),
    }
}
fn pattern_json(p: &Pattern) -> Value {
    json!({"name":p.name,"negation":p.negation,"args":p.args.iter().map(term_json).collect::<Vec<_>>()})
}
fn expression_json(e: &Expression) -> Value {
    match e {
        Expression::Term(t) => json!({"term":term_json(t)}),
        Expression::Call(name, args) | Expression::ExternalCall(name, args) => {
            json!({"call":name,"args":args.iter().map(expression_json).collect::<Vec<_>>()})
        }
    }
}
fn condition_json(c: &Condition) -> Value {
    match c {
        Condition::Logic(p) => json!({"logic":pattern_json(p)}),
        Condition::Bind(v, e) | Condition::BindValue(v, e) => {
            json!({"bind":v,"value":expression_json(e)})
        }
        Condition::Compare(op, a, b) => {
            json!({"compare":op,"left":expression_json(a),"right":expression_json(b)})
        }
        Condition::Fold(f) => {
            json!({"fold":{"result":f.result_var,"seed":f.seed.as_ref().map(expression_json),"reducer":f.reducer,
            "extract":expression_json(&f.extract),"pattern":pattern_json(&f.pattern),"grouping":f.grouping_vars}})
        }
    }
}
fn case_json(p: &Program, d: &[Term]) -> Value {
    json!({"rules":p.rules.iter().map(|r| json!({"label":r.label,"kind":match r.kind {
        RuleType::Fact=>"fact",RuleType::Strict=>"strict",RuleType::Defeasible=>"defeasible",RuleType::Defeater=>"defeater"},
        "heads":r.heads.iter().map(pattern_json).collect::<Vec<_>>(),"body":r.body.iter().map(condition_json).collect::<Vec<_>>() })).collect::<Vec<_>>(),
        "priorities":p.priorities,"domain":d.iter().map(term_json).collect::<Vec<_>>()})
}
type Tags = BTreeSet<(String, bool, Vec<String>, String)>;
fn normalize(j: &Value) -> Tags {
    j["conclusions"]
        .as_array()
        .expect("conclusion array")
        .iter()
        .map(|c| {
            let a = &c["atom"];
            (
                a["name"].as_str().expect("name").into(),
                a["negation"].as_bool().expect("negation"),
                a["args"]
                    .as_array()
                    .expect("arguments")
                    .iter()
                    .map(Value::to_string)
                    .collect(),
                c["tag"].as_str().expect("tag").into(),
            )
        })
        .collect()
}
fn rust_tags(result: &aggregation::Evaluation) -> Tags {
    result
        .conclusions
        .iter()
        .map(|c| {
            (
                c.atom.name.clone(),
                c.atom.negation,
                c.atom
                    .args
                    .iter()
                    .map(|t| term_json(t).to_string())
                    .collect(),
                c.tag.symbol().into(),
            )
        })
        .collect()
}
fn oracle(cases: &[Value]) -> Vec<Value> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../lean/.lake/build/bin/AggregationOracle");
    let mut child = Command::new(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("build AggregationOracle first ({}): {e}", path.display()));
    child
        .stdin
        .take()
        .unwrap()
        .write_all(json!({"cases":cases}).to_string().as_bytes())
        .expect("oracle input");
    let output = child.wait_with_output().expect("oracle process");
    assert!(
        output.status.success(),
        "oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: Value = serde_json::from_slice(&output.stdout).expect("oracle JSON");
    let results = response["results"]
        .as_array()
        .expect("oracle results")
        .clone();
    assert_eq!(results.len(), cases.len(), "truncated oracle batch");
    results
}

fn fixtures() -> Vec<(String, Program, Vec<Term>)> {
    let mut cases = vec![];
    let mut add = |name: &str, p| cases.push((name.into(), p, domain()));
    add(
        "duplicate rows / equal contributions / strict snapshot",
        base(),
    );
    let mut p = base();
    p.rules.reverse();
    add("rule permutation", p);
    let mut p = base();
    for r in &mut p.rules[..3] {
        r.kind = RuleType::Fact;
    }
    add("dual +D/+d input counted once", p);
    let mut p = base();
    p.rules[3].heads.push(atom("copy", vec![var("n")]));
    add("multiple heads", p);
    let mut f = total_fold();
    f.seed = Some(expr(int(7)));
    let mut p = base();
    p.rules[3] = total(f);
    add("nonidentity seed once", p);
    let mut p = base();
    let mut attack = atom("shift", vec![int(1), int(25)]);
    attack.negation = true;
    p.rules
        .push(rule("attack", RuleType::Defeater, vec![], attack));
    add("defeated row excluded", p.clone());
    p.priorities.push(("one".into(), "attack".into()));
    add("source priority expanded", p);
    let mut p = base();
    let mut f = total_fold();
    f.grouping_vars = vec!["id".into()];
    p.rules[3] = total(f);
    p.rules[3].heads = vec![atom("total", vec![var("id"), var("n")])];
    add("grouped folds", p);
    let mut p = base();
    let mut f = total_fold();
    f.pattern = atom("total", vec![var("pay")]);
    p.rules.push(rule(
        "grand",
        RuleType::Defeasible,
        vec![Condition::Fold(f)],
        atom("grand", vec![var("n")]),
    ));
    add("chained folds", p);
    let mut p = base();
    p.rules[3].body.extend([
        Condition::Bind(
            "m".into(),
            Expression::Call("*".into(), vec![expr(var("n")), expr(int(2))]),
        ),
        Condition::Compare(">".into(), expr(var("m")), expr(var("n"))),
    ]);
    p.rules[3].heads = vec![atom("double", vec![var("m")])];
    add("binds and comparisons", p);
    let mut p = base();
    p.rules[3].body.push(Condition::Logic(atom("gate", vec![])));
    add("residual false premise", p.clone());
    p.rules.push(rule(
        "gate",
        RuleType::Defeasible,
        vec![],
        atom("gate", vec![]),
    ));
    add("residual true premise", p);
    for reducer in ["sum", "min", "max"] {
        for seed in [None, Some(expr(int(0)))] {
            let mut f = total_fold();
            f.reducer = reducer.into();
            f.seed = seed;
            add(
                &format!("empty {reducer} seeded={}", f.seed.is_some()),
                Program {
                    rules: vec![total(f)],
                    priorities: vec![],
                },
            );
        }
    }
    let mut p = base();
    p.rules[3].heads = vec![atom("shift", vec![var("n"), int(25)])];
    add("aggregate cycle", p);
    let mut p = base();
    p.rules[1].label = p.rules[0].label.clone();
    add("duplicate source labels", p);
    let mut p = base();
    p.priorities.push(("missing".into(), "total".into()));
    add("unknown priority", p);
    let mut p = base();
    let mut f = total_fold();
    f.reducer = "unknown".into();
    p.rules[3] = total(f);
    add("unknown reducer", p.clone());
    cases.push(("unknown reducer with empty domain".into(), p, vec![]));
    cases.push((
        "out-of-domain result".into(),
        base(),
        vec![int(0), int(1), int(2), int(25)],
    ));
    let mut p = base();
    p.rules[0].heads[0].name = "?predicate".into();
    cases.push(("schematic predicate".into(), p, domain()));
    cases.push(("empty program".into(), Program::default(), vec![]));
    cases.push(("nonground domain".into(), base(), vec![var("bad")]));
    let mut p = base();
    p.rules[3].body.insert(
        0,
        Condition::Compare("=".into(), expr(int(0)), expr(int(1))),
    );
    p.rules[3].body.push(Condition::Bind(
        "bad".into(),
        expr(Term::Symbol("not-an-int".into())),
    ));
    cases.push((
        "false guard cannot hide expression error".into(),
        p,
        domain(),
    ));
    let mut p = base();
    p.rules[0].heads[0].args[0] = Term::Symbol("alice".into());
    p.rules.remove(2);
    let mut d = domain();
    d.push(Term::Symbol("alice".into()));
    cases.push(("symbol row identity".into(), p, d));
    let mut p = base();
    for r in &mut p.rules[..3] {
        r.heads[0].negation = true;
    }
    let mut f = total_fold();
    f.pattern.negation = true;
    p.rules[3] = total(f);
    cases.push(("strong-negated aggregate input".into(), p, domain()));
    let mut p = base();
    p.rules.push(rule(
        "recursive",
        RuleType::Defeasible,
        vec![Condition::Logic(atom("loop", vec![]))],
        atom("loop", vec![]),
    ));
    cases.push(("ordinary recursion allowed".into(), p, domain()));
    let mut target = atom("shift", vec![int(1), int(25)]);
    target.negation = true;
    let p = Program {
        rules: vec![
            rule("seed1", RuleType::Fact, vec![], atom("seed", vec![int(1)])),
            rule("seed2", RuleType::Fact, vec![], atom("seed", vec![int(2)])),
            rule(
                "producer",
                RuleType::Defeasible,
                vec![Condition::Logic(atom("seed", vec![var("id")]))],
                atom("shift", vec![var("id"), int(25)]),
            ),
            rule("attack", RuleType::Defeater, vec![], target),
            total(total_fold()),
        ],
        priorities: vec![("producer".into(), "attack".into())],
    };
    cases.push((
        "ground template priorities and instance retention".into(),
        p,
        domain(),
    ));
    // Deterministic small-scope generation: values, reducer, seed, input strength.
    for a in [0, 1, 2] {
        for b in [0, 1, 2] {
            for reducer in ["+", "min", "max"] {
                for seed in [None, Some(expr(int(1)))] {
                    let mut f = total_fold();
                    f.reducer = reducer.into();
                    f.seed = seed;
                    let mut one = shift("one", 1, a);
                    if a % 2 == 0 {
                        one.kind = RuleType::Fact;
                    }
                    let p = Program {
                        rules: vec![one, shift("two", 2, b), shift("dup", 1, a), total(f)],
                        priorities: vec![],
                    };
                    cases.push((
                        format!("generated {a} {b} {reducer}"),
                        p,
                        (0..=5).map(int).collect(),
                    ));
                }
            }
        }
    }
    cases
}

#[test]
#[ignore = "requires lake build AggregationOracle; enforced in Lean CI"]
fn aggregate_source_to_rust_differential() {
    let fixtures = fixtures();
    let inputs: Vec<_> = fixtures.iter().map(|(_, p, d)| case_json(p, d)).collect();
    let lean = oracle(&inputs);
    eprintln!(
        "Aggregate agreement suite: {} deterministic cases (known backend gap tested separately)",
        fixtures.len()
    );
    for ((name, p, d), expected) in fixtures.iter().zip(lean) {
        match aggregation::evaluate(p, d) {
            Err(e) => assert!(
                expected.get("error").is_some(),
                "{name}: Rust error {e}; Lean {expected}; input {}",
                case_json(p, d)
            ),
            Ok(actual) => {
                assert!(
                    expected.get("error").is_none(),
                    "{name}: Lean error {expected}; input {}",
                    case_json(p, d)
                );
                assert_eq!(
                    actual.stage_count,
                    expected["stage_count"].as_u64().unwrap() as usize,
                    "{name}: schedule"
                );
                let actual = rust_tags(&actual);
                let expected = normalize(&expected);
                assert_eq!(actual, expected, "{name}: input {}", case_json(p, d));
            }
        }
    }
}

#[test]
fn snapshot_policy_and_rows_regression() {
    let result = aggregation::evaluate(&base(), &domain()).unwrap();
    let query = atom("total", vec![int(50)]);
    assert!(
        result
            .conclusions
            .iter()
            .any(|c| c.atom == query && c.tag == ConclusionType::DefeasiblyProvable)
    );
    assert!(
        result
            .conclusions
            .iter()
            .any(|c| c.atom == query && c.tag == ConclusionType::DefinitelyNotProvable)
    );
    assert!(
        !result
            .conclusions
            .iter()
            .any(|c| c.atom == query && c.tag == ConclusionType::DefinitelyProvable)
    );
    assert!(aggregation::evaluate(&base(), &[int(25)]).is_err());
}

#[test]
fn checked_integer_overflow_is_an_error() {
    let p = Program {
        rules: vec![
            shift("a", 1, i64::MAX),
            shift("b", 2, 1),
            total(total_fold()),
        ],
        priorities: vec![],
    };
    assert!(
        aggregation::evaluate(&p, &[int(0), int(1), int(2), int(i64::MAX)])
            .unwrap_err()
            .contains("overflow")
    );
}

// The aggregate proof currently uses the older three-phase Lean reasoner.
// Rust uses constructive defeat-discard. Pin the *exact* known discrepancy;
// do not silently drop this case or call it a conformance success.
#[test]
#[ignore = "requires AggregationOracle; records the documented ordinary-backend gap"]
fn known_constructive_discard_changes_aggregate_snapshot() {
    let p = atom("p", vec![]);
    let mut not_p = p.clone();
    not_p.negation = true;
    let q = atom("q", vec![]);
    let mut not_q = q.clone();
    not_q.negation = true;
    let mut f = total_fold();
    f.pattern = q.clone();
    f.extract = expr(int(1));
    let program = Program {
        rules: vec![
            rule("p", RuleType::Defeasible, vec![], p.clone()),
            rule("not-p", RuleType::Defeasible, vec![], not_p),
            rule(
                "attack",
                RuleType::Defeater,
                vec![Condition::Logic(p)],
                not_q,
            ),
            rule("q", RuleType::Defeasible, vec![], q),
            total(f),
        ],
        priorities: vec![],
    };
    let d = vec![int(0), int(1)];
    let lean = normalize(&oracle(&[case_json(&program, &d)])[0]);
    let rust = rust_tags(&aggregation::evaluate(&program, &d).unwrap());
    let tag = |name: &str, args: Vec<Term>, t: &str| {
        (
            name.into(),
            false,
            args.iter().map(|v| term_json(v).to_string()).collect(),
            t.into(),
        )
    };
    let only_rust: Tags = rust.difference(&lean).cloned().collect();
    let only_lean: Tags = lean.difference(&rust).cloned().collect();
    assert_eq!(
        only_rust,
        BTreeSet::from([
            tag("q", vec![], "+d"),
            tag("total", vec![int(1)], "+d"),
            tag("total", vec![int(1)], "-D"),
        ])
    );
    assert_eq!(
        only_lean,
        BTreeSet::from([
            tag("q", vec![], "-d"),
            tag("total", vec![int(0)], "+d"),
            tag("total", vec![int(0)], "-D"),
        ])
    );
    eprintln!(
        "KNOWN BACKEND GAP: Rust count(q)=1; verified three-phase aggregate model count(q)=0 (constructive defeat-discard)"
    );
}

/// Exercise parser -> aggregate preparation -> ordinary Rust reasoner, not just the typed API.
#[test]
#[ignore = "requires the Lean AggregationOracle binary"]
fn spl_pipeline_agrees_with_lean_aggregate_oracle() {
    let mut sources = Vec::new();
    for reducer in ["+", "min", "max"] {
        for n in 0..4 {
            for required in [false, true] {
                let rows = (1..=n)
                    .map(|i| format!("(given (row {i}))"))
                    .collect::<String>();
                let policy = if required {
                    ":require-nonempty"
                } else {
                    ":initial 0"
                };
                sources.push(format!("(aggregate-domain 0 1 2 3 4 5 6) {rows}
                    (always total (bind ?n (fold {reducer} ?v :from (row ?v) {policy})) (total ?n))"));
            }
        }
    }
    sources.push(include_str!("../../../examples/aggregation.spl").into());
    sources.push(
        "(aggregate-domain 0 1 2 3 6) (given (row 1)) (given (row 2))
        (normally first (bind ?n (fold + (* 2 ?v) :from (row ?v) :initial 0)) (subtotal ?n))
        (always pass (subtotal ?n) (copy ?n))
        (normally last (bind ?n (fold + ?v :from (copy ?v) :initial 0)) (total ?n))"
            .into(),
    );
    let theories: Vec<_> = sources
        .iter()
        .map(|s| spindle_parser::parse_spl(s).unwrap())
        .collect();
    let cases: Vec<_> = theories
        .iter()
        .map(|t| {
            case_json(
                &aggregation::source::program(t).unwrap(),
                t.aggregate_domain().unwrap(),
            )
        })
        .collect();
    let answers = oracle(&cases);
    for ((source, theory), answer) in sources.iter().zip(theories).zip(answers) {
        assert!(answer.get("error").is_none(), "{source}: {answer}");
        let prepared = spindle_core::pipeline::prepare(&theory, Default::default()).unwrap();
        let mentioned: BTreeSet<_> = prepared
            .theory
            .rules()
            .flat_map(|r| {
                r.head.iter().cloned().chain(
                    r.body
                        .iter()
                        .filter_map(|b| b.as_logic().map(|b| b.to_literal())),
                )
            })
            .map(|l| l.to_spl())
            .collect();
        let tags: Tags = spindle_core::reason::reason_prepared(&prepared.theory)
            .unwrap()
            .iter()
            .filter(|c| mentioned.contains(&c.literal.to_spl()))
            .map(|c| {
                (
                    c.literal.name().into(),
                    c.literal.negation,
                    c.literal
                        .predicate_args()
                        .iter()
                        .map(|t| term_json(&aggregation::source::term(t).unwrap()).to_string())
                        .collect(),
                    c.conclusion_type.symbol().into(),
                )
            })
            .collect();
        assert_eq!(tags, normalize(&answer), "{source}");
    }
    eprintln!("26 SPL pipeline/Lean agreement cases passed");
}
