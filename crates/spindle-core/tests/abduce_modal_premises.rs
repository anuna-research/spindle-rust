//! abduce and requires judge body literals with the reasoner's premise
//! semantics: an outer-negated modal premise that holds by constructive
//! refutation is not reported as a missing fact.
use spindle_core::Literal;
use spindle_core::query::{abduce, requires};
use spindle_parser::parse_spl;

const WEAK_PERMISSION: &str = "(normally prohibit conservation-work (forbidden photograph))\n\
     (normally ok (and visitor (not (forbidden photograph))) photography-not-prohibited)";

#[test]
fn abduce_accepts_negative_modal_premise_by_refutation() {
    let theory = parse_spl(WEAK_PERMISSION).unwrap();
    let goal = Literal::simple("photography-not-prohibited");
    let result = abduce(&theory, &goal, 5).unwrap();
    let smallest = result.smallest_solution().expect("one candidate");
    let facts: Vec<_> = smallest.facts.iter().map(Literal::to_spl).collect();
    assert_eq!(facts, vec!["(visitor)".to_string()]);
}

#[test]
fn requires_verifies_single_fact_solution_for_weak_permission() {
    let theory = parse_spl(WEAK_PERMISSION).unwrap();
    let goal = Literal::simple("photography-not-prohibited");
    let facts: Vec<_> = requires(&theory, &goal)
        .unwrap()
        .iter()
        .map(Literal::to_spl)
        .collect();
    assert_eq!(facts, vec!["(visitor)".to_string()]);
}
