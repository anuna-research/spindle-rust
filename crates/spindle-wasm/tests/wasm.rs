//! WebAssembly integration tests for Spindle
//!
//! Run these tests with:
//! ```bash
//! cd crates/spindle-wasm
//! wasm-pack test --headless --firefox  # or --chrome, --safari, --node
//! ```

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use spindle_wasm::Spindle;

// =============================================================================
// BASIC FUNCTIONALITY TESTS
// =============================================================================

#[wasm_bindgen_test]
fn test_spindle_new() {
    let spindle = Spindle::new();
    assert_eq!(spindle.rule_count(), 0);
}

#[wasm_bindgen_test]
fn test_spindle_default() {
    let spindle = Spindle::default();
    assert_eq!(spindle.rule_count(), 0);
}

#[wasm_bindgen_test]
fn test_add_fact() {
    let mut spindle = Spindle::new();
    let label = spindle.add_fact("bird");
    assert!(!label.is_empty());
    assert_eq!(spindle.rule_count(), 1);
}

#[wasm_bindgen_test]
fn test_add_defeasible_rule() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    let label = spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
    assert!(!label.is_empty());
    assert_eq!(spindle.rule_count(), 2);
}

#[wasm_bindgen_test]
fn test_add_strict_rule() {
    let mut spindle = Spindle::new();
    spindle.add_fact("mammal");
    let label = spindle.add_strict_rule(vec!["mammal".to_string()], "animal");
    assert!(!label.is_empty());
    assert_eq!(spindle.rule_count(), 2);
}

#[wasm_bindgen_test]
fn test_add_defeater() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_fact("penguin");
    spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
    let label = spindle.add_defeater(vec!["penguin".to_string()], "~flies");
    assert!(!label.is_empty());
    assert_eq!(spindle.rule_count(), 4);
}

#[wasm_bindgen_test]
fn test_add_superiority() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_fact("penguin");
    let r1 = spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
    let r2 = spindle.add_defeasible_rule(vec!["penguin".to_string()], "~flies");
    spindle.add_superiority(&r2, &r1);
    // Just verify it doesn't panic
    assert_eq!(spindle.rule_count(), 4);
}

#[wasm_bindgen_test]
fn test_clear() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_fact("penguin");
    assert_eq!(spindle.rule_count(), 2);
    spindle.clear();
    assert_eq!(spindle.rule_count(), 0);
}

// =============================================================================
// PARSING TESTS
// =============================================================================

#[wasm_bindgen_test]
fn test_parse_spl_rejects_dfl() {
    let mut spindle = Spindle::new();
    let result = spindle.parse_spl("f1: >> bird\nr1: bird => flies");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_parse_spl() {
    let mut spindle = Spindle::new();
    spindle
        .parse_spl("(given bird)\n(normally bird flies)")
        .unwrap();
    assert_eq!(spindle.rule_count(), 2);
}

#[wasm_bindgen_test]
fn test_parse_spl_complex() {
    let mut spindle = Spindle::new();
    spindle
        .parse_spl(
            r#"
            (given bird)
            (given penguin)
            (normally r1 bird flies)
            (normally r2 penguin (not flies))
            (prefer r2 r1)
            "#,
        )
        .unwrap();
    assert_eq!(spindle.rule_count(), 4);
}

#[wasm_bindgen_test]
fn test_parse_spl_error() {
    let mut spindle = Spindle::new();
    let result = spindle.parse_spl("(invalid (nested (error");
    assert!(result.is_err());
}

// =============================================================================
// REASONING TESTS
// =============================================================================

#[wasm_bindgen_test]
fn test_reason() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");

    let result = spindle.reason();
    assert!(!result.is_null());
    assert!(!result.is_undefined());
}

#[wasm_bindgen_test]
fn test_get_positive_conclusions() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");

    let conclusions = spindle.get_positive_conclusions();
    assert!(!conclusions.is_empty());
    assert!(conclusions.iter().any(|c| c.contains("bird")));
    assert!(conclusions.iter().any(|c| c.contains("flies")));
}

#[wasm_bindgen_test]
fn test_reason_with_conflict() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_fact("penguin");
    let r1 = spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
    let r2 = spindle.add_defeasible_rule(vec!["penguin".to_string()], "~flies");
    spindle.add_superiority(&r2, &r1);

    let conclusions = spindle.get_positive_conclusions();
    // Should have bird, penguin, ~flies (penguin rule wins)
    assert!(conclusions.iter().any(|c| c.contains("bird")));
    assert!(conclusions.iter().any(|c| c.contains("penguin")));
}

