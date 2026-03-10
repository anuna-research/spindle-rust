//! Abduction operator: finding hypotheses that would make a goal provable.
//!
//! Given a goal literal and a theory, the [`abduce`] function uses backward
//! chaining to find minimal sets of facts whose addition would enable the
//! goal to be proven via the standard reasoning pipeline.
//!
//! Uses query-shape-aware support: atemporal goals and body literals match by
//! family, while bounded temporal literals remain exact per SPEC-020.
//!
//! `abduce()` returns raw hypothesis sets derived from rule bodies. It does
//! not verify that adding those facts actually makes the goal provable after
//! defeaters or conflicts are considered. Use [`super::requires`] for verified
//! fact-set search.

use std::collections::HashSet;
use std::fmt;

use crate::conclusion::Conclusion;
use crate::error::Result;
use crate::literal::Literal;
use crate::reason::reason;
use crate::rule::RuleType;
use crate::theory::Theory;

use super::{has_positive_match, semantic_literal_matches};

// =============================================================================
// TYPES
// =============================================================================

/// A solution to an abduction problem
#[derive(Debug, Clone)]
pub struct AbductionSolution {
    /// Facts that need to be assumed
    pub facts: HashSet<Literal>,
    /// Rules that would be used in the derivation
    pub rules_used: HashSet<String>,
    /// Confidence score (if trust-weighted)
    pub confidence: f64,
}

impl AbductionSolution {
    /// Create a new abduction solution
    pub fn new(facts: HashSet<Literal>) -> Self {
        Self {
            facts,
            rules_used: HashSet::new(),
            confidence: 1.0,
        }
    }

    /// Check if the goal is already provable (empty solution)
    pub fn is_already_provable(&self) -> bool {
        self.facts.is_empty()
    }

    /// Get the number of facts needed
    pub fn size(&self) -> usize {
        self.facts.len()
    }
}

/// Result of an abduction query
#[derive(Debug, Clone)]
pub struct AbductionResult {
    /// The goal literal
    pub goal: Literal,
    /// Possible solutions (sets of facts to assume)
    pub solutions: Vec<AbductionSolution>,
}

impl AbductionResult {
    /// Create a new abduction result
    pub fn new(goal: Literal) -> Self {
        Self {
            goal,
            solutions: Vec::new(),
        }
    }

    /// Check if any solution exists
    pub fn has_solutions(&self) -> bool {
        !self.solutions.is_empty()
    }

    /// Check if the goal is already provable
    pub fn is_already_provable(&self) -> bool {
        self.solutions.iter().any(|s| s.is_already_provable())
    }

    /// Get the smallest solution (fewest facts needed)
    pub fn smallest_solution(&self) -> Option<&AbductionSolution> {
        self.solutions.iter().min_by_key(|s| s.size())
    }
}

impl fmt::Display for AbductionResult {
    /// Convert to human-readable string
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.solutions.is_empty() {
            return write!(f, "No hypotheses found for {}", self.goal);
        }

        writeln!(f, "Abduction solutions for {}:", self.goal)?;

        for (i, sol) in self.solutions.iter().enumerate() {
            if sol.is_already_provable() {
                writeln!(f, "  {}. Already provable", i + 1)?;
            } else {
                let facts: Vec<_> = sol.facts.iter().map(|l| l.to_string()).collect();
                writeln!(f, "  {}. Add facts: {{{}}}", i + 1, facts.join(", "))?;
            }
        }
        Ok(())
    }
}

// =============================================================================
// OPERATORS
// =============================================================================

/// Check whether `goal` is positively supported according to SPEC-020 query
/// semantics: exact-temporal for bounded literals, family-aware for atemporal.
fn is_goal_provable(goal: &Literal, conclusions: &[Conclusion]) -> bool {
    has_positive_match(goal, conclusions)
}

/// Check whether `lit` is satisfied according to the literal's own match
/// semantics. Atemporal premises match by family; bounded premises stay exact.
fn is_body_satisfied(lit: &Literal, conclusions: &[Conclusion]) -> bool {
    has_positive_match(lit, conclusions)
}

