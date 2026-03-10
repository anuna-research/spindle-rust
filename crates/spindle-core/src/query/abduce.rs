//! Abduction operator: finding hypotheses that would make a goal provable.
//!
//! Given a goal literal and a theory, the [`abduce`] function uses backward
//! chaining to find minimal sets of facts whose addition would enable the
//! goal to be proven via the standard reasoning pipeline.
//!
//! Solutions are checked using **family-aware support**: both the initial
//! provability check and body-literal satisfaction use [`FamilyId`] matching
//! so that a temporal conclusion like `p[1,10]` satisfies an atemporal body
//! literal `p`. Each candidate solution is then **verified** by running the
//! reasoner with the hypothesized facts added, filtering out solutions that
//! would be blocked by defeaters or conflicts.

use std::collections::HashSet;
use std::fmt;

use crate::error::Result;
use crate::literal::Literal;
use crate::projection::FamilyId;
use crate::reason::reason;
use crate::rule::{Rule, RuleType};
use crate::theory::Theory;

use super::what_if::next_hyp_label;

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

/// Check whether `goal` is positively supported by any conclusion in its
/// family (atemporal identity match via [`FamilyId`]).
fn is_family_provable(goal: &Literal, conclusions: &[crate::conclusion::Conclusion]) -> bool {
    let family = FamilyId::from(goal);
    conclusions
        .iter()
        .any(|c| c.conclusion_type.is_positive() && FamilyId::from(&c.literal) == family)
}

/// Check whether `lit` is satisfied by any positive conclusion in the same
/// family.  Used to determine which body literals are already supported.
fn is_body_satisfied(lit: &Literal, proven_families: &HashSet<FamilyId>) -> bool {
    proven_families.contains(&FamilyId::from(lit))
}

/// Verify a candidate solution by actually running reasoning with the
/// hypothesized facts added. Returns `true` if the goal becomes provable.
fn verify_solution(
    theory: &Theory,
    goal: &Literal,
    hypothesized_facts: &HashSet<Literal>,
) -> Result<bool> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static ABDUCE_COUNTER: AtomicU64 = AtomicU64::new(0);

    let unique_id = ABDUCE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut modified = theory.clone();
    for (i, lit) in hypothesized_facts.iter().enumerate() {
        let label = next_hyp_label(&modified, unique_id, i + 1);
        let rule = Rule::fact(&label, lit.clone());
        modified.add_rule(rule);
    }

    let conclusions = reason(&modified)?;
    Ok(is_family_provable(goal, &conclusions))
}

/// Perform abductive reasoning: "What facts would make this goal provable?"
///
/// Uses backward chaining to find minimal sets of facts that would
/// enable the goal to be proven. Body-literal satisfaction uses
/// family-aware matching (via [`FamilyId`]) so that temporal conclusions
/// can satisfy atemporal body literals. Each candidate solution is
/// **verified** by running reasoning with the hypothesized facts added,
/// filtering out solutions that would be blocked by defeaters or
/// conflicts.
pub fn abduce(theory: &Theory, goal: &Literal, max_solutions: usize) -> Result<AbductionResult> {
    let mut result = AbductionResult::new(goal.clone());

    // First check if already provable (family-aware)
    let conclusions = reason(theory)?;
    if is_family_provable(goal, &conclusions) {
        result
            .solutions
            .push(AbductionSolution::new(HashSet::new()));
        return Ok(result);
    }

    // Collect families that are already positively proven
    let proven_families: HashSet<FamilyId> = conclusions
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| FamilyId::from(&c.literal))
        .collect();

    // Find rules that could derive the goal (family-aware head match)
    let goal_family = FamilyId::from(goal);
    let mut candidates: Vec<HashSet<Literal>> = Vec::new();

    for rule in theory.rules() {
        let head_family = FamilyId::from(rule.head_literal());
        if head_family == goal_family && rule.rule_type != RuleType::Defeater {
            // Find missing body literals (family-aware satisfaction)
            let body_lits: Vec<Literal> = rule
                .body
                .iter()
                .filter_map(|bl| bl.as_logic().map(|l| l.to_literal()))
                .collect();
            let missing: HashSet<_> = body_lits
                .into_iter()
                .filter(|b| !is_body_satisfied(b, &proven_families))
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

    // Verify each candidate and keep only those that actually work
    let mut solutions: Vec<HashSet<Literal>> = Vec::new();
    for facts in candidates {
        if solutions.len() >= max_solutions {
            break;
        }
        if verify_solution(theory, goal, &facts)? {
            solutions.push(facts);
        }
    }

    for facts in solutions {
        let mut sol = AbductionSolution::new(facts);
        // Track rules used — any rule whose head matches the goal family
        for rule in theory.rules() {
            if FamilyId::from(rule.head_literal()) == goal_family {
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
    fn test_abduce_verification_filters_blocked_solutions() {
        // Rule: bird => flies.  Defeater: broken_wing =X> ~flies.
        // Both bird and broken_wing are missing. Adding bird alone should NOT
        // make flies provable if broken_wing is also added — but here only
        // broken_wing is present, so bird should work.
        //
        // Actually: if broken_wing is already a fact, adding bird should NOT
        // pass verification because the defeater blocks it.
        let mut th = Theory::new();
        th.add_fact("broken_wing");
        th.add_defeasible_rule(&["bird"], "flies");
        th.add_defeater(&["broken_wing"], "~flies");

        let result = abduce(&th, &Literal::simple("flies"), 10).unwrap();
        // The candidate {bird} should fail verification because the defeater
        // blocks the conclusion. The only solution should be the trivial
        // {flies} fallback.
        for sol in &result.solutions {
            assert!(
                !sol.facts.contains(&Literal::simple("bird")) || sol.facts.len() > 1,
                "Solution containing only 'bird' should be filtered out \
                 because the defeater blocks 'flies'"
            );
        }
    }

    #[test]
    fn test_abduce_verification_accepts_valid_solutions() {
        // Rule: p => q.  No defeaters. Adding p should verify successfully.
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
        let temporal_head = Literal::new(
            "p",
            false,
            crate::mode::Mode::empty(),
            crate::temporal::Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
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
}
