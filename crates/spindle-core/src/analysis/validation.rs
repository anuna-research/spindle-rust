//! Semantic validation for defeasible logic theories.
//!
//! Scans a set of rules and produces [`ValidationDiagnostic`]s for
//! suspicious or incorrect patterns.  The main entry point is
//! [`validate_theory`], which accepts an iterator of `&Rule` so
//! callers can pass `&[Rule]`, `Vec<Rule>`, or a filtered iterator
//! without constructing the full mining pipeline.
//!
//! # Current checks
//!
//! | Code  | Severity | Description                                        |
//! |-------|----------|----------------------------------------------------|
//! | W001  | Warning  | Non-fact rule with empty body (never fires)         |
//! | W002  | Warning  | Tautological body (contains `p` and `~p`)           |
//! | W003  | Warning  | Defeasible rule shadowed by strict rule (same head) |
//! | W004  | Warning  | Unreachable rule (body literal never produced)      |
//! | E001  | Error    | Duplicate rule labels                               |

use std::collections::{HashMap, HashSet};

use super::{Severity, ValidationDiagnostic};
use crate::rule::{Rule, RuleType};

/// Validate a set of rules and produce diagnostics.
///
/// Accepts `impl IntoIterator<Item = &Rule>` so callers can pass
/// `&[Rule]`, `theory.rules()`, or any filtered iterator.
///
/// # Example
///
/// ```rust
/// use spindle_core::analysis::validation::validate_theory;
/// use spindle_core::rule::{Rule, RuleType};
/// use spindle_core::literal::Literal;
///
/// let rules = vec![
///     Rule::fact("f1", Literal::simple("bird")),
///     Rule::defeasible("r1", vec![Literal::simple("bird")], Literal::simple("flies")),
/// ];
/// let diags = validate_theory(&rules);
/// // No diagnostics for a well-formed theory.
/// assert!(diags.is_empty());
/// ```
pub fn validate_theory<'a, I>(rules: I) -> Vec<ValidationDiagnostic>
where
    I: IntoIterator<Item = &'a Rule>,
{
    let rules: Vec<&Rule> = rules.into_iter().collect();
    let mut diagnostics = Vec::new();

    check_duplicate_labels(&rules, &mut diagnostics);
    check_empty_body_non_facts(&rules, &mut diagnostics);
    check_tautological_bodies(&rules, &mut diagnostics);
    check_shadowed_rules(&rules, &mut diagnostics);
    check_unreachable_rules(&rules, &mut diagnostics);

    diagnostics
}

// ---------------------------------------------------------------------------
// E001: Duplicate rule labels
// ---------------------------------------------------------------------------

fn check_duplicate_labels(rules: &[&Rule], diags: &mut Vec<ValidationDiagnostic>) {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for rule in rules {
        let count = seen.entry(&rule.label).or_insert(0);
        *count += 1;
    }
    for (label, count) in &seen {
        if *count > 1 {
            diags.push(ValidationDiagnostic {
                severity: Severity::Error,
                code: "E001",
                message: format!(
                    "Duplicate rule label '{}' appears {} times; \
                     each rule must have a unique label.",
                    label, count,
                ),
                rules: vec![label.to_string()],
            });
        }
    }
}

// ---------------------------------------------------------------------------
// W001: Empty-body non-fact rules
// ---------------------------------------------------------------------------

fn check_empty_body_non_facts(rules: &[&Rule], diags: &mut Vec<ValidationDiagnostic>) {
    for rule in rules {
        if rule.body.is_empty() && rule.rule_type != RuleType::Fact {
            diags.push(ValidationDiagnostic {
                severity: Severity::Warning,
                code: "W001",
                message: format!(
                    "Rule '{}' has an empty body but is not a fact; \
                     it will never fire in the standard reasoner.",
                    rule.label,
                ),
                rules: vec![rule.label.clone()],
            });
        }
    }
}

// ---------------------------------------------------------------------------
// W002: Tautological bodies (body contains p and ~p)
// ---------------------------------------------------------------------------

fn check_tautological_bodies(rules: &[&Rule], diags: &mut Vec<ValidationDiagnostic>) {
    for rule in rules {
        if rule.body.len() < 2 {
            continue;
        }

        // Collect (name, negation) pairs from the body.
        let mut positive_names: HashSet<&str> = HashSet::new();
        let mut negative_names: HashSet<&str> = HashSet::new();

        for lit in &rule.body {
            if lit.is_negated() {
                negative_names.insert(lit.name());
            } else {
                positive_names.insert(lit.name());
            }
        }

        // If any name appears both positive and negative, the body is
        // unsatisfiable.
        for name in &positive_names {
            if negative_names.contains(name) {
                diags.push(ValidationDiagnostic {
                    severity: Severity::Warning,
                    code: "W002",
                    message: format!(
                        "Rule '{}' has a tautological body: both '{}' and '~{}' \
                         appear, so the rule can never fire.",
                        rule.label, name, name,
                    ),
                    rules: vec![rule.label.clone()],
                });
                break; // one diagnostic per rule is enough
            }
        }
    }
}

// ---------------------------------------------------------------------------
// W003: Defeasible rule shadowed by a strict rule with the same head
// ---------------------------------------------------------------------------

