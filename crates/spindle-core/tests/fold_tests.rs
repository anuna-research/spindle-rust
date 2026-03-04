//! Integration tests for fold (aggregation) construct and stratification.

use spindle_core::function_registry::{
    Arity, EvalError, ExtensionFunction, FunctionRegistry, FunctionSignature,
};
use spindle_core::intern::intern;
use spindle_core::pipeline::PrepareOptions;
use spindle_core::reason::reason_with_options;
use spindle_core::term::Term;

/// Parse an SPL theory and reason over it, returning conclusions as formatted strings.
fn reason_spl(input: &str) -> Vec<String> {
    let theory = spindle_parser::parse_spl(input).expect("parse failed");
    let conclusions =
        reason_with_options(&theory, PrepareOptions::default()).expect("reason failed");
    let mut result: Vec<String> = conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| format!("{}", c))
        .collect();
    result.sort();
    result
}

/// Check if a specific conclusion exists.
fn has_conclusion(conclusions: &[String], expected: &str) -> bool {
    conclusions.iter().any(|c| c == expected)
}

// =========================================================================
// Parser tests
// =========================================================================

#[test]
fn parse_fold_sum() {
    let input = r#"
        (given (pay-line alice 25))
        (given (pay-line alice 30))
        (normally r-total
            (fold ?total 0 + ?pay (pay-line ?emp ?pay))
            (total-pay ?emp ?total))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse failed");
    // Should parse without error; the fold should appear in the rule body
    assert!(theory.rule_count() >= 3); // 2 facts + 1 rule
}

#[test]
fn parse_fold_count() {
    let input = r#"
        (given (shift alice mon))
        (given (shift alice tue))
        (normally r-count
            (fold ?count 0 + 1 (shift ?emp ?d))
            (shift-count ?emp ?count))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse failed");
    assert!(theory.rule_count() >= 3);
}

#[test]
fn parse_fold_required() {
    let input = r#"
        (given (rate alice 25))
        (normally r-min
            (fold ?min required min ?rate (rate ?emp ?rate))
            (min-rate ?emp ?min))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse failed");
    assert!(theory.rule_count() >= 2);
}

#[test]
fn parse_fold_in_and_body() {
    let input = r#"
        (given (pay-line alice 25))
        (given (employee alice))
        (normally r-total
            (and (employee ?emp) (fold ?total 0 + ?pay (pay-line ?emp ?pay)))
            (total-pay ?emp ?total))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse failed");
    assert!(theory.rule_count() >= 3);
}

#[test]
fn parse_fold_rejects_malformed() {
    // Wrong number of arguments
    let input = r#"
        (normally r-bad
            (fold ?total 0 +)
            (result ?total))
    "#;
    let result = spindle_parser::parse_spl(input);
    assert!(result.is_err(), "should reject fold with too few args");
}

#[test]
fn parse_fold_rejects_non_variable_result() {
    let input = r#"
        (normally r-bad
            (fold total 0 + ?x (a ?x))
            (result total))
    "#;
    let result = spindle_parser::parse_spl(input);
    assert!(result.is_err(), "should reject non-variable result");
}

// =========================================================================
// Validation tests
// =========================================================================

#[test]
fn validate_fold_negated_pattern_rejected() {
    let input = r#"
        (normally r-bad
            (fold ?total 0 + ?x (not (a ?x)))
            (result ?total))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse should succeed");
    let result = reason_with_options(&theory, PrepareOptions::default());
    assert!(result.is_err(), "should reject fold with negated pattern");
}

#[test]
fn validate_fold_unknown_reducer_rejected() {
    let input = r#"
        (normally r-bad
            (fold ?total 0 unknown-func ?x (a ?x))
            (result ?total))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse should succeed");
    let result = reason_with_options(&theory, PrepareOptions::default());
    assert!(result.is_err(), "should reject fold with unknown reducer");
}

// =========================================================================
// Fold evaluation tests (single stratum — fold over base facts)
// =========================================================================

