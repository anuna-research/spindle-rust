//! Query Operators Module
//!
//! This module provides interactive query operators for defeasible logic theories:
//!
//! - **What-If**: Hypothetical reasoning - "What if we assumed X?"
//! - **Why-Not**: Explanation of failures - "Why isn't X provable?"
//! - **Abduction**: Finding hypotheses - "What facts would make X provable?"

use std::collections::HashSet;
use std::fmt;

use crate::conclusion::ConclusionType;
use crate::literal::Literal;
use crate::reason::reason;
use crate::rule::{Rule, RuleType};
use crate::theory::Theory;

// =============================================================================
// QUERY RESULT STRUCTURES
// =============================================================================

/// Status of a query result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryStatus {
    /// The literal is provable
    Provable,
    /// The literal is refuted (complement provable)
    Refuted,
    /// The status is unknown (neither provable nor refuted)
    Unknown,
}

impl fmt::Display for QueryStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QueryStatus::Provable => write!(f, "provable"),
            QueryStatus::Refuted => write!(f, "refuted"),
            QueryStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// Result of querying a literal from a theory
#[derive(Debug, Clone)]
pub struct QueryResult {
    /// The queried literal
    pub literal: Literal,
    /// Query status
    pub status: QueryStatus,
    /// Conclusion type (if provable)
    pub conclusion_type: Option<ConclusionType>,
}

impl QueryResult {
    /// Create a new query result
    pub fn new(literal: Literal, status: QueryStatus) -> Self {
        Self {
            literal,
            status,
            conclusion_type: None,
        }
    }

    /// Set conclusion type
    pub fn with_conclusion_type(mut self, ct: ConclusionType) -> Self {
        self.conclusion_type = Some(ct);
        self
    }

    /// Check if provable
    pub fn is_provable(&self) -> bool {
        self.status == QueryStatus::Provable
    }

    /// Check if definitely provable
    pub fn is_definitely_provable(&self) -> bool {
        self.conclusion_type == Some(ConclusionType::DefinitelyProvable)
    }

    /// Check if defeasibly provable
    pub fn is_defeasibly_provable(&self) -> bool {
        self.conclusion_type == Some(ConclusionType::DefeasiblyProvable)
    }
}

/// Query a literal against a theory
pub fn query(theory: &Theory, literal: &Literal) -> QueryResult {
    let conclusions = reason(theory);
    let complement = literal.complement();

    // Check if literal is provable
    for conc in &conclusions {
        if conc.literal == *literal && conc.conclusion_type.is_positive() {
            return QueryResult::new(literal.clone(), QueryStatus::Provable)
                .with_conclusion_type(conc.conclusion_type);
        }
    }

    // Check if complement is provable (refuted)
    for conc in &conclusions {
        if conc.literal == complement && conc.conclusion_type.is_positive() {
            return QueryResult::new(literal.clone(), QueryStatus::Refuted);
        }
    }

    // Unknown
    QueryResult::new(literal.clone(), QueryStatus::Unknown)
}

// =============================================================================
// WHAT-IF (HYPOTHETICAL REASONING)
// =============================================================================

/// A hypothetical claim to be assumed
#[derive(Debug, Clone)]
pub struct HypotheticalClaim {
    /// Source of the claim (if any)
    pub source: Option<String>,
    /// The claimed literal
    pub literal: Literal,
}

impl HypotheticalClaim {
    /// Create an anonymous hypothetical claim
    pub fn new(literal: Literal) -> Self {
        Self {
            source: None,
            literal,
        }
    }

    /// Create a claim with source attribution
    pub fn with_source(literal: Literal, source: impl Into<String>) -> Self {
        Self {
            source: Some(source.into()),
            literal,
        }
    }
}

/// Result of a what-if query
#[derive(Debug, Clone)]
pub struct WhatIfResult {
    /// The hypothetical claims that were assumed
    pub hypotheticals: Vec<HypotheticalClaim>,
    /// The query result under the hypotheticals
    pub result: QueryResult,
    /// New conclusions enabled by the hypotheticals
    pub new_conclusions: Vec<Literal>,
    /// Conclusions that changed status
    pub changed_conclusions: Vec<(Literal, ConclusionType, ConclusionType)>,
}

impl WhatIfResult {
    /// Check if the query succeeded under the hypotheticals
    pub fn is_provable(&self) -> bool {
        self.result.is_provable()
    }

    /// Get the new literals that became provable
    pub fn newly_provable(&self) -> &[Literal] {
        &self.new_conclusions
    }
}