fn check_shadowed_rules(rules: &[&Rule], diags: &mut Vec<ValidationDiagnostic>) {
    // Collect all head literals proved by strict rules or facts.
    let strict_heads: HashSet<(&str, bool)> = rules
        .iter()
        .filter(|r| matches!(r.rule_type, RuleType::Strict | RuleType::Fact))
        .flat_map(|r| r.head.iter().map(|h| (h.name(), h.is_negated())))
        .collect();

    for rule in rules {
        if rule.rule_type != RuleType::Defeasible {
            continue;
        }
        for h in &rule.head {
            let key = (h.name(), h.is_negated());
            if strict_heads.contains(&key) {
                let display = if h.is_negated() {
                    format!("~{}", h.name())
                } else {
                    h.name().to_string()
                };
                diags.push(ValidationDiagnostic {
                    severity: Severity::Warning,
                    code: "W003",
                    message: format!(
                        "Defeasible rule '{}' concludes '{}', which is already \
                         strictly proved; the defeasible rule is redundant.",
                        rule.label, display,
                    ),
                    rules: vec![rule.label.clone()],
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// W004: Unreachable rules (body literal never produced by any rule)
// ---------------------------------------------------------------------------

fn check_unreachable_rules(rules: &[&Rule], diags: &mut Vec<ValidationDiagnostic>) {
    // Collect all head literals (name, negated) across the whole theory.
    let produced: HashSet<(&str, bool)> = rules
        .iter()
        .flat_map(|r| r.head.iter().map(|h| (h.name(), h.is_negated())))
        .collect();

    for rule in rules {
        if rule.body.is_empty() {
            continue; // facts / empty-body rules checked elsewhere
        }
        for lit in &rule.body {
            let key = (lit.name(), lit.is_negated());
            if !produced.contains(&key) {
                let display = if lit.is_negated() {
                    format!("~{}", lit.name())
                } else {
                    lit.name().to_string()
                };
                diags.push(ValidationDiagnostic {
                    severity: Severity::Warning,
                    code: "W004",
                    message: format!(
                        "Rule '{}' requires '{}' in its body, but no rule \
                         in the theory produces it; the rule is unreachable.",
                        rule.label, display,
                    ),
                    rules: vec![rule.label.clone()],
                });
                break; // one diagnostic per rule is enough
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal::Literal;

    // Helper to collect codes from diagnostics.
    fn codes(diags: &[ValidationDiagnostic]) -> Vec<&'static str> {
        diags.iter().map(|d| d.code).collect()
    }

    #[test]
    fn well_formed_theory_produces_no_diagnostics() {
        let rules = vec![
            Rule::fact("f1", Literal::simple("bird")),
            Rule::defeasible(
                "r1",
                vec![Literal::simple("bird")],
                Literal::simple("flies"),
            ),
        ];
        let diags = validate_theory(&rules);
        assert!(
            diags.is_empty(),
            "expected no diagnostics, got: {:?}",
            diags
        );
    }

    #[test]
    fn w001_empty_body_non_fact() {
        let rules = vec![Rule::defeasible(
            "r1",
            Vec::<Literal>::new(),
            Literal::simple("flies"),
        )];
        let diags = validate_theory(&rules);
        assert!(codes(&diags).contains(&"W001"));
    }

    #[test]
    fn w001_fact_is_allowed() {
        let rules = vec![Rule::fact("f1", Literal::simple("bird"))];
        let diags = validate_theory(&rules);
        assert!(!codes(&diags).contains(&"W001"));
    }

    #[test]
    fn w002_tautological_body() {
        let rules = vec![Rule::defeasible(
            "r1",
            vec![Literal::simple("p"), Literal::negated("p")],
            Literal::simple("q"),
        )];
        let diags = validate_theory(&rules);
        assert!(codes(&diags).contains(&"W002"));
    }

    #[test]
    fn w003_shadowed_by_strict() {
        let rules = vec![
            Rule::strict("s1", vec![Literal::simple("a")], Literal::simple("b")),
            Rule::defeasible("r1", vec![Literal::simple("a")], Literal::simple("b")),
        ];
        let diags = validate_theory(&rules);
        assert!(codes(&diags).contains(&"W003"));
    }

    #[test]
    fn w003_different_polarity_is_not_shadowed() {
        let rules = vec![
            Rule::strict("s1", vec![Literal::simple("a")], Literal::simple("b")),
            Rule::defeasible("r1", vec![Literal::simple("a")], Literal::negated("b")),
        ];
        let diags = validate_theory(&rules);
        // ~b is not shadowed by b
        assert!(!codes(&diags).contains(&"W003"));
    }

    #[test]
    fn w004_unreachable_body_literal() {
        let rules = vec![Rule::defeasible(
            "r1",
            vec![Literal::simple("never_produced")],
            Literal::simple("q"),
        )];
        let diags = validate_theory(&rules);
        assert!(codes(&diags).contains(&"W004"));
    }

    #[test]
    fn w004_reachable_body_literal() {
        let rules = vec![
            Rule::fact("f1", Literal::simple("bird")),
            Rule::defeasible(
                "r1",
                vec![Literal::simple("bird")],
                Literal::simple("flies"),
            ),
        ];
        let diags = validate_theory(&rules);
        assert!(
            !codes(&diags).contains(&"W004"),
            "expected no W004, got: {:?}",
            diags
        );
    }

    #[test]
    fn e001_duplicate_labels() {
        let rules = vec![
            Rule::fact("dup", Literal::simple("a")),
            Rule::fact("dup", Literal::simple("b")),
        ];
        let diags = validate_theory(&rules);
        assert!(codes(&diags).contains(&"E001"));
    }

    #[test]
    fn display_impl_works() {
        let d = ValidationDiagnostic {
            severity: Severity::Warning,
            code: "W001",
            message: "test message".to_string(),
            rules: vec!["r1".to_string()],
        };
        let s = format!("{}", d);
        assert!(s.contains("Warning"));
        assert!(s.contains("W001"));
        assert!(s.contains("test message"));
    }

    #[test]
    fn severity_ordering() {
        assert!(Severity::Warning < Severity::Error);
    }
}
