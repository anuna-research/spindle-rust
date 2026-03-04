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

use crate::body::{BodyArg, BodyLiteral, BodyLogicLiteral};
use crate::function_registry::EvalContext;
use crate::intern::{SymbolId, resolve};

#[cfg(test)]
use crate::intern::intern;
use crate::literal::Literal;
use crate::mode::Mode;
use crate::rule::{Rule, RuleBody, RuleLabel, RuleType};
use crate::temporal::{
    AllenConstraint, Temporal, TemporalExpr, TemporalStateQuery, TimeExpr, TimePoint,
};
use crate::term::Term;
use crate::theory::Theory;

/// Variable substitution for grounding.
///
/// Contains both term bindings (variable -> typed Term value) and temporal
/// bindings (temporal variable -> concrete timepoint).
#[derive(Clone, Debug, Default)]
pub struct Substitution {
    /// Term variable bindings (e.g., ?x -> Term::Symbol(alice), ?n -> Term::Integer(42))
    pub terms: FxHashMap<SymbolId, Term>,
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

/// Check if a body literal contains variables (dispatches on variant).
fn body_literal_has_variables(bl: &BodyLiteral) -> bool {
    match bl {
        BodyLiteral::Logic(lit) => {
            is_variable(lit.name())
                || lit.predicate_args().iter().any(|a| match a {
                    BodyArg::Term(t) => {
                        if let Term::Symbol(id) = t {
                            is_variable(resolve(*id))
                        } else {
                            false
                        }
                    }
                    // Arithmetic args always need evaluation during grounding,
                    // even when constant (e.g. `(+ 1 2)`), because unevaluated
                    // BodyArg::Arith values are dropped by to_literal().
                    BodyArg::Arith(_) => true,
                })
                || lit.has_temporal_variables()
        }
        BodyLiteral::Arithmetic(_) => true, // arithmetic constraints always contain variables
    }
}

/// Check if a rule contains any variables.
///
/// Allen constraints and state queries are treated as variable-bearing because
/// they reference interval variables that must be validated during grounding.
pub fn has_variables(rule: &Rule) -> bool {
    rule.body.iter().any(body_literal_has_variables)
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
        subst
            .terms
            .insert(pattern_name_id, Term::Symbol(ground_name_id));
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
        if let Term::Symbol(parg_id) = parg {
            let parg_str = resolve(*parg_id);
            if is_variable(parg_str) {
                // Variable binds to the full Term value
                if let Some(existing) = subst.terms.get(parg_id) {
                    if *existing != *garg && !existing.numeric_eq(garg) {
                        return None;
                    }
                } else {
                    subst.terms.insert(*parg_id, garg.clone());
                }
                continue;
            }
        }
        // Non-variable: compare terms directly (with cross-type numeric promotion per REQ-010/CON-005)
        if parg != garg && !parg.numeric_eq(garg) {
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

    // Apply substitution to name (if it's a variable).
    // Literal names must be symbols, so extract SymbolId from Term::Symbol.
    let new_name_id = if is_variable(name) {
        match subst.terms.get(&name_id) {
            Some(Term::Symbol(id)) => *id,
            _ => name_id,
        }
    } else {
        name_id
    };

    // Apply substitution to predicate arguments.
    // Variables are resolved to their bound Term value directly.
    let new_pred_args: Vec<Term> = lit
        .predicate_args()
        .iter()
        .map(|term| {
            if let Term::Symbol(pid) = term {
                let p = resolve(*pid);
                if is_variable(p) {
                    return subst
                        .terms
                        .get(pid)
                        .cloned()
                        .unwrap_or_else(|| term.clone());
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

/// Apply a substitution to a rule, creating a ground instance.
///
/// Arithmetic body literals (`bind`/comparison constraints) are stripped from the
/// grounded rule because they have already been evaluated during the grounding phase.
/// The reasoner only understands logic body literals; keeping arithmetic constraints
/// would prevent the rule from firing.
fn apply_substitution_to_rule(
    rule: &Rule,
    subst: &Substitution,
    instance_num: usize,
    ctx: &EvalContext<'_>,
) -> Rule {
    let new_label = format!("{}_{}", rule.label, instance_num);
    let new_body: RuleBody = rule
        .body
        .iter()
        .filter(|bl| bl.as_logic().is_some())
        .map(|bl| apply_substitution_to_body_literal(bl, subst, ctx))
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

    // Merge term bindings (with cross-type numeric promotion per REQ-010/CON-005)
    for (k, v) in &s2.terms {
        if let Some(existing) = merged.terms.get(k) {
            if *existing != *v && !existing.numeric_eq(v) {
                return None;
            }
        } else {
            merged.terms.insert(*k, v.clone());
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

/// Normalize body literals in a grounded rule to match existing facts exactly.
///
/// When `bind` produces a value like `Decimal(8.00)` that is numerically equal
/// to a fact argument `Integer(8)`, the grounded body literal will contain
/// `Decimal(8.00)` while the fact has `Integer(8)`. The reasoner uses
/// structural equality, so these won't match. This function replaces body
/// literal arguments with the fact's values when they match via `numeric_eq`.
fn normalize_body_against_facts(
    rule: &mut Rule,
    fact_index: &FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>>,
) {
    let new_body: RuleBody = rule
        .body
        .iter()
        .map(|bl| {
            let logic = match bl.as_logic() {
                Some(l) => l,
                None => return bl.clone(),
            };
            let lit = logic.to_literal();
            let key = fact_index_key(&lit);
            let facts = match fact_index.get(&key) {
                Some(f) => f,
                None => return bl.clone(),
            };
            for fact in facts {
                if lit.name_id() == fact.name_id()
                    && lit.negation == fact.negation
                    && lit.mode == fact.mode
                    && lit.temporal == fact.temporal
                    && lit.predicate_args().len() == fact.predicate_args().len()
                    && lit
                        .predicate_args()
                        .iter()
                        .zip(fact.predicate_args().iter())
                        .all(|(a, b)| a == b || a.numeric_eq(b))
                {
                    return BodyLiteral::from(fact.clone());
                }
            }
            bl.clone()
        })
        .collect();
    rule.body = new_body;
}

/// Apply a substitution to a body literal, returning a new body literal.
///
/// For logic literals, `BodyArg::Arith` arguments are evaluated under the
/// substitution and replaced with concrete `Term` values.  This ensures the
/// grounded rule's body literal has the correct arity and can be matched by
/// the reasoner.
fn apply_substitution_to_body_literal(
    bl: &BodyLiteral,
    subst: &Substitution,
    ctx: &EvalContext<'_>,
) -> BodyLiteral {
    match bl {
        BodyLiteral::Logic(lit) => {
            // Use resolve_body_logic to evaluate any BodyArg::Arith arguments,
            // then apply substitution to produce a fully ground literal.
            match resolve_body_logic(lit, subst, ctx) {
                Some(resolved) => {
                    BodyLiteral::from(apply_substitution_to_literal(&resolved, subst))
                }
                None => {
                    // Arithmetic evaluation failed — fall back to dropping arith args.
                    let as_lit = lit.to_literal();
                    BodyLiteral::from(apply_substitution_to_literal(&as_lit, subst))
                }
            }
        }
        // Arithmetic constraints are passed through unchanged during body matching.
        // (Evaluation of arithmetic constraints is handled separately.)
        BodyLiteral::Arithmetic(_) => bl.clone(),
    }
}

/// Resolve a body logic literal into a [`Literal`] suitable for fact-matching.
///
/// - `BodyArg::Term` variables are resolved from the substitution.
/// - `BodyArg::Arith` arguments are evaluated under `subst` (via
///   [`ArithExpr::eval`]) and replaced with the resulting concrete [`Term`].
/// - Temporal variables and interval variables are also resolved.
///
/// Returns `None` if any arithmetic evaluation fails, signalling that this
/// substitution path should be discarded.
fn resolve_body_logic(
    lit: &BodyLogicLiteral,
    subst: &Substitution,
    ctx: &EvalContext<'_>,
) -> Option<Literal> {
    let mut terms = Vec::with_capacity(lit.predicate_args().len());
    for arg in lit.predicate_args() {
        match arg {
            BodyArg::Term(t) => {
                if let Term::Symbol(pid) = t
                    && is_variable(resolve(*pid))
                {
                    terms.push(subst.terms.get(pid).cloned().unwrap_or_else(|| t.clone()));
                    continue;
                }
                terms.push(t.clone());
            }
            BodyArg::Arith(expr) => {
                let val = expr.eval_with_context(subst, ctx).ok()?;
                terms.push(Term::try_from(val).ok()?);
            }
        }
    }

    // Resolve name if variable
    let name_id = lit.name_id();
    let resolved_name_id = if is_variable(resolve(name_id)) {
        match subst.terms.get(&name_id) {
            Some(Term::Symbol(id)) => *id,
            _ => name_id,
        }
    } else {
        name_id
    };

    let mut result = Literal::from_ids(
        resolved_name_id,
        lit.negation,
        lit.mode.clone(),
        lit.temporal.clone(),
        terms,
    );

    // Resolve interval variable (whole-interval binding)
    if let Some(var_id) = lit.interval_var {
        if let Some(interval) = subst.intervals.get(&var_id) {
            result.temporal = interval.clone();
        } else {
            result.interval_var = Some(var_id);
        }
    } else if let Some(ref texpr) = lit.temporal_expr {
        let resolved_start = resolve_time_expr(&texpr.start, &subst.temporal);
        let resolved_end = resolve_time_expr(&texpr.end, &subst.temporal);
        match (resolved_start, resolved_end) {
            (TimeExpr::Const(s), TimeExpr::Const(e)) => {
                result.temporal = Temporal::new(s, e);
            }
            (start, end) => {
                result.temporal_expr = Some(TemporalExpr::new(start, end));
            }
        }
    }

    Some(result)
}

/// Match body literals against facts, returning all valid substitutions.
///
/// Evaluates [`BodyLiteral`] elements in source order, threading
/// substitutions left-to-right (ADR-001b):
///
/// - **Logic** literals have their `BodyArg::Arith` arguments evaluated
///   under the current substitution, then are matched against facts.
/// - **Arithmetic** constraints are evaluated under the current
///   substitution: `Bind` extends it, `Compare` filters it.
///
/// On any evaluation failure the substitution path is discarded.
#[cfg(test)]
fn match_body_against_facts(
    body: &[BodyLiteral],
    fact_index: &FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>>,
    all_facts: &[Literal],
    current_subst: &Substitution,
) -> Vec<Substitution> {
    let prelude = crate::function_registry::FunctionRegistry::with_prelude();
    let ctx = EvalContext::with_registry(&prelude);
    match_body_against_facts_ctx(body, fact_index, all_facts, current_subst, &ctx)
}

#[cfg(test)]
fn match_body_against_facts_ctx(
    body: &[BodyLiteral],
    fact_index: &FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>>,
    all_facts: &[Literal],
    current_subst: &Substitution,
    ctx: &EvalContext<'_>,
) -> Vec<Substitution> {
    if body.is_empty() {
        return vec![current_subst.clone()];
    }

    let first = &body[0];
    let rest = &body[1..];

    match first {
        BodyLiteral::Logic(logic_lit) => {
            // Resolve BodyArg::Arith arguments and variable terms under current_subst
            let first_lit = match resolve_body_logic(logic_lit, current_subst, ctx) {
                Some(lit) => lit,
                None => return Vec::new(), // arith eval failed — discard path
            };

            // Get candidate facts
            let candidates: Vec<&Literal> = if is_variable(first_lit.name()) {
                all_facts.iter().collect()
            } else {
                fact_index
                    .get(&fact_index_key(&first_lit))
                    .map(|v| v.iter().collect())
                    .unwrap_or_default()
            };

            let mut results = Vec::new();

            for fact in candidates {
                if let Some(new_bindings) = match_literal(&first_lit, fact)
                    && let Some(merged) = merge_substitutions(current_subst, &new_bindings)
                {
                    results.extend(match_body_against_facts_ctx(
                        rest, fact_index, all_facts, &merged, ctx,
                    ));
                }
            }

            results
        }
        BodyLiteral::Arithmetic(constraint) => {
            // Evaluate the constraint under the current substitution.
            // Bind extends the substitution; Compare filters it.
            let mut sub_copy = current_subst.clone();
            if constraint.eval_with_context(&mut sub_copy, ctx).is_ok() {
                match_body_against_facts_ctx(rest, fact_index, all_facts, &sub_copy, ctx)
            } else {
                Vec::new() // evaluation failed — discard path
            }
        }
    }
}

/// Match body with at least one delta (new) fact.
///
/// Evaluates body elements in source order (left-to-right), threading
/// substitutions and tracking whether at least one delta fact was used.
/// Only substitutions that involve at least one delta fact are returned.
///
/// Logic literals are resolved via [`resolve_body_logic`] and matched
/// against all available facts. Arithmetic constraints are evaluated
/// inline under the current substitution.
fn match_body_with_delta(
    body: &[BodyLiteral],
    fact_index: &FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>>,
    all_facts: &[Literal],
    delta_facts: &[Literal],
    ctx: &EvalContext<'_>,
) -> Vec<Substitution> {
    if body.is_empty() {
        return vec![Substitution::default()];
    }

    // Build a set of delta fact keys for efficient membership testing
    let delta_keys: FxHashSet<_> = delta_facts.iter().map(literal_key).collect();

    // If body has no logic literals, arithmetic-only bodies are always delta-relevant
    // (the rule is essentially a ground generator).
    let has_logic_literals = body.iter().any(|bl| matches!(bl, BodyLiteral::Logic(_)));

    let mut results = Vec::new();
    let mut seen: FxHashSet<SubstitutionKey> = FxHashSet::default();

    for (subst, used_delta) in match_body_ordered_delta(
        body,
        fact_index,
        all_facts,
        &delta_keys,
        &Substitution::default(),
        false,
        ctx,
    ) {
        if used_delta || !has_logic_literals {
            let key = substitution_key(&subst);
            if !seen.contains(&key) {
                seen.insert(key);
                results.push(subst);
            }
        }
    }

    results
}

/// Recursive helper for source-order body matching with delta tracking.
///
/// Processes body elements left-to-right, accumulating `current_subst` and
/// tracking whether any matched fact belongs to the delta set.
fn match_body_ordered_delta(
    body: &[BodyLiteral],
    fact_index: &FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>>,
    all_facts: &[Literal],
    delta_keys: &FxHashSet<(SymbolId, bool, Vec<Term>, Mode, Temporal)>,
    current_subst: &Substitution,
    used_delta: bool,
    ctx: &EvalContext<'_>,
) -> Vec<(Substitution, bool)> {
    if body.is_empty() {
        return vec![(current_subst.clone(), used_delta)];
    }

    let first = &body[0];
    let rest = &body[1..];

    match first {
        BodyLiteral::Logic(logic_lit) => {
            let first_lit = match resolve_body_logic(logic_lit, current_subst, ctx) {
                Some(lit) => lit,
                None => return Vec::new(),
            };

            let candidates: Vec<&Literal> = if is_variable(first_lit.name()) {
                all_facts.iter().collect()
            } else {
                fact_index
                    .get(&fact_index_key(&first_lit))
                    .map(|v| v.iter().collect())
                    .unwrap_or_default()
            };

            let mut results = Vec::new();

            for fact in candidates {
                if let Some(new_bindings) = match_literal(&first_lit, fact)
                    && let Some(merged) = merge_substitutions(current_subst, &new_bindings)
                {
                    let is_delta = delta_keys.contains(&literal_key(fact));
                    results.extend(match_body_ordered_delta(
                        rest,
                        fact_index,
                        all_facts,
                        delta_keys,
                        &merged,
                        used_delta || is_delta,
                        ctx,
                    ));
                }
            }

            results
        }
        BodyLiteral::Arithmetic(constraint) => {
            let mut sub_copy = current_subst.clone();
            if constraint.eval_with_context(&mut sub_copy, ctx).is_ok() {
                match_body_ordered_delta(
                    rest, fact_index, all_facts, delta_keys, &sub_copy, used_delta, ctx,
                )
            } else {
                Vec::new()
            }
        }
    }
}

/// A hashable key representing a substitution (term, temporal, and interval bindings).
type SubstitutionKey = (
    Vec<(SymbolId, Term)>,
    Vec<(SymbolId, TimePoint)>,
    Vec<(SymbolId, Temporal)>,
);

/// Build a hashable key from a substitution for deduplication.
fn substitution_key(subst: &Substitution) -> SubstitutionKey {
    let mut term_pairs: Vec<_> = subst.terms.iter().map(|(k, v)| (*k, v.clone())).collect();
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
    let prelude = crate::function_registry::FunctionRegistry::with_prelude();
    let ctx = EvalContext::with_registry(&prelude);
    ground_theory_with_limit(theory, 100, usize::MAX, &ctx).0
}

/// Ground a theory with a maximum iteration limit and instance limit
/// Returns (grounded_theory, limit_hit)
pub fn ground_theory_with_limit(
    theory: &Theory,
    max_iterations: usize,
    max_instances: usize,
    ctx: &EvalContext<'_>,
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

        // For each rule with variables
        for rule in &var_rules {
            if limit_hit {
                break;
            }

            let substitutions =
                match_body_with_delta(&rule.body, &fact_index, &facts_list, &facts_new, ctx);

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

                    let mut ground_rule =
                        apply_substitution_to_rule(rule, &subst, instance_counter, ctx);
                    normalize_body_against_facts(&mut ground_rule, &fact_index);

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
        assert_eq!(subst.terms.get(&x_id), Some(&Term::Symbol(intern("alice"))));
        assert_eq!(subst.terms.get(&y_id), Some(&Term::Symbol(intern("bob"))));
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
        subst
            .terms
            .insert(intern("?x"), Term::Symbol(intern("alice")));
        subst
            .terms
            .insert(intern("?y"), Term::Symbol(intern("bob")));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(
            result.predicates(),
            vec!["alice".to_string(), "bob".to_string()]
        );
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
                && r.head.iter().any(|h| {
                    h.name() == "ancestor"
                        && h.predicates() == vec!["alice".to_string(), "bob".to_string()]
                })
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
                    .any(|h| h.name() == "r" && h.predicates() == vec!["a".to_string()])
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
        s1.terms.insert(intern("?x"), Term::Symbol(intern("alice")));

        let mut s2 = Substitution::default();
        s2.terms.insert(intern("?x"), Term::Symbol(intern("bob"))); // Conflict!

        let merged = merge_substitutions(&s1, &s2);
        assert!(
            merged.is_none(),
            "Conflicting substitutions should not merge"
        );
    }

    #[test]
    fn test_merge_substitutions_compatible() {
        let mut s1 = Substitution::default();
        s1.terms.insert(intern("?x"), Term::Symbol(intern("alice")));

        let mut s2 = Substitution::default();
        s2.terms.insert(intern("?y"), Term::Symbol(intern("bob")));

        let merged = merge_substitutions(&s1, &s2).unwrap();
        assert_eq!(merged.terms.len(), 2);
    }

    #[test]
    fn test_merge_substitutions_same_value() {
        let mut s1 = Substitution::default();
        s1.terms.insert(intern("?x"), Term::Symbol(intern("alice")));

        let mut s2 = Substitution::default();
        s2.terms.insert(intern("?x"), Term::Symbol(intern("alice"))); // Same value

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
        assert_eq!(
            subst.terms.get(&rel_id),
            Some(&Term::Symbol(intern("parent")))
        );
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
        subst
            .terms
            .insert(intern("?rel"), Term::Symbol(intern("parent")));
        subst
            .terms
            .insert(intern("?x"), Term::Symbol(intern("alice")));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(result.name(), "parent");
        assert_eq!(result.predicates(), vec!["alice".to_string()]);
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
            r.head.iter().any(|h| {
                h.name() == "grandparent"
                    && h.predicates() == vec!["alice".to_string(), "carol".to_string()]
            })
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
        let (grounded, _) = ground_theory_with_limit(&theory, 1, 1000, &EvalContext::empty());
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
        subst.terms.insert(x_id, Term::Symbol(intern("value")));

        let result = apply_substitution_to_literal(&lit, &subst);
        let preds = result.predicates();
        assert_eq!(preds[0], "constant");
        assert_eq!(preds[1], "value");
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

        let substitutions =
            match_body_with_delta(&[], &fact_index, &[], &[], &EvalContext::empty());
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
                    h.name() == "p"
                        && h.predicates() == vec!["a".to_string()]
                        && h.temporal_expr.is_some()
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
        assert_eq!(subst.terms.get(&x_id), Some(&Term::Symbol(intern("alice"))));
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
                    .any(|h| h.name() == "paid" && h.predicates() == vec!["alice".to_string()])
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
        assert_eq!(
            subst.terms.get(&intern("?x")),
            Some(&Term::Symbol(intern("alice")))
        );
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
        subst
            .terms
            .insert(intern("?x"), Term::Symbol(intern("alice")));
        subst.temporal.insert(intern("?t1"), TimePoint::Moment(100));
        subst.temporal.insert(intern("?t2"), TimePoint::Moment(200));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(result.predicates(), vec!["alice".to_string()]);
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
                        && h.predicates() == vec!["a".to_string()]
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

    // =====================================================================
    // Cross-type numeric matching (REQ-010/CON-005)
    // =====================================================================

    #[test]
    fn test_match_literal_integer_matches_decimal() {
        use rust_decimal::Decimal;

        // Pattern has Integer(100), ground has Decimal(100.00)
        let pattern = Literal::from_ids(
            intern("price"),
            false,
            Default::default(),
            Default::default(),
            vec![Term::Symbol(intern("item")), Term::Integer(100)],
        );
        let ground = Literal::from_ids(
            intern("price"),
            false,
            Default::default(),
            Default::default(),
            vec![
                Term::Symbol(intern("item")),
                Term::Decimal(Decimal::new(10000, 2)),
            ],
        );

        assert!(
            match_literal(&pattern, &ground).is_some(),
            "Integer(100) should match Decimal(100.00)"
        );
    }

    #[test]
    fn test_match_literal_integer_matches_float() {
        use crate::term::FiniteFloat;

        // Pattern has Integer(100), ground has Float(100.0)
        let pattern = Literal::from_ids(
            intern("price"),
            false,
            Default::default(),
            Default::default(),
            vec![Term::Symbol(intern("item")), Term::Integer(100)],
        );
        let ground = Literal::from_ids(
            intern("price"),
            false,
            Default::default(),
            Default::default(),
            vec![
                Term::Symbol(intern("item")),
                Term::Float(FiniteFloat::new(100.0).unwrap()),
            ],
        );

        assert!(
            match_literal(&pattern, &ground).is_some(),
            "Integer(100) should match Float(100.0)"
        );
    }

    #[test]
    fn test_match_literal_decimal_matches_float() {
        use crate::term::FiniteFloat;
        use rust_decimal::Decimal;

        // Pattern has Decimal(100.00), ground has Float(100.0)
        let pattern = Literal::from_ids(
            intern("price"),
            false,
            Default::default(),
            Default::default(),
            vec![
                Term::Symbol(intern("item")),
                Term::Decimal(Decimal::new(10000, 2)),
            ],
        );
        let ground = Literal::from_ids(
            intern("price"),
            false,
            Default::default(),
            Default::default(),
            vec![
                Term::Symbol(intern("item")),
                Term::Float(FiniteFloat::new(100.0).unwrap()),
            ],
        );

        assert!(
            match_literal(&pattern, &ground).is_some(),
            "Decimal(100.00) should match Float(100.0)"
        );
    }

    #[test]
    fn test_match_literal_symbol_never_matches_numeric() {
        // Symbol("100") must NOT match Integer(100)
        let pattern = Literal::from_ids(
            intern("val"),
            false,
            Default::default(),
            Default::default(),
            vec![Term::Symbol(intern("100"))],
        );
        let ground = Literal::from_ids(
            intern("val"),
            false,
            Default::default(),
            Default::default(),
            vec![Term::Integer(100)],
        );

        assert!(
            match_literal(&pattern, &ground).is_none(),
            "Symbol(\"100\") must not match Integer(100)"
        );
    }

    #[test]
    fn test_match_literal_cross_type_variable_consistency() {
        use rust_decimal::Decimal;

        // Variable ?x appears twice. First bound to Integer(100),
        // second position in ground has Decimal(100.00) — should still match.
        let pattern = Literal::from_ids(
            intern("eq"),
            false,
            Default::default(),
            Default::default(),
            vec![Term::Symbol(intern("?x")), Term::Symbol(intern("?x"))],
        );
        let ground = Literal::from_ids(
            intern("eq"),
            false,
            Default::default(),
            Default::default(),
            vec![Term::Integer(100), Term::Decimal(Decimal::new(10000, 2))],
        );

        assert!(
            match_literal(&pattern, &ground).is_some(),
            "Variable bound to Integer(100) should accept Decimal(100.00) on re-use"
        );
    }

    #[test]
    fn test_match_literal_cross_type_variable_inconsistency_rejects() {
        use rust_decimal::Decimal;

        // Variable ?x bound to Integer(100), second position has Decimal(99.00) — must reject.
        let pattern = Literal::from_ids(
            intern("eq"),
            false,
            Default::default(),
            Default::default(),
            vec![Term::Symbol(intern("?x")), Term::Symbol(intern("?x"))],
        );
        let ground = Literal::from_ids(
            intern("eq"),
            false,
            Default::default(),
            Default::default(),
            vec![Term::Integer(100), Term::Decimal(Decimal::new(9900, 2))],
        );

        assert!(
            match_literal(&pattern, &ground).is_none(),
            "Variable bound to Integer(100) should reject Decimal(99.00)"
        );
    }

    // -----------------------------------------------------------------------
    // Body eval order tests (ADR-001b)
    // -----------------------------------------------------------------------

    /// Helper: build a fact index from a slice of literals.
    fn build_fact_index(
        facts: &[Literal],
    ) -> FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>> {
        let mut idx: FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>> = FxHashMap::default();
        for lit in facts {
            idx.entry(fact_index_key(lit))
                .or_default()
                .push(lit.clone());
        }
        idx
    }

    #[test]
    fn body_eval_order_arithmetic_bind_threads_to_later_logic() {
        // Body: (bind ?y 10), (cost ?y)
        // Facts: cost(10)
        // Expected: one substitution with ?y = 10
        use crate::arith::{ArithConstraint, ArithExpr};
        use crate::body::{BodyArg, BodyLogicLiteral};
        use crate::term::NumericValue;

        let y_id = intern("?y");

        let bind_y = BodyLiteral::Arithmetic(ArithConstraint::Bind {
            var: y_id,
            expr: ArithExpr::Lit(NumericValue::Integer(10)),
        });

        let cost_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "cost",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![BodyArg::Term(Term::Symbol(y_id))],
        ));

        let body = vec![bind_y, cost_lit];

        let fact = Literal::from_ids(
            intern("cost"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Integer(10)],
        );
        let facts = vec![fact];
        let fact_index = build_fact_index(&facts);

        let results =
            match_body_against_facts(&body, &fact_index, &facts, &Substitution::default());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].terms.get(&y_id), Some(&Term::Integer(10)));
    }

    #[test]
    fn body_eval_order_arithmetic_bind_no_match() {
        // Body: (bind ?y 99), (cost ?y)
        // Facts: cost(10)
        // Expected: no substitutions (99 != 10)
        use crate::arith::{ArithConstraint, ArithExpr};
        use crate::body::{BodyArg, BodyLogicLiteral};
        use crate::term::NumericValue;

        let y_id = intern("?y");

        let bind_y = BodyLiteral::Arithmetic(ArithConstraint::Bind {
            var: y_id,
            expr: ArithExpr::Lit(NumericValue::Integer(99)),
        });

        let cost_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "cost",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![BodyArg::Term(Term::Symbol(y_id))],
        ));

        let body = vec![bind_y, cost_lit];

        let fact = Literal::from_ids(
            intern("cost"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Integer(10)],
        );
        let facts = vec![fact];
        let fact_index = build_fact_index(&facts);

        let results =
            match_body_against_facts(&body, &fact_index, &facts, &Substitution::default());
        assert!(results.is_empty(), "bind ?y=99 should not match cost(10)");
    }

    #[test]
    fn body_eval_order_compare_filters_substitution() {
        // Body: (price ?x ?p), (> ?p 50)
        // Facts: price(a, 100), price(b, 30)
        // Expected: only the {?x=a, ?p=100} substitution survives
        use crate::arith::{ArithConstraint, ArithExpr, CmpOp};
        use crate::body::{BodyArg, BodyLogicLiteral};
        use crate::term::NumericValue;

        let x_id = intern("?x");
        let p_id = intern("?p");

        let price_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "price",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(x_id)),
                BodyArg::Term(Term::Symbol(p_id)),
            ],
        ));

        let compare = BodyLiteral::Arithmetic(ArithConstraint::Compare {
            op: CmpOp::Gt,
            lhs: ArithExpr::Var(p_id),
            rhs: ArithExpr::Lit(NumericValue::Integer(50)),
        });

        let body = vec![price_lit, compare];

        let fact_a = Literal::from_ids(
            intern("price"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("a")), Term::Integer(100)],
        );
        let fact_b = Literal::from_ids(
            intern("price"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("b")), Term::Integer(30)],
        );
        let facts = vec![fact_a, fact_b];
        let fact_index = build_fact_index(&facts);

