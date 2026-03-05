//! Stratification pipeline stage for fold-based aggregation.
//!
//! Assigns rules to strata based on fold dependencies: if a rule folds over
//! a relation that is derived by another rule, the folding rule must be in a
//! later stratum. This ensures the folded relation is complete before the fold
//! evaluates.
//!
//! Programs without folds produce a single stratum and follow the existing
//! pipeline unchanged.

use rustc_hash::{FxHashMap, FxHashSet};

use super::{PipelineContext, PipelineStage};
use crate::body::BodyLiteral;
use crate::error::{Result, SpindleError};
use crate::intern::SymbolId;
use crate::rule::RuleLabel;
use crate::theory::Theory;

/// Stratum assignment for each rule in the theory.
#[derive(Debug, Clone, Default)]
pub struct StratumInfo {
    /// Maps each rule label to its stratum number (0-based).
    pub rule_strata: FxHashMap<RuleLabel, usize>,
    /// Total number of strata.
    pub num_strata: usize,
}

/// Stratification pipeline stage.
///
/// Inspects the unground theory for fold patterns, builds a dependency graph
/// between derived relations and folded-over relations, and assigns strata via
/// topological sort.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stratify;

impl PipelineStage for Stratify {
    fn name(&self) -> &'static str {
        "stratify"
    }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        let info = compute_strata(&theory)?;
        ctx.metadata.insert(
            "strata_count".to_string(),
            super::MetadataVal::Usize(info.num_strata),
        );
        // Store StratumInfo in context for later use by the reasoning pipeline.
        // We serialize it into a string metadata value (the actual StratumInfo is
        // retrieved via the stratum_info field on PipelineContext).
        ctx.stratum_info = Some(info);
        Ok(theory)
    }
}

/// Compute stratum assignments for all rules in the theory.
///
/// # Algorithm
///
/// 1. For each rule, determine which relation(s) it derives (head predicate name_id).
/// 2. For each rule with a fold, determine which relation the fold aggregates over
///    (fold pattern name_id).
/// 3. Build edges: if rule R folds over relation A, R's head relation depends on A.
///    The head relation must be in a stratum strictly after A's defining rules.
/// 4. Topological sort with cycle detection.
/// 5. Assign strata: stratum = max(stratum of dependencies) + 1.
pub fn compute_strata(theory: &Theory) -> Result<StratumInfo> {
    // Collect: which relations are derived by non-fact rules?
    // Facts don't count as "derived" for stratification purposes — they're base data.
    let mut derived_relations: FxHashSet<SymbolId> = FxHashSet::default();
    // Collect: which relations does each rule's head produce?
    let mut rule_head_relations: FxHashMap<RuleLabel, FxHashSet<SymbolId>> = FxHashMap::default();
    // Collect fold dependencies: rule label -> set of relations it folds over
    let mut fold_deps: FxHashMap<RuleLabel, FxHashSet<SymbolId>> = FxHashMap::default();

    for rule in theory.rules() {
        // Record head relations
        let mut head_rels = FxHashSet::default();
        for head in &rule.head {
            let name_id = head.name_id();
            head_rels.insert(name_id);
            if !rule.is_fact() {
                derived_relations.insert(name_id);
            }
        }
        rule_head_relations.insert(rule.label.clone(), head_rels);

        // Record fold dependencies
        for bl in &rule.body {
            if let BodyLiteral::Fold(fold) = bl {
                fold_deps
                    .entry(rule.label.clone())
                    .or_default()
                    .insert(fold.pattern.name_id());
            }
        }
    }

    // If no folds, everything is stratum 0
    if fold_deps.is_empty() {
        let mut rule_strata = FxHashMap::default();
        for rule in theory.rules() {
            rule_strata.insert(rule.label.clone(), 0);
        }
        return Ok(StratumInfo {
            rule_strata,
            num_strata: 1,
        });
    }

    // Build dependency graph between relations.
    // Edge: head_rel -> folded_rel means head_rel depends on folded_rel being complete.
    // In stratum terms: stratum(head_rel) > stratum(folded_rel) if folded_rel is derived.
    let mut relation_deps: FxHashMap<SymbolId, FxHashSet<SymbolId>> = FxHashMap::default();

    for (rule_label, folded_rels) in &fold_deps {
        if let Some(head_rels) = rule_head_relations.get(rule_label) {
            for &head_rel in head_rels {
                for &folded_rel in folded_rels {
                    // Only add dependency if the folded relation is actually derived (not just given as facts)
                    if derived_relations.contains(&folded_rel) {
                        relation_deps
                            .entry(head_rel)
                            .or_default()
                            .insert(folded_rel);
                    }
                }
            }
        }
    }

    // Assign strata to relations via topological sort.
    // relation_stratum[rel] = max(relation_stratum[dep] for dep in deps) + 1
    // Base facts (not derived or no fold deps) get stratum 0.
    let mut relation_stratum: FxHashMap<SymbolId, usize> = FxHashMap::default();
    let mut visiting: FxHashSet<SymbolId> = FxHashSet::default();
    let mut visited: FxHashSet<SymbolId> = FxHashSet::default();

    // Collect all relations that participate
    let all_relations: FxHashSet<SymbolId> = rule_head_relations
        .values()
        .flat_map(|s| s.iter().copied())
        .collect();

    for &rel in &all_relations {
        assign_stratum(
            rel,
            &relation_deps,
            &mut relation_stratum,
            &mut visiting,
            &mut visited,
        )?;
    }

    // Assign rules to strata based on their head relation's stratum.
    // A rule's stratum = max stratum of its head relations.
    let mut rule_strata = FxHashMap::default();
    let mut max_stratum = 0usize;

    for rule in theory.rules() {
        let stratum = rule
            .head
            .iter()
            .map(|h| relation_stratum.get(&h.name_id()).copied().unwrap_or(0))
            .max()
            .unwrap_or(0);
        rule_strata.insert(rule.label.clone(), stratum);
        max_stratum = max_stratum.max(stratum);
    }

    Ok(StratumInfo {
        rule_strata,
        num_strata: max_stratum + 1,
    })
}

