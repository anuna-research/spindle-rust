//! Grounding Module - First-Order Variable Support
//!
//! Implements Datalog-style bottom-up grounding to support rules with variables.
//! Variables are prefixed with `?` (e.g., `?x`, `?person`).
//!
//! # Algorithm
//!
//! 1. Extract ground facts from theory
//! 2. For each rule with variables, find matching facts
//! 3. Generate ground instances by substituting matched bindings
//! 4. Iterate until fixpoint (no new facts derived)
//!
//! # Example
//!
//! ```text
//! # Facts
//! f1: >> parent(alice, bob)
//! f2: >> parent(bob, carol)
//!
//! # Rule with variables
//! r1: parent(?x, ?y), parent(?y, ?z) => ancestor(?x, ?z)
//!
//! # After grounding, generates:
//! r1_1: parent(alice, bob), parent(bob, carol) => ancestor(alice, carol)
//! ```

use rustc_hash::{FxHashMap, FxHashSet};

use crate::intern::{SymbolId, resolve};

#[cfg(test)]
use crate::intern::intern;
use crate::literal::Literal;
use crate::mode::Mode;
use crate::rule::{Rule, RuleLabel, RuleType};
use crate::temporal::{
    AllenConstraint, Temporal, TemporalExpr, TemporalStateQuery, TimeExpr, TimePoint,
};
use crate::theory::Theory;

/// Variable substitution for grounding.
///
/// Contains both term bindings (variable -> interned value) and temporal
/// bindings (temporal variable -> concrete timepoint). Uses interned
/// `SymbolId` for O(1) hashing and zero-allocation lookups.
#[derive(Clone, Debug, Default)]
pub struct Substitution {
    /// Term variable bindings (e.g., ?x -> alice)
    pub terms: FxHashMap<SymbolId, SymbolId>,
    /// Temporal variable bindings (e.g., ?t1 -> TimePoint::Moment(100))
    pub temporal: FxHashMap<SymbolId, TimePoint>,
    /// Interval variable bindings (e.g., ?T -> Temporal[0, 10])
    pub intervals: FxHashMap<SymbolId, Temporal>,
}

/// Check if a term is a variable (starts with ?)
pub fn is_variable(term: &str) -> bool {
    term.starts_with('?')
}

/// Check if a literal contains any variables (term, temporal, or interval)
pub fn literal_has_variables(lit: &Literal) -> bool {
    is_variable(lit.name())
        || lit.predicates().iter().any(|p| is_variable(p))
        || lit.has_temporal_variables()
}

/// Check if a rule contains any variables.
///
/// Allen constraints and state queries are treated as variable-bearing because
/// they reference interval variables that must be validated during grounding.
pub fn has_variables(rule: &Rule) -> bool {
    rule.body.iter().any(literal_has_variables)
        || rule.head.iter().any(literal_has_variables)
        || !rule.constraints.is_empty()
        || !rule.state_queries.is_empty()
}

/// Try to match a pattern literal against a ground literal.
/// Returns a substitution if match succeeds, None otherwise.
///
/// When the pattern has a `temporal_expr`, temporal variables are bound
/// against the ground literal's concrete temporal endpoints.
pub fn match_literal(pattern: &Literal, ground: &Literal) -> Option<Substitution> {
    // Check negation matches
    if pattern.negation != ground.negation {
        return None;
    }

    // Check mode matches
    if pattern.mode != ground.mode {
        return None;
    }

    let mut subst = Substitution::default();

    // Match name (using interned symbols)
    let pattern_name_id = pattern.name_id();
    let ground_name_id = ground.name_id();
    let pattern_name = resolve(pattern_name_id);

    if is_variable(pattern_name) {
        subst.terms.insert(pattern_name_id, ground_name_id);
    } else if pattern_name_id != ground_name_id {
        return None;
    }

    // Match predicates/arguments (using Term values)
    let pattern_args = pattern.predicate_args();
    let ground_args = ground.predicate_args();
    if pattern_args.len() != ground_args.len() {
        return None;
    }

    for (parg, garg) in pattern_args.iter().zip(ground_args.iter()) {
        // Variables are always Term::Symbol(id) where resolve(id) starts with '?'
        if let crate::term::Term::Symbol(parg_id) = parg {
            let parg_str = resolve(*parg_id);
            if is_variable(parg_str) {
                // Ground side must be a symbol for variable binding
                if let crate::term::Term::Symbol(garg_id) = garg {
                    if let Some(existing) = subst.terms.get(parg_id) {
                        if *existing != *garg_id {
                            return None;
                        }
                    } else {
                        subst.terms.insert(*parg_id, *garg_id);
                    }
                } else {
                    // Variable can't bind to non-symbol term (yet)
                    return None;
                }
                continue;
            }
        }
        // Non-variable: compare terms directly
        if parg != garg {
            return None;
        }
    }

    // Match interval variable (whole-interval binding)
    if let Some(var_id) = pattern.interval_var {
        if ground.temporal.is_empty() {
            return None; // Can't bind interval from non-temporal fact
        }
        if let Some(existing) = subst.intervals.get(&var_id) {
            if *existing != ground.temporal {
                return None;
            }
        } else {
            subst.intervals.insert(var_id, ground.temporal.clone());
        }
    }

    // Match temporal variables from temporal_expr against ground temporal
    if let Some(ref texpr) = pattern.temporal_expr {
        // Pattern has temporal variables — ground fact must have concrete temporal
        if ground.temporal.is_empty() {
            return None;
        }

        match &texpr.start {
            TimeExpr::Var(var_id) => {
                if let Some(existing) = subst.temporal.get(var_id) {
                    if *existing != ground.temporal.start {
                        return None;
                    }
                } else {
                    subst.temporal.insert(*var_id, ground.temporal.start);
                }
            }
            TimeExpr::Const(tp) => {
                if *tp != ground.temporal.start {
                    return None;
                }
            }
        }

        match &texpr.end {
            TimeExpr::Var(var_id) => {
                if let Some(existing) = subst.temporal.get(var_id) {
                    if *existing != ground.temporal.end {
                        return None;
                    }
                } else {
                    subst.temporal.insert(*var_id, ground.temporal.end);
                }
            }
            TimeExpr::Const(tp) => {
                if *tp != ground.temporal.end {
                    return None;
                }
            }
        }
    }

    // Compare concrete temporal bounds — after temporal substitution, a pattern
    // may carry concrete temporal values (no interval_var / temporal_expr).
    // Reject the match when those concrete bounds differ from the ground fact.
    if pattern.interval_var.is_none()
        && pattern.temporal_expr.is_none()
        && !pattern.temporal.is_empty()
        && pattern.temporal != ground.temporal
    {
        return None;
    }

    Some(subst)
}

