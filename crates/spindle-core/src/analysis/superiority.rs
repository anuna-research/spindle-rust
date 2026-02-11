//! Superiority analysis for defeasible logic theories
//!
//! This module provides functions to:
//!
//! - **Suggest superiority relations** between conflicting rules based on
//!   specificity heuristics (more body literals = more specific = superior).
//! - **Check completeness** of declared superiority relations against known
//!   conflicts.
//!
//! The API is iterator-based so callers can feed hand-crafted pairs for
//! isolated unit testing without constructing the full mining pipeline.

use super::{SuperioritySuggestion, UnresolvedConflict};
use crate::rule::Rule;

/// Given a set of conflicting rule pairs, suggest superiority relations
/// based on specificity (more body literals = more specific = superior).
///
/// Rules with equal specificity produce no suggestion.
///
/// # Examples
///
/// ```
/// use spindle_core::rule::{Rule, RuleType};
/// use spindle_core::literal::Literal;
/// use spindle_core::analysis::superiority::suggest_superiorities;
///
/// let r1 = Rule::defeasible("r1", vec![Literal::simple("bird")], Literal::simple("flies"));
/// let r2 = Rule::defeasible(
///     "r2",
///     vec![Literal::simple("bird"), Literal::simple("penguin")],
///     Literal::negated("flies"),
/// );
///
/// let suggestions = suggest_superiorities(vec![(&r1, &r2)]);
/// assert_eq!(suggestions.len(), 1);
/// assert_eq!(suggestions[0].superior, "r2");
/// assert_eq!(suggestions[0].inferior, "r1");
/// ```
pub fn suggest_superiorities<'a, I>(conflicts: I) -> Vec<SuperioritySuggestion>
where
    I: IntoIterator<Item = (&'a Rule, &'a Rule)>,
{
    let mut suggestions = Vec::new();

    for (a, b) in conflicts {
        // Heuristic: the more specific rule (more body literals) should
        // be superior.  Equal specificity produces no suggestion.
        let spec_a = a.body.len();
        let spec_b = b.body.len();

        if spec_a > spec_b {
            suggestions.push(SuperioritySuggestion {
                superior: a.label.clone(),
                inferior: b.label.clone(),
                reason: format!(
                    "'{}' has {} body literals vs {} for '{}' (more specific)",
                    a.label, spec_a, spec_b, b.label,
                ),
                confidence: specificity_confidence(spec_a, spec_b),
            });
        } else if spec_b > spec_a {
            suggestions.push(SuperioritySuggestion {
                superior: b.label.clone(),
                inferior: a.label.clone(),
                reason: format!(
                    "'{}' has {} body literals vs {} for '{}' (more specific)",
                    b.label, spec_b, spec_a, a.label,
                ),
                confidence: specificity_confidence(spec_b, spec_a),
            });
        }
        // Equal specificity: no suggestion
    }

    suggestions
}

/// Confidence based on the ratio of specificity difference.
///
/// Returns a value in `[0.0, 1.0]`. A larger gap between the specific and
/// general rule yields higher confidence.
fn specificity_confidence(more: usize, fewer: usize) -> f64 {
    if more == 0 {
        return 0.0;
    }
    (more - fewer) as f64 / more as f64
}

