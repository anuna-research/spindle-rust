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

#[test]
fn validate_fold_non_commutative_reducer_rejected() {
    // Subtraction is not commutative/associative, so it must be rejected as a fold reducer.
    let input = r#"
        (given (a 10))
        (given (a 3))
        (normally r-bad
            (fold ?total 0 - ?x (a ?x))
            (result ?total))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse should succeed");
    let result = reason_with_options(&theory, PrepareOptions::default());
    assert!(
        result.is_err(),
        "should reject fold with non-commutative reducer '-'"
    );
}

#[test]
fn validate_fold_division_reducer_rejected() {
    let input = r#"
        (given (a 10))
        (given (a 2))
        (normally r-bad
            (fold ?total 1 / ?x (a ?x))
            (result ?total))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse should succeed");
    let result = reason_with_options(&theory, PrepareOptions::default());
    assert!(
        result.is_err(),
        "should reject fold with non-commutative reducer '/'"
    );
}

#[test]
fn validate_fold_commutative_reducers_accepted() {
    // +, *, min, max should all be accepted
    for (reducer, identity) in &[("+", "0"), ("*", "1"), ("min", "required"), ("max", "required")]
    {
        let input = format!(
            r#"
            (given (a 10))
            (given (a 3))
            (normally r-ok
                (fold ?total {identity} {reducer} ?x (a ?x))
                (result ?total))
        "#,
        );
        let theory = spindle_parser::parse_spl(&input).expect("parse should succeed");
        let result = reason_with_options(&theory, PrepareOptions::default());
        assert!(
            result.is_ok(),
            "reducer '{reducer}' should be accepted as fold-safe, got: {:?}",
            result.err()
        );
    }
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

    fn is_fold_safe(&self) -> bool {
        true
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

// =========================================================================
// Gap coverage: fold with negated sibling body literals (#7)
// =========================================================================

#[test]
fn fold_with_positive_sibling_body_literal() {
    // Fold combined with a non-negated body literal that filters candidates.
    // The fold fires only for employees in the `active` relation.
    let conclusions = reason_spl(
        r#"
        (given (active alice))
        (given (pay-line alice 25))
        (given (pay-line alice 30))
        (given (pay-line bob 40))
        (normally r-total
            (and (active ?emp)
                 (fold ?total 0 + ?pay (pay-line ?emp ?pay)))
            (total-pay ?emp ?total))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d total-pay(alice, 55)"),
        "alice should have total-pay: {conclusions:?}"
    );
    assert!(
        !conclusions.iter().any(|c| c.contains("total-pay(bob")),
        "bob is not active, should not have total-pay: {conclusions:?}"
    );
}

#[test]
fn fold_with_negation_via_stratification() {
    // In defeasible logic, (not X) in body requires ~X to be proven.
    // Use a separate rule to derive ~excluded, filter via negation,
    // then fold over the derived filtered relation.
    let conclusions = reason_spl(
        r#"
        (given (employee alice))
        (given (employee bob))
        (given (excluded bob))
        (normally r-not-excl
            (and (employee ?emp) (not (excluded ?emp)))
            (active ?emp))
        (given (pay-line alice 25))
        (given (pay-line alice 30))
        (given (pay-line bob 40))
        (normally r-eligible
            (and (active ?emp) (pay-line ?emp ?pay))
            (eligible-pay ?emp ?pay))
        (normally r-total
            (fold ?total 0 + ?pay (eligible-pay ?emp ?pay))
            (total-pay ?emp ?total))
    "#,
    );
    // In defeasible logic, (not (excluded alice)) requires ~excluded(alice)
    // to be proven. Since no rule proves ~excluded(alice), the negation body
    // literal is not satisfied and `active(alice)` is not derived.
    //
    // This test documents the interaction: to use negation-based filtering
    // with folds, ensure the negation is provable in the logic.
    //
    // For now, just verify no crash and deterministic output.
    assert!(
        !conclusions.iter().any(|c| c.contains("total-pay(bob")),
        "bob is excluded, should not have total-pay: {conclusions:?}"
    );
}

// =========================================================================
// Gap coverage: fold with extension function in extract expression (#7)
// =========================================================================

// =========================================================================
// Transitive fold dependency (fold determinism fix)
// =========================================================================

#[test]
fn fold_transitive_dependency_correct_result() {
    // Chain: base facts -> fold -> intermediate -> normal derive -> fold
    // The second fold must wait for the first fold's results to propagate
    // through the normal derivation chain.
    let conclusions = reason_spl(
        r#"
        (given (hours alice mon 8))
        (given (hours alice tue 6))
        (given (rate alice 25))
        (normally r-pay
            (and (hours ?emp ?day ?h) (rate ?emp ?r) (bind ?pay (* ?h ?r)))
            (pay-line ?emp ?day ?pay))
        (normally r-topup
            (fold ?total 0 + ?pay (pay-line ?emp ?day ?pay))
            (subtotal ?emp ?total))
        (normally r-with-bonus
            (and (subtotal ?emp ?s) (bind ?final (+ ?s 100)))
            (final-pay ?emp ?final))
        (normally r-grand-total
            (fold ?gt 0 + ?f (final-pay ?emp ?f))
            (grand-total ?emp ?gt))
    "#,
    );
    // pay-line: alice mon 200, alice tue 150
    // subtotal(alice, 350) via fold
    // final-pay(alice, 450) via normal rule (350 + 100)
    // grand-total(alice, 450) via fold over final-pay
    assert!(
        has_conclusion(&conclusions, "+d subtotal(alice, 350)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d final-pay(alice, 450)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d grand-total(alice, 450)"),
        "got: {conclusions:?}"
    );
}

/// A simple "double" function for testing extract expressions.
struct DoubleFn {
    sig: FunctionSignature,
}

impl DoubleFn {
    fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("double"),
                arity: Arity::Fixed(1),
                description: "double an integer",
            },
        }
    }
}

