//! Query Operators Module
//!
//! This module provides interactive query operators for defeasible logic theories:
//!
//! - **What-If**: Hypothetical reasoning - "What if we assumed X?"
//! - **Why-Not**: Explanation of failures - "Why isn't X provable?"
//! - **Abduction**: Finding hypotheses - "What facts would make X provable?"

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::conclusion::ConclusionType;
use crate::error::Result;
use crate::literal::Literal;
use crate::pipeline::PrepareOptions;
use crate::reason::{reason, reason_with_options};
use crate::rule::{Rule, RuleType};
use crate::theory::MetaValue;
use crate::theory::Theory;
use crate::trust::{TrustPolicy, TrustValue};

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

/// Filter for trust-based query results
#[derive(Debug, Clone, Default)]
pub struct TrustFilter {
    /// Minimum trust degree for a conclusion to be included
    pub min_degree: Option<TrustValue>,
    /// Only include conclusions from this source pattern
    pub source_pattern: Option<String>,
    /// Trust policy to use for evaluating trust
    pub policy: Option<TrustPolicy>,
}

impl TrustFilter {
    /// Create a new empty trust filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Set minimum trust degree
    pub fn with_min_degree(mut self, degree: TrustValue) -> Self {
        self.min_degree = Some(degree);
        self
    }

    /// Set source pattern filter
    pub fn with_source(mut self, pattern: impl Into<String>) -> Self {
        self.source_pattern = Some(pattern.into());
        self
    }

    /// Set trust policy
    pub fn with_policy(mut self, policy: TrustPolicy) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Check if a conclusion passes this filter.
    /// Requires a theory to look up rule metadata and a rule label.
    pub fn passes(&self, theory: &Theory, rule_label: Option<&str>) -> bool {
        let policy = match &self.policy {
            Some(p) => p,
            None => return true, // No policy means everything passes
        };

        // Get source from rule metadata, falling back to template label for grounded instances
        let source_id = rule_label.and_then(|label| {
            // Try the concrete label first
            let meta_source =
                theory
                    .get_meta(label)
                    .and_then(|meta| match meta.properties.get("source") {
                        Some(MetaValue::String(s)) => Some(s.as_str()),
                        _ => None,
                    });
            if meta_source.is_some() {
                return meta_source;
            }
            // Fall back to template label for grounded rule instances
            theory.get_rule(label).and_then(|rule| {
                let tl = rule.template_label();
                if tl != label {
                    theory
                        .get_meta(tl)
                        .and_then(|meta| match meta.properties.get("source") {
                            Some(MetaValue::String(s)) => Some(s.as_str()),
                            _ => None,
                        })
                } else {
                    None
                }
            })
        });

        // Check source pattern
        if let Some(ref pattern) = self.source_pattern {
            match source_id {
                Some(src) => {
                    if !src.contains(pattern.as_str()) {
                        return false;
                    }
                }
                None => return false, // No source but filter requires one
            }
        }

        // Check minimum degree
        if let Some(min) = self.min_degree {
            let trust = match source_id {
                Some(src) => policy.get_trust(src),
                None => policy.default_trust,
            };
            if trust < min {
                return false;
            }
        }

        true
    }
}

/// Query a literal against a theory
pub fn query(theory: &Theory, literal: &Literal) -> Result<QueryResult> {
    query_with_options(theory, literal, PrepareOptions::default())
}

