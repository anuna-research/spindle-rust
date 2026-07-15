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
use crate::projection::canonical_literal_key;
use crate::reason::reason;
use crate::rule::Rule;
use crate::theory::Theory;

use super::{QueryResult, QueryStatus, semantic_literal_matches};

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

/// Group conclusions by their injective [`canonical_literal_key`], keeping the
/// strongest status for each.
///
/// Keying on the canonical key rather than the `Literal` itself is essential:
/// `Literal` equality/hashing ignore temporal bounds, so distinct windows of the
/// same family (e.g. `p[1,10]` and `p[20,30]`) would otherwise collapse into a
/// single entry and merge their statuses. The rendered `to_spl()` form is not
/// usable either: it is not injective over typed terms, so `p(Symbol("1"))` and
/// `p(Integer(1))` would merge. The retained `Literal` is carried alongside the
/// status so callers can report the exact conclusion.
fn strongest_conclusions_by_literal(
    conclusions: &[crate::conclusion::Conclusion],
) -> HashMap<String, (Literal, ConclusionType)> {
    let mut by_lit: HashMap<String, (Literal, ConclusionType)> = HashMap::new();
    for conc in conclusions {
        by_lit
            .entry(canonical_literal_key(&conc.literal))
            .and_modify(|(_, old)| {
                if conclusion_strength(conc.conclusion_type) > conclusion_strength(*old) {
                    *old = conc.conclusion_type;
                }
            })
            .or_insert_with(|| (conc.literal.clone(), conc.conclusion_type));
    }
    by_lit
}