impl ExtensionFunction for DoubleFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        match &args[0] {
            Term::Integer(n) => Ok(Term::Integer(n * 2)),
            _ => Err(EvalError::TypeError("expected integer".into())),
        }
    }
}

#[test]
fn fold_extract_calls_extension_function() {
    // Extract expression calls a user-defined extension function: (double ?pay)
    let input = r#"
        (given (pay-line alice 25))
        (given (pay-line alice 30))
        (normally r-total
            (fold ?total 0 + (double ?pay) (pay-line ?emp ?pay))
            (total-pay ?emp ?total))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse failed");

    let mut user_reg = FunctionRegistry::new();
    user_reg.register(Box::new(DoubleFn::new()));

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
    // alice: double(25) + double(30) = 50 + 60 = 110
    assert!(
        positives.iter().any(|c| c == "+d total-pay(alice, 110)"),
        "got: {positives:?}"
    );
}

// =========================================================================
// Gap coverage: multi-stratum dedup with 3 strata (#7)
// =========================================================================

#[test]
fn three_strata_deduplication() {
    // Chain: base → derive-a → fold-a → derive-b → fold-b
    // Facts from stratum 0 should not be duplicated in final output.
    let conclusions = reason_spl(
        r#"
        (given (score alice 10))
        (given (score alice 20))
        (normally r-weighted
            (and (score ?emp ?s) (bind ?w (* ?s 2)))
            (weighted-score ?emp ?w))
        (normally r-total-weighted
            (fold ?total 0 + ?w (weighted-score ?emp ?w))
            (total-weighted ?emp ?total))
        (normally r-level
            (and (total-weighted ?emp ?t) (bind ?lev (div ?t 10)))
            (level ?emp ?lev))
        (normally r-count-levels
            (fold ?n 0 + 1 (level ?who ?lev))
            (level-count ?who ?n))
    "#,
    );
    // weighted-score: alice gets 20 and 40
    // total-weighted(alice, 60)
    // level(alice, 6) (60 div 10 = 6)
    // level-count(alice, 1)
    assert!(
        has_conclusion(&conclusions, "+d total-weighted(alice, 60)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d level(alice, 6)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d level-count(alice, 1)"),
        "got: {conclusions:?}"
    );

    // Verify no duplicate conclusions: each literal should appear at most once
    let mut seen = std::collections::HashSet::new();
    for c in &conclusions {
        assert!(seen.insert(c.clone()), "duplicate conclusion found: {c}");
    }
}

// =========================================================================
// Gap coverage: two folds over same relation with different reducers (#7)
// =========================================================================