/// Perform abductive reasoning: "What facts would make this goal provable?"
///
/// Uses backward chaining to find minimal sets of facts that would
/// enable the goal to be proven. Atemporal goals and premises use family
/// matching; bounded temporal literals stay exact. Returns at most
/// `max_solutions` raw hypothesis sets; callers that need verified fact-sets
/// should use [`super::requires`].
pub fn abduce(theory: &Theory, goal: &Literal, max_solutions: usize) -> Result<AbductionResult> {
    let mut result = AbductionResult::new(goal.clone());

    // First check if already provable using the goal's own match semantics.
    let conclusions = reason(theory)?;
    if is_goal_provable(goal, &conclusions) {
        result
            .solutions
            .push(AbductionSolution::new(HashSet::new()));
        return Ok(result);
    }

    // Find rules that could derive the goal according to the goal's own match
    // semantics: exact when bounded, family-aware when atemporal.
    let mut candidates: Vec<HashSet<Literal>> = Vec::new();

    for rule in theory.rules() {
        if semantic_literal_matches(goal, rule.head_literal())
            && rule.rule_type != RuleType::Defeater
        {
            // Find missing body literals using each premise's own match semantics.
            let body_lits: Vec<Literal> = rule
                .body
                .iter()
                .filter_map(|bl| bl.as_logic().map(|l| l.to_literal()))
                .collect();
            let missing: HashSet<_> = body_lits
                .into_iter()
                .filter(|b| !is_body_satisfied(b, &conclusions))
                .collect();

            if missing.is_empty() {
                // All body satisfied but goal not provable — blocked by
                // defeater/conflict; skip this rule path.
                continue;
            }

            candidates.push(missing);
        }
    }

    // If no direct rules, try the trivial solution (add goal itself)
    if candidates.is_empty() {
        let mut trivial = HashSet::new();
        trivial.insert(goal.clone());
        candidates.push(trivial);
    }

    // Sort by size (smallest first)
    candidates.sort_by_key(|s| s.len());

    for facts in candidates.into_iter().take(max_solutions) {
        let mut sol = AbductionSolution::new(facts);
        // Track rules used with the same goal/head matching semantics as above.
        for rule in theory.rules() {
            if semantic_literal_matches(goal, rule.head_literal()) {
                sol.rules_used.insert(rule.label.clone());
            }
        }
        result.solutions.push(sol);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule::Rule;
    use crate::temporal::{Temporal, TimePoint};

    // ==========================================================================
    // HELPER FUNCTIONS
    // ==========================================================================

    /// Create a minimal theory with just a single fact
    fn make_fact_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("bird");
        th
    }

    /// Create a theory where a premise is missing (tests_pass not asserted)
    fn make_missing_premise_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("code_written");
        th.add_defeasible_rule(&["code_written", "tests_pass"], "ready_review");
        th
    }

    fn temporal_lit(name: &str, start: i64, end: i64) -> Literal {
        Literal::new(
            name,
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::Moment(start), TimePoint::Moment(end)),
            vec![],
        )
    }

    // ==========================================================================
    // ABDUCTION OPERATOR TESTS - Basic Functionality
    // ==========================================================================

    #[test]
    fn test_abduce_returns_result() {
        let th = make_missing_premise_theory();
        let result = abduce(&th, &Literal::simple("ready_review"), 10).unwrap();
        assert!(result.has_solutions());
    }

    #[test]
    fn test_abduce_preserves_goal() {
        let th = make_missing_premise_theory();
        let goal = Literal::simple("ready_review");
        let result = abduce(&th, &goal, 10).unwrap();
        assert_eq!(result.goal.name(), "ready_review");
    }

    #[test]
    fn test_abduce_already_provable() {
        let th = make_fact_theory();
        let result = abduce(&th, &Literal::simple("bird"), 10).unwrap();
        assert!(result.is_already_provable());
    }

    #[test]
    fn test_abduce_already_provable_empty_solution() {
        let th = make_fact_theory();
        let result = abduce(&th, &Literal::simple("bird"), 10).unwrap();
        let sol = result.smallest_solution().unwrap();
        assert!(sol.is_already_provable());
        assert_eq!(sol.size(), 0);
    }

    // ==========================================================================
    // ABDUCTION OPERATOR TESTS - Finding Minimal Fact Sets
    // ==========================================================================

    #[test]
    fn test_abduce_finds_missing_facts() {
        let th = make_missing_premise_theory();
        let result = abduce(&th, &Literal::simple("ready_review"), 10).unwrap();
        let sol = result.smallest_solution().unwrap();
        // Should find tests_pass as needed
        assert!(sol.facts.iter().any(|l| l.name() == "tests_pass"));
    }

    #[test]
    fn test_abduce_single_missing_premise() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");

        let result = abduce(&theory, &Literal::simple("q"), 10).unwrap();
        let sol = result.smallest_solution().unwrap();
        assert_eq!(sol.size(), 1);
        assert!(sol.facts.contains(&Literal::simple("p")));
    }

    #[test]
    fn test_abduce_multiple_missing_premises() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a", "b", "c"], "goal");

        let result = abduce(&theory, &Literal::simple("goal"), 10).unwrap();
        let sol = result.smallest_solution().unwrap();
        assert_eq!(sol.size(), 3);
    }

    #[test]
    fn test_abduce_partial_satisfaction() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a", "b"], "goal");

        let result = abduce(&theory, &Literal::simple("goal"), 10).unwrap();
        let sol = result.smallest_solution().unwrap();
        assert_eq!(sol.size(), 1);
        assert!(sol.facts.contains(&Literal::simple("b")));
    }

    // ==========================================================================
    // ABDUCTION OPERATOR TESTS - Multiple Solutions
    // ==========================================================================

    #[test]
    fn test_abduce_finds_alternative_paths() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["bird"], "flies");
        theory.add_defeasible_rule(&["plane"], "flies");

        let result = abduce(&theory, &Literal::simple("flies"), 10).unwrap();
        assert_eq!(result.solutions.len(), 2);
    }

    #[test]
    fn test_abduce_solutions_contain_alternatives() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["bird"], "flies");
        theory.add_defeasible_rule(&["plane"], "flies");

        let result = abduce(&theory, &Literal::simple("flies"), 10).unwrap();

        let has_bird = result
            .solutions
            .iter()
            .any(|s| s.facts.contains(&Literal::simple("bird")));
        let has_plane = result
            .solutions
            .iter()
            .any(|s| s.facts.contains(&Literal::simple("plane")));

        assert!(has_bird);
        assert!(has_plane);
    }

    #[test]
    fn test_abduce_respects_max_solutions() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a"], "x");
        theory.add_defeasible_rule(&["b"], "x");
        theory.add_defeasible_rule(&["c"], "x");

        let result = abduce(&theory, &Literal::simple("x"), 1).unwrap();
        assert!(result.solutions.len() <= 1);
    }

    #[test]
    fn test_abduce_solutions_sorted_by_size() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a"], "goal");
        theory.add_defeasible_rule(&["b", "c"], "goal");

        let result = abduce(&theory, &Literal::simple("goal"), 10).unwrap();

        if result.solutions.len() > 1 {
            let sizes: Vec<_> = result.solutions.iter().map(|s| s.size()).collect();
            for i in 1..sizes.len() {
                assert!(
                    sizes[i - 1] <= sizes[i],
                    "Solutions should be sorted by size"
                );
            }
        }
    }

    // ==========================================================================
    // ABDUCTION OPERATOR TESTS - No Rules Case
    // ==========================================================================

    #[test]
    fn test_abduce_hypothesizes_goal_when_no_rules() {
        let th = Theory::new();
        let result = abduce(&th, &Literal::simple("unknown"), 10).unwrap();

        let sol = result.smallest_solution().unwrap();
        // Should hypothesize the literal itself
        assert!(sol.facts.contains(&Literal::simple("unknown")));
    }

    // ==========================================================================
    // ABDUCTION OPERATOR TESTS - Solution Properties
    // ==========================================================================

    #[test]
    fn test_abduction_solution_size() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a", "b"], "goal");

        let result = abduce(&theory, &Literal::simple("goal"), 10).unwrap();
        let sol = result.smallest_solution().unwrap();
        assert_eq!(sol.size(), 2);
    }

    #[test]
    fn test_abduction_solution_default_confidence() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");

        let result = abduce(&theory, &Literal::simple("q"), 10).unwrap();
        let sol = result.smallest_solution().unwrap();
        assert_eq!(sol.confidence, 1.0);
    }

    #[test]
    fn test_abduction_solution_tracks_rules() {
        let mut theory = Theory::new();
        let r1 = theory.add_defeasible_rule(&["p"], "q");

        let result = abduce(&theory, &Literal::simple("q"), 10).unwrap();
        let sol = result.smallest_solution().unwrap();
        assert!(sol.rules_used.contains(&r1));
    }

    #[test]
    fn test_smallest_solution() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a"], "goal");
        theory.add_defeasible_rule(&["b", "c"], "goal");

        let result = abduce(&theory, &Literal::simple("goal"), 10).unwrap();
        let smallest = result.smallest_solution().unwrap();
        assert_eq!(smallest.size(), 1);
    }

    #[test]
    fn test_smallest_solution_none_for_empty() {
        let result = AbductionResult::new(Literal::simple("x"));
        assert!(result.smallest_solution().is_none());
    }

    // ==========================================================================
    // ABDUCTION OPERATOR TESTS - Display and Formatting
    // ==========================================================================

    #[test]
    fn test_abduction_result_display() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");

        let result = abduce(&theory, &Literal::simple("q"), 10).unwrap();
        let s = result.to_string();
        assert!(s.contains("Abduction"));
        assert!(s.contains("Add facts"));
    }

    #[test]
    fn test_abduction_result_display_already_provable() {
        let mut theory = Theory::new();
        theory.add_fact("p");

        let result = abduce(&theory, &Literal::simple("p"), 10).unwrap();
        let s = result.to_string();
        assert!(s.contains("Already provable"));
    }

    #[test]
    fn test_abduction_result_display_no_solutions() {
        let result = AbductionResult::new(Literal::simple("x"));
        let s = result.to_string();
        assert!(s.contains("No hypotheses"));
    }

    // ==========================================================================
    // FAMILY-AWARE TESTS
    // ==========================================================================

    #[test]
    fn test_abduce_family_aware_already_provable() {
        // Temporal fact p[1,10] should satisfy atemporal goal query for p
        // via family matching.
        let mut th = Theory::new();
        let temporal_lit = Literal::new(
            "p",
            false,
            crate::mode::Mode::empty(),
            crate::temporal::Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        th.add_rule(Rule::fact("f1", temporal_lit));

        let result = abduce(&th, &Literal::simple("p"), 10).unwrap();
        assert!(
            result.is_already_provable(),
            "Temporal p[1,10] should family-satisfy atemporal goal p"
        );
    }

    #[test]
    fn test_abduce_family_aware_body_satisfaction() {
        // Rule: a => goal. Temporal fact a[5,15] should satisfy body literal a
        // via family matching, yielding no missing facts.
        let mut th = Theory::new();
        let temporal_a = Literal::new(
            "a",
            false,
            crate::mode::Mode::empty(),
            crate::temporal::Temporal::new(
                crate::temporal::TimePoint::Moment(5),
                crate::temporal::TimePoint::Moment(15),
            ),
            vec![],
        );
        th.add_rule(Rule::fact("f1", temporal_a));
        th.add_defeasible_rule(&["a"], "goal");

        let result = abduce(&th, &Literal::simple("goal"), 10).unwrap();
        assert!(
            result.is_already_provable(),
            "Temporal a[5,15] should family-satisfy body literal a"
        );
    }

    #[test]
    fn test_abduce_returns_raw_candidates_even_if_blocked() {
        // Rule: bird => flies.  Defeater: broken_wing =X> ~flies.
        // When broken_wing is already true, abduce should still report the
        // raw missing premise {bird}; verification belongs to requires().
        let mut th = Theory::new();
        th.add_fact("broken_wing");
        th.add_defeasible_rule(&["bird"], "flies");
        th.add_defeater(&["broken_wing"], "~flies");

        let result = abduce(&th, &Literal::simple("flies"), 10).unwrap();
        assert!(result.has_solutions());
        assert!(
            result
                .solutions
                .iter()
                .any(|sol| sol.facts == HashSet::from([Literal::simple("bird")]))
        );
    }

    #[test]
    fn test_abduce_returns_valid_raw_solutions() {
        // Rule: p => q. No defeaters. The raw hypothesis {p} should be found.
        let mut th = Theory::new();
        th.add_defeasible_rule(&["p"], "q");

        let result = abduce(&th, &Literal::simple("q"), 10).unwrap();
        assert!(result.has_solutions());
        let sol = result.smallest_solution().unwrap();
        assert!(sol.facts.contains(&Literal::simple("p")));
    }

    #[test]
    fn test_abduce_family_head_match() {
        // Rule with temporal head p[1,10] should be considered as a candidate
        // when the goal is atemporal p, because they share the same family.
        let mut th = Theory::new();
        let temporal_head = temporal_lit("p", 1, 10);
        let body = vec![Literal::simple("a")];
        th.add_rule(crate::rule::Rule::defeasible("r1", body, temporal_head));

        let result = abduce(&th, &Literal::simple("p"), 10).unwrap();
        assert!(result.has_solutions());
        let sol = result.smallest_solution().unwrap();
        assert!(
            sol.facts.contains(&Literal::simple("a")),
            "Should find body literal 'a' as hypothesis via family head match"
        );
    }

    #[test]
    fn test_abduce_temporal_goal_does_not_cross_match_other_window() {
        let mut th = Theory::new();
        th.add_rule(Rule::fact("f1", temporal_lit("p", 20, 30)));

        let goal = temporal_lit("p", 1, 10);
        let result = abduce(&th, &goal, 10).unwrap();

        assert!(
            !result.is_already_provable(),
            "A different temporal window must not satisfy a bounded goal"
        );
        let sol = result.smallest_solution().unwrap();
        let fact = sol.facts.iter().next().unwrap();
        assert_eq!(fact.to_spl(), goal.to_spl());
    }

    #[test]
    fn test_abduce_temporal_body_requires_exact_window() {
        let mut th = Theory::new();
        th.add_rule(Rule::fact("f1", temporal_lit("p", 20, 30)));
        th.add_fact("block");
        th.add_rule(Rule::defeasible(
            "r1",
            vec![temporal_lit("p", 1, 10)],
            Literal::simple("q"),
        ));
        th.add_defeater(&["block"], "~q");

        let result = abduce(&th, &Literal::simple("q"), 10).unwrap();
        let sol = result.smallest_solution().unwrap();
        let fact = sol.facts.iter().next().unwrap();

        assert_eq!(
            fact.to_spl(),
            temporal_lit("p", 1, 10).to_spl(),
            "A temporal premise should stay exact during body satisfaction"
        );
    }
}