/// Perform hypothetical reasoning: "What if we assumed these facts?"
///
/// Creates a copy of the theory with hypothetical facts added,
/// runs reasoning, and returns the result. The original theory is unchanged.
pub fn what_if(
    theory: &Theory,
    hypotheticals: Vec<HypotheticalClaim>,
    goal: &Literal,
) -> WhatIfResult {
    // Get baseline conclusions
    let baseline = reason(theory);
    let baseline_provable: HashSet<_> = baseline
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| c.literal.canonical_name())
        .collect();

    // Create modified theory with hypotheticals
    let mut modified = theory.clone();
    for (i, hyp) in hypotheticals.iter().enumerate() {
        let label = format!("hyp{}", i + 1);
        let rule = Rule::fact(&label, hyp.literal.clone());
        modified.add_rule(rule);
    }

    // Reason on modified theory
    let modified_conclusions = reason(&modified);

    // Query result
    let result = query(&modified, goal);

    // Find new conclusions
    let new_conclusions: Vec<Literal> = modified_conclusions
        .iter()
        .filter(|c| {
            c.conclusion_type.is_positive()
                && !baseline_provable.contains(&c.literal.canonical_name())
        })
        .map(|c| c.literal.clone())
        .collect();

    // Find changed conclusions (simplified - just track new positives)
    let changed_conclusions = Vec::new(); // Could be expanded to track full changes

    WhatIfResult {
        hypotheticals,
        result,
        new_conclusions,
        changed_conclusions,
    }
}

/// Convenience function: Check if a goal would be provable given hypotheticals
pub fn what_if_provable(
    theory: &Theory,
    hypotheticals: Vec<HypotheticalClaim>,
    goal: &Literal,
) -> bool {
    what_if(theory, hypotheticals, goal).is_provable()
}

// =============================================================================
// WHY-NOT (EXPLANATION OF FAILURES)
// =============================================================================

/// Type of blocking condition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockingType {
    /// Missing premise in rule body
    MissingPremise,
    /// Defeated by a defeater
    Defeated,
    /// Contradicted by opposing conclusion
    Contradicted,
}

impl fmt::Display for BlockingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockingType::MissingPremise => write!(f, "missing premise"),
            BlockingType::Defeated => write!(f, "defeated"),
            BlockingType::Contradicted => write!(f, "contradicted"),
        }
    }
}

/// A condition that blocks a derivation
#[derive(Debug, Clone)]
pub struct BlockingCondition {
    /// Type of blocking
    pub blocking_type: BlockingType,
    /// The rule that was blocked
    pub rule_label: String,
    /// Missing literals (for MissingPremise)
    pub missing_literals: Vec<Literal>,
    /// Blocking rule (for Defeated/Contradicted)
    pub blocking_rule: Option<String>,
    /// Human-readable explanation
    pub explanation: String,
}

impl BlockingCondition {
    /// Create a missing premise blocking condition
    pub fn missing_premise(rule_label: impl Into<String>, missing: Vec<Literal>) -> Self {
        let missing_str: Vec<_> = missing.iter().map(|l| l.to_string()).collect();
        Self {
            blocking_type: BlockingType::MissingPremise,
            rule_label: rule_label.into(),
            missing_literals: missing,
            blocking_rule: None,
            explanation: format!("Missing premises: {}", missing_str.join(", ")),
        }
    }

    /// Create a defeated blocking condition
    pub fn defeated(rule_label: impl Into<String>, by_rule: impl Into<String>) -> Self {
        let by = by_rule.into();
        Self {
            blocking_type: BlockingType::Defeated,
            rule_label: rule_label.into(),
            missing_literals: Vec::new(),
            blocking_rule: Some(by.clone()),
            explanation: format!("Defeated by rule {}", by),
        }
    }

    /// Create a contradicted blocking condition
    pub fn contradicted(rule_label: impl Into<String>, by_rule: impl Into<String>) -> Self {
        let by = by_rule.into();
        Self {
            blocking_type: BlockingType::Contradicted,
            rule_label: rule_label.into(),
            missing_literals: Vec::new(),
            blocking_rule: Some(by.clone()),
            explanation: format!("Contradicted by {}", by),
        }
    }
}

/// Result of a why-not query
#[derive(Debug, Clone)]
pub struct WhyNotResult {
    /// The literal that is not provable
    pub literal: Literal,
    /// Rule that would derive this literal (if body was satisfied)
    pub would_derive: Option<String>,
    /// Conditions blocking the derivation
    pub blocked_by: Vec<BlockingCondition>,
}