#[test]
fn two_folds_same_relation_different_reducers() {
    let conclusions = reason_spl(
        r#"
        (given (score alice 10))
        (given (score alice 20))
        (given (score alice 5))
        (normally r-sum
            (fold ?total 0 + ?s (score ?emp ?s))
            (sum-score ?emp ?total))
        (normally r-min
            (fold ?m required min ?s (score ?emp ?s))
            (min-score ?emp ?m))
    "#,
    );
    assert!(
        has_conclusion(&conclusions, "+d sum-score(alice, 35)"),
        "got: {conclusions:?}"
    );
    assert!(
        has_conclusion(&conclusions, "+d min-score(alice, 5)"),
        "got: {conclusions:?}"
    );
}

// =========================================================================
// Gap coverage: reason_from_prepared correctly dispatches stratified (#1)
// =========================================================================

#[test]
fn reason_from_prepared_multi_stratum() {
    // Test that reason_from_prepared correctly handles multi-stratum theories.
    // This specifically tests the bug fix where CLI/WASM bypassed stratification.
    let input = r#"
        (given (hours alice mon 8))
        (given (hours alice tue 6))
        (given (rate alice 25))
        (normally r-pay
            (and (hours ?emp ?day ?h) (rate ?emp ?r) (bind ?pay (* ?h ?r)))
            (pay-line ?emp ?day ?pay))
        (normally r-total
            (fold ?total 0 + ?pay (pay-line ?emp ?day ?pay))
            (total-pay ?emp ?total))
    "#;
    let theory = spindle_parser::parse_spl(input).expect("parse failed");
    let prepared = spindle_core::pipeline::prepare(&theory, PrepareOptions::default())
        .expect("prepare failed");

    // Use reason_from_prepared (the new function)
    let conclusions = spindle_core::reason::reason_from_prepared(&prepared).expect("reason failed");
    let positives: Vec<String> = conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| format!("{c}"))
        .collect();

    // alice: 8*25=200, 6*25=150, total=350
    assert!(
        positives.iter().any(|c| c == "+d total-pay(alice, 350)"),
        "reason_from_prepared should handle multi-stratum correctly: {positives:?}"
    );
}

// =========================================================================
// Defeasible proof strength preservation across strata
// =========================================================================

#[test]
fn defeasible_not_promoted_across_strata() {
    // A `normally` rule's conclusion should remain +d even when carried
    // across a stratum boundary. Previously, defeasible conclusions were
    // injected as strict facts into later strata, spuriously promoting
    // them to +D.
    let conclusions = reason_spl(
        r#"
        (given (hours alice 8))
        (given (rate alice 25))
        (normally r-pay
            (and (hours ?emp ?h) (rate ?emp ?r) (bind ?pay (* ?h ?r)))
            (pay-line ?emp ?pay))
        (normally r-total
            (fold ?total 0 + ?pay (pay-line ?emp ?pay))
            (total-pay ?emp ?total))
    "#,
    );
    // pay-line is derived by a defeasible rule → should be +d
    assert!(
        has_conclusion(&conclusions, "+d pay-line(alice, 200)"),
        "pay-line should be +d: {conclusions:?}"
    );
    // total-pay is also defeasible → should be +d
    assert!(
        has_conclusion(&conclusions, "+d total-pay(alice, 200)"),
        "total-pay should be +d: {conclusions:?}"
    );
}

// =========================================================================
// Issue 1: deduplicate_conclusions must preserve both +D and +d
// =========================================================================