#[wasm_bindgen_test]
fn test_reason_spl_rejects_dfl() {
    let mut spindle = Spindle::new();
    let result = spindle.reason_spl("f1: >> bird\nr1: bird => flies");
    assert!(result.is_err());
}

#[wasm_bindgen_test]
fn test_reason_spl() {
    let mut spindle = Spindle::new();
    let result = spindle
        .reason_spl("(given bird)\n(normally bird flies)")
        .unwrap();
    assert!(result.contains("bird"));
    assert!(result.contains("flies"));
}

#[wasm_bindgen_test]
fn test_reason_spl_with_meta() {
    let mut spindle = Spindle::new();
    let result = spindle
        .reason_spl(
            r#"(given bird)
(normally r1 bird flies)
(meta r1 (description "Birds typically fly"))"#,
        )
        .unwrap();
    assert!(result.contains("bird"));
    assert!(result.contains("flies"));
    assert!(result.contains("META"));
    assert!(result.contains("description"));
}

#[wasm_bindgen_test]
fn test_reason_spl_with_list_meta() {
    let mut spindle = Spindle::new();
    let result = spindle
        .reason_spl(
            r#"(given bird)
(normally r1 bird flies)
(meta r1 (tags ("nature" "biology")))"#,
        )
        .unwrap();
    assert!(result.contains("bird"));
    assert!(result.contains("META"));
}

#[wasm_bindgen_test]
fn test_reason_spl_error() {
    let mut spindle = Spindle::new();
    let result = spindle.reason_spl("(unclosed paren");
    assert!(result.is_err());
}

// =============================================================================
// QUERY TESTS
// =============================================================================

#[wasm_bindgen_test]
fn test_query_provable() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");

    let result = spindle.query("bird");
    assert!(!result.is_null());
    assert!(!result.is_undefined());
}

#[wasm_bindgen_test]
fn test_query_negated() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_defeasible_rule(vec!["bird".to_string()], "~flies");

    let result = spindle.query("~flies");
    assert!(!result.is_null());
}

#[wasm_bindgen_test]
fn test_query_unknown() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");

    let result = spindle.query("unknown_literal");
    assert!(!result.is_null());
}

// =============================================================================
// WHAT-IF TESTS
// =============================================================================

#[wasm_bindgen_test]
fn test_what_if() {
    let mut spindle = Spindle::new();
    spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");

    let result = spindle.what_if(vec!["bird".to_string()], "flies");
    assert!(!result.is_null());
    assert!(!result.is_undefined());
}

#[wasm_bindgen_test]
fn test_what_if_negated_goal() {
    let mut spindle = Spindle::new();
    spindle.add_defeasible_rule(vec!["penguin".to_string()], "~flies");

    let result = spindle.what_if(vec!["penguin".to_string()], "~flies");
    assert!(!result.is_null());
}

#[wasm_bindgen_test]
fn test_what_if_multiple_hypotheticals() {
    let mut spindle = Spindle::new();
    spindle.add_defeasible_rule(vec!["a".to_string(), "b".to_string()], "c");

    let result = spindle.what_if(vec!["a".to_string(), "b".to_string()], "c");
    assert!(!result.is_null());
}

// =============================================================================
// WHY-NOT TESTS
// =============================================================================

#[wasm_bindgen_test]
fn test_why_not() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_defeasible_rule(vec!["penguin".to_string()], "waddles");

    let result = spindle.why_not("waddles");
    assert!(!result.is_null());
    assert!(!result.is_undefined());
}

#[wasm_bindgen_test]
fn test_why_not_negated() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");

    let result = spindle.why_not("~flies");
    assert!(!result.is_null());
}

// =============================================================================
// ABDUCTION TESTS
// =============================================================================

#[wasm_bindgen_test]
fn test_abduce() {
    let mut spindle = Spindle::new();
    spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");

    let result = spindle.abduce("flies", 5);
    assert!(!result.is_null());
    assert!(!result.is_undefined());
}

#[wasm_bindgen_test]
fn test_abduce_negated_goal() {
    let mut spindle = Spindle::new();
    spindle.add_defeasible_rule(vec!["penguin".to_string()], "~flies");

    let result = spindle.abduce("~flies", 3);
    assert!(!result.is_null());
}

#[wasm_bindgen_test]
fn test_abduce_complex() {
    let mut spindle = Spindle::new();
    spindle.add_defeasible_rule(vec!["a".to_string()], "b");
    spindle.add_defeasible_rule(vec!["b".to_string()], "c");
    spindle.add_defeasible_rule(vec!["x".to_string()], "c");

    let result = spindle.abduce("c", 10);
    assert!(!result.is_null());
}