/// Query a literal against a theory with custom options
///
/// This is the primary API for as-of queries. Use `reference_time` in options
/// to query at a specific point in time:
///
/// ```rust
/// use spindle_core::prelude::*;
/// use spindle_core::query::query_with_options;
/// use spindle_core::pipeline::PrepareOptions;
/// use spindle_core::temporal::TimePoint;
///
/// let mut theory = Theory::new();
/// theory.add_fact("bird");
///
/// // Query at a specific time (milliseconds since epoch)
/// let opts = PrepareOptions {
///     reference_time: Some(TimePoint::from_millis(1707220800000)),
///     ..Default::default()
/// };
/// let result = query_with_options(&theory, &Literal::simple("bird"), opts).unwrap();
/// ```
pub fn query_with_options(
    theory: &Theory,
    literal: &Literal,
    opts: PrepareOptions,
) -> Result<QueryResult> {
    let conclusions = reason_with_options(theory, opts)?;
    let complement = literal.complement();

    // Check if literal is provable
    for conc in &conclusions {
        if conc.literal == *literal && conc.conclusion_type.is_positive() {
            return Ok(QueryResult::new(literal.clone(), QueryStatus::Provable)
                .with_conclusion_type(conc.conclusion_type));
        }
    }

    // Check if complement is provable (refuted)
    for conc in &conclusions {
        if conc.literal == complement && conc.conclusion_type.is_positive() {
            return Ok(QueryResult::new(literal.clone(), QueryStatus::Refuted));
        }
    }

    // Unknown
    Ok(QueryResult::new(literal.clone(), QueryStatus::Unknown))
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

/// Global counter for unique hypothetical labels to avoid collision
static HYP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[inline]
fn conclusion_strength(ct: ConclusionType) -> u8 {
    match ct {
        ConclusionType::DefinitelyProvable => 4,
        ConclusionType::DefeasiblyProvable => 3,
        ConclusionType::DefeasiblyNotProvable => 2,
        ConclusionType::DefinitelyNotProvable => 1,
    }
}

fn strongest_conclusions_by_literal(
    conclusions: &[crate::conclusion::Conclusion],
) -> HashMap<Literal, ConclusionType> {
    let mut by_lit = HashMap::new();
    for conc in conclusions {
        by_lit
            .entry(conc.literal.clone())
            .and_modify(|old| {
                if conclusion_strength(conc.conclusion_type) > conclusion_strength(*old) {
                    *old = conc.conclusion_type;
                }
            })
            .or_insert(conc.conclusion_type);
    }
    by_lit
}

fn next_hyp_label(theory: &Theory, unique_id: u64, start_index: usize) -> String {
    let mut index = start_index.max(1);
    loop {
        let candidate = format!("__hyp_{unique_id}_{index}");
        if theory.get_rule(&candidate).is_none() {
            return candidate;
        }
        index += 1;
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
) -> Result<WhatIfResult> {
    // Get baseline conclusions
    let baseline = reason(theory)?;
    let baseline_provable: HashSet<_> = baseline
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| c.literal.clone())
        .collect();

    // Create modified theory with hypotheticals using unique labels
    let unique_id = HYP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut modified = theory.clone();
    for (i, hyp) in hypotheticals.iter().enumerate() {
        let label = next_hyp_label(&modified, unique_id, i + 1);
        let rule = Rule::fact(&label, hyp.literal.clone());
        modified.add_rule(rule);
    }

    // Reason on modified theory (only once, not via query which reasons again)
    let modified_conclusions = reason(&modified)?;

    // Determine goal status directly from conclusions (avoids calling query->reason again)
    let goal_complement = goal.complement();
    let mut result = QueryResult::new(goal.clone(), QueryStatus::Unknown);
    for conc in &modified_conclusions {
        if conc.literal == *goal && conc.conclusion_type.is_positive() {
            result = QueryResult::new(goal.clone(), QueryStatus::Provable)
                .with_conclusion_type(conc.conclusion_type);
            break;
        }
    }
    if result.status == QueryStatus::Unknown {
        for conc in &modified_conclusions {
            if conc.literal == goal_complement && conc.conclusion_type.is_positive() {
                result = QueryResult::new(goal.clone(), QueryStatus::Refuted);
                break;
            }
        }
    }

    // Find new conclusions
    let new_conclusions: Vec<Literal> = modified_conclusions
        .iter()
        .filter(|c| c.conclusion_type.is_positive() && !baseline_provable.contains(&c.literal))
        .map(|c| c.literal.clone())
        .collect();

    // Track changed conclusions by comparing strongest status per literal.
    // This captures both positive->positive changes and positive->negative
    // transitions (e.g. a literal that becomes unprovable under hypotheticals).
    let mut changed_conclusions = Vec::new();
    let baseline_by_lit = strongest_conclusions_by_literal(&baseline);
    let modified_by_lit = strongest_conclusions_by_literal(&modified_conclusions);

    let mut all_literals: HashSet<Literal> = baseline_by_lit.keys().cloned().collect();
    all_literals.extend(modified_by_lit.keys().cloned());
    for lit in all_literals {
        if let (Some(&old_type), Some(&new_type)) =
            (baseline_by_lit.get(&lit), modified_by_lit.get(&lit))
            && old_type != new_type
        {
            changed_conclusions.push((lit, old_type, new_type));
        }
    }

    Ok(WhatIfResult {
        hypotheticals,
        result,
        new_conclusions,
        changed_conclusions,
    })
}

/// Convenience function: Check if a goal would be provable given hypotheticals
pub fn what_if_provable(
    theory: &Theory,
    hypotheticals: Vec<HypotheticalClaim>,
    goal: &Literal,
) -> Result<bool> {
    Ok(what_if(theory, hypotheticals, goal)?.is_provable())
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
            explanation: format!("Defeated by rule {by}"),
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
            explanation: format!("Contradicted by {by}"),
        }
    }
}

/// Result of a why-not query
#[derive(Debug, Clone)]
pub struct WhyNotResult {
    /// The literal queried
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