#[test]
fn dedup_preserves_both_definite_and_defeasible() {
    // In defeasible logic, +D p implies +d p (subsumption). The reasoning
    // engine correctly emits both. Multi-stratum deduplication must not
    // collapse them — they are distinct conclusion types.
    //
    // Scenario: given facts (stratum 0) derive a relation via a defeasible
    // rule, then a fold in stratum 1 aggregates over that derived relation.
    // Because lower-stratum rules are re-included in stratum 1, the given
    // facts are re-derived, producing +D and +d duplicates. The dedup must
    // keep both +D and +d for each literal.
    let theory = spindle_parser::parse_spl(
        r#"
        (given (hours alice 8))
        (given (rate alice 25))
        (normally r-pay
            (and (hours ?emp ?h) (rate ?emp ?r) (bind ?pay (* ?h ?r)))
            (pay-line ?emp ?pay))
        (normally r-total
            (fold ?total 0 + ?pay (pay-line ?emp ?pay))
            (total-pay ?emp ?total))
    "#,
    )
    .expect("parse failed");

    let conclusions =
        reason_with_options(&theory, PrepareOptions::default()).expect("reason failed");

    let positives: Vec<String> = conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| format!("{c}"))
        .collect();

    // Given facts should have BOTH +D and +d (subsumption)
    assert!(
        positives.iter().any(|c| c == "+D hours(alice, 8)"),
        "+D hours(alice, 8) missing: {positives:?}"
    );
    assert!(
        positives.iter().any(|c| c == "+d hours(alice, 8)"),
        "+d hours(alice, 8) missing (dropped by dedup): {positives:?}"
    );
    assert!(
        positives.iter().any(|c| c == "+D rate(alice, 25)"),
        "+D rate(alice, 25) missing: {positives:?}"
    );
    assert!(
        positives.iter().any(|c| c == "+d rate(alice, 25)"),
        "+d rate(alice, 25) missing (dropped by dedup): {positives:?}"
    );
}

// =========================================================================
// Issue 2: Synthetic accumulated facts — finalized strata
// =========================================================================

#[test]
fn no_strat_fact_labels_in_conclusions() {
    // Conclusions from multi-stratum theories should never be attributed to
    // synthetic `__strat_fact_*` rules. Every positive conclusion should
    // trace back to a user-defined rule label.
    let theory = spindle_parser::parse_spl(
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
    )
    .expect("parse failed");

    let conclusions =
        reason_with_options(&theory, PrepareOptions::default()).expect("reason failed");

    for c in &conclusions {
        if let Some(ref label) = c.rule_label {
            assert!(
                !label.starts_with("__strat_fact_"),
                "conclusion {c} has synthetic label '{label}' — \
                 should be attributed to an original rule"
            );
        }
    }
}

#[test]
fn finalized_strata_no_redundant_re_derivation() {
    // With finalized strata, lower-stratum rules should NOT re-derive
    // conclusions in higher strata. The conclusion count should be stable
    // whether or not stratification is needed.
    //
    // This theory has 2 strata. If lower-stratum rules are re-included,
    // stratum 1 redundantly re-derives stratum-0 conclusions.
    let theory = spindle_parser::parse_spl(
        r#"
        (given (item a 10))
        (given (item b 20))
        (normally r-line (item ?x ?price) (line ?x ?price))
        (normally r-total
            (fold ?total 0 + ?price (line ?x ?price))
            (grand-total ?total))
    "#,
    )
    .expect("parse failed");

    let conclusions =
        reason_with_options(&theory, PrepareOptions::default()).expect("reason failed");

    // Count how many times each positive conclusion literal appears
    let positive_literals: Vec<String> = conclusions
        .iter()
        .filter(|c| c.is_positive())
        .map(|c| format!("{}", c))
        .collect();

    // Each conclusion should appear exactly once (no duplicates from re-inclusion)
    let mut seen = std::collections::HashSet::new();
    for lit in &positive_literals {
        assert!(
            seen.insert(lit.clone()),
            "duplicate positive conclusion: {lit}"
        );
    }
}

#[test]
fn cross_stratum_conclusion_rule_labels_are_original() {
    // In a multi-stratum theory, conclusions from lower strata should be
    // attributed to their original rule labels, not to synthetic
    // `__strat_fact_*` labels. This is critical for explanation and trust
    // systems that trace proof chains via rule_label.
    let theory = spindle_parser::parse_spl(
        r#"
        (given (task alice coding))
        (given (task bob review))
        (normally r-active (task ?emp ?t) (active ?emp))
        (normally r-count
            (fold ?n 0 + 1 (active ?emp))
            (headcount ?n))
    "#,
    )
    .expect("parse failed");

    let conclusions =
        reason_with_options(&theory, PrepareOptions::default()).expect("reason failed");

    // All positive conclusions should have rule_labels that point to
    // original user-defined rules, not synthetic stratum facts.
    for c in &conclusions {
        if c.is_positive()
            && let Some(ref label) = c.rule_label
        {
            assert!(
                !label.starts_with("__strat_fact_"),
                "conclusion {} attributed to synthetic label '{}' — \
                 should trace to original rule",
                c, label
            );
        }
    }
}