/// Apply a substitution to a literal (using interned SymbolIds)
///
/// Resolves both term variables and temporal variables. If a `temporal_expr`
/// is fully resolved, it is converted to a concrete `temporal` field.
pub fn apply_substitution_to_literal(lit: &Literal, subst: &Substitution) -> Literal {
    let name_id = lit.name_id();
    let name = resolve(name_id);

    // Apply substitution to name (if it's a variable)
    let new_name_id = if is_variable(name) {
        subst.terms.get(&name_id).copied().unwrap_or(name_id)
    } else {
        name_id
    };

    // Apply substitution to predicate arguments
    let new_pred_args: Vec<crate::term::Term> = lit
        .predicate_args()
        .iter()
        .map(|term| {
            if let crate::term::Term::Symbol(pid) = term {
                let p = resolve(*pid);
                if is_variable(p) {
                    let bound_id = subst.terms.get(pid).copied().unwrap_or(*pid);
                    return crate::term::Term::Symbol(bound_id);
                }
            }
            term.clone()
        })
        .collect();

    // Resolve interval_var (whole-interval binding)
    let (new_temporal, new_temporal_expr, new_interval_var) = if let Some(var_id) = lit.interval_var
    {
        if let Some(interval) = subst.intervals.get(&var_id) {
            // Fully resolved — set concrete temporal, clear interval_var
            (interval.clone(), None, None)
        } else {
            // Still unresolved
            (Temporal::empty(), None, Some(var_id))
        }
    } else if let Some(ref texpr) = lit.temporal_expr {
        // Resolve temporal_expr (endpoint variables)
        let resolved_start = resolve_time_expr(&texpr.start, &subst.temporal);
        let resolved_end = resolve_time_expr(&texpr.end, &subst.temporal);

        match (resolved_start, resolved_end) {
            (TimeExpr::Const(s), TimeExpr::Const(e)) => {
                // Fully resolved — convert to concrete temporal
                (Temporal::new(s, e), None, None)
            }
            (start, end) => {
                // Partially resolved — keep as temporal_expr
                (Temporal::empty(), Some(TemporalExpr::new(start, end)), None)
            }
        }
    } else {
        (lit.temporal.clone(), None, None)
    };

    let mut result = Literal::from_ids(
        new_name_id,
        lit.negation,
        lit.mode.clone(),
        new_temporal,
        new_pred_args,
    );
    result.temporal_expr = new_temporal_expr;
    result.interval_var = new_interval_var;
    result
}

/// Resolve a single `TimeExpr`, substituting variables where bindings exist.
fn resolve_time_expr(
    expr: &TimeExpr,
    temporal_bindings: &FxHashMap<SymbolId, TimePoint>,
) -> TimeExpr {
    match expr {
        TimeExpr::Const(_) => expr.clone(),
        TimeExpr::Var(var_id) => {
            if let Some(tp) = temporal_bindings.get(var_id) {
                TimeExpr::Const(*tp)
            } else {
                expr.clone()
            }
        }
    }
}

/// Apply a substitution to a rule, creating a ground instance
fn apply_substitution_to_rule(rule: &Rule, subst: &Substitution, instance_num: usize) -> Rule {
    let new_label = format!("{}_{}", rule.label, instance_num);
    let new_body: Vec<Literal> = rule
        .body
        .iter()
        .map(|lit| apply_substitution_to_literal(lit, subst))
        .collect();
    let new_head: Vec<Literal> = rule
        .head
        .iter()
        .map(|lit| apply_substitution_to_literal(lit, subst))
        .collect();

    let mut new_rule = Rule::new(new_label, rule.rule_type, new_body, new_head);
    // Preserve the original rule's label as the template label for superiority
    new_rule.template_label = Some(rule.label.clone());
    // Carry forward rule-level properties that must survive grounding
    new_rule.temporal = rule.temporal.clone();
    new_rule.mode = rule.mode.clone();
    new_rule
}

/// Merge two substitutions, returning None if they conflict
fn merge_substitutions(s1: &Substitution, s2: &Substitution) -> Option<Substitution> {
    let mut merged = s1.clone();

    // Merge term bindings
    for (k, v) in &s2.terms {
        if let Some(existing) = merged.terms.get(k) {
            if *existing != *v {
                return None;
            }
        } else {
            merged.terms.insert(*k, *v);
        }
    }

    // Merge temporal bindings
    for (k, v) in &s2.temporal {
        if let Some(existing) = merged.temporal.get(k) {
            if *existing != *v {
                return None;
            }
        } else {
            merged.temporal.insert(*k, *v);
        }
    }

    // Merge interval bindings
    for (k, v) in &s2.intervals {
        if let Some(existing) = merged.intervals.get(k) {
            if *existing != *v {
                return None;
            }
        } else {
            merged.intervals.insert(*k, v.clone());
        }
    }

    Some(merged)
}

/// Create a key for indexing facts (using interned SymbolId, zero allocation)
#[inline]
fn fact_index_key(lit: &Literal) -> (SymbolId, bool, usize, Mode) {
    (
        lit.name_id(),
        lit.negation,
        lit.predicate_args().len(),
        lit.mode.clone(),
    )
}

/// Create a key for deduplicating literals (using Term args, minimal allocation)
///
/// Includes temporal so that `p[1,2]` and `p[3,4]` are treated as distinct facts.
#[inline]
fn literal_key(lit: &Literal) -> (SymbolId, bool, Vec<crate::term::Term>, Mode, Temporal) {
    (
        lit.name_id(),
        lit.negation,
        lit.predicate_args().to_vec(),
        lit.mode.clone(),
        lit.temporal.clone(),
    )
}

/// Match body literals against facts, returning all valid substitutions
fn match_body_against_facts(
    body: &[Literal],
    fact_index: &FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>>,
    all_facts: &[Literal],
) -> Vec<Substitution> {
    if body.is_empty() {
        return vec![Substitution::default()];
    }

    let first_lit = &body[0];
    let rest = &body[1..];

    // Get candidate facts
    let candidates: Vec<&Literal> = if is_variable(first_lit.name()) {
        all_facts.iter().collect()
    } else {
        fact_index
            .get(&fact_index_key(first_lit))
            .map(|v| v.iter().collect())
            .unwrap_or_default()
    };

    let mut results = Vec::new();

    for fact in candidates {
        if let Some(subst) = match_literal(first_lit, fact) {
            // Apply substitution to rest of body
            let substituted_rest: Vec<Literal> = rest
                .iter()
                .map(|l| apply_substitution_to_literal(l, &subst))
                .collect();

            // Recursively match rest
            for rest_subst in match_body_against_facts(&substituted_rest, fact_index, all_facts) {
                if let Some(merged) = merge_substitutions(&subst, &rest_subst) {
                    results.push(merged);
                }
            }
        }
    }

    results
}

