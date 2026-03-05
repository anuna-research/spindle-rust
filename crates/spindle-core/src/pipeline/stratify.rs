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
    // Collect ALL body dependencies per rule (normal and fold, only to derived relations)
    let mut normal_deps: FxHashMap<RuleLabel, FxHashSet<SymbolId>> = FxHashMap::default();
    let mut all_fold_deps: FxHashMap<RuleLabel, FxHashSet<SymbolId>> = FxHashMap::default();

    // First pass: collect head relations and derived relations.
    for rule in theory.rules() {
        let mut head_rels = FxHashSet::default();
        for head in &rule.head {
            let name_id = head.name_id();
            head_rels.insert(name_id);
            if !rule.is_fact() {
                derived_relations.insert(name_id);
            }
        }
        rule_head_relations.insert(rule.label.clone(), head_rels);
    }

    // Second pass: collect body dependencies (now derived_relations is complete).
    for rule in theory.rules() {
        let mut rule_normal_deps: FxHashSet<SymbolId> = FxHashSet::default();
        let mut rule_fold_deps: FxHashSet<SymbolId> = FxHashSet::default();
        for bl in &rule.body {
            match bl {
                BodyLiteral::Logic(lit) => {
                    let dep = lit.name_id();
                    if derived_relations.contains(&dep) {
                        rule_normal_deps.insert(dep);
                    }
                }
                BodyLiteral::Fold(fold) => {
                    let dep = fold.pattern.name_id();
                    if derived_relations.contains(&dep) {
                        rule_fold_deps.insert(dep);
                    }
                    fold_deps
                        .entry(rule.label.clone())
                        .or_default()
                        .insert(dep);
                }
                BodyLiteral::Arithmetic(_) => {}
            }
        }
        normal_deps.insert(rule.label.clone(), rule_normal_deps);
        all_fold_deps.insert(rule.label.clone(), rule_fold_deps);
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

    // Build dependency edges between relations.
    // normal_edges: stratum(head) >= stratum(dep)
    // fold_edges:   stratum(head) >= stratum(dep) + 1
    let mut normal_edges: FxHashMap<SymbolId, FxHashSet<SymbolId>> = FxHashMap::default();
    let mut fold_edges: FxHashMap<SymbolId, FxHashSet<SymbolId>> = FxHashMap::default();

    for rule in theory.rules() {
        if let Some(head_rels) = rule_head_relations.get(&rule.label) {
            // Normal body deps
            if let Some(deps) = normal_deps.get(&rule.label) {
                for &head_rel in head_rels.iter() {
                    for &dep in deps {
                        // Skip self-loops for normal edges (recursive rules stay in same stratum)
                        if dep != head_rel {
                            normal_edges.entry(head_rel).or_default().insert(dep);
                        }
                    }
                }
            }
            // Fold body deps
            if let Some(deps) = all_fold_deps.get(&rule.label) {
                for &head_rel in head_rels.iter() {
                    for &dep in deps {
                        fold_edges.entry(head_rel).or_default().insert(dep);
                    }
                }
            }
        }
    }

    // Fixed-point iteration to assign strata.
    let mut relation_stratum: FxHashMap<SymbolId, usize> = FxHashMap::default();
    let max_iterations = derived_relations.len() + 1;

    for iteration in 0..=max_iterations {
        if iteration == max_iterations {
            return Err(SpindleError::Validation {
                message: "Aggregation cycle detected. \
                         Cycles through aggregation are not supported.\n\
                         Hint: restructure so that aggregation dependencies flow in one direction."
                    .to_string(),
            });
        }
        let mut changed = false;
        for rule in theory.rules() {
            if let Some(head_rels) = rule_head_relations.get(&rule.label) {
                for &head_rel in head_rels.iter() {
                    let mut required = 0usize;
                    // Normal edges: stratum(head) >= stratum(dep)
                    if let Some(deps) = normal_edges.get(&head_rel) {
                        for &dep in deps {
                            let dep_s = relation_stratum.get(&dep).copied().unwrap_or(0);
                            required = required.max(dep_s);
                        }
                    }
                    // Fold edges: stratum(head) >= stratum(dep) + 1
                    if let Some(deps) = fold_edges.get(&head_rel) {
                        for &dep in deps {
                            let dep_s = relation_stratum.get(&dep).copied().unwrap_or(0);
                            required = required.max(dep_s + 1);
                        }
                    }
                    let current = relation_stratum.get(&head_rel).copied().unwrap_or(0);
                    if required > current {
                        relation_stratum.insert(head_rel, required);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
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

    #[test]
    fn transitive_fold_dependency_through_non_fold_rule() {
        // Rule B (fold): applicable-rate -> topup-pay (stratum 1)
        // Rule C (normal): topup-pay -> pay-line
        // Rule D (fold): pay-line -> total-pay
        // D should be in stratum 2 because pay-line depends on topup-pay which is stratum 1.
        let mut theory = Theory::new();
        theory.add_fact("base");
        theory.add_defeasible_rule(&["base"], "applicable-rate");
        theory.add_rule(make_fold_rule("r-topup", "applicable-rate", "topup-pay"));
        // Normal rule: topup-pay -> pay-line
        theory.add_defeasible_rule(&["topup-pay"], "pay-line");
        theory.add_rule(make_fold_rule("r-total", "pay-line", "total-pay"));

        let info = compute_strata(&theory).unwrap();
        // applicable-rate: stratum 0
        // topup-pay: stratum 1 (fold over applicable-rate)
        // pay-line: stratum 1 (normal dep on topup-pay)
        // total-pay: stratum 2 (fold over pay-line which is stratum 1)
        assert_eq!(info.rule_strata["r-topup"], 1);
        assert_eq!(info.rule_strata["r-total"], 2);
        assert!(info.num_strata >= 3);
    }

    #[test]
    fn non_fold_mutual_dependency_same_stratum() {
        // Mutual recursion through normal edges should not cause a cycle error.
        // a -> b -> a (both normal rules), plus a fold over some other relation.
        let mut theory = Theory::new();
        theory.add_fact("base");
        theory.add_defeasible_rule(&["base"], "x");
        theory.add_defeasible_rule(&["x"], "y");
        theory.add_defeasible_rule(&["y"], "x");
        // Add a fold so we don't hit the early no-fold return
        theory.add_rule(make_fold_rule("r-fold", "x", "result"));

        let info = compute_strata(&theory).unwrap();
        // x and y should be in the same stratum (0) due to mutual normal deps
        // result should be in stratum 1 (fold over x)
        assert_eq!(info.rule_strata["r-fold"], 1);
    }
}
