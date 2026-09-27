//! Independent four-tag modal reference model. Build with `cd lean && lake
//! build ModalOracle`, then run this target with `-- --ignored --nocapture`.
use std::collections::BTreeSet;
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use spindle_core::conclusion::ConclusionType;
use spindle_core::mode::Mode;
use spindle_core::rule::{Rule, RuleType};
use spindle_core::temporal::Temporal;
use spindle_core::{Literal, Theory, reason};

#[derive(Clone, Debug)]
struct Case {
    name: String,
    rules: Vec<Rule>,
    priority: Vec<(String, String)>,
}

fn lit(mode: &str, inner: bool, outer: bool) -> Literal {
    Literal::new(
        "p",
        inner,
        Mode {
            name: (mode != "plain").then(|| mode.to_owned()),
            negation: outer,
        },
        Temporal::empty(),
        vec![],
    )
}

fn rule(label: &str, kind: RuleType, body: Vec<Literal>, head: Literal) -> Rule {
    Rule::new(label, kind, body, vec![head])
}

fn wire_literal(l: &Literal) -> Value {
    let window = if l.temporal.is_empty() {
        Value::Null
    } else {
        use spindle_core::temporal::TimePoint;
        let (TimePoint::Moment(start), TimePoint::Moment(end)) =
            (&l.temporal.start, &l.temporal.end)
        else {
            panic!("finite fixture windows only")
        };
        json!([start, end])
    };
    json!({"atom": l.name(), "mode": l.mode.name.as_deref().unwrap_or("plain"), "inner": l.negation, "outer": l.mode.negation, "window": window})
}

fn domain(case: &Case) -> Vec<Literal> {
    let mut literals = Vec::new();
    let mut seen = BTreeSet::new();
    for r in &case.rules {
        for l in r.head.iter().cloned().chain(
            r.body
                .iter()
                .filter_map(|b| b.as_logic().map(|b| b.to_literal())),
        ) {
            let l = l.deontic_normalized();
            let variants = if l.mode.is_empty() {
                vec![l]
            } else {
                ["O", "P"]
                    .into_iter()
                    .flat_map(|mode| {
                        [false, true].into_iter().flat_map(move |inner| {
                            [false, true]
                                .into_iter()
                                .map(move |outer| (mode, inner, outer))
                        })
                    })
                    .map(|(mode, inner, outer)| {
                        let mut m = l.clone();
                        m.mode = Mode {
                            name: Some(mode.into()),
                            negation: outer,
                        };
                        m.negation = inner;
                        m
                    })
                    .collect()
            };
            for variant in variants {
                if seen.insert(variant.to_spl()) {
                    literals.push(variant);
                }
            }
        }
    }
    literals.sort_by_key(Literal::to_spl);
    literals
}

fn request(case: &Case) -> Value {
    json!({"rules": case.rules.iter().map(|r| json!({
        "label": r.label, "kind": match r.rule_type { RuleType::Fact => "fact", RuleType::Strict => "strict", RuleType::Defeasible => "defeasible", RuleType::Defeater => "defeater" },
        "head": wire_literal(r.head_literal()), "body": r.body.iter().map(|b| wire_literal(&b.as_logic().unwrap().to_literal())).collect::<Vec<_>>()
    })).collect::<Vec<_>>(), "priority": case.priority, "queries": domain(case).iter().map(wire_literal).collect::<Vec<_>>()})
}

fn rust_tags(case: &Case) -> Vec<Vec<bool>> {
    let mut theory = Theory::new();
    for r in &case.rules {
        theory.add_rule(r.clone());
    }
    for (a, b) in &case.priority {
        theory.add_superiority(a, b);
    }
    let conclusions = reason(&theory).unwrap();
    domain(case)
        .iter()
        .map(|l| {
            [
                ConclusionType::DefinitelyProvable,
                ConclusionType::DefinitelyNotProvable,
                ConclusionType::DefeasiblyProvable,
                ConclusionType::DefeasiblyNotProvable,
            ]
            .into_iter()
            .map(|tag| {
                conclusions.iter().any(|c| {
                    c.conclusion_type == tag
                        && c.literal.deontic_normalized() == *l
                        && c.literal.temporal == l.temporal
                })
            })
            .collect()
        })
        .collect()
}

fn batch(requests: &[Value]) -> Vec<Value> {
    let binary = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../lean/.lake/build/bin/ModalOracle");
    assert!(binary.exists(), "build ModalOracle first");
    let mut child = Command::new(binary)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let input = requests
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    let writer = std::thread::spawn(move || writeln!(stdin, "{input}").unwrap());
    let output = child.wait_with_output().unwrap();
    writer.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let results: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(results.len(), requests.len());
    results
}