#[test]
fn fold_sum_over_base_facts() {
    let conclusions = reason_spl(
        r#"
        (given (pay-line alice 25))
        (given (pay-line alice 30))
        (given (pay-line bob 40))
        (normally r-total
            (fold ?total 0 + ?pay (pay-line ?emp ?pay))
            (total-pay ?emp ?total))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(alice, 55)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(bob, 40)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn fold_count_over_base_facts() {
    let conclusions = reason_spl(
        r#"
        (given (shift alice mon))
        (given (shift alice tue))
        (given (shift alice wed))
        (given (shift bob mon))
        (normally r-count
            (fold ?n 0 + 1 (shift ?emp ?d))
            (shift-count ?emp ?n))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d shift-count(alice, 3)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d shift-count(bob, 1)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn fold_min_over_base_facts() {
    let conclusions = reason_spl(
        r#"
        (given (rate alice 30))
        (given (rate alice 25))
        (given (rate alice 40))
        (normally r-min
            (fold ?m required min ?rate (rate ?emp ?rate))
            (min-rate ?emp ?m))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d min-rate(alice, 25)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn fold_max_over_base_facts() {
    let conclusions = reason_spl(
        r#"
        (given (rate alice 30))
        (given (rate alice 25))
        (given (rate alice 40))
        (normally r-max
            (fold ?m required max ?rate (rate ?emp ?rate))
            (max-rate ?emp ?m))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d max-rate(alice, 40)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn fold_required_empty_set_no_fire() {
    // No facts match the fold pattern — "required" means the rule doesn't fire
    let conclusions = reason_spl(
        r#"
        (given (other-thing x))
        (normally r-min
            (fold ?m required min ?rate (rate ?emp ?rate))
            (min-rate ?emp ?m))
    "#,
    );
    assert!(
        !conclusions.iter().any(|c| c.contains("min-rate")),
        "fold with required should not fire on empty set: {conclusions:?}"
    );
}

#[test]
fn fold_with_identity_empty_set_fires() {
    // No facts match the fold pattern — identity 0 means the rule fires with 0
    let conclusions = reason_spl(
        r#"
        (given (employee alice))
        (normally r-total
            (and (employee ?emp) (fold ?total 0 + ?pay (pay-line ?emp ?pay)))
            (total-pay ?emp ?total))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(alice, 0)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn fold_scoped_by_outer_variable() {
    // Outer variable ?emp restricts which pay-line facts are folded
    let conclusions = reason_spl(
        r#"
        (given (employee alice))
        (given (employee bob))
        (given (pay-line alice 25))
        (given (pay-line alice 30))
        (given (pay-line bob 40))
        (normally r-total
            (and (employee ?emp) (fold ?total 0 + ?pay (pay-line ?emp ?pay)))
            (total-pay ?emp ?total))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(alice, 55)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(bob, 40)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn fold_extract_expression() {
    // Extract is a complex expression: (* ?hours ?rate)
    let conclusions = reason_spl(
        r#"
        (given (work alice 8 25))
        (given (work alice 6 30))
        (normally r-total
            (fold ?total 0 + (* ?hours ?rate) (work ?emp ?hours ?rate))
            (daily-pay ?emp ?total))
    "#,
    );
    // alice: 8*25 + 6*30 = 200 + 180 = 380
    assert!(
        has_conclusion(&conclusions, "+d daily-pay(alice, 380)"),
        "got: {conclusions:?}"
    );
}

// =========================================================================
// Stratified reasoning tests (fold over derived relations)
// =========================================================================

#[test]
fn fold_over_derived_relation() {
    // Step 1: derive pay-line from hours and rate
    // Step 2: fold over pay-line
    let conclusions = reason_spl(
        r#"
        (given (hours alice mon 8))
        (given (hours alice tue 6))
        (given (rate alice 25))
        (normally r-pay
            (and (hours ?emp ?day ?h) (rate ?emp ?r) (bind ?pay (* ?h ?r)))
            (pay-line ?emp ?day ?pay))
        (normally r-total
            (fold ?total 0 + ?pay (pay-line ?emp ?day ?pay))
            (total-pay ?emp ?total))
    "#,
    );
    // alice: 8*25=200, 6*25=150, total=350
    assert!(
        has_conclusion(&conclusions, "+d total-pay(alice, 350)"),
        "got: {conclusions:?}"
    );
}

// =========================================================================
// Award scenario from plan
// =========================================================================

#[test]
fn award_scenario() {
    let conclusions = reason_spl(
        r#"
        (given (pay-line alice mon 8 25))
        (given (pay-line alice tue 6 25))
        (given (pay-line bob mon 8 30))
        (normally r-total
            (fold ?total 0 + (* ?hours ?rate) (pay-line ?emp ?day ?hours ?rate))
            (daily-pay ?emp ?total))
    "#,
    );
    // alice: 8*25 + 6*25 = 200 + 150 = 350
    // bob: 8*30 = 240
    assert!(
        has_conclusion(&conclusions, "+d daily-pay(alice, 350)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d daily-pay(bob, 240)"),
        "got: {conclusions:?}"
    );
}

// =========================================================================
// Regression: existing programs without folds work unchanged
// =========================================================================

#[test]
fn regression_basic_defeasible_no_fold() {
    let conclusions = reason_spl(
        r#"
        (given bird)
        (normally r1 bird flies)
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d flies"),
        "got: {conclusions:?}"
    );
}

#[test]
fn regression_superiority_no_fold() {
    let conclusions = reason_spl(
        r#"
        (given bird)
        (given penguin)
        (normally r1 bird flies)
        (normally r2 penguin (not flies))
        (prefer r2 r1)
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d ~flies"),
        "got: {conclusions:?}"
    );
}

#[test]
fn regression_grounding_no_fold() {
    let conclusions = reason_spl(
        r#"
        (given (parent alice bob))
        (given (parent bob carol))
        (always r1 (and (parent ?x ?y) (parent ?y ?z)) (ancestor ?x ?z))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+D ancestor(alice, carol)"),
        "got: {conclusions:?}"
    );
}

// =========================================================================
// New tests: edge cases, validation, and multi-stratum coverage
// =========================================================================

#[test]
fn fold_stratified_with_superiority() {
    // Superiority between rules in the same stratum should work.
    // r-total and r-override both derive total-pay; r-override wins for bob.
    let conclusions = reason_spl(
        r#"
        (given (pay-line alice 25))
        (given (pay-line alice 30))
        (given (pay-line bob 40))
        (given (override-pay bob 999))
        (normally r-total
            (fold ?total 0 + ?pay (pay-line ?emp ?pay))
            (total-pay ?emp ?total))
        (normally r-override
            (override-pay ?emp ?val)
            (total-pay ?emp ?val))
        (prefer r-override r-total)
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(alice, 55)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(bob, 999)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn multiple_folds_in_same_rule() {
    // Two folds in an `and` body, each aggregating different relations.
    let conclusions = reason_spl(
        r#"
        (given (employee alice))
        (given (hours alice 8))
        (given (hours alice 6))
        (given (bonus alice 100))
        (given (bonus alice 50))
        (normally r-combined
            (and
                (employee ?emp)
                (fold ?total_hours 0 + ?h (hours ?emp ?h))
                (fold ?total_bonus 0 + ?b (bonus ?emp ?b)))
            (summary ?emp ?total_hours ?total_bonus))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d summary(alice, 14, 150)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn fold_with_strict_rules() {
    // `always` rule with fold produces +D conclusions.
    let conclusions = reason_spl(
        r#"
        (given (score alice 10))
        (given (score alice 20))
        (always r-total
            (fold ?total 0 + ?s (score ?who ?s))
            (total-score ?who ?total))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+D total-score(alice, 30)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn fold_over_empty_derived_relation_uses_identity() {
    // Derived relation `active-line` is empty for bob, but identity 0 means
    // the fold fires with 0.
    let conclusions = reason_spl(
        r#"
        (given (employee alice))
        (given (employee bob))
        (given (pay-line alice 25))
        (given (active alice))
        (normally r-active
            (and (pay-line ?emp ?pay) (active ?emp))
            (active-line ?emp ?pay))
        (normally r-total
            (and (employee ?emp) (fold ?total 0 + ?pay (active-line ?emp ?pay)))
            (total-pay ?emp ?total))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(alice, 25)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(bob, 0)"),
        "got: {conclusions:?}"
    );
}

#[test]
fn fold_result_var_collision_rejected() {
    // ?x is both the fold result variable and a pattern variable — should be rejected.
    let input = r#"
        (given (data a 1))
        (normally r-bad
            (fold ?x 0 + ?x (data ?x ?y))
            (result ?x))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse should succeed");
    let result = reason_with_options(&theory, PrepareOptions::default());
    assert!(
        result.is_err(),
        "should reject fold with result/pattern variable collision"
    );
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("?x") && msg.contains("pattern"),
        "error should mention the variable and pattern: {msg}"
    );
}

/// A custom reducer that multiplies two values (user-defined extension).
struct MultiplyReducer {
    sig: FunctionSignature,
}

impl MultiplyReducer {
    fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("mul"),
                arity: Arity::Fixed(2),
                description: "multiply two integers",
            },
        }
    }
}

impl ExtensionFunction for MultiplyReducer {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        match (&args[0], &args[1]) {
            (Term::Integer(a), Term::Integer(b)) => Ok(Term::Integer(a * b)),
            _ => Err(EvalError::TypeError("expected integers".into())),
        }
    }
}

#[test]
fn user_defined_reducer_in_multi_stratum() {
    // Use a custom "mul" reducer in a fold that depends on a derived relation
    // (forcing multi-stratum). This tests that user extensions survive into
    // the stratified reasoning path.
    let input = r#"
        (given (base alice 2))
        (given (factor alice 3))
        (given (factor alice 5))
        (normally r-derive (base ?emp ?b) (value ?emp ?b))
        (normally r-product
            (fold ?product 1 mul ?v (value ?emp ?v))
            (result ?emp ?product))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse failed");

    let mut user_reg = FunctionRegistry::new();
    user_reg.register(Box::new(MultiplyReducer::new()));

    let opts = PrepareOptions {
        function_registry: Some(user_reg),
        ..Default::default()
    };
    let conclusions = reason_with_options(&theory, opts).expect("reason failed");
    let positives: Vec<String> = conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| format!("{c}"))
        .collect();
    // value(alice, 2) is derived in stratum 0. The fold over value produces 1 * 2 = 2.
    // Note: factor facts are not derived as "value" — only base is.
    assert!(
        positives.iter().any(|c| c == "+d result(alice, 2)"),
        "got: {positives:?}"
    );
}
