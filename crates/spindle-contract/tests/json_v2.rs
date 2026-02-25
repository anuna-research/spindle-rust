//! TEST-012: v2 JSON typed argument serialization (8 scenarios).
//!
//! Verifies round-trip fidelity for all Term variants, large integer
//! string encoding, decimal scale preservation, and v1 backward compatibility.

use spindle_contract::literal::{LiteralStructJson, LiteralStructJsonV2};
use spindle_contract::reason::{SCHEMA_V1, SCHEMA_V2};
use spindle_contract::term::TermDto;
use spindle_core::intern::intern;
use spindle_core::literal::Literal;
use spindle_core::term::{FiniteFloat, Term};

/// Helper: create a literal with typed args.
fn make_literal(name: &str, args: Vec<Term>) -> Literal {
    use spindle_core::mode::Mode;
    use spindle_core::temporal::Temporal;
    Literal::from_ids(intern(name), false, Mode::empty(), Temporal::empty(), args)
}

// ===========================================================================
// TEST-012-01: Symbol round-trip
// ===========================================================================
#[test]
fn test_012_01_symbol_round_trip() {
    let dto = TermDto::Symbol("widget".to_string());
    let json = serde_json::to_string(&dto).unwrap();
    let back: TermDto = serde_json::from_str(&json).unwrap();
    assert_eq!(dto, back);

    // Verify JSON shape
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["type"], "symbol");
    assert_eq!(v["value"], "widget");
}

// ===========================================================================
// TEST-012-02: Integer round-trip (small — fits in JSON number)
// ===========================================================================
#[test]
fn test_012_02_integer_small_round_trip() {
    let dto = TermDto::Integer(25);
    let json = serde_json::to_string(&dto).unwrap();
    let back: TermDto = serde_json::from_str(&json).unwrap();
    assert_eq!(dto, back);

    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["type"], "integer");
    assert_eq!(v["value"], 25);
}

// ===========================================================================
// TEST-012-03: Large integer uses string to avoid precision loss
// ===========================================================================
#[test]
fn test_012_03_large_integer_string_serialization() {
    let big = i64::MAX; // 9223372036854775807 > 2^53
    let dto = TermDto::Integer(big);
    let json = serde_json::to_string(&dto).unwrap();

    // Value must be a string, not a number
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["type"], "integer");
    assert!(v["value"].is_string(), "large integer should be string");
    assert_eq!(v["value"], "9223372036854775807");

    // Round-trip back
    let back: TermDto = serde_json::from_str(&json).unwrap();
    assert_eq!(dto, back);

    // Negative large
    let neg = i64::MIN;
    let dto_neg = TermDto::Integer(neg);
    let json_neg = serde_json::to_string(&dto_neg).unwrap();
    let back_neg: TermDto = serde_json::from_str(&json_neg).unwrap();
    assert_eq!(dto_neg, back_neg);
}

// ===========================================================================
// TEST-012-04: Decimal preserves trailing zeros (scale)
// ===========================================================================
#[test]
fn test_012_04_decimal_preserves_scale() {
    let dto = TermDto::Decimal("8.00".to_string());
    let json = serde_json::to_string(&dto).unwrap();

    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["type"], "decimal");
    assert_eq!(v["value"], "8.00"); // trailing zeros preserved

    let back: TermDto = serde_json::from_str(&json).unwrap();
    assert_eq!(dto, back);
}

// ===========================================================================
// TEST-012-05: Float round-trip
// ===========================================================================
#[test]
fn test_012_05_float_round_trip() {
    let dto = TermDto::Float(3.14);
    let json = serde_json::to_string(&dto).unwrap();
    let back: TermDto = serde_json::from_str(&json).unwrap();
    assert_eq!(dto, back);

    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(v["type"], "float");
    assert_eq!(v["value"], 3.14);
}

// ===========================================================================
// TEST-012-06: Mixed args in v2 literal struct
// ===========================================================================
#[test]
fn test_012_06_mixed_args_literal_v2() {
    let lit = make_literal(
        "item",
        vec![
            Term::Symbol(intern("widget")),
            Term::Integer(25),
            Term::Decimal(rust_decimal::Decimal::new(8, 2)), // 0.08
        ],
    );

    let v2 = LiteralStructJsonV2::from(&lit);
    assert_eq!(v2.functor, "item");
    assert_eq!(v2.args.len(), 3);

    // Verify each arg type
    assert_eq!(v2.args[0], TermDto::Symbol("widget".to_string()));
    assert_eq!(v2.args[1], TermDto::Integer(25));
    assert_eq!(v2.args[2], TermDto::Decimal("0.08".to_string()));

    // JSON round-trip
    let json = serde_json::to_string(&v2).unwrap();
    let back: LiteralStructJsonV2 = serde_json::from_str(&json).unwrap();
    assert_eq!(v2.functor, back.functor);
    assert_eq!(v2.args, back.args);
}

// ===========================================================================
// TEST-012-07: v1 output unchanged for same theory
// ===========================================================================
#[test]
fn test_012_07_v1_output_unchanged() {
    let lit = make_literal(
        "item",
        vec![
            Term::Symbol(intern("widget")),
            Term::Integer(25),
            Term::Decimal(rust_decimal::Decimal::new(8, 2)),
        ],
    );

    let v1 = LiteralStructJson::from(&lit);
    assert_eq!(v1.functor, "item");
    // v1 args are all strings
    assert_eq!(v1.args, vec!["widget", "25", "0.08"]);

    // v1 schema version constant
    assert_eq!(SCHEMA_V1, "spindle.reason.v1");
    assert_eq!(SCHEMA_V2, "spindle.reason.v2");
}

// ===========================================================================
// TEST-012-08: TermDto ↔ Term round-trip for all variants
// ===========================================================================
#[test]
fn test_012_08_term_dto_to_term_round_trip() {
    let cases: Vec<(Term, TermDto)> = vec![
        (
            Term::Symbol(intern("hello")),
            TermDto::Symbol("hello".to_string()),
        ),
        (Term::Integer(42), TermDto::Integer(42)),
        (Term::Integer(-99), TermDto::Integer(-99)),
        (
            Term::Decimal(rust_decimal::Decimal::new(150, 2)),
            TermDto::Decimal("1.50".to_string()),
        ),
        (
            Term::Float(FiniteFloat::new(2.5).unwrap()),
            TermDto::Float(2.5),
        ),
    ];

    for (term, expected_dto) in &cases {
        // Term → TermDto
        let dto = TermDto::from(term);
        assert_eq!(&dto, expected_dto, "Term→TermDto mismatch for {:?}", term);

        // TermDto → Term → TermDto (full round-trip)
        let back_term = dto.to_term().expect("to_term should succeed");
        let back_dto = TermDto::from(&back_term);
        assert_eq!(
            &dto, &back_dto,
            "TermDto round-trip mismatch for {:?}",
            term
        );

        // JSON round-trip
        let json = serde_json::to_string(&dto).unwrap();
        let json_back: TermDto = serde_json::from_str(&json).unwrap();
        assert_eq!(&dto, &json_back, "JSON round-trip mismatch for {:?}", term);
    }
}
