//! Requires operator — verified fact-set search for goal provability.
//!
//! [`requires_with_options`] verifies each raw abduction candidate by injecting
//! candidate facts and re-running full reasoning. Only candidates that make the
//! goal positively provable are returned.

use std::collections::HashSet;

use crate::conclusion::Conclusion;
use crate::error::{Result, SpindleError};
use crate::literal::Literal;
use crate::reason::reason;
use crate::rule::Rule;
use crate::theory::Theory;

use super::abduce::{AbductionSolution, abduce};

/// Default upper bound for raw candidates verified by `requires`.
pub const DEFAULT_MAX_RAW_CANDIDATES: usize = 1000;

/// Options for verified `requires` search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequiresOptions {
    /// Maximum number of verified solutions to return.
    pub max_solutions: usize,
    /// Maximum number of raw candidates to examine.
    pub max_raw_candidates: usize,
}

impl Default for RequiresOptions {
    fn default() -> Self {
        Self {
            max_solutions: 1,
            max_raw_candidates: DEFAULT_MAX_RAW_CANDIDATES,
        }
    }
}

/// Termination status for verified `requires` search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiresSearchStatus {
    /// Search terminated within configured bounds:
    /// all available candidates were exhausted or `max_solutions` was reached.
    BoundedComplete,
    /// Search terminated because the raw-candidate budget was reached while
    /// more raw candidates exist.
    BudgetExhausted,
}

/// Verification effort counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RequiresVerificationStats {
    /// Count of raw candidates examined.
    pub raw_examined: usize,
    /// Count of examined candidates accepted by verification.
    pub accepted: usize,
    /// Count of examined candidates rejected by verification.
    pub rejected: usize,
}

/// Verified `requires` query result.
#[derive(Debug, Clone)]
pub struct RequiresResult {
    /// Whether the goal is already provable in the base theory.
    pub already_provable: bool,
    /// Verified candidate fact-sets (deduplicated and deterministic order).
    pub solutions: Vec<AbductionSolution>,
    /// Search termination status.
    pub search_status: RequiresSearchStatus,
    /// Verification effort counters.
    pub verification: RequiresVerificationStats,
}

fn canonical_fact_key(solution: &AbductionSolution) -> String {
    let mut facts: Vec<_> = solution.facts.iter().map(Literal::to_spl).collect();
    facts.sort();
    facts.join("\x1f")
}

fn is_goal_provable(conclusions: &[Conclusion], goal: &Literal) -> bool {
    conclusions
        .iter()
        .any(|c| c.literal == *goal && c.conclusion_type.is_positive())
}

fn verify_candidate(
    theory: &Theory,
    goal: &Literal,
    candidate: &AbductionSolution,
    candidate_index: usize,
) -> Result<bool> {
    let mut modified = theory.clone();
    for (fact_index, fact_lit) in candidate.facts.iter().enumerate() {
        let mut label_nonce = 0_usize;
        loop {
            let label = format!("__requires_verify_{candidate_index}_{fact_index}_{label_nonce}");
            if modified.get_rule(&label).is_none() {
                modified.add_rule(Rule::fact(label, fact_lit.clone()));
                break;
            }
            label_nonce = label_nonce.saturating_add(1);
        }
    }

    let conclusions = reason(&modified)?;
    Ok(is_goal_provable(&conclusions, goal))
}

/// Verified multi-solution requires API.
pub fn requires_with_options(
    theory: &Theory,
    goal: &Literal,
    options: RequiresOptions,
) -> Result<RequiresResult> {
    if options.max_solutions == 0 {
        return Err(SpindleError::Validation {
            message: "requires options: max_solutions must be >= 1".to_string(),
        });
    }
    if options.max_raw_candidates == 0 {
        return Err(SpindleError::Validation {
            message: "requires options: max_raw_candidates must be >= 1".to_string(),
        });
    }

    // Sentinel request lets us detect whether raw search exceeded budget.
    let raw_request_limit = options.max_raw_candidates.saturating_add(1);
    let raw_result = abduce(theory, goal, raw_request_limit)?;

    if raw_result.is_already_provable() {
        return Ok(RequiresResult {
            already_provable: true,
            solutions: vec![],
            search_status: RequiresSearchStatus::BoundedComplete,
            verification: RequiresVerificationStats::default(),
        });
    }

    let raw_budget_exceeded = raw_result.solutions.len() > options.max_raw_candidates;
    let mut raw_candidates: Vec<_> = raw_result
        .solutions
        .into_iter()
        .take(options.max_raw_candidates)
        .collect();

    raw_candidates.sort_by(|a, b| {
        let size_cmp = a.facts.len().cmp(&b.facts.len());
        if size_cmp != std::cmp::Ordering::Equal {
            return size_cmp;
        }
        canonical_fact_key(a).cmp(&canonical_fact_key(b))
    });

    let mut verification = RequiresVerificationStats::default();
    let mut seen_fact_keys = HashSet::new();
    let mut accepted = Vec::new();

    for (candidate_index, candidate) in raw_candidates.iter().enumerate() {
        if accepted.len() >= options.max_solutions {
            break;
        }

        let fact_key = canonical_fact_key(candidate);
        if !seen_fact_keys.insert(fact_key) {
            continue;
        }

        verification.raw_examined += 1;
        if verify_candidate(theory, goal, candidate, candidate_index)? {
            verification.accepted += 1;
            accepted.push(candidate.clone());
        } else {
            verification.rejected += 1;
        }
    }

    accepted.sort_by(|a, b| {
        let size_cmp = a.facts.len().cmp(&b.facts.len());
        if size_cmp != std::cmp::Ordering::Equal {
            return size_cmp;
        }
        canonical_fact_key(a).cmp(&canonical_fact_key(b))
    });

    let search_status = if accepted.len() >= options.max_solutions {
        RequiresSearchStatus::BoundedComplete
    } else if raw_budget_exceeded {
        RequiresSearchStatus::BudgetExhausted
    } else {
        RequiresSearchStatus::BoundedComplete
    };

    Ok(RequiresResult {
        already_provable: false,
        solutions: accepted,
        search_status,
        verification,
    })
}

/// Convenience compatibility wrapper: return a single verified minimal fact-set.
pub fn requires(theory: &Theory, goal: &Literal) -> Result<Vec<Literal>> {
    let result = requires_with_options(
        theory,
        goal,
        RequiresOptions {
            max_solutions: 1,
            max_raw_candidates: DEFAULT_MAX_RAW_CANDIDATES,
        },
    )?;
    Ok(result
        .solutions
        .first()
        .map(|s| s.facts.iter().cloned().collect())
        .unwrap_or_default())
}
