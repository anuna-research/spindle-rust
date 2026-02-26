//! Validate pipeline stage -- checks wildcard placement and range restriction.

use super::{Diagnostic, PipelineContext, PipelineStage, Severity};
use crate::arith::ArithConstraint;
use crate::body::BodyLiteral;
use crate::error::{Result, SpindleError};
use crate::grounding::is_variable;
use crate::intern::resolve;
use crate::theory::Theory;
use std::collections::HashSet;

/// Validates wildcard placement and range restriction.
#[derive(Debug, Clone)]
pub struct Validate {
    /// Whether to enforce range restriction (head variables must appear in body).
    pub enforce_range_restricted: bool,
    /// Whether to reject wildcard `_` in rule heads.
    pub reject_wildcard_in_head: bool,
}

impl Default for Validate {
    fn default() -> Self {
        Self {
            enforce_range_restricted: true,
            reject_wildcard_in_head: true,
        }
    }
}

impl PipelineStage for Validate {
    fn name(&self) -> &'static str {
        "validate"
    }

    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        if self.reject_wildcard_in_head {
            validate_wildcards(&theory)?;
        }
        if self.enforce_range_restricted {
            validate_range_restriction(&theory)?;
        }
        ctx.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            stage: self.name(),
            message: "validation passed".into(),
        });
        Ok(theory)
    }
}

fn validate_wildcards(theory: &Theory) -> Result<()> {
    for rule in theory.rules() {
        for head in &rule.head {
            if head.name() == "_" || head.predicates().iter().any(|p| p == "_") {
                return Err(SpindleError::Validation {
                    message: format!("Wildcard '_' found in rule head: {}", rule.label),
                });
            }
        }
    }
    Ok(())
}

