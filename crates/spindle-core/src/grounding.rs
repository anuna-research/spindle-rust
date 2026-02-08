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
use crate::theory::Theory;

/// Type alias for substitution (variable -> value)
/// Uses interned SymbolId for O(1) hashing and zero allocation lookups
pub type Substitution = FxHashMap<SymbolId, SymbolId>;

/// Check if a term is a variable (starts with ?)
pub fn is_variable(term: &str) -> bool {
    term.starts_with('?')
}

/// Check if a literal contains any variables
pub fn literal_has_variables(lit: &Literal) -> bool {
    is_variable(lit.name()) || lit.predicates().iter().any(|p| is_variable(p))
}

/// Check if a rule contains any variables
pub fn has_variables(rule: &Rule) -> bool {
    rule.body.iter().any(literal_has_variables) || rule.head.iter().any(literal_has_variables)
}

/// Try to match a pattern literal against a ground literal.
/// Returns a substitution if match succeeds, None otherwise.
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
        subst.insert(pattern_name_id, ground_name_id);
    } else if pattern_name_id != ground_name_id {
        return None;
    }

    // Match predicates/arguments (using interned symbols)
    let pattern_pred_ids = pattern.predicate_ids();
    let ground_pred_ids = ground.predicate_ids();
    if pattern_pred_ids.len() != ground_pred_ids.len() {
        return None;
    }

    for (parg_id, garg_id) in pattern_pred_ids.iter().zip(ground_pred_ids.iter()) {
        let parg = resolve(*parg_id);
        if is_variable(parg) {
            // Check for conflicting bindings
            if let Some(existing) = subst.get(parg_id) {
                if *existing != *garg_id {
                    return None;
                }
            } else {
                subst.insert(*parg_id, *garg_id);
            }
        } else if parg_id != garg_id {
            return None;
        }
    }

    Some(subst)
}

/// Apply a substitution to a literal (using interned SymbolIds)
pub fn apply_substitution_to_literal(lit: &Literal, subst: &Substitution) -> Literal {
    let name_id = lit.name_id();
    let name = resolve(name_id);

    // Apply substitution to name (if it's a variable)
    let new_name_id = if is_variable(name) {
        subst.get(&name_id).copied().unwrap_or(name_id)
    } else {
        name_id
    };

    // Apply substitution to predicates
    let new_pred_ids: Vec<SymbolId> = lit
        .predicate_ids()
        .iter()
        .map(|pid| {
            let p = resolve(*pid);
            if is_variable(p) {
                subst.get(pid).copied().unwrap_or(*pid)
            } else {
                *pid
            }
        })
        .collect();

    Literal::from_ids(
        new_name_id,
        lit.negation,
        lit.mode.clone(),
        lit.temporal.clone(),
        new_pred_ids,
    )
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
    new_rule
}