/// Match body with at least one delta (new) fact
fn match_body_with_delta(
    body: &[Literal],
    fact_index: &FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>>,
    delta_index: &FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>>,
    all_facts: &[Literal],
    delta_facts: &[Literal],
) -> Vec<Substitution> {
    if body.is_empty() {
        return vec![Substitution::default()];
    }

    let mut results = Vec::new();
    let mut seen: FxHashSet<SubstitutionKey> = FxHashSet::default();

    for (i, delta_lit) in body.iter().enumerate() {
        let rest: Vec<Literal> = body
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, l)| l.clone())
            .collect();

        let delta_candidates: Vec<&Literal> = if is_variable(delta_lit.name()) {
            delta_facts.iter().collect()
        } else {
            delta_index
                .get(&fact_index_key(delta_lit))
                .map(|v| v.iter().collect())
                .unwrap_or_default()
        };

        for fact in delta_candidates {
            if let Some(subst) = match_literal(delta_lit, fact) {
                let substituted_rest: Vec<Literal> = rest
                    .iter()
                    .map(|l| apply_substitution_to_literal(l, &subst))
                    .collect();

                for rest_subst in match_body_against_facts(&substituted_rest, fact_index, all_facts)
                {
                    if let Some(merged) = merge_substitutions(&subst, &rest_subst) {
                        let key = substitution_key(&merged);
                        if !seen.contains(&key) {
                            seen.insert(key);
                            results.push(merged);
                        }
                    }
                }
            }
        }
    }

    results
}

/// A hashable key representing a substitution (term, temporal, and interval bindings).
type SubstitutionKey = (
    Vec<(SymbolId, SymbolId)>,
    Vec<(SymbolId, TimePoint)>,
    Vec<(SymbolId, Temporal)>,
);

/// Build a hashable key from a substitution for deduplication.
fn substitution_key(subst: &Substitution) -> SubstitutionKey {
    let mut term_pairs: Vec<_> = subst.terms.iter().map(|(k, v)| (*k, *v)).collect();
    term_pairs.sort_by_key(|(k, _)| k.as_raw());

    let mut temporal_pairs: Vec<_> = subst.temporal.iter().map(|(k, v)| (*k, *v)).collect();
    temporal_pairs.sort_by_key(|(k, _)| k.as_raw());

    let mut interval_pairs: Vec<_> = subst
        .intervals
        .iter()
        .map(|(k, v)| (*k, v.clone()))
        .collect();
    interval_pairs.sort_by_key(|(k, _)| k.as_raw());

    (term_pairs, temporal_pairs, interval_pairs)
}

/// Evaluate all Allen constraints against bound interval variables.
///
/// Returns `true` if all constraints are satisfied. Returns `false` if any
/// constraint's interval variable is unbound or the relation doesn't hold.
fn evaluate_constraints(constraints: &[AllenConstraint], subst: &Substitution) -> bool {
    constraints.iter().all(|c| {
        match (
            subst.intervals.get(&c.interval1),
            subst.intervals.get(&c.interval2),
        ) {
            (Some(t1), Some(t2)) => c.holds(t1, t2),
            _ => false, // unbound interval → constraint fails
        }
    })
}

/// Evaluate all temporal state queries against bound interval variables.
///
/// Returns `true` if all queries are satisfied. Returns `false` if any
/// query's interval variable is unbound or the state predicate doesn't hold.
fn evaluate_state_queries(queries: &[TemporalStateQuery], subst: &Substitution) -> bool {
    queries.iter().all(|q| {
        let interval = match subst.intervals.get(&q.interval) {
            Some(t) => t,
            None => return false,
        };
        let time = match &q.time {
            TimeExpr::Const(tp) => *tp,
            TimeExpr::Var(id) => {
                // Try to resolve from temporal endpoint bindings
                match subst.temporal.get(id) {
                    Some(tp) => *tp,
                    None => return false,
                }
            }
        };
        q.holds(interval, time)
    })
}

/// Ground a theory by instantiating rules with variables
pub fn ground_theory(theory: &Theory) -> Theory {
    ground_theory_with_limit(theory, 100, usize::MAX).0
}

