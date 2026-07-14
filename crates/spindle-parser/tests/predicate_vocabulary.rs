//! SPEC-024 integration tests for SPL predicate declarations, structured
//! metadata targets, and undeclared-predicate compatibility (TEST-021,
//! TEST-022, TEST-023).

use spindle_core::vocabulary::{
    DeclarationOrigin, MetaTarget, PredicateSymbol, PrimitiveSort, TheorySignature, Vocabulary,
};
use spindle_core::{MetaValue, reason};
use spindle_parser::parse_spl;

fn sym(name: &str, arity: usize) -> PredicateSymbol {
    PredicateSymbol::try_new(name.into(), arity).unwrap()
}

// ---------------------------------------------------------------------------
// TEST-021: Predicate declaration syntax and storage
// ---------------------------------------------------------------------------

#[test]
fn test_021_positive_binary_and_zero_arity_declarations() {
    let theory =
        parse_spl("(predicate assign-to ((task symbol) (agent symbol)))\n(predicate emergency ())")
            .unwrap();

    let decls = theory.predicate_declarations();
    assert_eq!(decls.len(), 2);

    // arity equals binder count
    assert_eq!(decls[0].symbol(), sym("assign-to", 2));
    assert_eq!(decls[0].signature.arguments[0].name, "task");
    assert_eq!(decls[0].signature.arguments[0].sort, PrimitiveSort::Symbol);
    assert_eq!(decls[0].signature.arguments[1].name, "agent");

    assert_eq!(decls[1].symbol(), sym("emergency", 0));
    assert_eq!(decls[1].signature.arguments.len(), 0);

    // Source provenance is retained.
    assert!(matches!(decls[0].origin, DeclarationOrigin::Parsed(loc) if loc.line == 1));
    assert!(matches!(decls[1].origin, DeclarationOrigin::Parsed(loc) if loc.line == 2));

    // The form adds no fact or rule.
    assert_eq!(theory.rule_count(), 0);
}

#[test]
fn test_021_negative_malformed_declarations_rejected() {
    // Binder is not an (name sort) list.
    assert!(parse_spl("(predicate p (x))").is_err());
    // Unknown sort.
    assert!(parse_spl("(predicate p ((x widget)))").is_err());
    // Empty argument name (quoted empty atom).
    assert!(parse_spl("(predicate p ((\"\" symbol)))").is_err());
    // Duplicate argument name.
    assert!(parse_spl("(predicate p ((x symbol) (x integer)))").is_err());
    // Argument list is not a list.
    assert!(parse_spl("(predicate p x)").is_err());
}

#[test]
fn test_021_declaration_does_not_change_rule_or_fact_counts() {
    let with_decl =
        parse_spl("(given bird)\n(normally r1 (bird) (flies))\n(predicate flies ((who symbol)))")
            .unwrap();
    let without = parse_spl("(given bird)\n(normally r1 (bird) (flies))").unwrap();
    assert_eq!(with_decl.rule_count(), without.rule_count());
    assert_eq!(with_decl.predicate_declarations().len(), 1);
}

#[test]
fn test_021_inline_metadata_desugars_to_predicate_target() {
    let theory = parse_spl(
        "(predicate assign-to\n\
         ((task symbol) (agent symbol))\n\
         (description \"Assign a task to an agent.\")\n\
         (tags (\"planning\" \"scheduling\")))",
    )
    .unwrap();

    // The declaration is still stored, adds no rule/fact.
    assert_eq!(theory.predicate_declarations().len(), 1);
    assert_eq!(theory.rule_count(), 0);

    // Inline properties land in the same MetaTarget::Predicate store.
    let meta = theory
        .get_meta_target(&MetaTarget::Predicate(sym("assign-to", 2)))
        .expect("inline metadata present");
    assert_eq!(
        meta.properties.get("description"),
        Some(&MetaValue::String("Assign a task to an agent.".to_string()))
    );
    assert_eq!(
        meta.properties.get("tags"),
        Some(&MetaValue::List(vec![
            "planning".to_string(),
            "scheduling".to_string()
        ]))
    );
}

#[test]
fn test_021_inline_and_separate_meta_are_equivalent() {
    let inline =
        parse_spl("(predicate assign-to ((task symbol) (agent symbol)) (description \"d\"))")
            .unwrap();
    let separate = parse_spl(
        "(predicate assign-to ((task symbol) (agent symbol)))\n\
         (meta (predicate assign-to 2) (description \"d\"))",
    )
    .unwrap();

    let target = MetaTarget::Predicate(sym("assign-to", 2));
    assert_eq!(
        inline.get_meta_target(&target).map(|m| &m.properties),
        separate.get_meta_target(&target).map(|m| &m.properties),
    );
}