        let results =
            match_body_against_facts(&body, &fact_index, &facts, &Substitution::default());
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].terms.get(&x_id),
            Some(&Term::Symbol(intern("a")))
        );
        assert_eq!(results[0].terms.get(&p_id), Some(&Term::Integer(100)));
    }

    #[test]
    fn body_eval_order_arith_arg_in_logic_literal() {
        // Body: (val ?x ?n), (result ?x (+ ?n 1))
        // Facts: val(a, 5), result(a, 6)
        // Expected: one substitution {?x=a, ?n=5}
        use crate::arith::ArithExpr;
        use crate::body::{BodyArg, BodyLogicLiteral};
        use crate::term::NumericValue;

        let x_id = intern("?x");
        let n_id = intern("?n");

        let val_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "val",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(x_id)),
                BodyArg::Term(Term::Symbol(n_id)),
            ],
        ));

        let result_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "result",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(x_id)),
                BodyArg::Arith(ArithExpr::Call {
                    name: intern("+"),
                    args: vec![
                        ArithExpr::Var(n_id),
                        ArithExpr::Lit(NumericValue::Integer(1)),
                    ],
                }),
            ],
        ));

        let body = vec![val_lit, result_lit];

        let fact1 = Literal::from_ids(
            intern("val"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("a")), Term::Integer(5)],
        );
        let fact2 = Literal::from_ids(
            intern("result"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("a")), Term::Integer(6)],
        );
        let facts = vec![fact1, fact2];
        let fact_index = build_fact_index(&facts);

        let results =
            match_body_against_facts(&body, &fact_index, &facts, &Substitution::default());
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].terms.get(&x_id),
            Some(&Term::Symbol(intern("a")))
        );
        assert_eq!(results[0].terms.get(&n_id), Some(&Term::Integer(5)));
    }

    #[test]
    fn body_eval_order_arith_arg_eval_fail_discards_path() {
        // Body: (result ?x (+ ?unbound 1))
        // Facts: result(a, 6)
        // Expected: no substitutions (arith eval fails due to unbound ?unbound)
        use crate::arith::ArithExpr;
        use crate::body::{BodyArg, BodyLogicLiteral};
        use crate::term::NumericValue;

        let x_id = intern("?x");
        let unbound_id = intern("?unbound");

        let result_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "result",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(x_id)),
                BodyArg::Arith(ArithExpr::Call {
                    name: intern("+"),
                    args: vec![
                        ArithExpr::Var(unbound_id),
                        ArithExpr::Lit(NumericValue::Integer(1)),
                    ],
                }),
            ],
        ));

        let body = vec![result_lit];

        let fact = Literal::from_ids(
            intern("result"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("a")), Term::Integer(6)],
        );
        let facts = vec![fact];
        let fact_index = build_fact_index(&facts);

        let results =
            match_body_against_facts(&body, &fact_index, &facts, &Substitution::default());
        assert!(
            results.is_empty(),
            "unbound arith var should discard substitution path"
        );
    }

    #[test]
    fn body_eval_order_bind_then_arith_arg() {
        // Body: (val ?x ?n), (bind ?total (+ ?n 100)), (budget ?x ?total)
        // Facts: val(a, 5), budget(a, 105)
        // Tests that bind result threads into subsequent arith arg
        use crate::arith::{ArithConstraint, ArithExpr};
        use crate::body::{BodyArg, BodyLogicLiteral};
        use crate::term::NumericValue;

        let x_id = intern("?x");
        let n_id = intern("?n");
        let total_id = intern("?total");

        let val_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "val",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(x_id)),
                BodyArg::Term(Term::Symbol(n_id)),
            ],
        ));

        let bind_total = BodyLiteral::Arithmetic(ArithConstraint::Bind {
            var: total_id,
            expr: ArithExpr::Call {
                name: intern("+"),
                args: vec![
                    ArithExpr::Var(n_id),
                    ArithExpr::Lit(NumericValue::Integer(100)),
                ],
            },
        });

        let budget_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "budget",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(x_id)),
                BodyArg::Term(Term::Symbol(total_id)),
            ],
        ));

        let body = vec![val_lit, bind_total, budget_lit];

        let fact1 = Literal::from_ids(
            intern("val"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("a")), Term::Integer(5)],
        );
        let fact2 = Literal::from_ids(
            intern("budget"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("a")), Term::Integer(105)],
        );
        let facts = vec![fact1, fact2];
        let fact_index = build_fact_index(&facts);

        let results =
            match_body_against_facts(&body, &fact_index, &facts, &Substitution::default());
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].terms.get(&x_id),
            Some(&Term::Symbol(intern("a")))
        );
        assert_eq!(results[0].terms.get(&n_id), Some(&Term::Integer(5)));
        assert_eq!(results[0].terms.get(&total_id), Some(&Term::Integer(105)));
    }

    #[test]
    fn body_eval_order_delta_with_arithmetic() {
        // Test that match_body_with_delta handles arithmetic in source order
        // Body: (price ?x ?p), (> ?p 50)
        // Delta facts: price(a, 100)
        // All facts: price(a, 100)
        use crate::arith::{ArithConstraint, ArithExpr, CmpOp};
        use crate::body::{BodyArg, BodyLogicLiteral};
        use crate::term::NumericValue;

        let x_id = intern("?x");
        let p_id = intern("?p");

        let price_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "price",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(x_id)),
                BodyArg::Term(Term::Symbol(p_id)),
            ],
        ));

        let compare = BodyLiteral::Arithmetic(ArithConstraint::Compare {
            op: CmpOp::Gt,
            lhs: ArithExpr::Var(p_id),
            rhs: ArithExpr::Lit(NumericValue::Integer(50)),
        });

        let body = vec![price_lit, compare];

        let fact = Literal::from_ids(
            intern("price"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("a")), Term::Integer(100)],
        );
        let facts = vec![fact.clone()];
        let delta_facts = vec![fact];
        let fact_index = build_fact_index(&facts);

        let results = match_body_with_delta(
            &body,
            &fact_index,
            &facts,
            &delta_facts,
            &EvalContext::empty(),
        );
        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].terms.get(&x_id),
            Some(&Term::Symbol(intern("a")))
        );
        assert_eq!(results[0].terms.get(&p_id), Some(&Term::Integer(100)));
    }

    #[test]
    fn body_eval_order_end_to_end_grounding() {
        // End-to-end: rule with bind and compare in body
        // cost(widget, 10) >>
        // cost(?item, ?price), (bind ?tax (* ?price 2)), (> ?tax 15)
        //   => expensive(?item)
        use crate::arith::{ArithConstraint, ArithExpr, CmpOp};
        use crate::body::{BodyArg, BodyLogicLiteral};
        use crate::term::NumericValue;

        let item_id = intern("?item");
        let price_id = intern("?price");
        let tax_id = intern("?tax");

        let mut theory = Theory::new();

        // Fact: cost(widget, 10)
        theory.add_rule(Rule::fact(
            "f1",
            Literal::from_ids(
                intern("cost"),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![Term::Symbol(intern("widget")), Term::Integer(10)],
            ),
        ));

        // Fact: cost(pen, 5)
        theory.add_rule(Rule::fact(
            "f2",
            Literal::from_ids(
                intern("cost"),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![Term::Symbol(intern("pen")), Term::Integer(5)],
            ),
        ));

        // Rule: cost(?item, ?price), (bind ?tax (* ?price 2)), (> ?tax 15)
        //       => expensive(?item)
        let body: RuleBody = smallvec::smallvec![
            BodyLiteral::Logic(BodyLogicLiteral::new(
                "cost",
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![
                    BodyArg::Term(Term::Symbol(item_id)),
                    BodyArg::Term(Term::Symbol(price_id)),
                ],
            )),
            BodyLiteral::Arithmetic(ArithConstraint::Bind {
                var: tax_id,
                expr: ArithExpr::Call {
                    name: intern("*"),
                    args: vec![
                        ArithExpr::Var(price_id),
                        ArithExpr::Lit(NumericValue::Integer(2)),
                    ],
                },
            }),
            BodyLiteral::Arithmetic(ArithConstraint::Compare {
                op: CmpOp::Gt,
                lhs: ArithExpr::Var(tax_id),
                rhs: ArithExpr::Lit(NumericValue::Integer(15)),
            }),
        ];

        let head = vec![Literal::from_ids(
            intern("expensive"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(item_id)],
        )];

        theory.add_rule(Rule::new("r1", RuleType::Strict, body, head));

        let grounded = ground_theory(&theory);

        // widget: tax = 10*2 = 20 > 15 → expensive(widget) should be derived
        // pen:    tax = 5*2  = 10 ≤ 15 → no derivation
        let expensive_rules: Vec<_> = grounded
            .rules()
            .filter(|r| r.head.iter().any(|h| h.name() == "expensive"))
            .collect();

        assert_eq!(
            expensive_rules.len(),
            1,
            "only widget should produce expensive"
        );
        assert_eq!(
            expensive_rules[0].head[0].predicate_args(),
            &[Term::Symbol(intern("widget"))]
        );
    }

    // =====================================================================
    // Term propagation tests — Integer, Decimal, Float through heads
    // =====================================================================

    #[test]
    fn apply_subst_propagates_integer_to_head() {
        let lit = Literal::from_ids(
            intern("result"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?x")), Term::Symbol(intern("?n"))],
        );
        let mut subst = Substitution::default();
        subst
            .terms
            .insert(intern("?x"), Term::Symbol(intern("widget")));
        subst.terms.insert(intern("?n"), Term::Integer(42));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(
            result.predicate_args(),
            &[Term::Symbol(intern("widget")), Term::Integer(42)]
        );
    }

    #[test]
    fn apply_subst_propagates_decimal_to_head() {
        use rust_decimal::Decimal;

        let lit = Literal::from_ids(
            intern("price"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?item")), Term::Symbol(intern("?p"))],
        );
        let mut subst = Substitution::default();
        subst
            .terms
            .insert(intern("?item"), Term::Symbol(intern("coffee")));
        subst
            .terms
            .insert(intern("?p"), Term::Decimal(Decimal::new(399, 2)));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(
            result.predicate_args(),
            &[
                Term::Symbol(intern("coffee")),
                Term::Decimal(Decimal::new(399, 2))
            ]
        );
    }

    #[test]
    fn apply_subst_propagates_float_to_head() {
        use crate::term::FiniteFloat;

        let lit = Literal::from_ids(
            intern("measurement"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?sensor")), Term::Symbol(intern("?v"))],
        );
        let mut subst = Substitution::default();
        subst
            .terms
            .insert(intern("?sensor"), Term::Symbol(intern("temp1")));
        subst
            .terms
            .insert(intern("?v"), Term::Float(FiniteFloat::new(98.6).unwrap()));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(
            result.predicate_args(),
            &[
                Term::Symbol(intern("temp1")),
                Term::Float(FiniteFloat::new(98.6).unwrap())
            ]
        );
    }

    #[test]
    fn apply_subst_preserves_concrete_numeric_args() {
        // Non-variable numeric args in the literal should pass through unchanged.
        use rust_decimal::Decimal;

        let lit = Literal::from_ids(
            intern("fixed"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                Term::Integer(100),
                Term::Decimal(Decimal::new(50, 1)),
                Term::Symbol(intern("?x")),
            ],
        );
        let mut subst = Substitution::default();
        subst
            .terms
            .insert(intern("?x"), Term::Symbol(intern("val")));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(
            result.predicate_args(),
            &[
                Term::Integer(100),
                Term::Decimal(Decimal::new(50, 1)),
                Term::Symbol(intern("val")),
            ]
        );
    }

    #[test]
    fn apply_subst_to_rule_propagates_numeric_to_head() {
        // Rule: data(?x, ?n) => result(?x, ?n)
        // Substitution: ?x = "sensor", ?n = Integer(42)
        // Head should get result(sensor, 42)
        use crate::body::BodyLogicLiteral;

        let body_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "data",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(intern("?x"))),
                BodyArg::Term(Term::Symbol(intern("?n"))),
            ],
        ));
        let head = Literal::from_ids(
            intern("result"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?x")), Term::Symbol(intern("?n"))],
        );
        let rule = Rule::new(
            "r1",
            RuleType::Defeasible,
            smallvec::smallvec![body_lit],
            vec![head],
        );

        let mut subst = Substitution::default();
        subst
            .terms
            .insert(intern("?x"), Term::Symbol(intern("sensor")));
        subst.terms.insert(intern("?n"), Term::Integer(42));

        let ground = apply_substitution_to_rule(&rule, &subst, 1, &EvalContext::empty());
        assert_eq!(
            ground.head[0].predicate_args(),
            &[Term::Symbol(intern("sensor")), Term::Integer(42)]
        );
    }

    #[test]
    fn grounding_propagates_integer_to_derived_fact() {
        // Fact: sensor(temp, 42) with Integer(42)
        // Rule: sensor(?name, ?val) => reading(?name, ?val)
        // After grounding: reading(temp, 42) should have Term::Integer(42)
        let mut theory = Theory::new();

        theory.add_rule(Rule::fact(
            "f1",
            Literal::from_ids(
                intern("sensor"),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![Term::Symbol(intern("temp")), Term::Integer(42)],
            ),
        ));

        let body_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "sensor",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(intern("?name"))),
                BodyArg::Term(Term::Symbol(intern("?val"))),
            ],
        ));
        let head = Literal::from_ids(
            intern("reading"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?name")), Term::Symbol(intern("?val"))],
        );
        theory.add_rule(Rule::new(
            "r1",
            RuleType::Defeasible,
            smallvec::smallvec![body_lit],
            vec![head],
        ));

        let grounded = ground_theory(&theory);

        let reading_rules: Vec<_> = grounded
            .rules()
            .filter(|r| r.head.iter().any(|h| h.name() == "reading"))
            .collect();
        assert_eq!(reading_rules.len(), 1);
        assert_eq!(
            reading_rules[0].head[0].predicate_args(),
            &[Term::Symbol(intern("temp")), Term::Integer(42)],
            "Integer(42) must propagate through head"
        );
    }

    #[test]
    fn grounding_propagates_decimal_to_derived_fact() {
        use rust_decimal::Decimal;

        let mut theory = Theory::new();

        theory.add_rule(Rule::fact(
            "f1",
            Literal::from_ids(
                intern("price"),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![
                    Term::Symbol(intern("coffee")),
                    Term::Decimal(Decimal::new(399, 2)),
                ],
            ),
        ));

        let body_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "price",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(intern("?item"))),
                BodyArg::Term(Term::Symbol(intern("?p"))),
            ],
        ));
        let head = Literal::from_ids(
            intern("listed"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?item")), Term::Symbol(intern("?p"))],
        );
        theory.add_rule(Rule::new(
            "r1",
            RuleType::Defeasible,
            smallvec::smallvec![body_lit],
            vec![head],
        ));

        let grounded = ground_theory(&theory);

        let listed_rules: Vec<_> = grounded
            .rules()
            .filter(|r| r.head.iter().any(|h| h.name() == "listed"))
            .collect();
        assert_eq!(listed_rules.len(), 1);
        assert_eq!(
            listed_rules[0].head[0].predicate_args(),
            &[
                Term::Symbol(intern("coffee")),
                Term::Decimal(Decimal::new(399, 2))
            ],
            "Decimal(3.99) must propagate through head"
        );
    }

    #[test]
    fn grounding_propagates_float_to_derived_fact() {
        use crate::term::FiniteFloat;

        let mut theory = Theory::new();

        theory.add_rule(Rule::fact(
            "f1",
            Literal::from_ids(
                intern("measure"),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![
                    Term::Symbol(intern("weight")),
                    Term::Float(FiniteFloat::new(72.5).unwrap()),
                ],
            ),
        ));

        let body_lit = BodyLiteral::Logic(BodyLogicLiteral::new(
            "measure",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(intern("?kind"))),
                BodyArg::Term(Term::Symbol(intern("?v"))),
            ],
        ));
        let head = Literal::from_ids(
            intern("recorded"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?kind")), Term::Symbol(intern("?v"))],
        );
        theory.add_rule(Rule::new(
            "r1",
            RuleType::Defeasible,
            smallvec::smallvec![body_lit],
            vec![head],
        ));

        let grounded = ground_theory(&theory);

        let recorded_rules: Vec<_> = grounded
            .rules()
            .filter(|r| r.head.iter().any(|h| h.name() == "recorded"))
            .collect();
        assert_eq!(recorded_rules.len(), 1);
        assert_eq!(
            recorded_rules[0].head[0].predicate_args(),
            &[
                Term::Symbol(intern("weight")),
                Term::Float(FiniteFloat::new(72.5).unwrap())
            ],
            "Float(72.5) must propagate through head"
        );
    }

    #[test]
    fn grounding_numeric_derived_fact_chains_to_next_rule() {
        // Fact: data(a, 10)
        // Rule r1: data(?x, ?n) => processed(?x, ?n)
        // Rule r2: processed(?x, ?n) => final(?x, ?n)
        // After grounding: final(a, 10) with Term::Integer(10)
        let mut theory = Theory::new();

        theory.add_rule(Rule::fact(
            "f1",
            Literal::from_ids(
                intern("data"),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![Term::Symbol(intern("a")), Term::Integer(10)],
            ),
        ));

        // r1: data(?x, ?n) => processed(?x, ?n)
        theory.add_rule(Rule::new(
            "r1",
            RuleType::Defeasible,
            smallvec::smallvec![BodyLiteral::Logic(BodyLogicLiteral::new(
                "data",
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![
                    BodyArg::Term(Term::Symbol(intern("?x"))),
                    BodyArg::Term(Term::Symbol(intern("?n"))),
                ],
            ))],
            vec![Literal::from_ids(
                intern("processed"),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![Term::Symbol(intern("?x")), Term::Symbol(intern("?n"))],
            )],
        ));

        // r2: processed(?x, ?n) => final(?x, ?n)
        theory.add_rule(Rule::new(
            "r2",
            RuleType::Defeasible,
            smallvec::smallvec![BodyLiteral::Logic(BodyLogicLiteral::new(
                "processed",
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![
                    BodyArg::Term(Term::Symbol(intern("?x"))),
                    BodyArg::Term(Term::Symbol(intern("?n"))),
                ],
            ))],
            vec![Literal::from_ids(
                intern("final"),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![Term::Symbol(intern("?x")), Term::Symbol(intern("?n"))],
            )],
        ));

        let grounded = ground_theory(&theory);

        // The derived fact final(a, 10) must have Term::Integer(10)
        let final_rules: Vec<_> = grounded
            .rules()
            .filter(|r| r.head.iter().any(|h| h.name() == "final"))
            .collect();
        assert_eq!(final_rules.len(), 1);
        assert_eq!(
            final_rules[0].head[0].predicate_args(),
            &[Term::Symbol(intern("a")), Term::Integer(10)],
            "Integer(10) must chain through processed to final"
        );
    }

    #[test]
    fn grounding_arith_bind_result_propagates_to_head() {
        // Fact: base(item, 10)
        // Rule: base(?x, ?n), (bind ?total (+ ?n 5)) => total(?x, ?total)
        // After grounding: total(item, 15) with Term::Integer(15)
        use crate::arith::{ArithConstraint, ArithExpr};
        use crate::term::NumericValue;

        let x_id = intern("?x");
        let n_id = intern("?n");
        let total_id = intern("?total");

        let mut theory = Theory::new();

        theory.add_rule(Rule::fact(
            "f1",
            Literal::from_ids(
                intern("base"),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![Term::Symbol(intern("item")), Term::Integer(10)],
            ),
        ));

        let body: RuleBody = smallvec::smallvec![
            BodyLiteral::Logic(BodyLogicLiteral::new(
                "base",
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![
                    BodyArg::Term(Term::Symbol(x_id)),
                    BodyArg::Term(Term::Symbol(n_id)),
                ],
            )),
            BodyLiteral::Arithmetic(ArithConstraint::Bind {
                var: total_id,
                expr: ArithExpr::Call {
                    name: intern("+"),
                    args: vec![
                        ArithExpr::Var(n_id),
                        ArithExpr::Lit(NumericValue::Integer(5)),
                    ],
                },
            }),
        ];

        let head = vec![Literal::from_ids(
            intern("total"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(x_id), Term::Symbol(total_id)],
        )];

        theory.add_rule(Rule::new("r1", RuleType::Defeasible, body, head));

        let grounded = ground_theory(&theory);

        let total_rules: Vec<_> = grounded
            .rules()
            .filter(|r| r.head.iter().any(|h| h.name() == "total"))
            .collect();
        assert_eq!(total_rules.len(), 1);
        assert_eq!(
            total_rules[0].head[0].predicate_args(),
            &[Term::Symbol(intern("item")), Term::Integer(15)],
            "Arithmetic result Integer(15) must propagate to head"
        );
    }

    #[test]
    fn grounding_mixed_numeric_types_in_head() {
        // Verify that a head can contain a mix of Symbol, Integer, Decimal, Float
        use crate::term::FiniteFloat;
        use rust_decimal::Decimal;

        let lit = Literal::from_ids(
            intern("multi"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                Term::Symbol(intern("?a")),
                Term::Symbol(intern("?b")),
                Term::Symbol(intern("?c")),
                Term::Symbol(intern("?d")),
            ],
        );
        let mut subst = Substitution::default();
        subst
            .terms
            .insert(intern("?a"), Term::Symbol(intern("name")));
        subst.terms.insert(intern("?b"), Term::Integer(42));
        subst
            .terms
            .insert(intern("?c"), Term::Decimal(Decimal::new(314, 2)));
        subst
            .terms
            .insert(intern("?d"), Term::Float(FiniteFloat::new(1.23).unwrap()));

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(
            result.predicate_args(),
            &[
                Term::Symbol(intern("name")),
                Term::Integer(42),
                Term::Decimal(Decimal::new(314, 2)),
                Term::Float(FiniteFloat::new(1.23).unwrap()),
            ]
        );
    }

    // -- P1: normalize_body_against_facts must preserve temporal bounds --------

    #[test]
    fn normalize_body_preserves_temporal() {
        let t1 = Temporal::new(TimePoint::Moment(1), TimePoint::Moment(2));
        let t2 = Temporal::new(TimePoint::Moment(3), TimePoint::Moment(4));

        // Two facts with the same predicate/args but different temporal bounds.
        let fact_t1 = Literal::new("p", false, Mode::empty(), t1.clone(), vec![]);
        let fact_t2 = Literal::new("p", false, Mode::empty(), t2.clone(), vec![]);

        let mut fact_index: FxHashMap<(SymbolId, bool, usize, Mode), Vec<Literal>> =
            FxHashMap::default();
        let key = fact_index_key(&fact_t1);
        let key2 = fact_index_key(&fact_t2);
        fact_index.entry(key).or_default().push(fact_t1.clone());
        fact_index.entry(key2).or_default().push(fact_t2.clone());

        // Build a rule whose body literal has temporal t1.
        let body_lit = Literal::new("p", false, Mode::empty(), t1.clone(), vec![]);
        let head = Literal::new("q", false, Mode::empty(), Temporal::empty(), vec![]);
        let mut rule = Rule::defeasible("r1", vec![body_lit], head);

        normalize_body_against_facts(&mut rule, &fact_index);

        // The normalized body literal must still have temporal t1, not t2.
        let normalized = rule.body[0].as_logic().unwrap().to_literal();
        assert_eq!(normalized.temporal, t1);
    }

    // -- P1b: Constant arithmetic body args must trigger grounding ------------

    #[test]
    fn has_variables_true_for_constant_arith_body_arg() {
        use crate::arith::ArithExpr;
        use crate::body::{BodyArg, BodyLogicLiteral};
        use crate::term::NumericValue;

        // Build a body literal: target((+ 1 2))
        let arith = ArithExpr::Call {
            name: intern("+"),
            args: vec![
                ArithExpr::Lit(NumericValue::Integer(1)),
                ArithExpr::Lit(NumericValue::Integer(2)),
            ],
        };
        let body_lit = BodyLogicLiteral::new(
            "target",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![BodyArg::Arith(arith)],
        );
        let bl = BodyLiteral::Logic(body_lit);

        // Must report as variable-bearing so grounding evaluates the arithmetic.
        assert!(body_literal_has_variables(&bl));
    }
}