    /// Check if the literal is actually provable.
    ///
    /// A provable literal has a deriving rule but no blockers.
    pub fn is_provable(&self) -> bool {
        self.would_derive.is_some() && self.blocked_by.is_empty()
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
        if self.is_provable() {
            write!(f, "{} is provable", self.literal)?;
            if let Some(ref rule) = self.would_derive {
                write!(f, " (derived by rule: {rule})")?;
            }
            return Ok(());
        }

        if self.blocked_by.is_empty() {
            return write!(
                f,
                "{} is not provable: no rules can derive it",
                self.literal
            );
        }

        writeln!(f, "{} is not provable:", self.literal)?;

        if let Some(ref rule) = self.would_derive {
            writeln!(f, "  Would be derived by rule: {rule}")?;
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
pub fn why_not(theory: &Theory, literal: &Literal) -> Result<WhyNotResult> {
    let conclusions = reason(theory)?;

    // First check if it IS provable (then why-not doesn't apply)
    let is_provable = conclusions
        .iter()
        .any(|c| c.literal == *literal && c.conclusion_type.is_positive());

    if is_provable {
        // Return a result with would_derive taken from the conclusion's rule_label
        let mut result = WhyNotResult::new(literal.clone());
        result.would_derive = conclusions
            .iter()
            .find(|c| c.literal == *literal && c.conclusion_type.is_positive())
            .and_then(|c| c.rule_label.clone());
        return Ok(result);
    }

    // Collect proven literals for checking body satisfaction
    let proven: HashSet<_> = conclusions
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| c.literal.clone())
        .collect();

    let complement = literal.complement();
    let mut result = WhyNotResult::new(literal.clone());
    let mut found_rule = false;

    // Find rules that could derive this literal and why they don't fire
    for rule in theory.rules() {
        if rule.head_literal() == literal && rule.rule_type != RuleType::Defeater {
            found_rule = true;

            if result.would_derive.is_none() {
                result.would_derive = Some(rule.label.clone());
            }

            // Check which body literals are missing
            let missing: Vec<_> = rule
                .body
                .iter()
                .filter(|b| !proven.contains(*b))
                .cloned()
                .collect();

            if !missing.is_empty() {
                result
                    .blocked_by
                    .push(BlockingCondition::missing_premise(&rule.label, missing));
            } else {
                // Body is fully satisfied but conclusion not proven.
                // Check for defeater blocking.
                let mut blocked = false;
                for attacker in theory.rules() {
                    if attacker.head_literal() == &complement {
                        let attacker_body_satisfied =
                            attacker.body.iter().all(|b| proven.contains(b));
                        if !attacker_body_satisfied {
                            continue;
                        }

                        if attacker.rule_type == RuleType::Defeater {
                            // Defeaters block unless the rule is explicitly superior
                            let rule_superior = theory.is_superior(&rule.label, &attacker.label);
                            if !rule_superior {
                                result.blocked_by.push(BlockingCondition::defeated(
                                    &rule.label,
                                    &attacker.label,
                                ));
                                blocked = true;
                            }
                        } else {
                            // For defeasible rules: check superiority both directions
                            let attacker_superior =
                                theory.is_superior(&attacker.label, &rule.label);
                            let rule_superior = theory.is_superior(&rule.label, &attacker.label);

                            if rule_superior && !attacker_superior {
                                // Rule is superior — skip this attacker
                                continue;
                            }

                            // Report as blocker if attacker is superior or ambiguity
                            result.blocked_by.push(BlockingCondition::contradicted(
                                &rule.label,
                                &attacker.label,
                            ));
                            blocked = true;
                        }
                    }
                }
                if !blocked {
                    // Body satisfied, no attackers found, but still not provable.
                    // This can happen with ambiguity blocking.
                    result.blocked_by.push(BlockingCondition {
                        blocking_type: BlockingType::Contradicted,
                        rule_label: rule.label.clone(),
                        missing_literals: Vec::new(),
                        blocking_rule: None,
                        explanation: "Body satisfied but conclusion blocked by ambiguity"
                            .to_string(),
                    });
                }
            }
        }
    }

    // If no rules found at all
    if !found_rule {
        // Check if complement is proven (contradicted)
        if proven.contains(&complement) {
            result.blocked_by.push(BlockingCondition {
                blocking_type: BlockingType::Contradicted,
                rule_label: String::new(),
                missing_literals: Vec::new(),
                blocking_rule: None,
                explanation: format!("Complement {complement} is proven"),
            });
        }
    }

    Ok(result)
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
            // Find missing body literals
            let missing: HashSet<_> = rule
                .body
                .iter()
                .filter(|b| !proven.contains(*b))
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

    Ok(result)
}

/// Convenience function: Get the minimal facts needed to prove a goal
pub fn requires(theory: &Theory, goal: &Literal) -> Result<Vec<Literal>> {
    let result = abduce(theory, goal, 1)?;
    Ok(result
        .smallest_solution()
        .map(|s| s.facts.iter().cloned().collect())
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // HELPER FUNCTIONS - Theory Building
    // ==========================================================================

    /// Create a minimal theory with just a single fact
    fn make_fact_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("bird");
        th
    }

    /// Create a theory with a defeasible rule chain: bird => flies
    fn make_defeasible_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_defeasible_rule(&["bird"], "flies");
        th
    }

    /// Create a theory with strict rules: human -> mortal
    fn make_strict_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("human");
        th.add_strict_rule(&["human"], "mortal");
        th
    }

