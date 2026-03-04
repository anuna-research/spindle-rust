//! Validate pipeline stage -- checks wildcard placement and range restriction.

use super::{Diagnostic, PipelineContext, PipelineStage, Severity};
use crate::arith::{ArithConstraint, ArithExpr};
use crate::body::BodyLiteral;
use crate::error::{Result, SpindleError};
use crate::function_registry::FunctionRegistry;
use crate::grounding::is_variable;
use crate::intern::{SymbolId, resolve};
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

    fn apply(&self, mut theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        if self.reject_wildcard_in_head {
            validate_wildcards(&theory)?;
        }
        if self.enforce_range_restricted {
            validate_range_restriction(&theory)?;
        }
        validate_extension_calls(&theory, ctx.function_registry.as_ref())?;
        compute_fold_grouping(&mut theory);
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
            // Variables introduced by fold are also body variables:
            // - result_var (like bind)
            // - variables from the fold pattern that scope the aggregation
            if let BodyLiteral::Fold(fold) = lit {
                let var_name = resolve(fold.result_var);
                if is_variable(var_name) {
                    body_vars.insert(var_name.to_string());
                }
                // Fold pattern variables are body-introduced (they are iterated over)
                if is_variable(fold.pattern.name()) {
                    body_vars.insert(fold.pattern.name().to_string());
                }
                for pred in fold.pattern.predicates() {
                    if is_variable(&pred) {
                        body_vars.insert(pred);
                    }
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

/// Compute `grouping_vars` for every fold in the theory.
///
/// A fold's grouping variables are pattern variables that also appear
/// outside the fold — in the rule head or other body literals. These
/// variables partition the matched facts into groups, producing one
/// fold result per distinct combination of grouping variable values.
fn compute_fold_grouping(theory: &mut Theory) {
    let labels: Vec<_> = theory.rules().map(|r| r.label.clone()).collect();
    for label in &labels {
        let rule = theory.get_rule(label).unwrap();

        // Quick check: does this rule have any folds?
        let has_fold = rule.body.iter().any(|bl| bl.is_fold());
        if !has_fold {
            continue;
        }

        // Collect variables from head
        let mut outer_vars: HashSet<SymbolId> = HashSet::new();
        for head in &rule.head {
            collect_literal_vars(head, &mut outer_vars);
        }

        // Collect variables from non-fold body literals
        for bl in &rule.body {
            match bl {
                BodyLiteral::Logic(lit) => {
                    collect_body_logic_vars(lit, &mut outer_vars);
                }
                BodyLiteral::Arithmetic(ArithConstraint::Bind { var, .. }) => {
                    outer_vars.insert(*var);
                }
                BodyLiteral::Arithmetic(ArithConstraint::Compare { .. }) => {}
                BodyLiteral::Fold(_) => {
                    // Don't count other folds as "outer" context
                }
            }
        }

        // Now compute grouping_vars for each fold
        // We need to collect fold indices first, then mutate
        let fold_groupings: Vec<(usize, Vec<SymbolId>)> = rule
            .body
            .iter()
            .enumerate()
            .filter_map(|(i, bl)| {
                if let BodyLiteral::Fold(fold) = bl {
                    let mut pattern_vars: Vec<SymbolId> = Vec::new();
                    // Collect pattern name if it's a variable
                    let name = fold.pattern.name();
                    if is_variable(name) {
                        pattern_vars.push(fold.pattern.name_id());
                    }
                    // Collect pattern argument variables
                    for arg in fold.pattern.predicate_args() {
                        if let crate::body::BodyArg::Term(crate::term::Term::Symbol(id)) = arg
                            && is_variable(resolve(*id))
                        {
                            pattern_vars.push(*id);
                        }
                    }
                    // Grouping vars = pattern vars that appear in outer context
                    let grouping: Vec<SymbolId> = pattern_vars
                        .into_iter()
                        .filter(|v| outer_vars.contains(v))
                        .collect();
                    Some((i, grouping))
                } else {
                    None
                }
            })
            .collect();

        // Apply grouping vars
        let rule = theory.get_rule_mut(label).unwrap();
        for (i, grouping) in fold_groupings {
            if let BodyLiteral::Fold(ref mut fold) = rule.body[i] {
                fold.grouping_vars = grouping;
            }
        }
    }
}

/// Collect variable SymbolIds from a head literal.
fn collect_literal_vars(lit: &crate::literal::Literal, vars: &mut HashSet<SymbolId>) {
    if is_variable(lit.name()) {
        vars.insert(lit.name_id());
    }
    for term in lit.predicate_args() {
        if let crate::term::Term::Symbol(id) = term
            && is_variable(resolve(*id))
        {
            vars.insert(*id);
        }
    }
}

/// Collect variable SymbolIds from a body logic literal.
fn collect_body_logic_vars(lit: &crate::body::BodyLogicLiteral, vars: &mut HashSet<SymbolId>) {
    if is_variable(lit.name()) {
        vars.insert(lit.name_id());
    }
    for arg in lit.predicate_args() {
        if let crate::body::BodyArg::Term(crate::term::Term::Symbol(id)) = arg
            && is_variable(resolve(*id))
        {
            vars.insert(*id);
        }
    }
}

/// Validate extension function calls in arithmetic expressions.
///
/// Walks all `ArithExpr::Call` nodes in the theory and checks:
/// 1. A function registry is provided if any Call nodes exist
/// 2. Each called function exists in the registry
/// 3. Each call has the correct number of arguments
fn validate_extension_calls(theory: &Theory, registry: Option<&FunctionRegistry>) -> Result<()> {
    for rule in theory.rules() {
        for lit in &rule.body {
            match lit {
                BodyLiteral::Arithmetic(constraint) => match constraint {
                    ArithConstraint::Bind { expr, .. } => {
                        validate_expr_calls(expr, registry)?;
                    }
                    ArithConstraint::Compare { lhs, rhs, .. } => {
                        validate_expr_calls(lhs, registry)?;
                        validate_expr_calls(rhs, registry)?;
                    }
                },
                BodyLiteral::Logic(logic_lit) => {
                    validate_body_logic_arith_args(logic_lit, registry)?;
                }
                BodyLiteral::Fold(fold) => {
                    validate_fold(fold, registry)?;
                }
            }
        }
    }
    Ok(())
}

/// Validate arithmetic arguments in a body logic literal.
fn validate_body_logic_arith_args(
    logic_lit: &crate::body::BodyLogicLiteral,
    registry: Option<&FunctionRegistry>,
) -> Result<()> {
    for arg in logic_lit.predicate_args() {
        if let crate::body::BodyArg::Arith(expr) = arg {
            validate_expr_calls(expr, registry)?;
        }
    }
    Ok(())
}

/// Validate a fold literal.
fn validate_fold(
    fold: &crate::body::FoldLiteral,
    registry: Option<&FunctionRegistry>,
) -> Result<()> {
    let result_var_name = crate::intern::resolve(fold.result_var);

    // 1. result_var must be a ?-prefixed variable
    if !result_var_name.starts_with('?') {
        return Err(crate::error::SpindleError::Validation {
            message: format!(
                "Fold result must be a variable (starting with ?), got '{result_var_name}'"
            ),
        });
    }

    // 2. Fold pattern must not be negated
    if fold.pattern.is_negated() {
        return Err(crate::error::SpindleError::Validation {
            message: "Fold pattern must not be negated".to_string(),
        });
    }

    // 3. result_var must not collide with pattern variables
    {
        let mut pattern_vars: Vec<SymbolId> = Vec::new();
        if is_variable(fold.pattern.name()) {
            pattern_vars.push(fold.pattern.name_id());
        }
        for arg in fold.pattern.predicate_args() {
            if let crate::body::BodyArg::Term(crate::term::Term::Symbol(id)) = arg
                && is_variable(resolve(*id))
            {
                pattern_vars.push(*id);
            }
        }
        if pattern_vars.contains(&fold.result_var) {
            return Err(crate::error::SpindleError::Validation {
                message: format!(
                    "Fold result variable '{result_var_name}' also appears in the fold pattern — \
                     this creates an ambiguity between the accumulated result and the per-match binding"
                ),
            });
        }
    }

    // 4. Reducer must exist in the function registry and accept arity 2
    let reducer_name = crate::intern::resolve(fold.reducer);
    match registry {
        None => {
            return Err(crate::error::SpindleError::Validation {
                message: format!(
                    "Fold reducer '{reducer_name}' used but no function registry provided"
                ),
            });
        }
        Some(reg) => match reg.get(fold.reducer) {
            None => {
                return Err(crate::error::SpindleError::Validation {
                    message: format!("Unknown fold reducer function '{reducer_name}'"),
                });
            }
            Some(func) => {
                let sig = func.signature();
                if !sig.arity.accepts(2) {
                    return Err(crate::error::SpindleError::Validation {
                        message: format!(
                            "Fold reducer '{reducer_name}' must accept 2 arguments, but accepts {}",
                            sig.arity
                        ),
                    });
                }
            }
        },
    }

    // 5. Validate extension function calls in extract and identity expressions
    validate_expr_calls(&fold.extract, registry)?;
    if let Some(ref identity) = fold.identity {
        validate_expr_calls(identity, registry)?;
    }

    // 6. Validate arith args in fold pattern
    validate_body_logic_arith_args(&fold.pattern, registry)?;

    Ok(())
}

/// Recursively validate Call nodes in an expression tree.
fn validate_expr_calls(expr: &ArithExpr, registry: Option<&FunctionRegistry>) -> Result<()> {
    match expr {
        ArithExpr::Call { name, args } => {
            let func_name = resolve(*name);
            match registry {
                None => {
                    return Err(SpindleError::Validation {
                        message: format!(
                            "Extension function '{func_name}' used but no function registry provided"
                        ),
                    });
                }
                Some(reg) => match reg.get(*name) {
                    None => {
                        return Err(SpindleError::Validation {
                            message: format!(
                                "Unknown extension function '{func_name}' in expression"
                            ),
                        });
                    }
                    Some(func) => {
                        let sig = func.signature();
                        if !sig.arity.accepts(args.len()) {
                            return Err(SpindleError::Validation {
                                message: format!(
                                    "Extension function '{}' expects {} argument(s), got {}",
                                    func_name,
                                    sig.arity,
                                    args.len()
                                ),
                            });
                        }
                    }
                },
            }
            // Validate arguments recursively
            for arg in args {
                validate_expr_calls(arg, registry)?;
            }
            Ok(())
        }
        ArithExpr::Lit(_) | ArithExpr::Var(_) => Ok(()),
    }
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