impl WhyNotResult {
    /// Create a new why-not result
    pub fn new(literal: Literal) -> Self {
        Self {
            literal,
            would_derive: None,
            blocked_by: Vec::new(),
        }
    }

    /// Check if there are any blocking conditions
    pub fn has_blockers(&self) -> bool {
        !self.blocked_by.is_empty()
    }

    /// Get missing premises from all blocking conditions
    pub fn get_missing_premises(&self) -> Vec<&Literal> {
        self.blocked_by
            .iter()
            .filter(|b| b.blocking_type == BlockingType::MissingPremise)
            .flat_map(|b| b.missing_literals.iter())
            .collect()
    }
}

impl fmt::Display for WhyNotResult {
    /// Convert to human-readable string
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.blocked_by.is_empty() {
            return write!(
                f,
                "{} is not provable: no rules can derive it",
                self.literal
            );
        }

        writeln!(f, "{} is not provable:", self.literal)?;

        if let Some(ref rule) = self.would_derive {
            writeln!(f, "  Would be derived by rule: {}", rule)?;
        }

        writeln!(f, "  Blocked by:")?;
        for bc in &self.blocked_by {
            writeln!(f, "    - Rule {}: {}", bc.rule_label, bc.blocking_type)?;
            writeln!(f, "      ({})", bc.explanation)?;
        }
        Ok(())
    }
}

/// Explain why a literal is NOT provable
pub fn why_not(theory: &Theory, literal: &Literal) -> WhyNotResult {
    let conclusions = reason(theory);

    // First check if it IS provable (then why-not doesn't apply)
    let is_provable = conclusions
        .iter()
        .any(|c| c.literal == *literal && c.conclusion_type.is_positive());

    if is_provable {
        return WhyNotResult::new(literal.clone());
    }

    // Collect proven literals for checking body satisfaction
    let proven: HashSet<_> = conclusions
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| c.literal.canonical_name())
        .collect();

    let mut result = WhyNotResult::new(literal.clone());
    let mut found_rule = false;

    // Find rules that could derive this literal and why they don't fire
    for rule in theory.rules() {
        if rule.head_literal() == literal {
            found_rule = true;

            if result.would_derive.is_none() {
                result.would_derive = Some(rule.label.clone());
            }

            // Check which body literals are missing
            let missing: Vec<_> = rule
                .body
                .iter()
                .filter(|b| !proven.contains(&b.canonical_name()))
                .cloned()
                .collect();

            if !missing.is_empty() {
                result
                    .blocked_by
                    .push(BlockingCondition::missing_premise(&rule.label, missing));
            }
        }
    }

    // If no rules found at all
    if !found_rule {
        // Check if complement is proven (contradicted)
        let complement = literal.complement();
        if proven.contains(&complement.canonical_name()) {
            result.blocked_by.push(BlockingCondition {
                blocking_type: BlockingType::Contradicted,
                rule_label: String::new(),
                missing_literals: Vec::new(),
                blocking_rule: None,
                explanation: format!("Complement {} is proven", complement),
            });
        }
    }

    result
}

// =============================================================================
// ABDUCTION (FINDING HYPOTHESES)
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

