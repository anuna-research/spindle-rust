//! What-If (Hypothetical Reasoning) Operator
//!
//! Provides the [`what_if`] and [`what_if_provable`] functions for hypothetical
//! reasoning over defeasible logic theories. Given a set of hypothetical claims
//! and a goal literal, these functions determine what would be provable if the
//! claims were assumed as facts.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::conclusion::ConclusionType;
use crate::error::Result;
use crate::literal::Literal;
use crate::reason::reason;
use crate::rule::Rule;
use crate::theory::Theory;

use super::{QueryResult, QueryStatus, TemporalLitKey, matches_literal_temporal};

// =============================================================================
// TYPES
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

// =============================================================================
// INTERNAL HELPERS
// =============================================================================

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
) -> HashMap<TemporalLitKey, (Literal, ConclusionType)> {
    let mut by_lit = HashMap::new();
    for conc in conclusions {
        let key = TemporalLitKey::from_literal(&conc.literal);
        by_lit
            .entry(key)
            .and_modify(|(lit, old)| {
                if conclusion_strength(conc.conclusion_type) > conclusion_strength(*old) {
                    *lit = conc.literal.clone();
                    *old = conc.conclusion_type;
                }
            })
            .or_insert_with(|| (conc.literal.clone(), conc.conclusion_type));
    }
    by_lit
}

pub(crate) fn next_hyp_label(theory: &Theory, unique_id: u64, start_index: usize) -> String {
    let mut index = start_index.max(1);
    loop {
        let candidate = format!("__hyp_{unique_id}_{index}");
        if theory.get_rule(&candidate).is_none() {
            return candidate;
        }
        index += 1;
    }
}

// =============================================================================
// PUBLIC API
// =============================================================================

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
        .map(|c| TemporalLitKey::from_literal(&c.literal))
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
        if conc.conclusion_type.is_positive() && matches_literal_temporal(goal, &conc.literal) {
            result = QueryResult::new(goal.clone(), QueryStatus::Provable)
                .with_conclusion_type(conc.conclusion_type);
            break;
        }
    }
    if result.status == QueryStatus::Unknown {
        for conc in &modified_conclusions {
            if conc.conclusion_type.is_positive()
                && matches_literal_temporal(&goal_complement, &conc.literal)
            {
                result = QueryResult::new(goal.clone(), QueryStatus::Refuted);
                break;
            }
        }
    }

    // Find new conclusions
    let mut seen_new = HashSet::new();
    let new_conclusions: Vec<Literal> = modified_conclusions
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .filter_map(|c| {
            let key = TemporalLitKey::from_literal(&c.literal);
            if baseline_provable.contains(&key) || !seen_new.insert(key) {
                None
            } else {
                Some(c.literal.clone())
            }
        })
        .collect();

    // Track changed conclusions by comparing strongest status per literal.
    // This captures both positive->positive changes and positive->negative
    // transitions (e.g. a literal that becomes unprovable under hypotheticals).
    let mut changed_conclusions = Vec::new();
    let baseline_by_lit = strongest_conclusions_by_literal(&baseline);
    let modified_by_lit = strongest_conclusions_by_literal(&modified_conclusions);

    let mut all_literals: HashSet<TemporalLitKey> = baseline_by_lit.keys().cloned().collect();
    all_literals.extend(modified_by_lit.keys().cloned());
    for lit in all_literals {
        if let (Some((_, old_type)), Some((new_lit, new_type))) =
            (baseline_by_lit.get(&lit), modified_by_lit.get(&lit))
            && old_type != new_type
        {
            changed_conclusions.push((new_lit.clone(), *old_type, *new_type));
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