fn validate_range_restriction(theory: &Theory) -> Result<()> {
    for rule in theory.rules() {
        // Collect body variables
        let mut body_vars = HashSet::new();
        for lit in &rule.body {
            if is_variable(lit.name()) {
                body_vars.insert(lit.name().to_string());
            }
            for pred in lit.predicates() {
                if is_variable(&pred) {
                    body_vars.insert(pred);
                }
            }
            // Variables introduced by bind constraints are also body variables.
            if let BodyLiteral::Arithmetic(ArithConstraint::Bind { var, .. }) = lit {
                let var_name = resolve(*var);
                if is_variable(var_name) {
                    body_vars.insert(var_name.to_string());
                }
            }
        }

        // Check head variables
        for lit in &rule.head {
            if is_variable(lit.name()) && !body_vars.contains(lit.name()) {
                return Err(SpindleError::Validation {
                    message: format!(
                        "Unsafe rule '{}': variable {} in head but not in body",
                        rule.label,
                        lit.name()
                    ),
                });
            }
            for pred in lit.predicates() {
                if is_variable(&pred) && !body_vars.contains(&pred) {
                    return Err(SpindleError::Validation {
                        message: format!(
                            "Unsafe rule '{}': variable {} in head but not in body",
                            rule.label, pred
                        ),
                    });
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal::Literal;
    use crate::mode::Mode;
    use crate::rule::Rule;
    use crate::temporal::Temporal;

    // -----------------------------------------------------------------------
    // Wildcard in head tests
    // -----------------------------------------------------------------------

    #[test]
    fn wildcard_in_head_name_rejected() {
        let mut theory = Theory::new();
        // Rule: a => _  (wildcard as the head literal name)
        let head = Literal::simple("_");
        let body = vec![Literal::simple("a")];
        let rule = Rule::defeasible("r1", body, head);
        theory.add_rule(rule);

        let err = validate_wildcards(&theory).unwrap_err();
        match err {
            SpindleError::Validation { message } => {
                assert!(
                    message.contains("Wildcard '_' found in rule head"),
                    "unexpected message: {message}"
                );
                assert!(
                    message.contains("r1"),
                    "message should mention rule label: {message}"
                );
            }
            other => panic!("expected Validation error, got: {other:?}"),
        }
    }

    #[test]
    fn wildcard_in_head_predicate_rejected() {
        let mut theory = Theory::new();
        // Rule: a => foo(_)  (wildcard as a predicate argument in head)
        let head = Literal::new(
            "foo",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["_".into()],
        );
        let body = vec![Literal::simple("a")];
        let rule = Rule::defeasible("r2", body, head);
        theory.add_rule(rule);

        let err = validate_wildcards(&theory).unwrap_err();
        match err {
            SpindleError::Validation { message } => {
                assert!(
                    message.contains("Wildcard '_' found in rule head"),
                    "unexpected message: {message}"
                );
                assert!(
                    message.contains("r2"),
                    "message should mention rule label: {message}"
                );
            }
            other => panic!("expected Validation error, got: {other:?}"),
        }
    }

    #[test]
    fn no_wildcard_in_head_passes() {
        let mut theory = Theory::new();
        let head = Literal::simple("b");
        let body = vec![Literal::simple("a")];
        let rule = Rule::defeasible("r3", body, head);
        theory.add_rule(rule);

        assert!(validate_wildcards(&theory).is_ok());
    }

    #[test]
    fn wildcard_in_body_is_fine() {
        let mut theory = Theory::new();
        // Rule: _ => b  (wildcard in body, not in head -- body is not checked)
        let head = Literal::simple("b");
        let body = vec![Literal::simple("_")];
        let rule = Rule::defeasible("r4", body, head);
        theory.add_rule(rule);

        assert!(validate_wildcards(&theory).is_ok());
    }

    // -----------------------------------------------------------------------
    // Range restriction (unsafe variable) tests
    // -----------------------------------------------------------------------

    #[test]
    fn unsafe_variable_in_head_name_rejected() {
        let mut theory = Theory::new();
        // Rule: a => ?X  (variable ?X as head name, not appearing in body)
        let head = Literal::simple("?X");
        let body = vec![Literal::simple("a")];
        let rule = Rule::defeasible("r5", body, head);
        theory.add_rule(rule);

        let err = validate_range_restriction(&theory).unwrap_err();
        match err {
            SpindleError::Validation { message } => {
                assert!(
                    message.contains("Unsafe rule 'r5'"),
                    "unexpected message: {message}"
                );
                assert!(
                    message.contains("?X"),
                    "message should mention the variable: {message}"
                );
            }
            other => panic!("expected Validation error, got: {other:?}"),
        }
    }

    #[test]
    fn unsafe_variable_in_head_predicate_rejected() {
        let mut theory = Theory::new();
        // Rule: a => foo(?Y)  (variable ?Y in head predicate, not in body)
        let head = Literal::new(
            "foo",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["?Y".into()],
        );
        let body = vec![Literal::simple("a")];
        let rule = Rule::defeasible("r6", body, head);
        theory.add_rule(rule);

        let err = validate_range_restriction(&theory).unwrap_err();
        match err {
            SpindleError::Validation { message } => {
                assert!(
                    message.contains("Unsafe rule 'r6'"),
                    "unexpected message: {message}"
                );
                assert!(
                    message.contains("?Y"),
                    "message should mention the variable: {message}"
                );
            }
            other => panic!("expected Validation error, got: {other:?}"),
        }
    }

    #[test]
    fn safe_variable_in_head_name_passes() {
        let mut theory = Theory::new();
        // Rule: ?X => ?X  (variable in head also in body name)
        let head = Literal::simple("?X");
        let body = vec![Literal::simple("?X")];
        let rule = Rule::defeasible("r7", body, head);
        theory.add_rule(rule);

        assert!(validate_range_restriction(&theory).is_ok());
    }

    #[test]
    fn safe_variable_in_head_predicate_passes() {
        let mut theory = Theory::new();
        // Rule: bar(?Z) => foo(?Z)  (variable in head predicate also in body predicate)
        let head = Literal::new(
            "foo",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["?Z".into()],
        );
        let body = vec![Literal::new(
            "bar",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["?Z".into()],
        )];
        let rule = Rule::defeasible("r8", body, head);
        theory.add_rule(rule);

        assert!(validate_range_restriction(&theory).is_ok());
    }

    #[test]
    fn safe_variable_from_body_predicate_to_head_name() {
        let mut theory = Theory::new();
        // Rule: bar(?X) => ?X  (variable in head name appears in body predicate)
        let head = Literal::simple("?X");
        let body = vec![Literal::new(
            "bar",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["?X".into()],
        )];
        let rule = Rule::defeasible("r9", body, head);
        theory.add_rule(rule);

        assert!(validate_range_restriction(&theory).is_ok());
    }

    #[test]
    fn no_variables_passes() {
        let mut theory = Theory::new();
        // Rule: a => b  (no variables at all)
        let head = Literal::simple("b");
        let body = vec![Literal::simple("a")];
        let rule = Rule::defeasible("r10", body, head);
        theory.add_rule(rule);

        assert!(validate_range_restriction(&theory).is_ok());
    }

    #[test]
    fn fact_with_no_body_no_variables_passes() {
        let mut theory = Theory::new();
        theory.add_fact("a");

        assert!(validate_range_restriction(&theory).is_ok());
    }

    // -----------------------------------------------------------------------
    // PipelineStage integration tests
    // -----------------------------------------------------------------------

    #[test]
    fn validate_stage_rejects_wildcard_in_head() {
        let mut theory = Theory::new();
        let head = Literal::simple("_");
        let body = vec![Literal::simple("a")];
        let rule = Rule::defeasible("r_wc", body, head);
        theory.add_rule(rule);

        let stage = Validate::default();
        let mut ctx = PipelineContext::default();
        let result = stage.apply(theory, &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn validate_stage_rejects_unsafe_variable() {
        let mut theory = Theory::new();
        let head = Literal::simple("?X");
        let body = vec![Literal::simple("a")];
        let rule = Rule::defeasible("r_unsafe", body, head);
        theory.add_rule(rule);

        let stage = Validate::default();
        let mut ctx = PipelineContext::default();
        let result = stage.apply(theory, &mut ctx);
        assert!(result.is_err());
    }

    #[test]
    fn validate_stage_passes_valid_theory() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");

        let stage = Validate::default();
        let mut ctx = PipelineContext::default();
        let result = stage.apply(theory, &mut ctx);
        assert!(result.is_ok());
        assert_eq!(ctx.diagnostics.len(), 1);
        assert_eq!(ctx.diagnostics[0].message, "validation passed");
    }

    #[test]
    fn validate_stage_skips_checks_when_disabled() {
        let mut theory = Theory::new();
        // Add wildcard in head AND unsafe variable -- both should be ignored
        let head_wc = Literal::simple("_");
        let body_wc = vec![Literal::simple("a")];
        theory.add_rule(Rule::defeasible("r_wc", body_wc, head_wc));

        let head_unsafe = Literal::simple("?X");
        let body_unsafe = vec![Literal::simple("a")];
        theory.add_rule(Rule::defeasible("r_unsafe", body_unsafe, head_unsafe));

        let stage = Validate {
            enforce_range_restricted: false,
            reject_wildcard_in_head: false,
        };
        let mut ctx = PipelineContext::default();
        let result = stage.apply(theory, &mut ctx);
        assert!(result.is_ok());
        assert_eq!(ctx.diagnostics.len(), 1);
        assert_eq!(ctx.diagnostics[0].message, "validation passed");
    }
}