fn check(cases: &[Case]) {
    for chunk in cases.chunks(128) {
        let requests: Vec<_> = chunk.iter().map(request).collect();
        for ((case, req), result) in chunk.iter().zip(&requests).zip(batch(&requests)) {
            assert!(result.get("error").is_none(), "{}: {result}", case.name);
            assert_eq!(
                result["tags"],
                json!(rust_tags(case)),
                "{}\n{req}",
                case.name
            );
        }
    }
    eprintln!("{} modal theories agree on all four tags", cases.len());
}

#[test]
#[ignore = "requires Lean ModalOracle"]
fn exhaustive_modal_pairs() {
    let mut literals = vec![lit("plain", false, false), lit("plain", true, false)];
    for mode in ["O", "P", "F"] {
        for inner in [false, true] {
            for outer in [false, true] {
                literals.push(lit(mode, inner, outer));
            }
        }
    }
    let mut cases = Vec::new();
    for (i, a) in literals.iter().enumerate() {
        for (j, b) in literals.iter().enumerate() {
            for direction in 0..3 {
                for kind in [
                    RuleType::Defeasible,
                    RuleType::Strict,
                    RuleType::Defeater,
                    RuleType::Fact,
                ] {
                    // Consistent explicit facts avoid intentional inconsistent-input behavior.
                    let rules = vec![
                        rule("a", RuleType::Defeasible, vec![], a.clone()),
                        rule("b", kind, vec![], b.clone()),
                    ];
                    let priority = match direction {
                        1 => vec![("a".into(), "b".into())],
                        2 => vec![("b".into(), "a".into())],
                        _ => vec![],
                    };
                    cases.push(Case {
                        name: format!("pair-{i}-{j}-{direction}-{kind:?}"),
                        rules,
                        priority,
                    });
                }
            }
        }
    }
    check(&cases);
}

