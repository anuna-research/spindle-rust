//! Temporary probe: observe the engine's temporal-family semantics on
//! small ground theories, as ground truth for the Lean family model.

use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::reason::reason;
use spindle_core::rule::Rule;
use spindle_core::temporal::Temporal;
use spindle_core::theory::Theory;

fn lit(name: &str, neg: bool, window: Option<(i64, i64)>) -> Literal {
    let temporal = match window {
        Some((s, e)) => Temporal::from_bounds(s, e),
        None => Temporal::empty(),
    };
    Literal::new(name, neg, Mode::empty(), temporal, Vec::<String>::new())
}

fn show(theory: &Theory, label: &str) {
    let conclusions = reason(theory).unwrap();
    let mut lines: Vec<String> = conclusions
        .iter()
        .map(|c| format!("{:?} {}", c.conclusion_type, c.literal.to_spl()))
        .collect();
    lines.sort();
    eprintln!("=== {label} ===");
    for l in lines {
        eprintln!("  {l}");
    }
}

#[test]
#[ignore]
fn probe_family_semantics() {
    // P1: temporal fact supports atemporal body?
    let mut t1 = Theory::new();
    t1.add_rule(Rule::fact("f1", lit("p", false, Some((1, 10)))));
    t1.add_rule(Rule::defeasible(
        "r1",
        vec![lit("p", false, None)],
        lit("q", false, None),
    ));
    show(&t1, "P1: fact p[1,10]; p => q (atemporal body)");

    // P2: atemporal fact supports temporal body?
    let mut t2 = Theory::new();
    t2.add_rule(Rule::fact("f1", lit("p", false, None)));
    t2.add_rule(Rule::defeasible(
        "r1",
        vec![lit("p", false, Some((1, 10)))],
        lit("q", false, None),
    ));
    show(&t2, "P2: fact p; p[1,10] => q (temporal body)");

    // P3: temporal fact supports temporal body with DIFFERENT window?
    let mut t3 = Theory::new();
    t3.add_rule(Rule::fact("f1", lit("p", false, Some((20, 30)))));
    t3.add_rule(Rule::defeasible(
        "r1",
        vec![lit("p", false, Some((1, 10)))],
        lit("q", false, None),
    ));
    show(&t3, "P3: fact p[20,30]; p[1,10] => q");

    // P4: same window body match
    let mut t4 = Theory::new();
    t4.add_rule(Rule::fact("f1", lit("p", false, Some((1, 10)))));
    t4.add_rule(Rule::defeasible(
        "r1",
        vec![lit("p", false, Some((1, 10)))],
        lit("q", false, None),
    ));
    show(&t4, "P4: fact p[1,10]; p[1,10] => q");

    // P5: conflict across windows: does ~p[20,30] attack p[1,10]?
    let mut t5 = Theory::new();
    t5.add_rule(Rule::defeasible(
        "r1",
        Vec::<Literal>::new(),
        lit("p", false, Some((1, 10))),
    ));
    t5.add_rule(Rule::defeasible(
        "r2",
        Vec::<Literal>::new(),
        lit("p", true, Some((20, 30))),
    ));
    show(&t5, "P5: => p[1,10]; => ~p[20,30] (disjoint windows)");

    // P6: conflict same window
    let mut t6 = Theory::new();
    t6.add_rule(Rule::defeasible(
        "r1",
        Vec::<Literal>::new(),
        lit("p", false, Some((1, 10))),
    ));
    t6.add_rule(Rule::defeasible(
        "r2",
        Vec::<Literal>::new(),
        lit("p", true, Some((1, 10))),
    ));
    show(&t6, "P6: => p[1,10]; => ~p[1,10] (same window)");

    // P7: atemporal defeasible vs temporal attacker
    let mut t7 = Theory::new();
    t7.add_rule(Rule::defeasible(
        "r1",
        Vec::<Literal>::new(),
        lit("p", false, None),
    ));
    t7.add_rule(Rule::defeasible(
        "r2",
        Vec::<Literal>::new(),
        lit("p", true, Some((1, 10))),
    ));
    show(&t7, "P7: => p (atemporal); => ~p[1,10]");

    // P8: overlapping windows conflict
    let mut t8 = Theory::new();
    t8.add_rule(Rule::defeasible(
        "r1",
        Vec::<Literal>::new(),
        lit("p", false, Some((1, 10))),
    ));
    t8.add_rule(Rule::defeasible(
        "r2",
        Vec::<Literal>::new(),
        lit("p", true, Some((5, 15))),
    ));
    show(&t8, "P8: => p[1,10]; => ~p[5,15] (overlapping)");

    // P9: superiority across family: r1 (temporal) > r2 (atemporal attacker)
    let mut t9 = Theory::new();
    t9.add_rule(Rule::defeasible(
        "r1",
        Vec::<Literal>::new(),
        lit("p", false, Some((1, 10))),
    ));
    t9.add_rule(Rule::defeasible(
        "r2",
        Vec::<Literal>::new(),
        lit("p", true, None),
    ));
    t9.add_superiority("r1", "r2");
    show(&t9, "P9: => p[1,10]; => ~p (atemporal); r1 > r2");

    // P11: strict rule with atemporal body + temporal fact: definite family support?
    let mut t11 = Theory::new();
    t11.add_rule(Rule::fact("f1", lit("p", false, Some((1, 10)))));
    t11.add_rule(Rule::strict(
        "r1",
        vec![lit("p", false, None)],
        lit("q", false, None),
    ));
    show(&t11, "P11: fact p[1,10]; p -> q (strict, atemporal body)");

    // P12: defeater with atemporal body, temporal support; attacker of q
    let mut t12 = Theory::new();
    t12.add_rule(Rule::fact("f1", lit("p", false, Some((1, 10)))));
    t12.add_rule(Rule::defeasible(
        "r1",
        Vec::<Literal>::new(),
        lit("q", false, None),
    ));
    t12.add_rule(Rule::defeater(
        "r2",
        vec![lit("p", false, None)],
        lit("q", true, None),
    ));
    show(
        &t12,
        "P12: fact p[1,10]; => q; p ~> ~q (defeater, atemporal body)",
    );

    // P10: strict temporal fact + defeasible atemporal complement (gate across family?)
    let mut t10 = Theory::new();
    t10.add_rule(Rule::fact("f1", lit("p", false, Some((1, 10)))));
    t10.add_rule(Rule::fact("f2", lit("p", true, None)));
    show(&t10, "P10: fact p[1,10]; fact ~p (atemporal)");
}
