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
//! | W002  | Warning  | Tautological body (contains `p` and `~p`)           |
//! | W003  | Warning  | Defeasible rule shadowed by strict rule (same head) |
//! | W004  | Warning  | Unreachable rule (body literal never produced)      |
//! | E001  | Error    | Duplicate rule labels                               |

use std::collections::{HashMap, HashSet};

use super::{Severity, ValidationDiagnostic, analysis_key, analysis_key_unsigned};
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
                    "Duplicate rule label '{label}' appears {count} times; \
                     each rule must have a unique label."
                ),
                rules: vec![label.to_string()],
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

        // A body is tautological only when it contains two literals that
        // are exact complements: same name, args, mode, and temporal but
        // opposite negation.  Comparing by name alone produces false
        // positives for bodies like `p(a), ~p(b)`.
        // Only logic body literals participate in tautology checks.
        let logic_lits: Vec<_> = rule
            .body
            .iter()
            .filter_map(|bl| bl.as_logic().map(|l| l.to_literal()))
            .collect();
        let positive: Vec<_> = logic_lits.iter().filter(|l| !l.is_negated()).collect();
        let negative: Vec<_> = logic_lits.iter().filter(|l| l.is_negated()).collect();

        'outer: for pos in &positive {
            let pos_key = analysis_key_unsigned(pos);
            for neg in &negative {
                if pos_key == analysis_key_unsigned(neg) {
                    diags.push(ValidationDiagnostic {
                        severity: Severity::Warning,
                        code: "W002",
                        message: format!(
                            "Rule '{}' has a tautological body: both '{}' and '~{}' \
                             appear, so the rule can never fire.",
                            rule.label,
                            pos.name(),
                            pos.name(),
                        ),
                        rules: vec![rule.label.clone()],
                    });
                    break 'outer; // one diagnostic per rule is enough
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// W003: Defeasible rule shadowed by a strict rule with the same head
// ---------------------------------------------------------------------------

fn check_shadowed_rules(rules: &[&Rule], diags: &mut Vec<ValidationDiagnostic>) {
    // Collect full identity keys for all head literals proved by strict
    // rules or facts.  Using the full key (name, negation, args, mode,
    // temporal) avoids false positives where `p(a)` strict-shadows `p(b)`.
    let strict_heads: HashSet<_> = rules
        .iter()
        .filter(|r| matches!(r.rule_type, RuleType::Strict | RuleType::Fact))
        .flat_map(|r| r.head.iter().map(analysis_key))
        .collect();

    for rule in rules {
        if rule.rule_type != RuleType::Defeasible {
            continue;
        }
        for h in &rule.head {
            if strict_heads.contains(&analysis_key(h)) {
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
    // Collect full identity keys for all produced head literals.
    // Using the full key (name, negation, args, mode, temporal) ensures
    // that `p(b)` being produced does not suppress a warning for `p(a)`.
    let produced: HashSet<_> = rules
        .iter()
        .flat_map(|r| r.head.iter().map(analysis_key))
        .collect();

    for rule in rules {
        if rule.body.is_empty() {
            continue; // facts / empty-body rules checked elsewhere
        }
        for bl in &rule.body {
            let lit = match bl.as_logic() {
                Some(l) => l.to_literal(),
                None => continue, // skip arithmetic constraints
            };
            if !produced.contains(&analysis_key(&lit)) {
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

    // ---------------------------------------------------------------
    // Predicate-argument discrimination tests
    // ---------------------------------------------------------------

    #[test]
    fn w002_different_args_not_tautological() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // Body: p(a), ~p(b) — different ground atoms, NOT tautological
        let pos = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".into()],
        );
        let neg = Literal::new(
            "p",
            true,
            Mode::empty(),
            Temporal::empty(),
            vec!["b".into()],
        );
        let rules = vec![Rule::defeasible("r1", vec![pos, neg], Literal::simple("q"))];
        let diags = validate_theory(&rules);
        assert!(
            !codes(&diags).contains(&"W002"),
            "p(a) and ~p(b) should not be flagged as tautological: {:?}",
            diags,
        );
    }

    #[test]
    fn w002_same_args_is_tautological() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // Body: p(a), ~p(a) — same ground atom, IS tautological
        let pos = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".into()],
        );
        let neg = Literal::new(
            "p",
            true,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".into()],
        );
        let rules = vec![Rule::defeasible("r1", vec![pos, neg], Literal::simple("q"))];
        let diags = validate_theory(&rules);
        assert!(codes(&diags).contains(&"W002"));
    }

    #[test]
    fn w002_different_modes_not_tautological() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // Body: O(p), ~p — different modalities, NOT tautological
        let pos = Literal::new("p", false, Mode::obligation(), Temporal::empty(), vec![]);
        let neg = Literal::new("p", true, Mode::empty(), Temporal::empty(), vec![]);
        let rules = vec![Rule::defeasible("r1", vec![pos, neg], Literal::simple("q"))];
        let diags = validate_theory(&rules);
        assert!(
            !codes(&diags).contains(&"W002"),
            "O(p) and ~p should not be flagged as tautological: {:?}",
            diags,
        );
    }

    #[test]
    fn w002_different_temporals_not_tautological() {
        use crate::mode::Mode;
        use crate::temporal::{Temporal, TimePoint};
        // Body: p@[0,10], ~p@[20,30] — different time scopes, NOT tautological
        let t1 = Temporal::new(TimePoint::Moment(0), TimePoint::Moment(10));
        let t2 = Temporal::new(TimePoint::Moment(20), TimePoint::Moment(30));
        let pos = Literal::new("p", false, Mode::empty(), t1, vec![]);
        let neg = Literal::new("p", true, Mode::empty(), t2, vec![]);
        let rules = vec![Rule::defeasible("r1", vec![pos, neg], Literal::simple("q"))];
        let diags = validate_theory(&rules);
        assert!(
            !codes(&diags).contains(&"W002"),
            "p@[0,10] and ~p@[20,30] should not be flagged as tautological: {:?}",
            diags,
        );
    }

    #[test]
    fn w003_different_args_not_shadowed() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // Strict p(a) should NOT shadow defeasible p(b)
        let head_a = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".into()],
        );
        let head_b = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["b".into()],
        );
        let rules = vec![
            Rule::strict("s1", vec![Literal::simple("x")], head_a),
            Rule::defeasible("r1", vec![Literal::simple("y")], head_b),
        ];
        let diags = validate_theory(&rules);
        assert!(
            !codes(&diags).contains(&"W003"),
            "strict p(a) should not shadow defeasible p(b): {:?}",
            diags,
        );
    }

    #[test]
    fn w003_same_args_is_shadowed() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // Strict p(a) SHOULD shadow defeasible p(a)
        let head_a = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".into()],
        );
        let head_b = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".into()],
        );
        let rules = vec![
            Rule::strict("s1", vec![Literal::simple("x")], head_a),
            Rule::defeasible("r1", vec![Literal::simple("x")], head_b),
        ];
        let diags = validate_theory(&rules);
        assert!(codes(&diags).contains(&"W003"));
    }

    #[test]
    fn w003_different_modes_not_shadowed() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // Strict O(p) should NOT shadow defeasible p (different modalities)
        let head_a = Literal::new("p", false, Mode::obligation(), Temporal::empty(), vec![]);
        let head_b = Literal::new("p", false, Mode::empty(), Temporal::empty(), vec![]);
        let rules = vec![
            Rule::strict("s1", vec![Literal::simple("x")], head_a),
            Rule::defeasible("r1", vec![Literal::simple("y")], head_b),
        ];
        let diags = validate_theory(&rules);
        assert!(
            !codes(&diags).contains(&"W003"),
            "strict O(p) should not shadow defeasible p: {:?}",
            diags,
        );
    }

    #[test]
    fn w004_different_args_is_unreachable() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // Only p(b) is produced; rule requiring p(a) SHOULD be unreachable
        let head = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["b".into()],
        );
        let body = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".into()],
        );
        let rules = vec![
            Rule::fact("f1", head),
            Rule::defeasible("r1", vec![body], Literal::simple("q")),
        ];
        let diags = validate_theory(&rules);
        assert!(
            codes(&diags).contains(&"W004"),
            "p(a) should be unreachable when only p(b) is produced: {:?}",
            diags,
        );
    }

    #[test]
    fn w004_same_args_is_reachable() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // p(a) is produced; rule requiring p(a) should NOT be flagged
        let head = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".into()],
        );
        let body = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".into()],
        );
        let rules = vec![
            Rule::fact("f1", head),
            Rule::defeasible("r1", vec![body], Literal::simple("q")),
        ];
        let diags = validate_theory(&rules);
        assert!(
            !codes(&diags).contains(&"W004"),
            "p(a) should be reachable when p(a) is produced: {:?}",
            diags,
        );
    }

    #[test]
    fn w004_different_modes_is_unreachable() {
        use crate::mode::Mode;
        use crate::temporal::Temporal;
        // Only O(p) is produced; rule requiring p (no mode) SHOULD be unreachable
        let head = Literal::new("p", false, Mode::obligation(), Temporal::empty(), vec![]);
        let body = Literal::new("p", false, Mode::empty(), Temporal::empty(), vec![]);
        let rules = vec![
            Rule::fact("f1", head),
            Rule::defeasible("r1", vec![body], Literal::simple("q")),
        ];
        let diags = validate_theory(&rules);
        assert!(
            codes(&diags).contains(&"W004"),
            "p should be unreachable when only O(p) is produced: {:?}",
            diags,
        );
    }

    // ---------------------------------------------------------------
    // Combined modal+temporal+args integration test
    // ---------------------------------------------------------------

    #[test]
    fn well_formed_modal_temporal_theory_produces_no_false_positives() {
        use crate::mode::Mode;
        use crate::temporal::{Temporal, TimePoint};
        // A theory using modes, temporals, and args that should produce
        // zero diagnostics.
        let t_morning = Temporal::new(TimePoint::Moment(0), TimePoint::Moment(12));
        let t_evening = Temporal::new(TimePoint::Moment(18), TimePoint::Moment(24));

        let rules = vec![
            // Fact: employed(alice, acme)
            Rule::fact(
                "f1",
                Literal::new(
                    "employed",
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    vec!["alice".into(), "acme".into()],
                ),
            ),
            // Fact: employed(bob, globex)
            Rule::fact(
                "f2",
                Literal::new(
                    "employed",
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    vec!["bob".into(), "globex".into()],
                ),
            ),
            // O(pay(acme, alice)) if employed(alice, acme)
            Rule::defeasible(
                "r1",
                vec![Literal::new(
                    "employed",
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    vec!["alice".into(), "acme".into()],
                )],
                Literal::new(
                    "pay",
                    false,
                    Mode::obligation(),
                    Temporal::empty(),
                    vec!["acme".into(), "alice".into()],
                ),
            ),
            // ~O(pay(acme, alice))@evening if complaint(alice)@morning
            // This should NOT conflict with r1 (different temporal)
            Rule::defeasible(
                "r2",
                vec![Literal::new(
                    "complaint",
                    false,
                    Mode::empty(),
                    t_morning,
                    vec!["alice".into()],
                )],
                Literal::new(
                    "pay",
                    true,
                    Mode::obligation(),
                    t_evening,
                    vec!["acme".into(), "alice".into()],
                ),
            ),
        ];
        let diags = validate_theory(&rules);
        // Should have W004 for complaint(alice)@morning (not produced) but
        // should NOT have W002 or W003 false positives.
        let diag_codes = codes(&diags);
        assert!(
            !diag_codes.contains(&"W002"),
            "no tautology false positives expected: {:?}",
            diags,
        );
        assert!(
            !diag_codes.contains(&"W003"),
            "no shadowing false positives expected: {:?}",
            diags,
        );
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