#[test]
#[ignore = "requires Lean ModalOracle"]
fn modal_premises_cycles_and_team_defeat() {
    let o = lit("O", false, false);
    let p = lit("P", false, false);
    let f = lit("F", false, false);
    let mut cases = Vec::new();
    cases.push(Case {
        name: "priority-is-not-transitive".into(),
        rules: vec![
            rule("s", RuleType::Defeasible, vec![], p.clone()),
            rule("middle", RuleType::Defeasible, vec![], Literal::simple("q")),
            rule("a", RuleType::Defeasible, vec![], f.clone()),
        ],
        priority: vec![("s".into(), "middle".into()), ("middle".into(), "a".into())],
    });
    for support in [o.clone(), p.clone()] {
        for attack in [f.clone(), lit("P", true, false)] {
            for defense in [o.clone(), p.clone()] {
                for kind in [RuleType::Defeasible, RuleType::Defeater] {
                    cases.push(Case {
                        name: format!("team-{}-{}-{}-{kind:?}", support, attack, defense),
                        rules: vec![
                            rule("s", RuleType::Defeasible, vec![], support.clone()),
                            rule("a", RuleType::Defeasible, vec![], attack.clone()),
                            rule("d", kind, vec![], defense.clone()),
                        ],
                        priority: vec![("d".into(), "a".into())],
                    });
                }
            }
        }
    }
    for mode in ["O", "P", "F"] {
        for outer in [false, true] {
            for seed in 0..4 {
                let premise = lit(mode, false, outer);
                let mut rules = vec![rule(
                    "use",
                    RuleType::Defeasible,
                    vec![premise.clone()],
                    Literal::simple("q"),
                )];
                match seed {
                    0 => {}
                    1 => rules.push(rule(
                        "seed",
                        RuleType::Defeasible,
                        vec![],
                        lit(mode, false, false),
                    )),
                    2 => rules.push(rule(
                        "cycle",
                        RuleType::Defeasible,
                        vec![lit(mode, false, false)],
                        lit(mode, false, false),
                    )),
                    _ => rules.push(rule("explicit", RuleType::Fact, vec![], premise)),
                }
                cases.push(Case {
                    name: format!("premise-{mode}-{outer}-{seed}"),
                    rules,
                    priority: vec![],
                });
            }
        }
    }
    // Temporal families and exact-window opposition, with both positive and negative premises.
    for outer in [false, true] {
        for window in [(1, 2), (3, 4)] {
            let mut bounded = p.clone();
            bounded.temporal = Temporal::from_bounds(1, 2);
            let mut prohibition = f.clone();
            prohibition.temporal = Temporal::from_bounds(window.0, window.1);
            cases.push(Case {
                name: format!("temporal-{outer}-{window:?}"),
                rules: vec![
                    rule("p", RuleType::Defeasible, vec![], bounded),
                    rule("f", RuleType::Defeasible, vec![], prohibition),
                    rule(
                        "use",
                        RuleType::Defeasible,
                        vec![lit("P", false, outer)],
                        Literal::simple("q"),
                    ),
                ],
                priority: vec![("p".into(), "f".into())],
            });
        }
    }
    cases.push(Case {
        name: "wait-for-superior-modal-defender".into(),
        rules: vec![
            rule(
                "defend",
                RuleType::Defeasible,
                vec![Literal::simple("q")],
                p.clone(),
            ),
            rule("attack", RuleType::Defeasible, vec![], f.clone()),
            rule(
                "chain",
                RuleType::Strict,
                vec![Literal::simple("r")],
                Literal::simple("q"),
            ),
            rule("seed", RuleType::Defeasible, vec![], Literal::simple("r")),
        ],
        priority: vec![("defend".into(), "attack".into())],
    });
    cases.push(Case {
        name: "modal-defeat-discards-plain-attacker".into(),
        rules: vec![
            rule("yes", RuleType::Defeasible, vec![], o.clone()),
            rule("no", RuleType::Defeasible, vec![], f.clone()),
            rule(
                "attack",
                RuleType::Defeater,
                vec![o.clone()],
                Literal::negated("q"),
            ),
            rule(
                "support",
                RuleType::Defeasible,
                vec![],
                Literal::simple("q"),
            ),
        ],
        priority: vec![],
    });
    cases.push(Case {
        name: "inconsistent-definite-obligations-are-retained".into(),
        rules: vec![
            rule("yes", RuleType::Fact, vec![], o),
            rule("no", RuleType::Fact, vec![], f),
        ],
        priority: vec![],
    });
    let mut early = p.clone();
    early.temporal = Temporal::from_bounds(1, 2);
    let mut late = p.clone();
    late.temporal = Temporal::from_bounds(3, 4);
    cases.push(Case {
        name: "negative-family-needs-every-member-refuted".into(),
        rules: vec![
            rule(
                "refuted",
                RuleType::Defeasible,
                vec![Literal::simple("missing")],
                early.clone(),
            ),
            rule(
                "undecided",
                RuleType::Defeasible,
                vec![late.clone()],
                late.clone(),
            ),
            rule(
                "use",
                RuleType::Defeasible,
                vec![lit("P", false, true)],
                Literal::simple("q"),
            ),
        ],
        priority: vec![],
    });
    for second_supported in [false, true] {
        let mut rules = vec![
            rule("early", RuleType::Fact, vec![], early.clone()),
            rule("late", RuleType::Fact, vec![], late.clone()),
            rule(
                "use",
                RuleType::Defeasible,
                vec![p.clone(), Literal::simple("other")],
                Literal::simple("q"),
            ),
        ];
        if second_supported {
            rules.push(rule(
                "other",
                RuleType::Fact,
                vec![],
                Literal::simple("other"),
            ));
        }
        cases.push(Case {
            name: format!("family-alternatives-do-not-fill-second-slot-{second_supported}"),
            rules,
            priority: vec![],
        });
    }
    let reverse_order: Vec<_> = cases
        .iter()
        .map(|case| {
            let mut reversed = case.clone();
            reversed.name.push_str("-reversed-order");
            reversed.rules.reverse();
            reversed
        })
        .collect();
    cases.extend(reverse_order);
    check(&cases);
}

#[test]
#[ignore = "requires Lean ModalOracle"]
fn oracle_rejects_invalid_modal_inputs() {
    let case = Case {
        name: "invalid".into(),
        rules: vec![rule("r", RuleType::Fact, vec![], lit("P", false, false))],
        priority: vec![],
    };
    let mut bad_mode = request(&case);
    bad_mode["rules"][0]["head"]["mode"] = json!("invalid");
    let mut bad_scope = request(&case);
    bad_scope["rules"][0]["head"]["mode"] = json!("plain");
    bad_scope["rules"][0]["head"]["outer"] = json!(true);
    let mut bad_fact = request(&case);
    bad_fact["rules"][0]["body"] = json!([wire_literal(&lit("P", false, false))]);
    let mut duplicate = request(&case);
    duplicate["rules"] = json!([duplicate["rules"][0].clone(), duplicate["rules"][0].clone()]);
    for output in batch(&[bad_mode, bad_scope, bad_fact, duplicate]) {
        assert!(output.get("error").is_some(), "{output}");
    }
}
