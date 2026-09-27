//! Execute the actual mdBook SPL examples, including indented list fences.
use spindle_core::{ConclusionType, reason};
use spindle_parser::parse_spl;

fn examples() -> Vec<String> {
    let mut examples = Vec::new();
    let mut current = None;
    for line in include_str!("../../../docs/src/guides/modal.md").lines() {
        if line.trim() == "```spl" {
            assert!(current.is_none());
            current = Some(String::new());
        } else if line.trim() == "```" {
            if let Some(source) = current.take() {
                examples.push(source);
            }
        } else if let Some(source) = &mut current {
            source.push_str(line.trim_start());
            source.push('\n');
        }
    }
    assert!(current.is_none(), "unclosed SPL fence");
    assert_eq!(
        examples.len(),
        14,
        "update the example audit when SPL fences change"
    );
    examples
}

fn example_containing(marker: &str) -> String {
    let mut found = examples()
        .into_iter()
        .filter(|source| source.contains(marker));
    let source = found.next().expect("documented example exists");
    assert!(found.next().is_none(), "example marker must be unique");
    source
}

fn check(source: &str, expected: &[&str], absent: &[&str]) {
    let conclusions = reason(&parse_spl(source).unwrap()).unwrap();
    let proved = |spl: &str| {
        let goal = parse_spl(&format!("(given {spl})")).unwrap();
        let literal = goal
            .rules()
            .next()
            .unwrap()
            .head_literal()
            .deontic_normalized();
        conclusions.iter().any(|c| {
            c.conclusion_type == ConclusionType::DefeasiblyProvable
                && c.literal.deontic_normalized() == literal
        })
    };
    for goal in expected {
        assert!(proved(goal), "expected {goal} in {source}");
    }
    for goal in absent {
        assert!(!proved(goal), "unexpected {goal} in {source}");
    }
}

#[test]
fn every_modal_chapter_spl_block_parses_and_reasons() {
    for (i, source) in examples().iter().enumerate() {
        let theory = parse_spl(source).unwrap_or_else(|e| panic!("example {}: {e}", i + 1));
        reason(&theory).unwrap_or_else(|e| panic!("example {}: {e}", i + 1));
    }
}

#[test]
fn modal_chapter_examples_match_their_explanations() {
    let permission = example_containing("normally no-play");
    check(
        &permission,
        &["hat", "(may play)"],
        &["(forbidden play)", "play"],
    );
    check(
        &permission.replace("(prefer hat-play no-play)", "(prefer no-play hat-play)"),
        &["(forbidden play)"],
        &["(may play)"],
    );
    check(
        &permission.replace("(prefer hat-play no-play)", ""),
        &[],
        &["(may play)", "(forbidden play)"],
    );
    check(
        &permission.replace("(given hat)", ""),
        &["(forbidden play)"],
        &["(may play)"],
    );

    check(
        &example_containing("normally check"),
        &["weakly-allowed"],
        &["(may play)"],
    );
    check(
        &example_containing("employee alice"),
        &[
            "(must (report-hours alice))",
            "(must (report-hours bob))",
            "(may (approve-expenses alice))",
            "(forbidden (access-internal charlie))",
        ],
        &[],
    );

    check(
        &example_containing("; Obligation: (must <literal>)"),
        &["(must pay)", "(may access)", "(forbidden enter)"],
        &[],
    );
    check(
        &format!(
            "{} (given exemption) (given revoked) (given authorized)",
            example_containing("; Negated obligation:")
        ),
        &[
            "(not (must pay))",
            "(not (may access))",
            "(not (forbidden enter))",
        ],
        &[],
    );
    check(
        &format!(
            "{} (given signed-contract) (given (not paid)) (given member)
        (given unauthorized) (given exemption) (given pending-review) (given payment-disputed)",
            example_containing("except d1 payment-disputed")
        ),
        &[
            "(not (must pay))",
            "(forbidden enter)",
            "(forbidden access)",
        ],
        &["(must pay)", "violation", "(may access)"],
    );
    check(
        &format!(
            "{} (given company) (given small-company) (given public-company) (given in-bankruptcy)",
            example_containing("must file-annual-report")
        ),
        &[
            "(not (must file-annual-report))",
            "(not (must disclose-finances))",
        ],
        &["(must file-annual-report)", "(must disclose-finances)"],
    );
    check(
        &format!(
            "{} (given employee) (given suspended) (given manager) (given (not has-clearance))",
            example_containing("may access-office")
        ),
        &[
            "(not (may access-office))",
            "(may access-restricted)",
            "(forbidden access-server-room)",
        ],
        &["(may access-office)"],
    );
    check(
        &format!(
            "{} (given citizen) (given minor) (given convicted-felon)",
            example_containing("must pay-taxes")
        ),
        &["(not (must pay-taxes))", "(forbidden vote)"],
        &["(must pay-taxes)", "(may vote)"],
    );
    check(
        &format!(
            "{} (given employee) (given on-leave)",
            example_containing("must attend-meeting")
        ),
        &["(not (must attend-meeting))"],
        &["(must attend-meeting)"],
    );

    let tracking = example_containing("normally r3 in-violation");
    let overdue = format!("{tracking} (given signed-contract) (given (not paid))");
    check(
        &overdue,
        &["(must pay)", "in-violation", "(must remedy)"],
        &[],
    );
    check(
        &format!("{overdue} (given paid-within-grace)"),
        &["(must pay)", "(not in-violation)"],
        &["in-violation", "(must remedy)"],
    );

    check(
        &format!(
            "{} (given (must pay))",
            example_containing("obligation-implies-permission")
        ),
        &["(may pay)"],
        &[],
    );
    check(
        &format!(
            "{} (given endangered-species)",
            example_containing("normally r1 penguin bird")
        ),
        &["(forbidden hunt)"],
        &[],
    );
    check(
        &format!(
            "{} (given (must pay))",
            example_containing("obligatory-action ?x")
        ),
        &["(obligatory-action pay)"],
        &[],
    );
}