#[test]
fn test_021_inline_and_separate_meta_merge() {
    // Inline description plus a later separate meta property merge into one entry.
    let theory = parse_spl(
        "(predicate assign-to ((task symbol) (agent symbol)) (description \"d\"))\n\
         (meta (predicate assign-to 2) (source \"handbook\"))",
    )
    .unwrap();
    let meta = theory
        .get_meta_target(&MetaTarget::Predicate(sym("assign-to", 2)))
        .unwrap();
    assert_eq!(
        meta.properties.get("description"),
        Some(&MetaValue::String("d".to_string()))
    );
    assert_eq!(
        meta.properties.get("source"),
        Some(&MetaValue::String("handbook".to_string()))
    );
}

// ---------------------------------------------------------------------------
// TEST-022: Structured predicate metadata target
// ---------------------------------------------------------------------------

#[test]
fn test_022_predicate_metadata_target_is_distinct() {
    let theory = parse_spl(
        "(predicate assign-to ((task symbol) (agent symbol)))\n\
         (meta (predicate assign-to 2) (description \"Assign a task to an agent.\"))\n\
         (normally assign-to (request) (done))\n\
         (meta assign-to (note \"rule-level\"))",
    )
    .unwrap();

    // The predicate target carries the description.
    let pred_meta = theory
        .get_meta_target(&MetaTarget::Predicate(sym("assign-to", 2)))
        .expect("predicate metadata present");
    assert_eq!(
        pred_meta.properties.get("description"),
        Some(&MetaValue::String("Assign a task to an agent.".to_string()))
    );

    // It does not collide with the same-name rule label.
    let label_meta = theory
        .get_meta("assign-to")
        .expect("label metadata present");
    assert_eq!(
        label_meta.properties.get("note"),
        Some(&MetaValue::String("rule-level".to_string()))
    );
    assert!(!label_meta.properties.contains_key("description"));

    // It does not collide with a different arity.
    assert!(
        theory
            .get_meta_target(&MetaTarget::Predicate(sym("assign-to", 1)))
            .is_none()
    );

    // Vocabulary derivation obtains the description without parsing a string key.
    let report = Vocabulary::derive(&theory);
    let entry = report
        .vocabulary
        .entries
        .iter()
        .find(|e| e.symbol == sym("assign-to", 2))
        .unwrap();
    assert_eq!(
        entry.description.as_deref(),
        Some("Assign a task to an agent.")
    );
}

#[test]
fn test_022_negative_malformed_arity_rejected() {
    assert!(parse_spl("(meta (predicate assign-to x) (description \"d\"))").is_err());
    assert!(parse_spl("(meta (predicate assign-to 01) (description \"d\"))").is_err());
    // Malformed structured target (wrong head atom).
    assert!(parse_spl("(meta (widget assign-to 2) (description \"d\"))").is_err());
}

// ---------------------------------------------------------------------------
// TEST-023: Undeclared predicate compatibility
// ---------------------------------------------------------------------------

#[test]
fn test_023_declaration_free_theory_parses_and_reasons() {
    let src = "(given bird)\n(normally r1 (bird) (flies))";
    let theory = parse_spl(src).unwrap();

    // Reasoning is unaffected by the absence of declarations.
    let conclusions = reason(&theory).unwrap();
    assert!(
        conclusions
            .iter()
            .any(|c| c.literal.name() == "flies" && c.literal.is_positive())
    );

    // Vocabulary derivation can report undeclared uses per caller policy, but
    // the theory has no declarations at all.
    let report = Vocabulary::derive(&theory);
    assert!(report.summary.undeclared_uses >= 1);
    assert_eq!(report.summary.declarations, 0);
}

#[test]
fn test_023_optional_diagnostic_does_not_change_conclusions() {
    // Same rules, one with a matching declaration and one without.
    let undeclared = parse_spl("(given bird)\n(normally r1 (bird) (flies))").unwrap();
    let declared =
        parse_spl("(given bird)\n(normally r1 (bird) (flies))\n(predicate flies ((who symbol)))")
            .unwrap();

    let a: Vec<String> = reason(&undeclared)
        .unwrap()
        .iter()
        .map(|c| format!("{} {}", c.conclusion_type.symbol(), c.literal))
        .collect();
    let b: Vec<String> = reason(&declared)
        .unwrap()
        .iter()
        .map(|c| format!("{} {}", c.conclusion_type.symbol(), c.literal))
        .collect();

    let mut a_sorted = a.clone();
    let mut b_sorted = b.clone();
    a_sorted.sort();
    b_sorted.sort();
    assert_eq!(a_sorted, b_sorted);
}

// ---------------------------------------------------------------------------
// End-to-end: SPL -> TheorySignature (spot check REQ-009 through the parser)
// ---------------------------------------------------------------------------

#[test]
fn spl_theory_signature_includes_declared_and_observed() {
    let theory = parse_spl(
        "(given bird)\n\
         (normally r1 (bird) (flies))\n\
         (predicate unused ((x symbol)))",
    )
    .unwrap();
    let signature = TheorySignature::derive(&theory);
    assert!(signature.symbols.contains(&sym("bird", 0)));
    assert!(signature.symbols.contains(&sym("flies", 0)));
    // Declared but unobserved symbol is present.
    assert!(signature.symbols.contains(&sym("unused", 1)));
}