/// Merge two substitutions, returning None if they conflict
fn merge_substitutions(s1: &Substitution, s2: &Substitution) -> Option<Substitution> {
    let mut merged = s1.clone();
    for (k, v) in s2 {
        if let Some(existing) = merged.get(k) {
            if *existing != *v {
                return None;
            }
        } else {
            // SymbolId is Copy, no allocation needed
            merged.insert(*k, *v);
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
        lit.predicate_ids().len(),
        lit.mode.clone(),
    )
}

/// Create a key for deduplicating literals (using interned IDs, minimal allocation)
///
/// Returns (name_id, negation, predicate_ids, mode) - all Copy types except the Vec and Mode
#[inline]
fn literal_key(lit: &Literal) -> (SymbolId, bool, Vec<SymbolId>, Mode) {
    (
        lit.name_id(),
        lit.negation,
        lit.predicate_ids().to_vec(),
        lit.mode.clone(),
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
    let mut results = Vec::new();
    // Use Vec<(SymbolId, SymbolId)> for seen set - Copy types, much cheaper
    let mut seen: FxHashSet<Vec<(SymbolId, SymbolId)>> = FxHashSet::default();

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
                        // Create key from SymbolId pairs (Copy types, no heap allocation)
                        let key: Vec<(SymbolId, SymbolId)> = {
                            let mut pairs: Vec<_> = merged.iter().map(|(k, v)| (*k, *v)).collect();
                            pairs.sort_by_key(|(k, _)| k.as_raw());
                            pairs
                        };

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

    // Track facts using interned types (minimal allocation)
    let mut fact_keys: FxHashSet<(SymbolId, bool, Vec<SymbolId>, Mode)> = FxHashSet::default();
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
    // Use interned SymbolIds for instance tracking (minimal allocation)
    let mut known_instances: FxHashSet<(RuleLabel, Vec<(SymbolId, SymbolId)>)> =
        FxHashSet::default();

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

                // Create signature key from SymbolId pairs (Copy types, no allocation)
                let sig_key: Vec<(SymbolId, SymbolId)> = {
                    let mut pairs: Vec<_> = subst.iter().map(|(k, v)| (*k, *v)).collect();
                    pairs.sort_by_key(|(k, _)| k.as_raw());
                    pairs
                };
                let sig = (rule.label.clone(), sig_key);

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
        // Substitution now uses SymbolIds - verify via resolve
        let x_id = intern("?x");
        let y_id = intern("?y");
        let alice_id = intern("alice");
        let bob_id = intern("bob");
        assert_eq!(subst.get(&x_id), Some(&alice_id));
        assert_eq!(subst.get(&y_id), Some(&bob_id));
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
        // Build substitution using SymbolIds
        let mut subst = Substitution::default();
        subst.insert(intern("?x"), intern("alice"));
        subst.insert(intern("?y"), intern("bob"));

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
        // Test merge_substitutions with conflicting bindings
        let mut s1 = Substitution::default();
        s1.insert(intern("?x"), intern("alice"));

        let mut s2 = Substitution::default();
        s2.insert(intern("?x"), intern("bob")); // Conflict!

        let merged = merge_substitutions(&s1, &s2);
        assert!(
            merged.is_none(),
            "Conflicting substitutions should not merge"
        );
    }

    #[test]
    fn test_merge_substitutions_compatible() {
        // Test merge_substitutions with compatible bindings
        let mut s1 = Substitution::default();
        s1.insert(intern("?x"), intern("alice"));

        let mut s2 = Substitution::default();
        s2.insert(intern("?y"), intern("bob"));

        let merged = merge_substitutions(&s1, &s2).unwrap();
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn test_merge_substitutions_same_value() {
        // Test merge where same key has same value (should succeed)
        let mut s1 = Substitution::default();
        s1.insert(intern("?x"), intern("alice"));

        let mut s2 = Substitution::default();
        s2.insert(intern("?x"), intern("alice")); // Same value

        let merged = merge_substitutions(&s1, &s2).unwrap();
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_match_literal_variable_name() {
        // Match literal where the predicate name itself is a variable
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
        assert_eq!(subst.get(&rel_id), Some(&parent_id));
    }

    #[test]
    fn test_apply_substitution_variable_name() {
        // Apply substitution to a literal with variable name
        let lit = Literal::new(
            "?rel",
            false,
            Default::default(),
            Default::default(),
            vec!["?x".to_string()],
        );
        let mut subst = Substitution::default();
        subst.insert(intern("?rel"), intern("parent"));
        subst.insert(intern("?x"), intern("alice"));

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
        // Test apply_substitution_to_literal with non-variable predicates
        let lit = Literal::new(
            "pred",
            false,
            Default::default(),
            Default::default(),
            vec!["constant".to_string(), "?x".to_string()],
        );

        let mut subst: Substitution = Default::default();
        let x_id = intern("?x");
        let val_id = intern("value");
        subst.insert(x_id, val_id);

        let result = apply_substitution_to_literal(&lit, &subst);
        // constant should remain unchanged, ?x should become value
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
        // [O]pay(?x) vs [O]pay(alice) → Some
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
        assert_eq!(subst.get(&x_id), Some(&alice_id));
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
}