    /// Create a theory with conflicting rules and superiority
    /// bird => flies, penguin => ~flies, penguin > bird
    fn make_conflict_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("penguin");
        let r1 = th.add_defeasible_rule(&["bird"], "flies");
        let r2 = th.add_defeasible_rule(&["penguin"], "~flies");
        th.add_superiority(&r2, &r1);
        th
    }

    /// Create a theory with a defeater
    /// bird => flies, broken_wing =X> ~flies (defeater)
    fn make_defeater_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("broken_wing");
        th.add_defeasible_rule(&["bird"], "flies");
        th.add_defeater(&["broken_wing"], "~flies");
        th
    }

    /// Create a theory with missing premises
    /// tests_pass + code_complete => ready_review (but tests_pass is missing)
    fn make_missing_premise_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("code_complete");
        th.add_defeasible_rule(&["tests_pass", "code_complete"], "ready_review");
        th
    }

    /// Create a multi-step chain theory: a => b => c => d
    fn make_chain_theory() -> Theory {
        let mut th = Theory::new();
        th.add_fact("a");
        th.add_defeasible_rule(&["a"], "b");
        th.add_defeasible_rule(&["b"], "c");
        th.add_defeasible_rule(&["c"], "d");
        th
    }

    // ==========================================================================
    // QUERY OPERATOR TESTS - Basic Query Status
    // ==========================================================================

    #[test]
    fn test_query_provable_fact() {
        let th = make_fact_theory();
        let result = query(&th, &Literal::simple("bird")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
        assert!(result.is_provable());
    }

    #[test]
    fn test_query_unknown_for_nonexistent() {
        let th = make_fact_theory();
        let result = query(&th, &Literal::simple("unknown")).unwrap();
        assert_eq!(result.status, QueryStatus::Unknown);
        assert!(!result.is_provable());
    }

    #[test]
    fn test_query_refuted_when_complement_proven() {
        let mut theory = Theory::new();
        theory.add_fact("~bird");

        let result = query(&theory, &Literal::simple("bird")).unwrap();
        assert_eq!(result.status, QueryStatus::Refuted);
    }

    // ==========================================================================
    // QUERY OPERATOR TESTS - Definite vs Defeasible Conclusions
    // ==========================================================================

    #[test]
    fn test_query_detects_definite_conclusions() {
        let th = make_fact_theory();
        let result = query(&th, &Literal::simple("bird")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
        assert!(result.is_definitely_provable());
    }

    #[test]
    fn test_query_detects_defeasible_conclusions() {
        let th = make_defeasible_theory();
        let result = query(&th, &Literal::simple("flies")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
        assert!(result.is_defeasibly_provable());
    }

    #[test]
    fn test_query_strict_rule_produces_definite() {
        let th = make_strict_theory();
        let result = query(&th, &Literal::simple("mortal")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
        assert!(result.is_definitely_provable());
    }

    #[test]
    fn test_query_chain_derivation() {
        let th = make_chain_theory();

        // All should be provable through the chain
        assert_eq!(
            query(&th, &Literal::simple("a")).unwrap().status,
            QueryStatus::Provable
        );
        assert_eq!(
            query(&th, &Literal::simple("b")).unwrap().status,
            QueryStatus::Provable
        );
        assert_eq!(
            query(&th, &Literal::simple("c")).unwrap().status,
            QueryStatus::Provable
        );
        assert_eq!(
            query(&th, &Literal::simple("d")).unwrap().status,
            QueryStatus::Provable
        );
    }

    #[test]
    fn test_query_conflict_with_superiority() {
        let th = make_conflict_theory();
        // flies should be refuted because penguin > bird and penguin => ~flies
        let result = query(&th, &Literal::simple("flies")).unwrap();
        assert_eq!(result.status, QueryStatus::Refuted);
    }

    #[test]
    fn test_query_defeater_blocks() {
        let th = make_defeater_theory();
        // With broken_wing, flies should be unknown (defeated but not refuted)
        let result = query(&th, &Literal::simple("flies")).unwrap();
        // Defeaters prevent positive conclusion but don't prove negative
        assert!(result.status == QueryStatus::Unknown || result.status == QueryStatus::Refuted);
    }

    #[test]
    fn test_query_negated_literal() {
        let mut theory = Theory::new();
        theory.add_fact("~happy");
        theory.add_defeasible_rule(&["~happy"], "sad");

        let result = query(&theory, &Literal::negated("happy")).unwrap();
        assert_eq!(result.status, QueryStatus::Provable);
    }

    #[test]
    fn test_query_result_literal_preserved() {
        let th = make_fact_theory();
        let lit = Literal::simple("bird");
        let result = query(&th, &lit).unwrap();
        assert_eq!(result.literal.name(), "bird");
    }

    #[test]
    fn test_query_status_display() {
        assert_eq!(format!("{}", QueryStatus::Provable), "provable");
        assert_eq!(format!("{}", QueryStatus::Refuted), "refuted");
        assert_eq!(format!("{}", QueryStatus::Unknown), "unknown");
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - Missing Premise Detection
    // ==========================================================================

    #[test]
    fn test_why_not_returns_result() {
        let th = make_missing_premise_theory();
        let result = why_not(&th, &Literal::simple("ready_review")).unwrap();
        assert!(result.has_blockers());
    }

    #[test]
    fn test_why_not_preserves_literal() {
        let th = make_missing_premise_theory();
        let lit = Literal::simple("ready_review");
        let result = why_not(&th, &lit).unwrap();
        assert_eq!(result.literal.name(), "ready_review");
    }

    #[test]
    fn test_why_not_shows_missing_premise() {
        let th = make_missing_premise_theory();
        let result = why_not(&th, &Literal::simple("ready_review")).unwrap();

        let missing = result.get_missing_premises();
        assert!(!missing.is_empty());
        assert!(missing.iter().any(|l| l.name() == "tests_pass"));
    }

    #[test]
    fn test_why_not_multiple_missing_premises() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a", "b", "c"], "goal");
        // All three premises are missing

        let result = why_not(&theory, &Literal::simple("goal")).unwrap();
        let missing = result.get_missing_premises();
        assert_eq!(missing.len(), 3);
    }

    #[test]
    fn test_why_not_partial_premises() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_defeasible_rule(&["a", "b", "c"], "goal");

        let result = why_not(&theory, &Literal::simple("goal")).unwrap();
        let missing = result.get_missing_premises();
        assert_eq!(missing.len(), 1);
        assert!(missing.iter().any(|l| l.name() == "c"));
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - No Rules Case
    // ==========================================================================

    #[test]
    fn test_why_not_no_rules_for_literal() {
        let th = Theory::new();
        let result = why_not(&th, &Literal::simple("unknown")).unwrap();
        assert!(result.would_derive.is_none());
    }

    #[test]
    fn test_why_not_no_applicable_rules() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["x"], "y"); // Rule exists but for different literal

        let result = why_not(&theory, &Literal::simple("z")).unwrap();
        assert!(result.would_derive.is_none());
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - Contradicted Detection
    // ==========================================================================

    #[test]
    fn test_why_not_contradicted() {
        let mut theory = Theory::new();
        theory.add_fact("~flies"); // Complement is proven

        let result = why_not(&theory, &Literal::simple("flies")).unwrap();
        assert!(
            result
                .blocked_by
                .iter()
                .any(|b| b.blocking_type == BlockingType::Contradicted)
        );
    }

    #[test]
    fn test_why_not_contradicted_explanation() {
        let mut theory = Theory::new();
        theory.add_fact("~happy");

        let result = why_not(&theory, &Literal::simple("happy")).unwrap();
        let contradicted = result
            .blocked_by
            .iter()
            .find(|b| b.blocking_type == BlockingType::Contradicted);

        assert!(contradicted.is_some());
        assert!(contradicted.unwrap().explanation.contains("~happy"));
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - For Provable Literals
    // ==========================================================================

    #[test]
    fn test_why_not_for_provable_literal() {
        let th = make_fact_theory();
        let result = why_not(&th, &Literal::simple("bird")).unwrap();
        // For provable literals, why-not should return no blockers
        assert!(!result.has_blockers());
    }

    // ==========================================================================
    // WHY-NOT OPERATOR TESTS - Display and Formatting
    // ==========================================================================

    #[test]
    fn test_why_not_display_format() {
        let th = make_missing_premise_theory();
        let result = why_not(&th, &Literal::simple("ready_review")).unwrap();
        let s = result.to_string();
        assert!(s.contains("not provable"));
        assert!(s.contains("ready_review"));
    }

    #[test]
    fn test_why_not_display_no_rules() {
        let th = Theory::new();
        let result = why_not(&th, &Literal::simple("unknown")).unwrap();
        let s = result.to_string();
        assert!(s.contains("no rules"));
    }

    #[test]
    fn test_blocking_condition_missing_premise() {
        let missing = vec![Literal::simple("a"), Literal::simple("b")];
        let bc = BlockingCondition::missing_premise("r1", missing);
        assert_eq!(bc.blocking_type, BlockingType::MissingPremise);
        assert_eq!(bc.rule_label, "r1");
        assert_eq!(bc.missing_literals.len(), 2);
    }

    #[test]
    fn test_blocking_condition_defeated() {
        let bc = BlockingCondition::defeated("r1", "d1");
        assert_eq!(bc.blocking_type, BlockingType::Defeated);
        assert_eq!(bc.blocking_rule, Some("d1".to_string()));
    }

    #[test]
    fn test_blocking_condition_contradicted() {
        let bc = BlockingCondition::contradicted("r1", "r2");
        assert_eq!(bc.blocking_type, BlockingType::Contradicted);
        assert!(bc.explanation.contains("Contradicted"));
    }

    #[test]
    fn test_blocking_type_display() {
        assert_eq!(
            format!("{}", BlockingType::MissingPremise),
            "missing premise"
        );
        assert_eq!(format!("{}", BlockingType::Defeated), "defeated");
        assert_eq!(format!("{}", BlockingType::Contradicted), "contradicted");
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

    // ==========================================================================
    // ABDUCTION OPERATOR TESTS - Convenience Functions
    // ==========================================================================

    #[test]
    fn test_requires_convenience_function() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["precondition"], "result");

        let needed = requires(&theory, &Literal::simple("result")).unwrap();
        assert!(!needed.is_empty());
        assert!(needed.iter().any(|l| l.name() == "precondition"));
    }

    #[test]
    fn test_requires_returns_empty_for_provable() {
        let mut theory = Theory::new();
        theory.add_fact("bird");

        let needed = requires(&theory, &Literal::simple("bird")).unwrap();
        assert!(needed.is_empty());
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
    // WHAT-IF OPERATOR TESTS - Basic Hypothetical Reasoning
    // ==========================================================================

    #[test]
    fn test_what_if_returns_result() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_what_if_enables_conclusion() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert!(result.is_provable());
        assert!(
            result
                .new_conclusions
                .iter()
                .any(|l| l.name() == "ready_review")
        );
    }

    #[test]
    fn test_what_if_does_not_modify_original_theory() {
        let th = make_missing_premise_theory();
        let original_count = th.rule_count();

        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
        let _ = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();

        // Original theory unchanged
        assert_eq!(th.rule_count(), original_count);
    }

    #[test]
    fn test_what_if_provable_convenience() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        assert!(what_if_provable(&th, hypotheticals, &Literal::simple("ready_review")).unwrap());
    }

    #[test]
    fn test_what_if_provable_false_when_insufficient() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("irrelevant"))];

        assert!(!what_if_provable(&th, hypotheticals, &Literal::simple("ready_review")).unwrap());
    }

    // ==========================================================================
    // WHAT-IF OPERATOR TESTS - Source Attribution
    // ==========================================================================

    #[test]
    fn test_what_if_with_source() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::with_source(
            Literal::simple("tests_pass"),
            "ci_server",
        )];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert!(result.is_provable());
        assert_eq!(
            result.hypotheticals[0].source,
            Some("ci_server".to_string())
        );
    }
    #[test]
    fn test_hypothetical_claim_anonymous() {
        let claim = HypotheticalClaim::new(Literal::simple("fact"));
        assert!(claim.source.is_none());
        assert_eq!(claim.literal.name(), "fact");
    }

    #[test]
    fn test_hypothetical_claim_with_source() {
        let claim = HypotheticalClaim::with_source(Literal::simple("verified"), "qa_team");
        assert_eq!(claim.source, Some("qa_team".to_string()));
    }

    // ==========================================================================
    // WHAT-IF OPERATOR TESTS - Multiple Hypotheticals
    // ==========================================================================

    #[test]
    fn test_what_if_multiple_claims() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a", "b"], "c");

        let hypotheticals = vec![
            HypotheticalClaim::new(Literal::simple("a")),
            HypotheticalClaim::new(Literal::simple("b")),
        ];

        assert!(what_if_provable(&theory, hypotheticals, &Literal::simple("c")).unwrap());
    }

    #[test]
    fn test_what_if_hypotheticals_preserved() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![
            HypotheticalClaim::new(Literal::simple("tests_pass")),
            HypotheticalClaim::new(Literal::simple("extra")),
        ];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert_eq!(result.hypotheticals.len(), 2);
    }

    // ==========================================================================
    // WHAT-IF OPERATOR TESTS - Counterfactual Queries
    // ==========================================================================

    #[test]
    fn test_what_if_chain_derivation() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");

        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("a"))];

        let result = what_if(&theory, hypotheticals, &Literal::simple("c")).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_what_if_negated_hypothetical() {
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["~broken"], "works");

        let hypotheticals = vec![HypotheticalClaim::new(Literal::negated("broken"))];

        let result = what_if(&theory, hypotheticals, &Literal::simple("works")).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_what_if_tracks_new_conclusions() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();

        // new_conclusions should contain ready_review and tests_pass
        assert!(!result.new_conclusions.is_empty());
    }

    #[test]
    fn test_what_if_newly_provable() {
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();

        let newly = result.newly_provable();
        assert!(newly.iter().any(|l| l.name() == "ready_review"));
    }

    // ==========================================================================
    // WHAT-IF OPERATOR TESTS - Already Provable
    // ==========================================================================

    #[test]
    fn test_what_if_already_provable() {
        let th = make_fact_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("extra"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("bird")).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_what_if_goal_already_satisfied() {
        let th = make_defeasible_theory();
        let hypotheticals = vec![]; // No hypotheticals needed

        let result = what_if(&th, hypotheticals, &Literal::simple("flies")).unwrap();
        assert!(result.is_provable());
    }

    // ==========================================================================
    // INTEGRATION TESTS - Cross-Operator Consistency
    // ==========================================================================

    #[test]
    fn test_query_and_why_not_consistency_for_provable() {
        let th = make_defeasible_theory();
        let flies = Literal::simple("flies");

        // Query says provable
        let q_result = query(&th, &flies).unwrap();
        assert!(q_result.is_provable());

        // Why-not should have no blockers
        let wn_result = why_not(&th, &flies).unwrap();
        assert!(!wn_result.has_blockers());
    }

    #[test]
    fn test_query_and_why_not_consistency_for_non_provable() {
        let th = make_missing_premise_theory();
        let ready = Literal::simple("ready_review");

        // Query says not provable
        let q_result = query(&th, &ready).unwrap();
        assert!(!q_result.is_provable());

        // Why-not should explain why
        let wn_result = why_not(&th, &ready).unwrap();
        assert!(wn_result.has_blockers());
    }

    #[test]
    fn test_why_not_and_requires_consistency() {
        let th = make_missing_premise_theory();
        let ready = Literal::simple("ready_review");

        // Why-not shows blocking
        let wn = why_not(&th, &ready).unwrap();
        let missing = wn.get_missing_premises();
        assert!(!missing.is_empty());

        // Requires finds what to add
        let needed = requires(&th, &ready).unwrap();
        assert!(!needed.is_empty());

        // Both should identify tests_pass
        assert!(missing.iter().any(|l| l.name() == "tests_pass"));
        assert!(needed.iter().any(|l| l.name() == "tests_pass"));
    }

    #[test]
    fn test_requires_and_what_if_consistency() {
        let th = make_missing_premise_theory();
        let ready = Literal::simple("ready_review");

        // Get what's needed via abduction
        let needed = requires(&th, &ready).unwrap();

        // Use those as hypotheticals
        let hypotheticals: Vec<_> = needed.into_iter().map(HypotheticalClaim::new).collect();

        // What-if should now prove the goal
        let result = what_if(&th, hypotheticals, &ready).unwrap();
        assert!(result.is_provable());
    }

    #[test]
    fn test_full_workflow_non_provable() {
        let th = make_missing_premise_theory();
        let ready = Literal::simple("ready_review");

        // 1. Query shows not provable
        let q = query(&th, &ready).unwrap();
        assert!(!q.is_provable());

        // 2. Why-not explains the blocker
        let wn = why_not(&th, &ready).unwrap();
        assert!(wn.has_blockers());
        assert!(!wn.get_missing_premises().is_empty());

        // 3. Abduction finds solution
        let ab = abduce(&th, &ready, 10).unwrap();
        assert!(ab.has_solutions());
        let sol = ab.smallest_solution().unwrap();
        assert!(sol.facts.iter().any(|l| l.name() == "tests_pass"));

        // 4. What-if verifies solution works
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
        let wi = what_if(&th, hypotheticals, &ready).unwrap();
        assert!(wi.is_provable());
    }

    #[test]
    fn test_full_workflow_provable() {
        let th = make_defeasible_theory();
        let flies = Literal::simple("flies");

        // 1. Query shows provable
        let q = query(&th, &flies).unwrap();
        assert!(q.is_provable());

        // 2. Why-not has no blockers
        let wn = why_not(&th, &flies).unwrap();
        assert!(!wn.has_blockers());

        // 3. Abduction shows already provable
        let ab = abduce(&th, &flies, 10).unwrap();
        assert!(ab.is_already_provable());
    }

    // ==========================================================================
    // query_with_options API TESTS (spec §3.2, Milestone 5)
    // ==========================================================================

    #[test]
    fn test_query_with_options_default_matches_query() {
        let th = make_defeasible_theory();
        let bird = Literal::simple("bird");

        let result_default = query(&th, &bird).unwrap();
        let result_with_opts = query_with_options(&th, &bird, PrepareOptions::default()).unwrap();

        assert_eq!(result_default.status, result_with_opts.status);
        assert_eq!(result_default.is_provable(), result_with_opts.is_provable());
    }

    #[test]
    fn test_query_with_options_accepts_reference_time() {
        use crate::mode::Mode;
        use crate::rule::Rule;
        use crate::temporal::{Temporal, TimePoint};

        let mut theory = Theory::new();

        // Add a fact with a specific temporal window
        let bird_lit = Literal::new(
            "bird",
            false,
            Mode::default(),
            Temporal::from_bounds(1000, 2000), // active from 1000 to 2000
            vec![],
        );
        theory.add_rule(Rule::fact("f1", bird_lit.clone()));

        let bird_query = Literal::simple("bird");

        // Query at time 1500 (inside the window)
        let opts_inside = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(1500)),
            ..Default::default()
        };
        let result_inside = query_with_options(&theory, &bird_query, opts_inside).unwrap();
        assert!(
            result_inside.is_provable(),
            "bird should be provable at time 1500"
        );

        // Query at time 3000 (outside the window)
        let opts_outside = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(3000)),
            ..Default::default()
        };
        let result_outside = query_with_options(&theory, &bird_query, opts_outside).unwrap();
        assert!(
            !result_outside.is_provable(),
            "bird should NOT be provable at time 3000 (outside temporal window)"
        );
    }

    #[test]
    fn test_query_with_options_temporal_disjoint_no_conflict() {
        use crate::mode::Mode;
        use crate::rule::Rule;
        use crate::temporal::{Temporal, TimePoint};

        let mut theory = Theory::new();

        // Add bird fact active from 1000 to 2000
        let bird_lit = Literal::new(
            "bird",
            false,
            Mode::default(),
            Temporal::from_bounds(1000, 2000),
            vec![],
        );
        theory.add_rule(Rule::fact("f1", bird_lit));

        // Add ~bird fact active from 3000 to 4000 (disjoint!)
        let not_bird_lit = Literal::new(
            "bird",
            true, // negated
            Mode::default(),
            Temporal::from_bounds(3000, 4000),
            vec![],
        );
        theory.add_rule(Rule::fact("f2", not_bird_lit));

        // At time 1500, bird should be provable (no conflict because ~bird is disjoint)
        let opts = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(1500)),
            ..Default::default()
        };
        let result = query_with_options(&theory, &Literal::simple("bird"), opts).unwrap();
        assert!(
            result.is_provable(),
            "bird should be provable at time 1500 (disjoint temporals don't conflict)"
        );

        // At time 3500, ~bird should be provable
        let opts = PrepareOptions {
            reference_time: Some(TimePoint::from_millis(3500)),
            ..Default::default()
        };
        let result = query_with_options(&theory, &Literal::negated("bird"), opts).unwrap();
        assert!(
            result.is_provable(),
            "~bird should be provable at time 3500"
        );
    }

    // ==========================================================================
    // REGRESSION TESTS - Bug Hunt Fixes
    // ==========================================================================

    #[test]
    fn test_why_not_detects_defeater_blocking() {
        // Regression: why_not should detect when a defeater blocks a conclusion
        let th = make_defeater_theory();
        // bird => flies, broken_wing ~> ~flies (defeater)
        let result = why_not(&th, &Literal::simple("flies")).unwrap();

        // Should detect the defeater blocking
        let has_defeated = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_type == BlockingType::Defeated);
        let has_contradicted = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_type == BlockingType::Contradicted);

        assert!(
            has_defeated || has_contradicted,
            "why_not should detect defeater or contradiction blocking, got: {:?}",
            result.blocked_by
        );
    }

    #[test]
    fn test_what_if_no_triple_reasoning() {
        // Regression: what_if should not call reason() 3 times.
        // We verify correctness (the performance fix is structural).
        let th = make_missing_premise_theory();
        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];

        let result = what_if(&th, hypotheticals, &Literal::simple("ready_review")).unwrap();
        assert!(result.is_provable());
        assert!(!result.new_conclusions.is_empty());
    }

    #[test]
    fn test_what_if_unique_labels_no_collision() {
        // Regression: hypothetical labels should not collide with user labels
        let mut theory = Theory::new();
        // User has a rule labeled "hyp1" (which old code would collide with)
        theory.add_rule(Rule::defeasible(
            "hyp1",
            vec![Literal::simple("a")],
            Literal::simple("b"),
        ));
        theory.add_fact("x");
        theory.add_defeasible_rule(&["x"], "goal");

        let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("a"))];
        let result = what_if(&theory, hypotheticals, &Literal::simple("goal")).unwrap();

        // Should still work correctly despite user having "hyp1" label
        assert!(result.is_provable());
    }

    #[test]
    fn test_next_hyp_label_skips_existing_rule_labels() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("__hyp_42_1", Literal::simple("a")));
        theory.add_rule(Rule::fact("__hyp_42_2", Literal::simple("b")));

        let label = next_hyp_label(&theory, 42, 1);
        assert_eq!(label, "__hyp_42_3");
    }

    #[test]
    fn test_what_if_changed_conclusions_no_false_positive_when_status_unchanged() {
        // Regression: baseline and modified can both contain +D and +d for the same literal.
        // This should not be reported as a status change.
        let mut theory = Theory::new();
        theory.add_fact("p");

        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::simple("irrelevant"))],
            &Literal::simple("p"),
        )
        .unwrap();

        assert!(
            result.changed_conclusions.is_empty(),
            "No status change expected for p, got {:?}",
            result.changed_conclusions
        );
    }

    #[test]
    fn test_what_if_changed_conclusions_includes_becomes_unprovable() {
        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_defeasible_rule(&["p"], "q");

        // Baseline: +d q. Hypothetical ~q fact should block q.
        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::negated("q"))],
            &Literal::simple("q"),
        )
        .unwrap();

        let has_transition = result.changed_conclusions.iter().any(|(lit, old, new)| {
            lit == &Literal::simple("q")
                && *old == ConclusionType::DefeasiblyProvable
                && !new.is_positive()
        });
        assert!(
            has_transition,
            "Expected q to appear in changed_conclusions when it becomes unprovable, got {:?}",
            result.changed_conclusions
        );
    }

    #[test]
    fn test_what_if_changed_conclusions_detects_real_upgrade() {
        // When a hypothetical adds a strict rule that upgrades a literal
        // from +d to +D, changed_conclusions should report it.
        use crate::rule::Rule;

        let mut theory = Theory::new();
        // p is only defeasibly provable in baseline
        theory.add_rule(Rule::new(
            "d1",
            RuleType::Defeasible,
            vec![],
            vec![Literal::simple("p")],
        ));
        // q depends on a premise we'll supply
        theory.add_defeasible_rule(&["trigger"], "q");

        // Use a strict rule body: trigger => p (strict)
        theory.add_rule(Rule::strict(
            "s1",
            vec![Literal::simple("trigger")],
            Literal::simple("p"),
        ));

        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::simple("trigger"))],
            &Literal::simple("q"),
        )
        .unwrap();

        // p should go from +d (baseline) to +D (modified) — a real change
        let p_change = result
            .changed_conclusions
            .iter()
            .find(|(lit, _, _)| lit.name() == "p" && !lit.negation);
        assert!(
            p_change.is_some(),
            "Should detect p upgrading from +d to +D, got: {:?}",
            result.changed_conclusions
        );
        let (_, old, new) = p_change.unwrap();
        assert_eq!(*old, ConclusionType::DefeasiblyProvable);
        assert_eq!(*new, ConclusionType::DefinitelyProvable);
    }

    #[test]
    fn test_what_if_new_conclusion_also_in_changed() {
        // A literal that becomes provable under hypotheticals should appear
        // in new_conclusions AND in changed_conclusions (negative → positive).
        let mut theory = Theory::new();
        theory.add_defeasible_rule(&["trigger"], "q");

        let result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::simple("trigger"))],
            &Literal::simple("q"),
        )
        .unwrap();

        // q is new (not provable in baseline)
        assert!(
            result.new_conclusions.iter().any(|l| l.name() == "q"),
            "q should appear in new_conclusions"
        );
        // q should also appear in changed_conclusions (it went from -d to +d)
        let q_in_changed = result.changed_conclusions.iter().any(|(lit, old, new)| {
            lit.name() == "q" && !lit.negation && !old.is_positive() && new.is_positive()
        });
        assert!(
            q_in_changed,
            "q should appear in changed_conclusions as a negative→positive transition, got {:?}",
            result.changed_conclusions
        );
    }

    #[test]
    fn test_why_not_provable_literal_no_false_blockers() {
        // Regression: why_not on a provable literal should return no blockers
        // and should identify the deriving rule
        let th = make_defeasible_theory();
        let result = why_not(&th, &Literal::simple("flies")).unwrap();

        assert!(!result.has_blockers());
        assert!(
            result.would_derive.is_some(),
            "Should identify deriving rule"
        );
    }

    #[test]
    fn test_why_not_respects_superiority() {
        // r1 > r2: why_not for r1's head should NOT list r2 as blocker
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("penguin");
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r1, &r2);

        // flies should be provable since r1 > r2
        let q = query(&theory, &Literal::simple("flies")).unwrap();
        assert!(q.is_provable(), "flies should be provable with r1 > r2");

        // why_not should have no blockers (since it IS provable)
        let result = why_not(&theory, &Literal::simple("flies")).unwrap();
        assert!(
            !result.has_blockers(),
            "why_not should not report blockers for provable literal with superior rule"
        );
    }

    #[test]
    fn test_why_not_superiority_inferior_not_blocker() {
        // When r1 > r2 and flies is NOT provable for other reasons,
        // r2 should still not be listed as a blocker for r1
        let mut theory = Theory::new();
        theory.add_fact("penguin");
        // bird is NOT a fact, so r1's body is unsatisfied
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
        theory.add_superiority(&r1, &r2);

        let result = why_not(&theory, &Literal::simple("flies")).unwrap();
        assert!(result.has_blockers());

        // The blocker should be missing premise (bird), not r2 as contradicted
        let has_r2_blocker = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_rule.as_deref() == Some(&r2));
        assert!(
            !has_r2_blocker,
            "Inferior rule r2 should not be listed as a blocker when r1 > r2"
        );
    }

    #[test]
    fn test_why_not_defeater_not_blocker_when_superior() {
        // When rule is superior to a defeater, the defeater should not block
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_fact("broken_wing");
        let r1 = theory.add_defeasible_rule(&["bird"], "flies");
        let d1 = theory.add_defeater(&["broken_wing"], "~flies");
        theory.add_superiority(&r1, &d1);

        let result = why_not(&theory, &Literal::simple("flies")).unwrap();
        // flies should be provable since r1 > d1
        assert!(
            !result.has_blockers(),
            "Defeater should not block when rule is superior"
        );
    }

    // =========================================================================
    // Trust Filter Tests
    // =========================================================================

    #[test]
    fn test_trust_filter_default_passes_all() {
        let filter = TrustFilter::new();
        let theory = Theory::new();
        assert!(filter.passes(&theory, None));
        assert!(filter.passes(&theory, Some("r1")));
    }

    #[test]
    fn test_trust_filter_min_degree() {
        let policy = TrustPolicy::new(0.5)
            .with_trust("agent:trusted", 0.9)
            .with_trust("agent:untrusted", 0.3);

        let filter = TrustFilter::new().with_min_degree(0.7).with_policy(policy);

        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        let labels: Vec<String> = theory.rules().map(|r| r.label.clone()).collect();
        theory.add_meta_string(&labels[0], "source", "agent:trusted");
        theory.add_meta_string(&labels[1], "source", "agent:untrusted");

        assert!(filter.passes(&theory, Some(&labels[0]))); // 0.9 >= 0.7
        assert!(!filter.passes(&theory, Some(&labels[1]))); // 0.3 < 0.7
    }

    #[test]
    fn test_trust_filter_source_pattern() {
        let policy = TrustPolicy::new(0.5);
        let filter = TrustFilter::new().with_source("agent:").with_policy(policy);

        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        let labels: Vec<String> = theory.rules().map(|r| r.label.clone()).collect();
        theory.add_meta_string(&labels[0], "source", "agent:coder");
        theory.add_meta_string(&labels[1], "source", "system:policy");

        assert!(filter.passes(&theory, Some(&labels[0]))); // matches "agent:"
        assert!(!filter.passes(&theory, Some(&labels[1]))); // doesn't match "agent:"
    }

    #[test]
    fn test_trust_filter_no_policy_passes_all() {
        let filter = TrustFilter::new()
            .with_min_degree(0.9)
            .with_source("agent:");
        // No policy set, so filter should pass everything
        let theory = Theory::new();
        assert!(filter.passes(&theory, None));
    }

    #[test]
    fn test_trust_filter_combined() {
        let policy = TrustPolicy::new(0.5)
            .with_trust("agent:trusted", 0.9)
            .with_trust("agent:low", 0.3);

        let filter = TrustFilter::new()
            .with_min_degree(0.5)
            .with_source("agent:")
            .with_policy(policy);

        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_fact("c");
        let labels: Vec<String> = theory.rules().map(|r| r.label.clone()).collect();
        theory.add_meta_string(&labels[0], "source", "agent:trusted");
        theory.add_meta_string(&labels[1], "source", "agent:low");
        theory.add_meta_string(&labels[2], "source", "system:policy");

        assert!(filter.passes(&theory, Some(&labels[0]))); // agent: match + 0.9 >= 0.5
        assert!(!filter.passes(&theory, Some(&labels[1]))); // agent: match + 0.3 < 0.5
        assert!(!filter.passes(&theory, Some(&labels[2]))); // system: no match
    }
}