/// Check whether every conflicting pair in the theory has a declared
/// superiority relation.
///
/// Returns [`UnresolvedConflict`] entries for each pair that has no
/// declared superiority in either direction.
///
/// # Examples
///
/// ```
/// use spindle_core::analysis::superiority::check_completeness;
///
/// let conflicts = vec![("r1", "r2"), ("r3", "r4")];
/// let declared = vec![("r1", "r2")]; // only r1 > r2 declared
///
/// let unresolved = check_completeness(conflicts, declared);
/// assert_eq!(unresolved.len(), 1);
/// assert_eq!(unresolved[0].rule_a, "r3");
/// assert_eq!(unresolved[0].rule_b, "r4");
/// ```
pub fn check_completeness<'a, I, S>(conflicts: I, declared: S) -> Vec<UnresolvedConflict>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
    S: IntoIterator<Item = (&'a str, &'a str)>,
{
    let declared_set: std::collections::HashSet<(&str, &str)> = declared.into_iter().collect();

    conflicts
        .into_iter()
        .filter(|(a, b)| !declared_set.contains(&(*a, *b)) && !declared_set.contains(&(*b, *a)))
        .map(|(a, b)| UnresolvedConflict {
            rule_a: a.to_string(),
            rule_b: b.to_string(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal::Literal;
    use crate::rule::Rule;

    #[test]
    fn test_suggest_more_specific_wins() {
        let r1 = Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        );
        let r2 = Rule::defeasible(
            "r2",
            vec![Literal::simple("bird"), Literal::simple("penguin")],
            Literal::negated("flies"),
        );

        let suggestions = suggest_superiorities(vec![(&r1, &r2)]);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].superior, "r2");
        assert_eq!(suggestions[0].inferior, "r1");
        assert!(suggestions[0].confidence > 0.0);
        assert!(suggestions[0].confidence <= 1.0);
    }

    #[test]
    fn test_suggest_equal_specificity_no_suggestion() {
        let r1 = Rule::defeasible("r1", vec![Literal::simple("a")], Literal::simple("c"));
        let r2 = Rule::defeasible("r2", vec![Literal::simple("b")], Literal::negated("c"));

        let suggestions = suggest_superiorities(vec![(&r1, &r2)]);
        assert!(
            suggestions.is_empty(),
            "Equal specificity should produce no suggestion"
        );
    }

    #[test]
    fn test_suggest_empty_input() {
        let suggestions = suggest_superiorities(Vec::<(&Rule, &Rule)>::new());
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_suggest_multiple_pairs() {
        let r1 = Rule::defeasible("r1", vec![Literal::simple("a")], Literal::simple("x"));
        let r2 = Rule::defeasible(
            "r2",
            vec![Literal::simple("a"), Literal::simple("b")],
            Literal::negated("x"),
        );
        let r3 = Rule::defeasible(
            "r3",
            vec![
                Literal::simple("a"),
                Literal::simple("b"),
                Literal::simple("c"),
            ],
            Literal::simple("y"),
        );
        let r4 = Rule::defeasible("r4", vec![Literal::simple("d")], Literal::negated("y"));

        let suggestions = suggest_superiorities(vec![(&r1, &r2), (&r3, &r4)]);
        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].superior, "r2");
        assert_eq!(suggestions[1].superior, "r3");
    }

    #[test]
    fn test_specificity_confidence_values() {
        // 2 vs 1 => (2-1)/2 = 0.5
        assert!((specificity_confidence(2, 1) - 0.5).abs() < 1e-10);
        // 3 vs 1 => (3-1)/3 = 0.667
        assert!((specificity_confidence(3, 1) - 2.0 / 3.0).abs() < 1e-10);
        // 1 vs 0 => (1-0)/1 = 1.0
        assert!((specificity_confidence(1, 0) - 1.0).abs() < 1e-10);
        // 0 vs 0 => 0.0 (guard)
        assert!((specificity_confidence(0, 0) - 0.0).abs() < 1e-10);
    }

    #[test]
    fn test_check_completeness_all_resolved() {
        let conflicts = vec![("r1", "r2"), ("r3", "r4")];
        let declared = vec![("r1", "r2"), ("r4", "r3")]; // note reversed order for second

        let unresolved = check_completeness(conflicts, declared);
        assert!(
            unresolved.is_empty(),
            "All conflicts have declared superiorities"
        );
    }

    #[test]
    fn test_check_completeness_some_unresolved() {
        let conflicts = vec![("r1", "r2"), ("r3", "r4"), ("r5", "r6")];
        let declared = vec![("r1", "r2")];

        let unresolved = check_completeness(conflicts, declared);
        assert_eq!(unresolved.len(), 2);
        assert_eq!(unresolved[0].rule_a, "r3");
        assert_eq!(unresolved[0].rule_b, "r4");
        assert_eq!(unresolved[1].rule_a, "r5");
        assert_eq!(unresolved[1].rule_b, "r6");
    }

    #[test]
    fn test_check_completeness_empty_conflicts() {
        let conflicts: Vec<(&str, &str)> = vec![];
        let declared = vec![("r1", "r2")];

        let unresolved = check_completeness(conflicts, declared);
        assert!(unresolved.is_empty());
    }

    #[test]
    fn test_check_completeness_empty_declared() {
        let conflicts = vec![("r1", "r2")];
        let declared: Vec<(&str, &str)> = vec![];

        let unresolved = check_completeness(conflicts, declared);
        assert_eq!(unresolved.len(), 1);
    }

    #[test]
    fn test_check_completeness_reverse_declared() {
        // Declaring (r2, r1) should satisfy conflict (r1, r2)
        let conflicts = vec![("r1", "r2")];
        let declared = vec![("r2", "r1")];

        let unresolved = check_completeness(conflicts, declared);
        assert!(
            unresolved.is_empty(),
            "Reverse declaration should resolve the conflict"
        );
    }
}