/// Perform abductive reasoning: "What facts would make this goal provable?"
///
/// Uses backward chaining to find minimal sets of facts that would
/// enable the goal to be proven.
pub fn abduce(theory: &Theory, goal: &Literal, max_solutions: usize) -> AbductionResult {
    let mut result = AbductionResult::new(goal.clone());

    // First check if already provable
    let conclusions = reason(theory);
    let is_provable = conclusions
        .iter()
        .any(|c| c.literal == *goal && c.conclusion_type.is_positive());

    if is_provable {
        result
            .solutions
            .push(AbductionSolution::new(HashSet::new()));
        return result;
    }

    // Collect what's already proven
    let proven: HashSet<_> = conclusions
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| c.literal.canonical_name())
        .collect();

    // Find rules that could derive the goal
    let mut solutions: Vec<HashSet<Literal>> = Vec::new();

    for rule in theory.rules() {
        if rule.head_literal() == goal && rule.rule_type != RuleType::Defeater {
            // Find missing body literals
            let missing: HashSet<_> = rule
                .body
                .iter()
                .filter(|b| !proven.contains(&b.canonical_name()))
                .cloned()
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

    result
}

/// Convenience function: Get the minimal facts needed to prove a goal
pub fn requires(theory: &Theory, goal: &Literal) -> Vec<Literal> {
    let result = abduce(theory, goal, 1);
    result
        .smallest_solution()
        .map(|s| s.facts.iter().cloned().collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // QUERY TESTS
    // ==========================================================================

    #[test]
    fn test_query_provable() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let result = query(&theory, &Literal::simple("bird"));
        assert_eq!(result.status, QueryStatus::Provable);
        assert!(result.is_definitely_provable());
    }

    #[test]
    fn test_query_unknown() {
        let theory = Theory::new();

        let result = query(&theory, &Literal::simple("bird"));
        assert_eq!(result.status, QueryStatus::Unknown);
    }

    #[test]
    fn test_query_refuted() {
        let mut theory = Theory::new();
        theory.add_fact("~bird");

        let result = query(&theory, &Literal::simple("bird"));
        assert_eq!(result.status, QueryStatus::Refuted);
    }

    // ==========================================================================
    // WHAT-IF TESTS
    // ==========================================================================

    #[test]
    fn test_what_if_enables_conclusion() {
        let mut theory = Theory::new();
        theory.add_fact("code_complete");
        theory.add_defeasible_rule(&["tests_pass", "code_complete"], "ready_review");

        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        let result = what_if(&theory, hypotheticals, &Literal::simple("ready_review"));
        assert!(result.is_provable());
        assert!(result
            .new_conclusions
            .iter()
            .any(|l| l.name() == "ready_review"));
    }

    #[test]
    fn test_what_if_provable() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");

        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("p"))];

        assert!(what_if_provable(
            &theory,
            hypotheticals,
            &Literal::simple("q")
        ));
    }

    #[test]
    fn test_what_if_with_source() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["verified"], "trusted");

        let hypotheticals = vec![HypotheticalClaim::with_source(
            Literal::simple("verified"),
            "qa_team",
        )];

        let result = what_if(&theory, hypotheticals, &Literal::simple("trusted"));
        assert!(result.is_provable());
        assert_eq!(result.hypotheticals[0].source, Some("qa_team".to_string()));
    }

    // ==========================================================================
    // WHY-NOT TESTS
    // ==========================================================================

    #[test]
    fn test_why_not_missing_premise() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p", "q"], "r");
        theory.add_fact("p"); // q is missing

        let result = why_not(&theory, &Literal::simple("r"));
        assert!(result.has_blockers());

        let missing = result.get_missing_premises();
        assert!(missing.iter().any(|l| l.name() == "q"));
    }

    #[test]
    fn test_why_not_no_rules() {
        let theory = Theory::new();

        let result = why_not(&theory, &Literal::simple("unknown"));
        assert!(result.would_derive.is_none());
    }

    #[test]
    fn test_why_not_contradicted() {
        let mut theory = Theory::new();
        theory.add_fact("~flies"); // Complement is proven

        let result = why_not(&theory, &Literal::simple("flies"));
        assert!(result
            .blocked_by
            .iter()
            .any(|b| b.blocking_type == BlockingType::Contradicted));
    }

    #[test]
    fn test_why_not_string() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");

        let result = why_not(&theory, &Literal::simple("q"));
        let s = result.to_string();
        assert!(s.contains("not provable"));
        assert!(s.contains("missing premise") || s.contains("Missing"));
    }

    // ==========================================================================
    // ABDUCTION TESTS
    // ==========================================================================

    #[test]
    fn test_abduce_already_provable() {
        let mut theory = Theory::new();
        theory.add_fact("p");

        let result = abduce(&theory, &Literal::simple("p"), 10);
        assert!(result.is_already_provable());
    }

    #[test]
    fn test_abduce_finds_missing() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");

        let result = abduce(&theory, &Literal::simple("q"), 10);
        assert!(result.has_solutions());

        let sol = result.smallest_solution().unwrap();
        assert!(sol.facts.contains(&Literal::simple("p")));
    }

    #[test]
    fn test_abduce_multiple_missing() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a", "b", "c"], "goal");

        let result = abduce(&theory, &Literal::simple("goal"), 10);
        let sol = result.smallest_solution().unwrap();

        assert_eq!(sol.size(), 3); // a, b, c all missing
    }

    #[test]
    fn test_requires_convenience() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["precondition"], "result");

        let needed = requires(&theory, &Literal::simple("result"));
        assert!(needed.iter().any(|l| l.name() == "precondition"));
    }

    #[test]
    fn test_abduction_string() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["p"], "q");

        let result = abduce(&theory, &Literal::simple("q"), 10);
        let s = result.to_string();
        assert!(s.contains("Abduction"));
        assert!(s.contains("Add facts"));
    }
}