pub(crate) fn next_hyp_label(theory: &Theory, unique_id: u64, start_index: usize) -> String {
    let mut index = start_index.max(1);
    let limit = index + 10_000;
    loop {
        let candidate = format!("__hyp_{unique_id}_{index}");
        if theory.get_rule(&candidate).is_none() {
            return candidate;
        }
        index += 1;
        assert!(
            index < limit,
            "next_hyp_label: exceeded 10,000 collision attempts"
        );
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
    // Key on the injective canonical key: `Literal` equality ignores temporal
    // bounds, so a set of `Literal`s would treat p[20,30] as already present
    // when only p[1,10] was, hiding genuinely new windows below. The rendered
    // `to_spl()` form is not injective over typed terms either — it would hide
    // a genuinely new p(Integer(1)) behind a baseline p(Symbol("1")).
    let baseline_provable: HashSet<String> = baseline
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| canonical_literal_key(&c.literal))
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

    // Determine goal status directly from conclusions (avoids calling query->reason again).
    // Use semantic_literal_matches for consistency with query()/why_not()/abduce().
    let goal_complement = goal.complement();
    let mut result = QueryResult::new(goal.clone(), QueryStatus::Unknown);
    for conc in &modified_conclusions {
        if semantic_literal_matches(goal, &conc.literal) && conc.conclusion_type.is_positive() {
            result = QueryResult::new(goal.clone(), QueryStatus::Provable)
                .with_conclusion_type(conc.conclusion_type);
            break;
        }
    }
    if result.status == QueryStatus::Unknown {
        for conc in &modified_conclusions {
            if semantic_literal_matches(&goal_complement, &conc.literal)
                && conc.conclusion_type.is_positive()
            {
                result = QueryResult::new(goal.clone(), QueryStatus::Refuted);
                break;
            }
        }
    }

    // Find new conclusions. Deduplicate on the same injective canonical key
    // used for the baseline: one literal can appear in `modified_conclusions`
    // under several tags or via several rules, and `newly_provable` is a set
    // of newly-provable literals — without this a consumer prints the same
    // "now provable: X" line once per derivation path.
    let mut emitted: HashSet<String> = HashSet::new();
    let new_conclusions: Vec<Literal> = modified_conclusions
        .iter()
        .filter(|c| {
            c.conclusion_type.is_positive()
                && !baseline_provable.contains(&canonical_literal_key(&c.literal))
        })
        .filter(|c| emitted.insert(canonical_literal_key(&c.literal)))
        .map(|c| c.literal.clone())
        .collect();

    // Track changed conclusions by comparing strongest status per literal.
    // This captures both positive->positive changes and positive->negative
    // transitions (e.g. a literal that becomes unprovable under hypotheticals).
    let mut changed_conclusions = Vec::new();
    let baseline_by_lit = strongest_conclusions_by_literal(&baseline);
    let modified_by_lit = strongest_conclusions_by_literal(&modified_conclusions);

    // Union the exact-temporal keys; sort for deterministic output order.
    let mut all_keys: Vec<String> = baseline_by_lit.keys().cloned().collect();
    all_keys.extend(modified_by_lit.keys().cloned());
    all_keys.sort();
    all_keys.dedup();
    for key in &all_keys {
        if let (Some((_, old_type)), Some((lit, new_type))) =
            (baseline_by_lit.get(key), modified_by_lit.get(key))
            && old_type != new_type
        {
            changed_conclusions.push((lit.clone(), *old_type, *new_type));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::{Temporal, TimePoint};

    fn temporal_lit(name: &str, start: i64, end: i64) -> Literal {
        Literal::new(
            name,
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::Moment(start), TimePoint::Moment(end)),
            vec![],
        )
    }

    #[test]
    fn what_if_reports_new_temporal_window_as_new_conclusion() {
        // Baseline derives p[1,10]. A hypothetical p[20,30] is a genuinely new
        // temporal window of the same family. Because `Literal` equality ignores
        // temporal bounds, comparing on `Literal` alone would treat p[20,30] as
        // already present and omit it; the canonical exact-temporal key keeps it.
        let mut th = Theory::new();
        th.add_rule(Rule::fact("f1", temporal_lit("p", 1, 10)));

        let hyp = temporal_lit("p", 20, 30);
        let result = what_if(
            &th,
            vec![HypotheticalClaim::new(hyp.clone())],
            &temporal_lit("p", 20, 30),
        )
        .unwrap();

        assert!(
            result
                .new_conclusions
                .iter()
                .any(|l| l.to_spl() == hyp.to_spl()),
            "New temporal window p[20,30] must be reported as a new conclusion"
        );
        // The pre-existing window must not be reported as new.
        assert!(
            !result
                .new_conclusions
                .iter()
                .any(|l| l.to_spl() == temporal_lit("p", 1, 10).to_spl()),
            "Baseline window p[1,10] must not appear among new conclusions"
        );
    }

    #[test]
    fn what_if_deduplicates_new_conclusions() {
        // `q` becomes provable under the hypothetical `a` both strictly (+D via
        // r1) and defeasibly (+d via r2), so it appears at two positive tags in
        // the modified conclusion set. `newly_provable` is a set of literals, so
        // it must list `q` once — without dedup a consumer prints
        // "now provable: q" once per tag/derivation path.
        let mut th = Theory::new();
        th.add_rule(Rule::strict(
            "r1",
            vec![Literal::simple("a")],
            Literal::simple("q"),
        ));
        th.add_rule(Rule::defeasible(
            "r2",
            vec![Literal::simple("a")],
            Literal::simple("q"),
        ));

        let result = what_if(
            &th,
            vec![HypotheticalClaim::new(Literal::simple("a"))],
            &Literal::simple("q"),
        )
        .unwrap();

        let q_count = result
            .new_conclusions
            .iter()
            .filter(|l| l.name() == "q" && !l.negation)
            .count();
        assert_eq!(q_count, 1, "q must be reported once, not once per tag/rule");
    }

    #[test]
    fn what_if_reports_new_typed_literal_as_new_conclusion() {
        use crate::term::Term;

        // Baseline proves p(Symbol("1")); the hypothetical adds p(Integer(1)).
        // Both render as (p 1) in SPL, so a to_spl()-keyed baseline would hide
        // the genuinely new typed literal from new_conclusions.
        let typed_p = |term: Term| {
            Literal::from_ids(
                crate::intern::intern("p"),
                false,
                crate::mode::Mode::empty(),
                Temporal::empty(),
                vec![term],
            )
        };
        let p_sym = typed_p(Term::Symbol(crate::intern::intern("1")));
        let p_int = typed_p(Term::Integer(1));

        let mut th = Theory::new();
        th.add_rule(Rule::fact("f1", p_sym));

        let result = what_if(&th, vec![HypotheticalClaim::new(p_int.clone())], &p_int).unwrap();

        assert!(result.is_provable());
        assert!(
            result
                .new_conclusions
                .iter()
                .any(|l| l.predicate_args() == p_int.predicate_args()),
            "p(Integer(1)) is new under the hypothetical and must be reported"
        );
    }
}
