//! Rule-level conflict detection for defeasible logic theories.
//!
//! Provides an iterator-based API for detecting conflicting rule pairs
//! (rules whose heads are classically negated versions of each other).
//!
//! This module is distinct from the trace-level conflict detection in
//! `mining.rs` which operates on `EventLog` + `PetriNet`.

use super::{analysis_key_unsigned, ConflictKind, ConflictReport};
use crate::rule::Rule;

/// Check whether two rules have conflicting heads.
///
/// Two heads conflict when one is the classical negation of the other
/// **and** they refer to the same ground atom (same name, predicate
/// arguments, mode, and temporal scope).  For example `flies` vs
/// `~flies` conflict, but `O(p)` vs `~p` or `p@t1` vs `~p@t2` do not.
pub fn is_conflicting(a: &Rule, b: &Rule) -> bool {
    a.head.iter().any(|ha| {
        b.head.iter().any(|hb| {
            ha.is_negated() != hb.is_negated()
                && analysis_key_unsigned(ha) == analysis_key_unsigned(hb)
        })
    })
}

/// Scan an iterator of rules and return every pair whose heads conflict.
///
/// Accepts `impl IntoIterator` so callers can pass `&[Rule]`, `Vec<Rule>`,
/// or a filtered iterator without allocation.
pub fn find_conflicts<'a, I>(rules: I) -> Vec<ConflictReport>
where
    I: IntoIterator<Item = &'a Rule>,
{
    let rules: Vec<&Rule> = rules.into_iter().collect();
    let mut reports = Vec::new();

    for i in 0..rules.len() {
        for j in (i + 1)..rules.len() {
            if is_conflicting(rules[i], rules[j]) {
                reports.push(ConflictReport {
                    rule_a: rules[i].label.clone(),
                    rule_b: rules[j].label.clone(),
                    head_a: format!("{}", rules[i].head[0]),
                    head_b: format!("{}", rules[j].head[0]),
                    conflict_type: ConflictKind::Negation,
                });
            }
        }
    }

    reports
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal::Literal;
    use crate::rule::Rule;

    #[test]
    fn test_is_conflicting_negation() {
        let a = Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        );
        let b = Rule::defeasible(
            "r2",
            vec![Literal::simple("penguin")],
            Literal::negated("flies"),
        );
        assert!(is_conflicting(&a, &b));
    }

    #[test]
    fn test_not_conflicting_same_polarity() {
        let a = Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        );
        let b = Rule::defeasible("r2", vec![Literal::simple("bat")], Literal::simple("flies"));
        assert!(!is_conflicting(&a, &b));
    }

    #[test]
    fn test_not_self_conflicting() {
        let r = Rule::defeasible("r1", vec![Literal::simple("a")], Literal::simple("b"));
        assert!(!is_conflicting(&r, &r));
    }

    #[test]
    fn test_is_conflicting_symmetric() {
        let a = Rule::defeasible("r1", vec![Literal::simple("x")], Literal::simple("p"));
        let b = Rule::defeasible("r2", vec![Literal::simple("y")], Literal::negated("p"));
        assert_eq!(is_conflicting(&a, &b), is_conflicting(&b, &a));
    }

    #[test]
    fn test_not_conflicting_different_predicate_args() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // p(a) vs ~p(b) should NOT conflict — different ground atoms
        let head_a = Literal::new("p", false, Mode::empty(), Temporal::empty(), vec!["a".into()]);
        let head_b = Literal::new("p", true, Mode::empty(), Temporal::empty(), vec!["b".into()]);
        let a = Rule::defeasible("r1", vec![Literal::simple("x")], head_a);
        let b = Rule::defeasible("r2", vec![Literal::simple("y")], head_b);
        assert!(!is_conflicting(&a, &b));
    }

    #[test]
    fn test_conflicting_same_predicate_args() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // p(a) vs ~p(a) SHOULD conflict — same ground atom, opposite negation
        let head_a = Literal::new("p", false, Mode::empty(), Temporal::empty(), vec!["a".into()]);
        let head_b = Literal::new("p", true, Mode::empty(), Temporal::empty(), vec!["a".into()]);
        let a = Rule::defeasible("r1", vec![Literal::simple("x")], head_a);
        let b = Rule::defeasible("r2", vec![Literal::simple("y")], head_b);
        assert!(is_conflicting(&a, &b));
    }

    #[test]
    fn test_not_conflicting_different_modes() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // O(p) vs ~p should NOT conflict — different modalities
        let head_a = Literal::new("p", false, Mode::obligation(), Temporal::empty(), vec![]);
        let head_b = Literal::new("p", true, Mode::empty(), Temporal::empty(), vec![]);
        let a = Rule::defeasible("r1", vec![Literal::simple("x")], head_a);
        let b = Rule::defeasible("r2", vec![Literal::simple("y")], head_b);
        assert!(!is_conflicting(&a, &b));
    }

    #[test]
    fn test_conflicting_same_mode() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // O(p) vs ~O(p) SHOULD conflict — same modality, opposite negation
        let head_a = Literal::new("p", false, Mode::obligation(), Temporal::empty(), vec![]);
        let head_b = Literal::new("p", true, Mode::obligation(), Temporal::empty(), vec![]);
        let a = Rule::defeasible("r1", vec![Literal::simple("x")], head_a);
        let b = Rule::defeasible("r2", vec![Literal::simple("y")], head_b);
        assert!(is_conflicting(&a, &b));
    }

    #[test]
    fn test_not_conflicting_different_temporals() {
        use crate::mode::Mode;
        use crate::temporal::{Temporal, TimePoint};
        // p@[0,10] vs ~p@[20,30] should NOT conflict — different time scopes
        let t1 = Temporal::new(TimePoint::Moment(0), TimePoint::Moment(10));
        let t2 = Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30));
        let head_a = Literal::new("p", false, Mode::empty(), t1, vec![]);
        let head_b = Literal::new("p", true, Mode::empty(), t2, vec![]);
        let a = Rule::defeasible("r1", vec![Literal::simple("x")], head_a);
        let b = Rule::defeasible("r2", vec![Literal::simple("y")], head_b);
        assert!(!is_conflicting(&a, &b));
    }

    #[test]
    fn test_conflicting_same_temporal() {
        use crate::mode::Mode;
        use crate::temporal::{Temporal, TimePoint};
        // p@[0,10] vs ~p@[0,10] SHOULD conflict — same temporal scope
        let t = Temporal::new(TimePoint::Moment(0), TimePoint::Moment(10));
        let head_a = Literal::new("p", false, Mode::empty(), t.clone(), vec![]);
        let head_b = Literal::new("p", true, Mode::empty(), t, vec![]);
        let a = Rule::defeasible("r1", vec![Literal::simple("x")], head_a);
        let b = Rule::defeasible("r2", vec![Literal::simple("y")], head_b);
        assert!(is_conflicting(&a, &b));
    }

    #[test]
    fn test_find_conflicts_returns_pair() {
        let rules = vec![
            Rule::defeasible(
                "r1",
                vec![Literal::simple("bird")],
                Literal::simple("flies"),
            ),
            Rule::defeasible(
                "r2",
                vec![Literal::simple("penguin")],
                Literal::negated("flies"),
            ),
        ];
        let reports = find_conflicts(&rules);
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].rule_a, "r1");
        assert_eq!(reports[0].rule_b, "r2");
    }
}
