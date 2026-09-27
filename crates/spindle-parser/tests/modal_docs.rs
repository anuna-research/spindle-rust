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
        7,
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
    // Every runnable example is followed by its complete positive output.
    let chapter = include_str!("../../../docs/src/guides/modal.md");
    let sections: Vec<_> = chapter.split("```spl\n").skip(1).collect();
    assert_eq!(
        sections.len(),
        examples().len(),
        "every example needs an output check"
    );
    for section in sections {
        let (source, rest) = section.split_once("```").unwrap();
        let output = rest
            .split_once("```text\n")
            .unwrap()
            .1
            .split_once("```")
            .unwrap()
            .0;
        let mut expected: Vec<_> = output.lines().map(str::to_owned).collect();
        let mut actual: Vec<_> = reason(&parse_spl(source).unwrap())
            .unwrap()
            .iter()
            .filter(|c| c.conclusion_type == ConclusionType::DefeasiblyProvable)
            .map(|c| c.literal.to_spl())
            .collect();
        expected.sort();
        actual.sort();
        actual.dedup();
        assert_eq!(actual, expected, "documented output for {source}");
    }

    let permission = example_containing("normally archive-closed");
    check(
        &permission.replace(
            "(prefer pass-allows-entry archive-closed)",
            "(prefer archive-closed pass-allows-entry)",
        ),
        &["(forbidden enter-archive)"],
        &["(may enter-archive)"],
    );
    check(
        &permission.replace("(prefer pass-allows-entry archive-closed)", ""),
        &[],
        &["(may enter-archive)", "(forbidden enter-archive)"],
    );
    check(
        &permission.replace("(given access-pass)", ""),
        &["(forbidden enter-archive)"],
        &["(may enter-archive)"],
    );
    check(
        &example_containing("normally outstanding-sign-in").replace("(given (not signed-in))", ""),
        &["(must sign-in)"],
        &["sign-in-outstanding"],
    );
}