/// Ground a theory with a maximum iteration limit and instance limit
/// Returns (grounded_theory, limit_hit)
pub fn ground_theory_with_limit(
    theory: &Theory,
    max_iterations: usize,
    max_instances: usize,
) -> (Theory, bool) {
    // Separate ground rules from rules with variables
    let (ground_rules, var_rules): (Vec<_>, Vec<_>) =
        theory.rules().partition(|r| !has_variables(r));

    // If no rules with variables, return as-is
    if var_rules.is_empty() {
        return (theory.clone(), false);
    }

    // Track facts using Term-based keys (minimal allocation)
    let mut fact_keys: FxHashSet<(SymbolId, bool, Vec<crate::term::Term>, Mode, Temporal)> =
        FxHashSet::default();
    let mut facts_list: Vec<Literal> = Vec::new();
    let mut fact_index: FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>> =
        FxHashMap::default();

    // Initialize with ground facts
    for rule in theory.facts() {
        if !has_variables(rule) {
            let lit = rule.head_literal().clone();
            let key = literal_key(&lit);
            if !fact_keys.contains(&key) {
                fact_keys.insert(key);
                fact_index
                    .entry(fact_index_key(&lit))
                    .or_default()
                    .push(lit.clone());
                facts_list.push(lit);
            }
        }
    }

    let mut all_generated_rules: Vec<Rule> = ground_rules.into_iter().cloned().collect();
    let mut instance_counter = 0;
    // Use substitution keys for instance tracking (includes temporal bindings)
    let mut known_instances: FxHashSet<(RuleLabel, SubstitutionKey)> = FxHashSet::default();

    // Iterate until fixpoint
    let mut facts_new = facts_list.clone();
    let mut limit_hit = false;

    for iteration in 0..max_iterations {
        if iteration >= max_iterations {
            panic!("Max iterations ({max_iterations}) reached, possible infinite loop");
        }

        let mut new_facts_this_round: Vec<Literal> = Vec::new();
        let mut new_rules_this_round: Vec<Rule> = Vec::new();

        // Build delta index (using interned types)
        let mut delta_index: FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>> =
            FxHashMap::default();
        for lit in &facts_new {
            delta_index
                .entry(fact_index_key(lit))
                .or_default()
                .push(lit.clone());
        }

        // For each rule with variables
        for rule in &var_rules {
            if limit_hit {
                break;
            }

            let substitutions = match_body_with_delta(
                &rule.body,
                &fact_index,
                &delta_index,
                &facts_list,
                &facts_new,
            );

            for subst in substitutions {
                if instance_counter >= max_instances {
                    limit_hit = true;
                    break;
                }

                // Evaluate Allen constraints — reject substitutions that fail
                if !rule.constraints.is_empty() && !evaluate_constraints(&rule.constraints, &subst)
                {
                    continue;
                }

                // Evaluate temporal state queries — reject substitutions that fail
                if !rule.state_queries.is_empty()
                    && !evaluate_state_queries(&rule.state_queries, &subst)
                {
                    continue;
                }

                let sig = (rule.label.clone(), substitution_key(&subst));

                if !known_instances.contains(&sig) {
                    known_instances.insert(sig);
                    instance_counter += 1;

                    let ground_rule = apply_substitution_to_rule(rule, &subst, instance_counter);

                    // Add head literals as new facts (for non-defeaters)
                    if ground_rule.rule_type != RuleType::Defeater {
                        for head_lit in &ground_rule.head {
                            let key = literal_key(head_lit);
                            if !fact_keys.contains(&key) {
                                fact_keys.insert(key);
                                fact_index
                                    .entry(fact_index_key(head_lit))
                                    .or_default()
                                    .push(head_lit.clone());
                                new_facts_this_round.push(head_lit.clone());
                            }
                        }
                    }

                    new_rules_this_round.push(ground_rule);
                }
            }
        }

        // Update facts
        facts_list.extend(new_facts_this_round.iter().cloned());
        all_generated_rules.extend(new_rules_this_round);

        if new_facts_this_round.is_empty() || limit_hit {
            break;
        }

        facts_new = new_facts_this_round;
    }

    // Build final theory
    let mut grounded = Theory::new();
    for rule in all_generated_rules {
        grounded.add_rule(rule);
    }
    for sup in theory.superiorities() {
        grounded.add_superiority(&sup.superior, &sup.inferior);
    }

    grounded.copy_metadata_from(theory);
    *grounded.trust_policy_mut() = theory.trust_policy().clone();

    (grounded, limit_hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_variable() {
        assert!(is_variable("?x"));
        assert!(is_variable("?person"));
        assert!(!is_variable("alice"));
        assert!(!is_variable(""));
    }

    #[test]
    fn test_match_literal() {
        let pattern = Literal::new(
            "parent",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string(), "?y".to_string()],
        );
        let ground = Literal::new(
            "parent",
            false,
            Default::default(),
            Default::default(),
            vec!["alice".to_string(), "bob".to_string()],
        );

        let subst = match_literal(&pattern, &ground).unwrap();
        let x_id = intern("?x");
        let y_id = intern("?y");
        let alice_id = intern("alice");
        let bob_id = intern("bob");
        assert_eq!(subst.terms.get(&x_id), Some(&alice_id));
        assert_eq!(subst.terms.get(&y_id), Some(&bob_id));
    }

    #[test]
    fn test_match_literal_fails() {
        let pattern = Literal::new(
            "parent",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string(), "?y".to_string()],
        );
        let ground = Literal::new(
            "child",
            false,
            Default::default(),
            Default::default(),
            vec!["alice".to_string(), "bob".to_string()],
        );

        assert!(match_literal(&pattern, &ground).is_none());
    }

    #[test]
    fn test_apply_substitution() {
        let lit = Literal::new(
            "ancestor",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string(), "?y".to_string()],
        );
        let mut subst = Substitution::default();
        subst.terms.insert(intern("?x"), intern("alice"));
        subst.terms.insert(intern("?y"), intern("bob"));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(result.predicates(), vec!["alice", "bob"]);
    }

    #[test]
    fn test_ground_theory_simple() {
        let mut theory = Theory::new();

        // Add fact: parent(alice, bob)
        let f1 = Rule::fact(
            "f1",
            Literal::new(
                "parent",
                false,
                Default::default(),
                Default::default(),
                vec!["alice".to_string(), "bob".to_string()],
            ),
        );
        theory.add_rule(f1);

        // Add rule: parent(?x, ?y) => ancestor(?x, ?y)
        let r1 = Rule::defeasible(
            "r1",
            vec![Literal::new(
                "parent",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string(), "?y".to_string()],
            )],
            Literal::new(
                "ancestor",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string(), "?y".to_string()],
            ),
        );
        theory.add_rule(r1);

        let grounded = ground_theory(&theory);

        // Should have original fact + grounded rule
        assert!(grounded.rule_count() >= 2);

        // Check that grounded rule exists
        let has_grounded = grounded.rules().any(|r| {
            r.label.starts_with("r1_")
                && r.head
                    .iter()
                    .any(|h| h.name() == "ancestor" && h.predicates() == vec!["alice", "bob"])
        });
        assert!(has_grounded);
    }

    #[test]
    fn test_ground_theory_chain() {
        let mut theory = Theory::new();

        // p(a)
        let f1 = Rule::fact(
            "f1",
            Literal::new(
                "p",
                false,
                Default::default(),
                Default::default(),
                vec!["a".to_string()],
            ),
        );
        theory.add_rule(f1);

        // p(?x) => q(?x)
        let r1 = Rule::defeasible(
            "r1",
            vec![Literal::new(
                "p",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            )],
            Literal::new(
                "q",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            ),
        );
        theory.add_rule(r1);

        // q(?x) => r(?x)
        let r2 = Rule::defeasible(
            "r2",
            vec![Literal::new(
                "q",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            )],
            Literal::new(
                "r",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            ),
        );
        theory.add_rule(r2);

        let grounded = ground_theory(&theory);

        // Should have grounded r2 with q(a) => r(a)
        let has_r2_grounded = grounded.rules().any(|r| {
            r.label.starts_with("r2_")
                && r.head
                    .iter()
                    .any(|h| h.name() == "r" && h.predicates() == vec!["a"])
        });
        assert!(
            has_r2_grounded,
            "Grounding should produce r2 instance q(a) => r(a)"
        );
    }

    #[test]
    fn test_match_literal_negation_mismatch() {
        let pattern = Literal::new(
            "flies",
            false, // positive
            Default::default(),
            Default::default(),
            vec![],
        );
        let ground = Literal::new(
            "flies",
            true, // negated
            Default::default(),
            Default::default(),
            vec![],
        );
        assert!(match_literal(&pattern, &ground).is_none());
    }

    #[test]
    fn test_match_literal_arity_mismatch() {
        let pattern = Literal::new(
            "parent",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string()],
        );
        let ground = Literal::new(
            "parent",
            false,
            Default::default(),
            Default::default(),
            vec!["alice".to_string(), "bob".to_string()],
        );
        assert!(match_literal(&pattern, &ground).is_none());
    }

    #[test]
    fn test_match_literal_constant_mismatch() {
        let pattern = Literal::new(
            "parent",
            false,
            Default::default(),
            Default::default(),
            vec!["alice".to_string(), "?y".to_string()],
        );
        let ground = Literal::new(
            "parent",
            false,
            Default::default(),
            Default::default(),
            vec!["bob".to_string(), "carol".to_string()],
        );
        // alice != bob, should fail
        assert!(match_literal(&pattern, &ground).is_none());
    }

    #[test]
    fn test_literal_has_variables_name() {
        let lit = Literal::new("?x", false, Default::default(), Default::default(), vec![]);
        assert!(literal_has_variables(&lit));
    }

    #[test]
    fn test_literal_has_variables_predicate() {
        let lit = Literal::new(
            "parent",
            false,
            Default::default(),
            Default::default(),
            vec!["alice".to_string(), "?y".to_string()],
        );
        assert!(literal_has_variables(&lit));
    }

    #[test]
    fn test_ground_theory_with_superiorities() {
        let mut theory = Theory::new();

        let f1 = Rule::fact(
            "f1",
            Literal::new(
                "bird",
                false,
                Default::default(),
                Default::default(),
                vec!["tweety".to_string()],
            ),
        );
        theory.add_rule(f1);

        let r1 = Rule::defeasible(
            "r1",
            vec![Literal::new(
                "bird",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            )],
            Literal::new(
                "flies",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            ),
        );
        theory.add_rule(r1);

        // Add superiority relation
        theory.add_superiority("r2", "r1");

        let grounded = ground_theory(&theory);

        // Should preserve superiority
        assert_eq!(grounded.superiorities().len(), 1);
        assert_eq!(grounded.superiorities()[0].superior, "r2");
        assert_eq!(grounded.superiorities()[0].inferior, "r1");
    }

    #[test]
    fn test_ground_theory_no_variables() {
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let grounded = ground_theory(&theory);
        // Should return essentially the same theory
        assert_eq!(grounded.rule_count(), theory.rule_count());
    }

    // =========================================================================
    // ADDITIONAL COVERAGE TESTS
    // =========================================================================

    #[test]
    fn test_merge_substitutions_conflict() {
        let mut s1 = Substitution::default();
        s1.terms.insert(intern("?x"), intern("alice"));

        let mut s2 = Substitution::default();
        s2.terms.insert(intern("?x"), intern("bob")); // Conflict!

        let merged = merge_substitutions(&s1, &s2);
        assert!(
            merged.is_none(),
            "Conflicting substitutions should not merge"
        );
    }

    #[test]
    fn test_merge_substitutions_compatible() {
        let mut s1 = Substitution::default();
        s1.terms.insert(intern("?x"), intern("alice"));

        let mut s2 = Substitution::default();
        s2.terms.insert(intern("?y"), intern("bob"));

        let merged = merge_substitutions(&s1, &s2).unwrap();
        assert_eq!(merged.terms.len(), 2);
    }

    #[test]
    fn test_merge_substitutions_same_value() {
        let mut s1 = Substitution::default();
        s1.terms.insert(intern("?x"), intern("alice"));

        let mut s2 = Substitution::default();
        s2.terms.insert(intern("?x"), intern("alice")); // Same value

        let merged = merge_substitutions(&s1, &s2).unwrap();
        assert_eq!(merged.terms.len(), 1);
    }

    #[test]
    fn test_match_literal_variable_name() {
        let pattern = Literal::new(
            "?rel",
            false,
            Default::default(),
            Default::default(),
            vec!["alice".to_string(), "bob".to_string()],
        );
        let ground = Literal::new(
            "parent",
            false,
            Default::default(),
            Default::default(),
            vec!["alice".to_string(), "bob".to_string()],
        );

        let subst = match_literal(&pattern, &ground).unwrap();
        let rel_id = intern("?rel");
        let parent_id = intern("parent");
        assert_eq!(subst.terms.get(&rel_id), Some(&parent_id));
    }

    #[test]
    fn test_apply_substitution_variable_name() {
        let lit = Literal::new(
            "?rel",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string()],
        );
        let mut subst = Substitution::default();
        subst.terms.insert(intern("?rel"), intern("parent"));
        subst.terms.insert(intern("?x"), intern("alice"));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(result.name(), "parent");
        assert_eq!(result.predicates(), vec!["alice"]);
    }

    #[test]
    fn test_ground_theory_multi_body_rule() {
        // Test grounding with multi-body rules
        let mut theory = Theory::new();

        // parent(alice, bob)
        let f1 = Rule::fact(
            "f1",
            Literal::new(
                "parent",
                false,
                Default::default(),
                Default::default(),
                vec!["alice".to_string(), "bob".to_string()],
            ),
        );
        theory.add_rule(f1);

        // parent(bob, carol)
        let f2 = Rule::fact(
            "f2",
            Literal::new(
                "parent",
                false,
                Default::default(),
                Default::default(),
                vec!["bob".to_string(), "carol".to_string()],
            ),
        );
        theory.add_rule(f2);

        // parent(?x, ?y), parent(?y, ?z) => grandparent(?x, ?z)
        let r1 = Rule::defeasible(
            "r1",
            vec![
                Literal::new(
                    "parent",
                    false,
                    Default::default(),
                    Default::default(),
                    vec!["?x".to_string(), "?y".to_string()],
                ),
                Literal::new(
                    "parent",
                    false,
                    Default::default(),
                    Default::default(),
                    vec!["?y".to_string(), "?z".to_string()],
                ),
            ],
            Literal::new(
                "grandparent",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string(), "?z".to_string()],
            ),
        );
        theory.add_rule(r1);

        let grounded = ground_theory(&theory);

        // Should have grandparent(alice, carol)
        let has_grandparent = grounded.rules().any(|r| {
            r.head
                .iter()
                .any(|h| h.name() == "grandparent" && h.predicates() == vec!["alice", "carol"])
        });
        assert!(
            has_grandparent,
            "Should ground to grandparent(alice, carol)"
        );
    }

    #[test]
    fn test_ground_theory_with_limit_exceeded() {
        // Test that grounding respects iteration limit
        let mut theory = Theory::new();

        // Create a recursive-ish pattern
        let f1 = Rule::fact(
            "f1",
            Literal::new(
                "p",
                false,
                Default::default(),
                Default::default(),
                vec!["a".to_string()],
            ),
        );
        theory.add_rule(f1);

        let r1 = Rule::defeasible(
            "r1",
            vec![Literal::new(
                "p",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            )],
            Literal::new(
                "q",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            ),
        );
        theory.add_rule(r1);

        // Ground with limit of 1
        let (grounded, _) = ground_theory_with_limit(&theory, 1, 1000);
        // Should still produce results
        assert!(grounded.rule_count() >= 1);
    }

    #[test]
    fn test_has_variables_in_head() {
        // Test has_variables when only head has variables
        let rule = Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::new(
                "flies",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            ),
        );
        assert!(has_variables(&rule));
    }

    #[test]
    fn test_has_variables_with_allen_constraints() {
        let mut rule = Rule::defeasible("r1", vec![Literal::simple("p")], Literal::simple("q"));
        rule.constraints.push(AllenConstraint::new(
            crate::temporal::AllenRelation::Before,
            intern("?T"),
            intern("?S"),
        ));

        assert!(has_variables(&rule));
    }

    #[test]
    fn test_unbound_allen_constraints_do_not_produce_ground_rules() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("p")));

        let mut constrained =
            Rule::defeasible("r1", vec![Literal::simple("p")], Literal::simple("result"));
        constrained.constraints.push(AllenConstraint::new(
            crate::temporal::AllenRelation::Before,
            intern("?T"),
            intern("?S"),
        ));
        theory.add_rule(constrained);

        let grounded = ground_theory(&theory);

        assert!(
            grounded.get_rule("r1").is_none(),
            "Constrained template rule should not survive as unconditional"
        );
        assert!(
            !grounded
                .rules()
                .any(|r| r.head.iter().any(|h| h.name() == "result")),
            "No grounded instance should be produced with unbound intervals"
        );
    }

    #[test]
    fn test_ground_theory_empty() {
        // Ground an empty theory
        let theory = Theory::new();
        let grounded = ground_theory(&theory);
        assert_eq!(grounded.rule_count(), 0);
    }

    #[test]
    fn test_match_literal_same_variable_twice() {
        // Pattern where same variable appears twice must match same value
        let pattern = Literal::new(
            "equal",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string(), "?x".to_string()],
        );

        // Ground literal with same value
        let ground_same = Literal::new(
            "equal",
            false,
            Default::default(),
            Default::default(),
            vec!["a".to_string(), "a".to_string()],
        );
        assert!(match_literal(&pattern, &ground_same).is_some());

        // Ground literal with different values
        let ground_diff = Literal::new(
            "equal",
            false,
            Default::default(),
            Default::default(),
            vec!["a".to_string(), "b".to_string()],
        );
        assert!(match_literal(&pattern, &ground_diff).is_none());
    }

    #[test]
    fn test_apply_substitution_non_variable_predicate() {
        let lit = Literal::new(
            "pred",
            false,
            Default::default(),
            Default::default(),
            vec!["constant".to_string(), "?x".to_string()],
        );

        let mut subst = Substitution::default();
        let x_id = intern("?x");
        let val_id = intern("value");
        subst.terms.insert(x_id, val_id);

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(result.predicates()[0], "constant");
        assert_eq!(result.predicates()[1], "value");
    }

    #[test]
    fn test_ground_with_variable_name_predicate() {
        // Test grounding where first body literal has variable as name
        // This is an unusual case but tests the variable name branch
        let mut theory = Theory::new();

        // Add some facts
        let f1 = Rule::fact(
            "f1",
            Literal::new(
                "data",
                false,
                Default::default(),
                Default::default(),
                vec!["a".to_string()],
            ),
        );
        theory.add_rule(f1);

        // Rule with concrete body
        let r1 = Rule::defeasible(
            "r1",
            vec![Literal::new(
                "data",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            )],
            Literal::new(
                "result",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            ),
        );
        theory.add_rule(r1);

        let grounded = ground_theory(&theory);
        assert!(grounded.rule_count() >= 2);
    }

    #[test]
    fn test_semi_naive_with_delta_variable() {
        // Test semi-naive grounding with variable in delta literal
        let mut theory = Theory::new();

        // Initial facts
        let f1 = Rule::fact(
            "f1",
            Literal::new(
                "edge",
                false,
                Default::default(),
                Default::default(),
                vec!["a".to_string(), "b".to_string()],
            ),
        );
        theory.add_rule(f1);

        let f2 = Rule::fact(
            "f2",
            Literal::new(
                "edge",
                false,
                Default::default(),
                Default::default(),
                vec!["b".to_string(), "c".to_string()],
            ),
        );
        theory.add_rule(f2);

        // Transitive closure rule
        let r1 = Rule::defeasible(
            "r1",
            vec![
                Literal::new(
                    "edge",
                    false,
                    Default::default(),
                    Default::default(),
                    vec!["?x".to_string(), "?y".to_string()],
                ),
                Literal::new(
                    "edge",
                    false,
                    Default::default(),
                    Default::default(),
                    vec!["?y".to_string(), "?z".to_string()],
                ),
            ],
            Literal::new(
                "path",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string(), "?z".to_string()],
            ),
        );
        theory.add_rule(r1);

        let grounded = ground_theory(&theory);
        // Should produce path(a, c) from transitivity
        let has_path = grounded
            .rules()
            .any(|r| r.head.iter().any(|h| h.name() == "path"));
        assert!(has_path);
    }

    #[test]
    fn test_match_body_with_variable_first_literal() {
        // Test when first literal in body is a variable (triggers line 200)
        let mut theory = Theory::new();

        // Add facts with predicates
        theory.add_rule(Rule::new(
            "f1".to_string(),
            RuleType::Fact,
            vec![],
            vec![Literal::new(
                "item",
                false,
                Default::default(),
                Default::default(),
                vec!["apple".to_string()],
            )],
        ));
        theory.add_rule(Rule::new(
            "f2".to_string(),
            RuleType::Fact,
            vec![],
            vec![Literal::new(
                "item",
                false,
                Default::default(),
                Default::default(),
                vec!["banana".to_string()],
            )],
        ));

        // Rule where first body literal is a predicate with variable
        let rule = Rule::new(
            "r1".to_string(),
            RuleType::Defeasible,
            vec![Literal::new(
                "item",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            )],
            vec![Literal::new(
                "edible",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            )],
        );
        theory.add_rule(rule);

        let grounded = ground_theory(&theory);
        // Should produce edible(apple) and edible(banana)
        let edible_count = grounded
            .rules()
            .filter(|r| r.head.iter().any(|h| h.name() == "edible"))
            .count();
        assert!(edible_count >= 2);
    }

    #[test]
    fn test_semi_naive_with_variable_delta_literal() {
        // Test semi-naive iteration with variable in delta position (line 251)
        let mut theory = Theory::new();

        // Initial facts
        theory.add_rule(Rule::new(
            "f1".to_string(),
            RuleType::Fact,
            vec![],
            vec![Literal::new(
                "node",
                false,
                Default::default(),
                Default::default(),
                vec!["a".to_string()],
            )],
        ));
        theory.add_rule(Rule::new(
            "f2".to_string(),
            RuleType::Fact,
            vec![],
            vec![Literal::new(
                "node",
                false,
                Default::default(),
                Default::default(),
                vec!["b".to_string()],
            )],
        ));

        // Rule that needs delta iteration
        let rule = Rule::new(
            "r1".to_string(),
            RuleType::Defeasible,
            vec![Literal::new(
                "node",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            )],
            vec![Literal::new(
                "visited",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            )],
        );
        theory.add_rule(rule);

        let grounded = ground_theory(&theory);
        let visited_rules = grounded
            .rules()
            .filter(|r| r.head.iter().any(|h| h.name() == "visited"))
            .count();
        assert!(visited_rules >= 2);
    }

    #[test]
    fn test_match_body_with_delta_empty_body_returns_identity_substitution() {
        let fact_index: FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>> =
            FxHashMap::default();
        let delta_index: FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>> =
            FxHashMap::default();

        let substitutions = match_body_with_delta(&[], &fact_index, &delta_index, &[], &[]);
        assert_eq!(
            substitutions.len(),
            1,
            "empty-body matching should yield exactly one identity substitution"
        );
        assert!(
            substitutions[0].terms.is_empty()
                && substitutions[0].temporal.is_empty()
                && substitutions[0].intervals.is_empty(),
            "identity substitution should contain no bindings"
        );
    }

    #[test]
    fn test_ground_theory_keeps_empty_body_temporal_variable_fact() {
        // Variable-bearing empty-body facts must survive grounding so
        // TemporalVarValidation can report unresolved temporal variables.
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact(
            "f_var",
            Literal::new_with_temporal_expr(
                "p",
                false,
                Mode::empty(),
                TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
                vec!["a".to_string()],
            ),
        ));

        let grounded = ground_theory(&theory);
        let has_grounded_var_fact = grounded.rules().any(|r| {
            r.label.starts_with("f_var_")
                && r.rule_type == RuleType::Fact
                && r.body.is_empty()
                && r.head.iter().any(|h| {
                    h.name() == "p" && h.predicates() == vec!["a"] && h.temporal_expr.is_some()
                })
        });

        assert!(
            has_grounded_var_fact,
            "grounding should preserve empty-body temporal-variable facts"
        );
    }

    // =========================================================================
    // MODE-AWARE GROUNDING TESTS
    // =========================================================================

    #[test]
    fn test_match_literal_mode_mismatch() {
        // [O]pay(?x) vs pay(alice) (no mode) → None
        let pattern = Literal::new(
            "pay",
            false,
            Mode::obligation(),
            Default::default(),
            vec!["?x".to_string()],
        );
        let ground = Literal::new(
            "pay",
            false,
            Mode::empty(),
            Default::default(),
            vec!["alice".to_string()],
        );
        assert!(
            match_literal(&pattern, &ground).is_none(),
            "[O]pay(?x) should not match pay(alice) with no mode"
        );
    }

    #[test]
    fn test_match_literal_mode_match() {
        let pattern = Literal::new(
            "pay",
            false,
            Mode::obligation(),
            Default::default(),
            vec!["?x".to_string()],
        );
        let ground = Literal::new(
            "pay",
            false,
            Mode::obligation(),
            Default::default(),
            vec!["alice".to_string()],
        );
        let result = match_literal(&pattern, &ground);
        assert!(result.is_some(), "[O]pay(?x) should match [O]pay(alice)");
        let subst = result.unwrap();
        let x_id = intern("?x");
        let alice_id = intern("alice");
        assert_eq!(subst.terms.get(&x_id), Some(&alice_id));
    }

    #[test]
    fn test_ground_theory_mode_discrimination() {
        // Theory with [O]pay(alice) fact and non-modal rule pay(?x) => paid(?x)
        // The rule should NOT be grounded because modes don't match
        let mut theory = Theory::new();

        // Fact: [O]pay(alice)
        theory.add_rule(Rule::fact(
            "f1",
            Literal::new(
                "pay",
                false,
                Mode::obligation(),
                Default::default(),
                vec!["alice".to_string()],
            ),
        ));

        // Rule: pay(?x) => paid(?x)  (no mode on body literal)
        let r1 = Rule::defeasible(
            "r1",
            vec![Literal::new(
                "pay",
                false,
                Mode::empty(),
                Default::default(),
                vec!["?x".to_string()],
            )],
            Literal::new(
                "paid",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            ),
        );
        theory.add_rule(r1);

        let grounded = ground_theory(&theory);

        // Should NOT have any grounded instance of r1 since modes don't match
        let has_grounded_r1 = grounded.rules().any(|r| r.label.starts_with("r1_"));
        assert!(
            !has_grounded_r1,
            "Rule with non-modal body should not match [O] fact"
        );
    }

    #[test]
    fn test_ground_theory_same_mode_matches() {
        // Both fact and rule use [O] mode → grounded correctly
        let mut theory = Theory::new();

        // Fact: [O]pay(alice)
        theory.add_rule(Rule::fact(
            "f1",
            Literal::new(
                "pay",
                false,
                Mode::obligation(),
                Default::default(),
                vec!["alice".to_string()],
            ),
        ));

        // Rule: [O]pay(?x) => paid(?x)
        let r1 = Rule::defeasible(
            "r1",
            vec![Literal::new(
                "pay",
                false,
                Mode::obligation(),
                Default::default(),
                vec!["?x".to_string()],
            )],
            Literal::new(
                "paid",
                false,
                Default::default(),
                Default::default(),
                vec!["?x".to_string()],
            ),
        );
        theory.add_rule(r1);

        let grounded = ground_theory(&theory);

        // Should have a grounded instance of r1
        let has_grounded_r1 = grounded.rules().any(|r| {
            r.label.starts_with("r1_")
                && r.head
                    .iter()
                    .any(|h| h.name() == "paid" && h.predicates() == vec!["alice"])
        });
        assert!(
            has_grounded_r1,
            "Rule with [O] body should match [O] fact and produce paid(alice)"
        );
    }

    // =========================================================================
    // TEMPORAL VARIABLE GROUNDING TESTS
    // =========================================================================

    #[test]
    fn test_match_literal_temporal_var_binding() {
        // Pattern with temporal variables should bind against ground temporal
        let pattern = Literal::new_with_temporal_expr(
            "p",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        );
        let ground = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(100), TimePoint::Moment(200)),
            vec!["alice".to_string()],
        );

        let subst = match_literal(&pattern, &ground).unwrap();
        assert_eq!(subst.terms.get(&intern("?x")), Some(&intern("alice")));
        assert_eq!(
            subst.temporal.get(&intern("?t1")),
            Some(&TimePoint::Moment(100))
        );
        assert_eq!(
            subst.temporal.get(&intern("?t2")),
            Some(&TimePoint::Moment(200))
        );
    }

    #[test]
    fn test_match_literal_temporal_var_no_ground_temporal() {
        // Pattern with temporal vars should fail against non-temporal fact
        let pattern = Literal::new_with_temporal_expr(
            "p",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec![],
        );
        let ground = Literal::simple("p");

        assert!(match_literal(&pattern, &ground).is_none());
    }

    #[test]
    fn test_match_literal_temporal_mixed_const_var() {
        // Pattern: (during p 100 ?t2) against p[100, 300]
        let pattern = Literal::new_with_temporal_expr(
            "p",
            false,
            Mode::empty(),
            TemporalExpr::new(
                TimeExpr::Const(TimePoint::Moment(100)),
                TimeExpr::var("?t2"),
            ),
            vec![],
        );
        let ground = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(100), TimePoint::Moment(300)),
            vec![],
        );

        let subst = match_literal(&pattern, &ground).unwrap();
        assert!(subst.terms.is_empty());
        assert_eq!(
            subst.temporal.get(&intern("?t2")),
            Some(&TimePoint::Moment(300))
        );
    }

    #[test]
    fn test_match_literal_temporal_const_mismatch() {
        // Pattern: (during p 100 ?t2) against p[200, 300] — start mismatch
        let pattern = Literal::new_with_temporal_expr(
            "p",
            false,
            Mode::empty(),
            TemporalExpr::new(
                TimeExpr::Const(TimePoint::Moment(100)),
                TimeExpr::var("?t2"),
            ),
            vec![],
        );
        let ground = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(200), TimePoint::Moment(300)),
            vec![],
        );

        assert!(match_literal(&pattern, &ground).is_none());
    }

    #[test]
    fn test_match_literal_temporal_var_conflict() {
        // Same temporal variable used for both start and end (e.g., ?t, ?t)
        // Should succeed when start == end, fail otherwise.
        let pattern = Literal::new_with_temporal_expr(
            "p",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t"), TimeExpr::var("?t")),
            vec![],
        );

        // start != end → should fail
        let ground1 = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(100), TimePoint::Moment(200)),
            vec![],
        );
        assert!(match_literal(&pattern, &ground1).is_none());

        // start == end → should succeed
        let ground2 = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(100), TimePoint::Moment(100)),
            vec![],
        );
        let subst = match_literal(&pattern, &ground2).unwrap();
        assert_eq!(
            subst.temporal.get(&intern("?t")),
            Some(&TimePoint::Moment(100))
        );
    }

    #[test]
    fn test_apply_substitution_resolves_temporal_expr() {
        // Literal with temporal_expr should resolve to concrete temporal
        let lit = Literal::new_with_temporal_expr(
            "q",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        );

        let mut subst = Substitution::default();
        subst.terms.insert(intern("?x"), intern("alice"));
        subst.temporal.insert(intern("?t1"), TimePoint::Moment(100));
        subst.temporal.insert(intern("?t2"), TimePoint::Moment(200));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(result.predicates(), vec!["alice"]);
        assert!(result.temporal_expr.is_none(), "Should be fully resolved");
        assert_eq!(result.temporal.start, TimePoint::Moment(100));
        assert_eq!(result.temporal.end, TimePoint::Moment(200));
    }

    #[test]
    fn test_apply_substitution_partial_temporal_resolution() {
        // If only one temporal var is bound, result keeps temporal_expr
        let lit = Literal::new_with_temporal_expr(
            "q",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec![],
        );

        let mut subst = Substitution::default();
        subst.temporal.insert(intern("?t1"), TimePoint::Moment(100));
        // ?t2 not bound

        let result = apply_substitution_to_literal(&lit, &subst);
        assert!(result.temporal_expr.is_some(), "Should remain symbolic");
        let texpr = result.temporal_expr.unwrap();
        assert_eq!(texpr.start, TimeExpr::Const(TimePoint::Moment(100)));
        assert!(texpr.end.is_var());
    }

    #[test]
    fn test_merge_substitutions_temporal_conflict() {
        let mut s1 = Substitution::default();
        s1.temporal.insert(intern("?t"), TimePoint::Moment(100));

        let mut s2 = Substitution::default();
        s2.temporal.insert(intern("?t"), TimePoint::Moment(200)); // Conflict!

        assert!(
            merge_substitutions(&s1, &s2).is_none(),
            "Conflicting temporal bindings should reject"
        );
    }

    #[test]
    fn test_merge_substitutions_temporal_compatible() {
        let mut s1 = Substitution::default();
        s1.temporal.insert(intern("?t1"), TimePoint::Moment(100));

        let mut s2 = Substitution::default();
        s2.temporal.insert(intern("?t2"), TimePoint::Moment(200));

        let merged = merge_substitutions(&s1, &s2).unwrap();
        assert_eq!(merged.temporal.len(), 2);
    }

    #[test]
    fn test_ground_theory_temporal_variable_propagation() {
        // Full integration: fact with temporal, rule with temporal vars, grounding propagates
        let mut theory = Theory::new();

        // Fact: p(a)[100, 200]
        theory.add_rule(Rule::fact(
            "f1",
            Literal::new(
                "p",
                false,
                Mode::empty(),
                Temporal::new(TimePoint::Moment(100), TimePoint::Moment(200)),
                vec!["a".to_string()],
            ),
        ));

        // Rule: (during (p ?x) ?t1 ?t2) => (during (q ?x) ?t1 ?t2)
        let body = Literal::new_with_temporal_expr(
            "p",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        );
        let head = Literal::new_with_temporal_expr(
            "q",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        );
        let r1 = Rule::new(
            "r1".to_string(),
            RuleType::Defeasible,
            vec![body],
            vec![head],
        );
        theory.add_rule(r1);

        let grounded = ground_theory(&theory);

        // Should produce grounded rule with q(a)[100, 200]
        let has_grounded = grounded.rules().any(|r| {
            r.label.starts_with("r1_")
                && r.head.iter().any(|h| {
                    h.name() == "q"
                        && h.predicates() == vec!["a"]
                        && h.temporal.start == TimePoint::Moment(100)
                        && h.temporal.end == TimePoint::Moment(200)
                        && h.temporal_expr.is_none()
                })
        });
        assert!(
            has_grounded,
            "Temporal variable propagation should produce q(a)[100, 200]"
        );
    }

    #[test]
    fn test_ground_theory_multiple_temporal_facts() {
        // Two temporal facts for same predicate should produce two groundings
        let mut theory = Theory::new();

        // f1: p(a)[100, 200]
        theory.add_rule(Rule::fact(
            "f1",
            Literal::new(
                "p",
                false,
                Mode::empty(),
                Temporal::new(TimePoint::Moment(100), TimePoint::Moment(200)),
                vec!["a".to_string()],
            ),
        ));

        // f2: p(a)[300, 400]
        theory.add_rule(Rule::fact(
            "f2",
            Literal::new(
                "p",
                false,
                Mode::empty(),
                Temporal::new(TimePoint::Moment(300), TimePoint::Moment(400)),
                vec!["a".to_string()],
            ),
        ));

        // Rule: (during (p ?x) ?t1 ?t2) => (during (q ?x) ?t1 ?t2)
        let body = Literal::new_with_temporal_expr(
            "p",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        );
        let head = Literal::new_with_temporal_expr(
            "q",
            false,
            Mode::empty(),
            TemporalExpr::new(TimeExpr::var("?t1"), TimeExpr::var("?t2")),
            vec!["?x".to_string()],
        );
        let r1 = Rule::new(
            "r1".to_string(),
            RuleType::Defeasible,
            vec![body],
            vec![head],
        );
        theory.add_rule(r1);

        let grounded = ground_theory(&theory);

        // Should produce two grounded rules: q(a)[100,200] and q(a)[300,400]
        let grounded_rules: Vec<_> = grounded
            .rules()
            .filter(|r| r.label.starts_with("r1_"))
            .collect();
        assert_eq!(
            grounded_rules.len(),
            2,
            "Two temporal facts should produce two groundings"
        );
    }
}