// =============================================================================
// GET RULES TEST
// =============================================================================

#[wasm_bindgen_test]
fn test_get_rules() {
    let mut spindle = Spindle::new();
    spindle.add_fact("bird");
    spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
    spindle.add_strict_rule(vec!["mammal".to_string()], "animal");

    let result = spindle.get_rules();
    assert!(!result.is_null());
    assert!(!result.is_undefined());
}

#[wasm_bindgen_test]
fn test_get_rules_empty() {
    let spindle = Spindle::new();
    let result = spindle.get_rules();
    assert!(!result.is_null());
}

// =============================================================================
// INTEGRATION TESTS
// =============================================================================

#[wasm_bindgen_test]
fn test_penguin_example() {
    let mut spindle = Spindle::new();

    spindle.add_fact("bird");
    spindle.add_fact("penguin");
    let r1 = spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
    let r2 = spindle.add_defeasible_rule(vec!["penguin".to_string()], "~flies");
    spindle.add_superiority(&r2, &r1);

    let conclusions = spindle.get_positive_conclusions();

    // bird and penguin are facts
    assert!(conclusions.iter().any(|c| c.contains("bird")));
    assert!(conclusions.iter().any(|c| c.contains("penguin")));

    // ~flies should be derived (penguin exception beats bird rule)
    assert!(conclusions.iter().any(|c| c.contains("~flies")));
}

#[wasm_bindgen_test]
fn test_chain_reasoning() {
    let mut spindle = Spindle::new();

    spindle.add_fact("a");
    spindle.add_defeasible_rule(vec!["a".to_string()], "b");
    spindle.add_defeasible_rule(vec!["b".to_string()], "c");
    spindle.add_defeasible_rule(vec!["c".to_string()], "d");

    let conclusions = spindle.get_positive_conclusions();

    assert!(conclusions.iter().any(|c| c.contains(" a")));
    assert!(conclusions.iter().any(|c| c.contains(" b")));
    assert!(conclusions.iter().any(|c| c.contains(" c")));
    assert!(conclusions.iter().any(|c| c.contains(" d")));
}

#[wasm_bindgen_test]
fn test_strict_rule_chain() {
    let mut spindle = Spindle::new();

    spindle.add_fact("premise");
    spindle.add_strict_rule(vec!["premise".to_string()], "intermediate");
    spindle.add_strict_rule(vec!["intermediate".to_string()], "conclusion");

    let conclusions = spindle.get_positive_conclusions();

    assert!(conclusions.iter().any(|c| c.contains("premise")));
    assert!(conclusions.iter().any(|c| c.contains("intermediate")));
    assert!(conclusions.iter().any(|c| c.contains("conclusion")));
}

#[wasm_bindgen_test]
fn test_defeater_blocks() {
    let mut spindle = Spindle::new();

    spindle.add_fact("bird");
    spindle.add_fact("injured");
    spindle.add_defeasible_rule(vec!["bird".to_string()], "flies");
    spindle.add_defeater(vec!["injured".to_string()], "~flies");

    let conclusions = spindle.get_positive_conclusions();

    // bird and injured are facts
    assert!(conclusions.iter().any(|c| c.contains("bird")));
    assert!(conclusions.iter().any(|c| c.contains("injured")));

    // flies should NOT be derived (blocked by defeater)
    // Note: defeaters block but don't prove the opposite
    assert!(!conclusions.iter().any(|c| c.contains("+d flies")));
}

#[wasm_bindgen_test]
fn test_full_workflow() {
    let mut spindle = Spindle::new();

    // Parse theory
    spindle
        .parse_spl(
            r#"
        (given expert)
        (given novice)
        (normally r1 expert reliable)
        (normally r2 novice (not reliable))
        (prefer r1 r2)
        "#,
        )
        .unwrap();

    // Reason
    let conclusions = spindle.get_positive_conclusions();
    assert!(!conclusions.is_empty());

    // Query
    let query_result = spindle.query("reliable");
    assert!(!query_result.is_null());

    // What-if
    let what_if_result = spindle.what_if(vec!["certified".to_string()], "trusted");
    assert!(!what_if_result.is_null());

    // Why-not
    let why_not_result = spindle.why_not("~reliable");
    assert!(!why_not_result.is_null());

    // Abduce
    let abduce_result = spindle.abduce("trusted", 5);
    assert!(!abduce_result.is_null());

    // Get rules
    let rules = spindle.get_rules();
    assert!(!rules.is_null());

    // Clear and verify
    spindle.clear();
    assert_eq!(spindle.rule_count(), 0);
}
