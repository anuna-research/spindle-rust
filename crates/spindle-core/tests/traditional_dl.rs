//! Published DL(partial) proof conditions (Maher et al., sections 2 and 4):
//! https://doras.dcu.ie/24726/1/Scalable_Defeasible_Logic.pdf
use spindle_core::{ConclusionType as Tag, Theory, reason};

fn tags(theory: &Theory, name: &str) -> Vec<Tag> {
    reason(theory)
        .unwrap()
        .into_iter()
        .filter(|c| c.literal.canonical_name() == name)
        .map(|c| c.conclusion_type)
        .collect()
}

#[test]
fn strict_self_loop_is_undecided_at_both_levels() {
    let mut t = Theory::new();
    t.add_strict_rule(&["p"], "p");
    assert!(tags(&t, "p").is_empty());
}

#[test]
fn defeasible_self_loop_has_definite_failure_but_no_defeasible_verdict() {
    let mut t = Theory::new();
    t.add_defeasible_rule(&["p"], "p");
    assert_eq!(tags(&t, "p"), vec![Tag::DefinitelyNotProvable]);
}

#[test]
fn undecided_strict_complement_blocks_defeasible_proof() {
    let mut t = Theory::new();
    t.add_strict_rule(&["~q"], "~q");
    t.add_defeasible_rule(&[], "q");
    assert_eq!(tags(&t, "q"), vec![Tag::DefinitelyNotProvable]);
}

#[test]
fn an_undecided_attacker_is_not_discarded() {
    let mut t = Theory::new();
    t.add_defeasible_rule(&["p"], "p");
    t.add_defeasible_rule(&[], "~p");
    // -d p can be derived from the applicable unbeaten contrary. That later
    // discards the self-loop and permits ~p: this is a proof, not unfoundedness.
    assert!(tags(&t, "p").contains(&Tag::DefeasiblyNotProvable));
    assert!(tags(&t, "~p").contains(&Tag::DefeasiblyProvable));

    let mut t = Theory::new();
    t.add_defeasible_rule(&["p"], "p");
    t.add_defeater(&["p"], "~q");
    t.add_defeasible_rule(&[], "q");
    assert_eq!(tags(&t, "q"), vec![Tag::DefinitelyNotProvable]);
}

#[test]
fn strict_inconsistency_subsumes_but_does_not_explode() {
    let mut t = Theory::new();
    t.add_fact("p");
    t.add_fact("~p");
    t.add_strict_rule(&["missing"], "unrelated");
    for l in ["p", "~p"] {
        let ts = tags(&t, l);
        assert!(ts.contains(&Tag::DefinitelyProvable));
        assert!(ts.contains(&Tag::DefeasiblyProvable));
        assert!(!ts.contains(&Tag::DefinitelyNotProvable));
        assert!(!ts.contains(&Tag::DefeasiblyNotProvable));
    }
    assert!(!tags(&t, "unrelated").contains(&Tag::DefeasiblyProvable));
}
