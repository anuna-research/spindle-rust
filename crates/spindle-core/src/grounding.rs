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

use std::collections::{HashMap, HashSet};

use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel, RuleType};
use crate::theory::Theory;

/// Type alias for substitution (variable -> value)
pub type Substitution = HashMap<String, String>;

/// Check if a term is a variable (starts with ?)
pub fn is_variable(term: &str) -> bool {
    term.starts_with('?')
}

/// Check if a literal contains any variables
pub fn literal_has_variables(lit: &Literal) -> bool {
    is_variable(&lit.name) || lit.predicates.iter().any(|p| is_variable(p))
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

    let mut subst = Substitution::new();

    // Match name
    if is_variable(&pattern.name) {
        subst.insert(pattern.name.clone(), ground.name.clone());
    } else if pattern.name != ground.name {
        return None;
    }

    // Match predicates/arguments
    if pattern.predicates.len() != ground.predicates.len() {
        return None;
    }

    for (parg, garg) in pattern.predicates.iter().zip(ground.predicates.iter()) {
        if is_variable(parg) {
            // Check for conflicting bindings
            if let Some(existing) = subst.get(parg) {
                if existing != garg {
                    return None;
                }
            } else {
                subst.insert(parg.clone(), garg.clone());
            }
        } else if parg != garg {
            return None;
        }
    }

    Some(subst)
}

/// Apply a substitution to a literal
pub fn apply_substitution_to_literal(lit: &Literal, subst: &Substitution) -> Literal {
    let new_name = if is_variable(&lit.name) {
        subst.get(&lit.name).cloned().unwrap_or_else(|| lit.name.clone())
    } else {
        lit.name.clone()
    };

    let new_predicates: Vec<String> = lit
        .predicates
        .iter()
        .map(|p| {
            if is_variable(p) {
                subst.get(p).cloned().unwrap_or_else(|| p.clone())
            } else {
                p.clone()
            }
        })
        .collect();

    Literal::new(
        new_name,
        lit.negation,
        lit.mode.clone(),
        lit.temporal.clone(),
        new_predicates,
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

    Rule::new(new_label, rule.rule_type, new_body, new_head)
}

/// Merge two substitutions, returning None if they conflict
fn merge_substitutions(s1: &Substitution, s2: &Substitution) -> Option<Substitution> {
    let mut merged = s1.clone();
    for (k, v) in s2 {
        if let Some(existing) = merged.get(k) {
            if existing != v {
                return None;
            }
        } else {
            merged.insert(k.clone(), v.clone());
        }
    }
    Some(merged)
}

/// Create a key for indexing facts
fn fact_index_key(lit: &Literal) -> (String, bool, usize) {
    (lit.name.clone(), lit.negation, lit.predicates.len())
}

/// Create a key for deduplicating literals
fn literal_key(lit: &Literal) -> (String, bool, Vec<String>) {
    (lit.name.clone(), lit.negation, lit.predicates.clone())
}

/// Match body literals against facts, returning all valid substitutions
fn match_body_against_facts(
    body: &[Literal],
    fact_index: &HashMap<(String, bool, usize), Vec<Literal>>,
    all_facts: &[Literal],
) -> Vec<Substitution> {
    if body.is_empty() {
        return vec![Substitution::new()];
    }

    let first_lit = &body[0];
    let rest = &body[1..];

    // Get candidate facts
    let candidates: Vec<&Literal> = if is_variable(&first_lit.name) {
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
    fact_index: &HashMap<(String, bool, usize), Vec<Literal>>,
    delta_index: &HashMap<(String, bool, usize), Vec<Literal>>,
    all_facts: &[Literal],
    delta_facts: &[Literal],
) -> Vec<Substitution> {
    let mut results = Vec::new();
    let mut seen: HashSet<Vec<(String, String)>> = HashSet::new();

    for (i, delta_lit) in body.iter().enumerate() {
        let rest: Vec<Literal> = body
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != i)
            .map(|(_, l)| l.clone())
            .collect();

        let delta_candidates: Vec<&Literal> = if is_variable(&delta_lit.name) {
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
                        let key: Vec<(String, String)> = {
                            let mut pairs: Vec<_> = merged.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                            pairs.sort();
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
    ground_theory_with_limit(theory, 100)
}

/// Ground a theory with a maximum iteration limit
pub fn ground_theory_with_limit(theory: &Theory, max_iterations: usize) -> Theory {
    // Separate ground rules from rules with variables
    let (ground_rules, var_rules): (Vec<_>, Vec<_>) =
        theory.rules().partition(|r| !has_variables(r));

    // If no rules with variables, return as-is
    if var_rules.is_empty() {
        return theory.clone();
    }

    // Track facts
    let mut fact_keys: HashSet<(String, bool, Vec<String>)> = HashSet::new();
    let mut facts_list: Vec<Literal> = Vec::new();
    let mut fact_index: HashMap<(String, bool, usize), Vec<Literal>> = HashMap::new();

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
    let mut known_instances: HashSet<(RuleLabel, Vec<(String, String)>)> = HashSet::new();

    // Iterate until fixpoint
    let mut facts_new = facts_list.clone();

    for iteration in 0..max_iterations {
        if iteration >= max_iterations {
            panic!("Max iterations ({}) reached, possible infinite loop", max_iterations);
        }

        let mut new_facts_this_round: Vec<Literal> = Vec::new();
        let mut new_rules_this_round: Vec<Rule> = Vec::new();

        // Build delta index
        let mut delta_index: HashMap<(String, bool, usize), Vec<Literal>> = HashMap::new();
        for lit in &facts_new {
            delta_index
                .entry(fact_index_key(lit))
                .or_default()
                .push(lit.clone());
        }

        // For each rule with variables
        for rule in &var_rules {
            let substitutions = match_body_with_delta(
                &rule.body,
                &fact_index,
                &delta_index,
                &facts_list,
                &facts_new,
            );

            for subst in substitutions {
                let sig_key: Vec<(String, String)> = {
                    let mut pairs: Vec<_> = subst.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    pairs.sort();
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

        if new_facts_this_round.is_empty() {
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

    grounded
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
        assert_eq!(subst.get("?x"), Some(&"alice".to_string()));
        assert_eq!(subst.get("?y"), Some(&"bob".to_string()));
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
        let mut subst = Substitution::new();
        subst.insert("?x".to_string(), "alice".to_string());
        subst.insert("?y".to_string(), "bob".to_string());

        let result = apply_substitution_to_literal(&lit, &subst);
        assert_eq!(result.predicates, vec!["alice".to_string(), "bob".to_string()]);
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
                    h.name == "ancestor"
                        && h.predicates == vec!["alice".to_string(), "bob".to_string()]
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
                    .any(|h| h.name == "r" && h.predicates == vec!["a".to_string()])
        });
        assert!(has_r2_grounded, "Grounding should produce r2 instance q(a) => r(a)");
    }
}
