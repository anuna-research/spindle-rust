//! Direct ZK-program versus Lean comparison, not a transitive Rust comparison.
//! Build `lean/.lake/build/bin/AggregationOracle`, then run this test with
//! `-- --ignored`. SPINDLE_AGGREGATION_ORACLE may select another built binary.
//! Absence of the executable is a failure, never a skipped success.

use serde_json::{Value, json};
use spindle_core::{Rule, RuleType, aggregation as a};
use spindle_zk::{Program, parse_facts};
use std::{
    collections::BTreeSet,
    io::Write,
    process::{Command, Stdio},
};

fn term(t: &a::Term) -> Value {
    match t {
        a::Term::Integer(n) => json!({"integer": n}),
        a::Term::Symbol(s) => json!({"symbol": s}),
        a::Term::Variable(v) => json!({"variable": v}),
    }
}

fn pattern(p: &a::Pattern) -> Value {
    json!({"name":p.name,"negation":p.negation,"args":p.args.iter().map(term).collect::<Vec<_>>()})
}

fn expression(e: &a::Expression) -> Value {
    match e {
        a::Expression::Term(t) => json!({"term":term(t)}),
        a::Expression::Call(name, args) | a::Expression::ExternalCall(name, args) => {
            json!({"call":name,"args":args.iter().map(expression).collect::<Vec<_>>()})
        }
    }
}

fn condition(c: &a::Condition) -> Value {
    match c {
        a::Condition::Logic(p) => json!({"logic":pattern(p)}),
        a::Condition::Bind(v, e) | a::Condition::BindValue(v, e) => {
            json!({"bind":v,"value":expression(e)})
        }
        a::Condition::Compare(op, x, y) => {
            json!({"compare":op,"left":expression(x),"right":expression(y)})
        }
        a::Condition::Fold(f) => json!({"fold":{
            "result":f.result_var,"seed":f.seed.as_ref().map(expression),"reducer":f.reducer,
            "extract":expression(&f.extract),"pattern":pattern(&f.pattern),"grouping":f.grouping_vars,
        }}),
    }
}

fn case(p: &a::Program) -> Value {
    json!({"rules":p.rules.iter().map(|r| json!({
        "label":r.label,"kind":match r.kind {RuleType::Fact=>"fact",RuleType::Strict=>"strict",RuleType::Defeasible=>"defeasible",RuleType::Defeater=>"defeater"},
        "heads":r.heads.iter().map(pattern).collect::<Vec<_>>(),"body":r.body.iter().map(condition).collect::<Vec<_>>()
    })).collect::<Vec<_>>(),"priorities":p.priorities,"domain":[{"integer":0},{"integer":1},{"integer":2},{"integer":3}]})
}

#[test]
#[ignore = "requires the independently built Lean AggregationOracle"]
fn four_tags_match_lean_for_ground_and_private_aggregate_policies() {
    let mut sources = vec![
        "(normally r a q)".to_owned(),
        "(normally r (and a b) q)".to_owned(),
        "(always r q q)".to_owned(),
        "(normally r q q)".to_owned(),
        "(always r (not q) (not q)) (normally s (and) q)".to_owned(),
        "(normally r a q) (normally s b (not q)) (prefer r s)".to_owned(),
        "(normally r a q) (normally s b (not q)) (normally t a q) (prefer r s) (prefer s t)".to_owned(),
        "(given q) (given (not q))".to_owned(),
        "(normally p (and) p) (normally n (and) (not p)) (except attack p (not q)) (normally claim (and) q)".to_owned(),
    ];
    for reducer in ["sum", "count", "min-of", "max-of"] {
        sources.push(format!(
            "(aggregate-domain 0 1 2 3)
            (normally r a (row 1)) (normally s b (row 2))
            (normally total (agg ?n {reducer} ?v (row ?v)) (total ?n))
            (normally claim (total 2) q)"
        ));
    }
    sources.push(
        "(aggregate-domain 0 1 2 3)
        (normally r a (row 1)) (normally s b (not (row 1))) (prefer s r)
        (always total (agg ?n count ?v (row ?v)) (total ?n))
        (normally next (agg ?n sum ?v (total ?v)) (next ?n))
        (normally claim (next 1) q)"
            .into(),
    );
    let inputs = parse_facts("(given a) (given b)").unwrap();
    let mut requests = Vec::new();
    let mut expected = Vec::new();
    for source in sources {
        // The oracle reports mentioned literals only. Mention both polarities
        // downstream without adding support for either query, so undecided
        // states are compared rather than filtered out of the common universe.
        let source = format!(
            "{source} (normally observe_q q observed_q) (normally observe_not_q (not q) observed_not_q)"
        );
        let theory = spindle_parser::parse_spl(&source).unwrap();
        let program = Program::compile(&theory, &inputs).unwrap();
        for mask in 0..4 {
            let facts: Vec<_> = inputs
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, l)| l.clone())
                .collect();
            let tags: BTreeSet<_> = program
                .evaluate(&facts)
                .unwrap()
                .into_iter()
                .filter(|(l, _)| l.name() == "q" && l.predicate_args().is_empty())
                .map(|(l, t)| (l.negation, t.symbol().to_owned()))
                .collect();
            let mut instance = theory.clone();
            for (i, fact) in facts.into_iter().enumerate() {
                instance.add_rule(Rule::fact(format!("private_{i}"), fact));
            }
            requests.push(case(&a::source::program(&instance).unwrap()));
            expected.push((source.clone(), mask, tags));
        }
    }
    let path = std::env::var_os("SPINDLE_AGGREGATION_ORACLE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../lean/.lake/build/bin/AggregationOracle")
        });
    let mut child = Command::new(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("build Lean oracle {}: {e}", path.display()));
    child
        .stdin
        .take()
        .unwrap()
        .write_all(json!({"cases":requests}).to_string().as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    let results = output["results"].as_array().expect("oracle results");
    assert_eq!(results.len(), expected.len());
    for (result, (source, mask, expected)) in results.iter().zip(expected) {
        assert!(result.get("error").is_none(), "{source}: {result}");
        let actual: BTreeSet<_> = result["conclusions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| {
                c["atom"]["name"] == "q" && c["atom"]["args"].as_array().unwrap().is_empty()
            })
            .map(|c| {
                (
                    c["atom"]["negation"].as_bool().unwrap(),
                    c["tag"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        assert_eq!(actual, expected, "mask {mask}: {source}");
    }
}