/// Recursively assign a stratum to a relation via DFS with cycle detection.
fn assign_stratum(
    rel: SymbolId,
    deps: &FxHashMap<SymbolId, FxHashSet<SymbolId>>,
    strata: &mut FxHashMap<SymbolId, usize>,
    visiting: &mut FxHashSet<SymbolId>,
    visited: &mut FxHashSet<SymbolId>,
) -> Result<usize> {
    if let Some(&s) = strata.get(&rel) {
        return Ok(s);
    }
    if visiting.contains(&rel) {
        let rel_name = crate::intern::resolve(rel);
        return Err(SpindleError::Validation {
            message: format!(
                "Aggregation cycle detected involving relation '{rel_name}'. \
                 Cycles through aggregation are not supported.\n\
                 Hint: restructure so that aggregation dependencies flow in one direction."
            ),
        });
    }

    visiting.insert(rel);

    let stratum = if let Some(dep_rels) = deps.get(&rel) {
        let mut max_dep = 0usize;
        for &dep in dep_rels {
            let dep_stratum = assign_stratum(dep, deps, strata, visiting, visited)?;
            max_dep = max_dep.max(dep_stratum);
        }
        max_dep + 1
    } else {
        0
    };

    visiting.remove(&rel);
    visited.insert(rel);
    strata.insert(rel, stratum);
    Ok(stratum)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arith::ArithExpr;
    use crate::body::{BodyLogicLiteral, FoldLiteral};
    use crate::intern::intern;
    use crate::literal::Literal;
    use crate::rule::{Rule, RuleBody};
    use crate::term::Term;
    use smallvec::smallvec;

    fn make_fold_rule(label: &str, fold_over: &str, head_name: &str) -> Rule {
        let fold = FoldLiteral {
            result_var: intern("?total"),
            identity: Some(ArithExpr::Lit(Term::Integer(0))),
            reducer: intern("+"),
            extract: ArithExpr::Var(intern("?val")),
            pattern: BodyLogicLiteral::new(
                fold_over,
                false,
                crate::mode::Mode::empty(),
                crate::temporal::Temporal::empty(),
                vec![crate::body::BodyArg::Term(crate::term::Term::Symbol(
                    intern("?x"),
                ))],
            ),
            grouping_vars: Vec::new(),
        };
        let body: RuleBody = smallvec![BodyLiteral::Fold(fold)];
        let head = Literal::new(
            head_name,
            false,
            crate::mode::Mode::empty(),
            crate::temporal::Temporal::empty(),
            vec!["?x".into(), "?total".into()],
        );
        Rule::defeasible(label, body, head)
    }

    #[test]
    fn no_folds_single_stratum() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");

        let info = compute_strata(&theory).unwrap();
        assert_eq!(info.num_strata, 1);
        for (_, &s) in &info.rule_strata {
            assert_eq!(s, 0);
        }
    }

    #[test]
    fn fold_over_base_facts_single_stratum() {
        // Fold over a relation only given as facts — no derived dependency
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_rule(make_fold_rule("r1", "a", "total-a"));

        let info = compute_strata(&theory).unwrap();
        // "a" is only given as a fact, not derived, so fold dependency doesn't create a new stratum
        assert_eq!(info.num_strata, 1);
    }

    #[test]
    fn fold_over_derived_relation_two_strata() {
        let mut theory = Theory::new();
        theory.add_fact("base");
        theory.add_defeasible_rule(&["base"], "derived");
        theory.add_rule(make_fold_rule("r-fold", "derived", "result"));

        let info = compute_strata(&theory).unwrap();
        assert_eq!(info.num_strata, 2);
        // derived is at stratum 0, result is at stratum 1
        assert_eq!(info.rule_strata["r-fold"], 1);
    }

    #[test]
    fn chain_three_strata() {
        // a -> fold -> b -> fold -> c
        let mut theory = Theory::new();
        theory.add_fact("base");
        theory.add_defeasible_rule(&["base"], "a");
        theory.add_rule(make_fold_rule("r-fold-a", "a", "b"));
        theory.add_rule(make_fold_rule("r-fold-b", "b", "c"));

        let info = compute_strata(&theory).unwrap();
        assert_eq!(info.num_strata, 3);
        assert_eq!(info.rule_strata["r-fold-a"], 1);
        assert_eq!(info.rule_strata["r-fold-b"], 2);
    }

    #[test]
    fn multiple_rules_same_stratum() {
        let mut theory = Theory::new();
        theory.add_fact("base");
        theory.add_defeasible_rule(&["base"], "derived");
        // Two different rules fold over the same derived relation
        theory.add_rule(make_fold_rule("r-fold-1", "derived", "result-1"));
        theory.add_rule(make_fold_rule("r-fold-2", "derived", "result-2"));

        let info = compute_strata(&theory).unwrap();
        assert_eq!(info.num_strata, 2);
        assert_eq!(info.rule_strata["r-fold-1"], 1);
        assert_eq!(info.rule_strata["r-fold-2"], 1);
    }

    #[test]
    fn self_referencing_fold_cycle_error() {
        // Rule folds over its own head relation
        let mut theory = Theory::new();
        theory.add_rule(make_fold_rule("r-self", "result", "result"));

        let result = compute_strata(&theory);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Aggregation cycle"), "got: {msg}");
    }

    #[test]
    fn mutual_fold_dependency_cycle_error() {
        // a folds over b, b folds over a
        let mut theory = Theory::new();
        theory.add_rule(make_fold_rule("r-a", "b", "a"));
        theory.add_rule(make_fold_rule("r-b", "a", "b"));

        let result = compute_strata(&theory);
        assert!(result.is_err());
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Aggregation cycle"), "got: {msg}");
    }
}
