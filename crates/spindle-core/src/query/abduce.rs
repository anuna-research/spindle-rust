//! Abduction operator: finding hypotheses that would make a goal provable.
//!
//! Given a goal literal and a theory, the [`abduce`] function uses backward
//! chaining to find minimal sets of facts whose addition would enable the
//! goal to be proven via the standard reasoning pipeline.

use std::collections::HashSet;
use std::fmt;

use crate::error::Result;
use crate::literal::Literal;
use crate::reason::reason;
use crate::rule::RuleType;
use crate::theory::Theory;

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

/// Perform abductive reasoning: "What facts would make this goal provable?"
///
/// Uses backward chaining to find minimal sets of facts that would
/// enable the goal to be proven.
pub fn abduce(theory: &Theory, goal: &Literal, max_solutions: usize) -> Result<AbductionResult> {
    let mut result = AbductionResult::new(goal.clone());

    // First check if already provable
    let conclusions = reason(theory)?;
    let is_provable = conclusions
        .iter()
        .any(|c| c.literal == *goal && c.conclusion_type.is_positive());

    if is_provable {
        result
            .solutions
            .push(AbductionSolution::new(HashSet::new()));
        return Ok(result);
    }

    // Collect what's already proven
    let proven: HashSet<_> = conclusions
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| c.literal.clone())
        .collect();

    // Find rules that could derive the goal
    let mut solutions: Vec<HashSet<Literal>> = Vec::new();

    for rule in theory.rules() {
        if rule.head_literal() == goal && rule.rule_type != RuleType::Defeater {
            // Find missing body literals (only logic literals)
            let body_lits: Vec<Literal> = rule
                .body
                .iter()
                .filter_map(|bl| bl.as_logic().map(|l| l.to_literal()))
                .collect();
            let missing: HashSet<_> = body_lits
                .into_iter()
                .filter(|b| !proven.contains(b))
                .collect();

            if missing.is_empty() {
                // All body satisfied, shouldn't happen if not provable
                // (could be blocked by defeater/conflict)
                continue;
            }

            // Simple: just add missing as hypotheses
            // More sophisticated: recursively find hypotheses for each missing
            solutions.push(missing);

            if solutions.len() >= max_solutions {
                break;
            }
        }
    }

    // If no direct rules, try to find indirect paths (simplified)
    if solutions.is_empty() {
        // Add the goal itself as a hypothesis (trivial solution)
        let mut trivial = HashSet::new();
        trivial.insert(goal.clone());
        solutions.push(trivial);
    }

    // Sort by size (smallest first) and limit
    solutions.sort_by_key(|s| s.len());
    solutions.truncate(max_solutions);

    for facts in solutions {
        let mut sol = AbductionSolution::new(facts);
        // Track rules used (simplified)
        for rule in theory.rules() {
            if rule.head_literal() == goal {
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
}
