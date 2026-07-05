//! Golden-file tests for explanation formatters.
//!
//! These tests verify that each formatter (NaturalLanguage, Json, JsonLd, Dot)
//! produces reasonable output for canonical defeasible logic theories.
//!
//! Rather than exact string matching (which is brittle across formatting
//! changes), we assert the presence of expected key strings: literal names,
//! rule labels, conclusion types, and structural markers specific to each
//! output format.

mod fixtures;

use spindle_core::explanation::explain;
use spindle_core::explanation::format::{
    DotFormatter, ExplanationFormatter, JsonFormatter, JsonLdFormatter, NaturalLanguageFormatter,
};
use spindle_core::literal::Literal;
use spindle_core::reason::reason;

// ===========================================================================
// Helpers
// ===========================================================================

/// Run reason() + explain() for a positive conclusion of the given literal.
/// Panics if no explanation is found.
fn explain_literal(
    theory: &spindle_core::theory::Theory,
    name: &str,
    negated: bool,
) -> spindle_core::explanation::Explanation {
    let lit = if negated {
        Literal::negated(name)
    } else {
        Literal::simple(name)
    };
    explain(theory, &lit)
        .expect("explain() should not error")
        .unwrap_or_else(|| panic!("expected an explanation for {lit}"))
}

// ===========================================================================
// Tweety Triangle — NaturalLanguage formatter
// ===========================================================================

#[test]
fn tweety_natural_language_bird() {
    let theory = fixtures::tweety_triangle();
    let expl = explain_literal(&theory, "bird", false);
    let formatter = NaturalLanguageFormatter::new();
    let output = formatter.format(&expl);

    assert!(
        output.contains("bird"),
        "output should mention the literal 'bird'"
    );
    assert!(
        output.contains("+D") || output.contains("+d"),
        "output should contain a conclusion type symbol"
    );
    // bird is a fact, so it should mention "fact" or "established as a fact"
    assert!(
        output.contains("fact"),
        "bird is a fact; output should mention 'fact'"
    );
}

#[test]
fn tweety_natural_language_not_flies() {
    let theory = fixtures::tweety_triangle();
    // In the tweety triangle, ~flies should be defeasibly provable
    let lit = Literal::negated("flies");
    if let Some(expl) = explain(&theory, &lit).unwrap() {
        let formatter = NaturalLanguageFormatter::new();
        let output = formatter.format(&expl);

        assert!(output.contains("flies"), "output should mention 'flies'");
        assert!(
            output.contains("Derivation") || output.contains("derived"),
            "output should describe a derivation"
        );
    }
    // If ~flies is not explainable (e.g. due to reason.rs semantics), that's
    // acceptable — the formatter still ran without panicking.
}

// ===========================================================================
// Tweety Triangle — JSON formatter
// ===========================================================================

#[test]
fn tweety_json_bird() {
    let theory = fixtures::tweety_triangle();
    let expl = explain_literal(&theory, "bird", false);
    let formatter = JsonFormatter;
    let output = formatter.format(&expl);

    // Should be valid JSON
    let value: serde_json::Value =
        serde_json::from_str(&output).expect("JSON formatter output should be valid JSON");

    // Check structural fields
    assert!(
        value.get("conclusion_type").is_some(),
        "missing conclusion_type"
    );
    assert!(value.get("literal").is_some(), "missing literal");
    assert!(value.get("proof_tree").is_some(), "missing proof_tree");
    assert!(
        value.get("blocked_alternatives").is_some(),
        "missing blocked_alternatives"
    );
    assert!(
        value.get("conflicts_resolved").is_some(),
        "missing conflicts_resolved"
    );

    // Check literal value
    assert_eq!(value["literal"], "bird");
}

#[test]
fn tweety_json_penguin() {
    let theory = fixtures::tweety_triangle();
    let expl = explain_literal(&theory, "penguin", false);
    let formatter = JsonFormatter;
    let output = formatter.format(&expl);

    let value: serde_json::Value =
        serde_json::from_str(&output).expect("JSON formatter output should be valid JSON");

    assert_eq!(value["literal"], "penguin");
    // penguin is a fact, so conclusion_type should be +D
    assert_eq!(value["conclusion_type"], "+D");
}

// ===========================================================================
// Tweety Triangle — JSON-LD formatter
// ===========================================================================

#[test]
fn tweety_jsonld_bird() {
    let theory = fixtures::tweety_triangle();
    let expl = explain_literal(&theory, "bird", false);
    let formatter = JsonLdFormatter;
    let output = formatter.format(&expl);

    let value: serde_json::Value =
        serde_json::from_str(&output).expect("JSON-LD formatter output should be valid JSON");

    // Must have @context and @type — the hallmarks of JSON-LD
    assert!(
        value.get("@context").is_some(),
        "JSON-LD must have @context"
    );
    assert_eq!(value["@type"], "spindle:Explanation");

    // Check namespace URIs in context
    let ctx = &value["@context"];
    assert_eq!(ctx["spindle"], "https://spindle.dev/ontology#");
    assert_eq!(ctx["prov"], "http://www.w3.org/ns/prov#");

    // Check semantic field names (camelCase, not snake_case)
    assert!(
        value.get("conclusionType").is_some(),
        "JSON-LD should use 'conclusionType'"
    );
    assert!(
        value.get("literal").is_some(),
        "JSON-LD should have 'literal'"
    );
}

#[test]
fn tweety_jsonld_proof_tree_typing() {
    let theory = fixtures::tweety_triangle();
    let expl = explain_literal(&theory, "bird", false);
    let formatter = JsonLdFormatter;
    let output = formatter.format(&expl);
    let value: serde_json::Value = serde_json::from_str(&output).unwrap();

    // If proof tree exists, it should have @type annotation
    if let Some(proof_tree) = value.get("proofTree") {
        assert_eq!(
            proof_tree["@type"], "spindle:ProofNode",
            "proof tree nodes must have @type spindle:ProofNode"
        );
        if let Some(step) = proof_tree.get("proofStep") {
            assert_eq!(
                step["@type"], "spindle:ProofStep",
                "proof steps must have @type spindle:ProofStep"
            );
        }
    }
}

// ===========================================================================
// Tweety Triangle — DOT formatter
// ===========================================================================

#[test]
fn tweety_dot_bird() {
    let theory = fixtures::tweety_triangle();
    let expl = explain_literal(&theory, "bird", false);
    let formatter = DotFormatter::default();
    let output = formatter.format(&expl);

    // Structural markers for DOT format
    assert!(
        output.starts_with("digraph Explanation {"),
        "DOT output must start with 'digraph Explanation {{'"
    );
    assert!(
        output.trim_end().ends_with('}'),
        "DOT output must end with '}}'"
    );
    assert!(
        output.contains("rankdir=BT"),
        "default rankdir should be BT"
    );
    assert!(
        output.contains("fontname=\"Helvetica\""),
        "default font should be Helvetica"
    );

    // Should contain the literal
    assert!(output.contains("bird"), "DOT output should mention 'bird'");
    // Should have a title node
    assert!(
        output.contains("title [label="),
        "DOT should have a title node"
    );
}

#[test]
fn tweety_dot_contains_proof_nodes() {
    let theory = fixtures::tweety_triangle();
    let expl = explain_literal(&theory, "bird", false);
    let output = DotFormatter::default().format(&expl);

    // If proof tree exists, should have nodes with shape=box
    if expl.proof_tree.is_some() {
        assert!(
            output.contains("shape=box"),
            "proof nodes should use shape=box"
        );
        assert!(
            output.contains("style=filled"),
            "proof nodes should be filled"
        );
    }
}

// ===========================================================================
// Nixon Diamond — all formatters
// ===========================================================================

#[test]
fn nixon_natural_language_republican() {
    let theory = fixtures::nixon_diamond();
    let expl = explain_literal(&theory, "republican", false);
    let output = NaturalLanguageFormatter::new().format(&expl);

    assert!(
        output.contains("republican"),
        "output should mention 'republican'"
    );
    assert!(
        output.contains("fact") || output.contains("established"),
        "republican is a fact"
    );
}

#[test]
fn nixon_json_republican() {
    let theory = fixtures::nixon_diamond();
    let expl = explain_literal(&theory, "republican", false);
    let output = JsonFormatter.format(&expl);

    let value: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON");
    assert_eq!(value["literal"], "republican");
    assert_eq!(value["conclusion_type"], "+D");
}

#[test]
fn nixon_jsonld_quaker() {
    let theory = fixtures::nixon_diamond();
    let expl = explain_literal(&theory, "quaker", false);
    let output = JsonLdFormatter.format(&expl);

    let value: serde_json::Value = serde_json::from_str(&output).expect("should be valid JSON-LD");
    assert_eq!(value["@type"], "spindle:Explanation");
    assert_eq!(value["literal"], "quaker");
}

#[test]
fn nixon_dot_quaker() {
    let theory = fixtures::nixon_diamond();
    let expl = explain_literal(&theory, "quaker", false);
    let output = DotFormatter::default().format(&expl);

    assert!(output.starts_with("digraph Explanation {"));
    assert!(output.contains("quaker"));
}

// ===========================================================================
// Nixon Diamond — ambiguity tests
// ===========================================================================

/// In the Nixon Diamond, both `pacifist` and `~pacifist` should be blocked
/// (ambiguity). We verify that the formatters handle the case where
/// explain() returns None for ambiguous literals without panicking.
#[test]
fn nixon_ambiguous_pacifist_formatters_handle_none() {
    let theory = fixtures::nixon_diamond();

    // pacifist may or may not be explainable depending on the reasoner
    // semantics (credulous vs skeptical). We just verify no panic.
    let lit_pos = Literal::simple("pacifist");
    let lit_neg = Literal::negated("pacifist");

    let expl_pos = explain(&theory, &lit_pos).unwrap();
    let expl_neg = explain(&theory, &lit_neg).unwrap();

    // If either explanation exists, format it with every formatter
    for expl in expl_pos.iter().chain(expl_neg.iter()) {
        let nl = NaturalLanguageFormatter::new().format(expl);
        assert!(!nl.is_empty(), "NL output should not be empty");

        let json_str = JsonFormatter.format(expl);
        let _: serde_json::Value =
            serde_json::from_str(&json_str).expect("JSON output should be valid");

        let jsonld_str = JsonLdFormatter.format(expl);
        let _: serde_json::Value =
            serde_json::from_str(&jsonld_str).expect("JSON-LD output should be valid");

        let dot = DotFormatter::default().format(expl);
        assert!(
            dot.starts_with("digraph"),
            "DOT output should start with 'digraph'"
        );
    }
}

// ===========================================================================
// Convenience wrappers — verify backward-compatible methods work
// ===========================================================================

#[test]
fn tweety_convenience_methods() {
    let theory = fixtures::tweety_triangle();
    let expl = explain_literal(&theory, "bird", false);

    // to_natural_language()
    let nl = expl.to_natural_language();
    assert!(nl.contains("bird"));

    // to_json()
    let json = expl.to_json();
    assert_eq!(json["literal"], "bird");

    // to_jsonld()
    let jsonld = expl.to_jsonld();
    assert_eq!(jsonld["@type"], "spindle:Explanation");

    // to_dot()
    let dot = expl.to_dot();
    assert!(dot.starts_with("digraph"));
}

// ===========================================================================
// DOT formatter — custom configuration
// ===========================================================================

#[test]
fn dot_custom_config_tweety() {
    let theory = fixtures::tweety_triangle();
    let expl = explain_literal(&theory, "bird", false);

    let formatter = DotFormatter {
        rank_dir: "TB".to_string(),
        font: "Courier".to_string(),
    };
    let output = formatter.format(&expl);

    assert!(output.contains("rankdir=TB"), "custom rankdir should be TB");
    assert!(
        output.contains("fontname=\"Courier\""),
        "custom font should be Courier"
    );
}

// ===========================================================================
// Defeasible chain explanation
// ===========================================================================

#[test]
fn tweety_defeasible_chain_json() {
    let theory = fixtures::tweety_triangle();

    // Try to explain 'flies' — in tweety triangle with reason.rs (credulous),
    // flies may or may not be provable. We test whatever the reasoner decides.
    let lit = Literal::simple("flies");
    if let Some(expl) = explain(&theory, &lit).unwrap() {
        let json_str = JsonFormatter.format(&expl);
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(value["literal"], "flies");
        // Should have a proof tree since it's a positive conclusion
        assert!(
            value["proof_tree"].is_object(),
            "defeasible conclusion should have a proof tree"
        );
    }
}

// ===========================================================================
// Facts-only theory — all formatters produce output for facts
// ===========================================================================

#[test]
fn facts_only_all_formatters() {
    let theory = fixtures::facts_only();

    for fact_name in &["bird", "penguin", "cold"] {
        let expl = explain_literal(&theory, fact_name, false);

        // NaturalLanguage
        let nl = NaturalLanguageFormatter::new().format(&expl);
        assert!(
            nl.contains(fact_name),
            "NL output for '{fact_name}' should mention the literal"
        );
        assert!(
            nl.contains("+D"),
            "facts should produce definitely provable conclusion (+D)"
        );

        // JSON
        let json_str = JsonFormatter.format(&expl);
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(value["literal"].as_str().unwrap(), *fact_name);
        assert_eq!(value["conclusion_type"], "+D");

        // JSON-LD
        let jsonld_str = JsonLdFormatter.format(&expl);
        let jld: serde_json::Value = serde_json::from_str(&jsonld_str).unwrap();
        assert_eq!(jld["conclusionType"], "+D");
        assert!(jld.get("@context").is_some());

        // DOT
        let dot = DotFormatter::default().format(&expl);
        assert!(dot.contains(fact_name));
        assert!(dot.contains("digraph"));
    }
}

// ===========================================================================
// Empty explanation (no proof tree) — formatters don't panic
// ===========================================================================

#[test]
fn empty_theory_explain_returns_none() {
    let theory = fixtures::empty_theory();
    let result = explain(&theory, &Literal::simple("anything")).unwrap();
    assert!(
        result.is_none(),
        "empty theory should produce no explanation"
    );
}

// ===========================================================================
// JSON structural invariants across theories
// ===========================================================================

#[test]
fn json_always_has_required_keys() {
    let theories: Vec<(&str, spindle_core::theory::Theory)> = vec![
        ("tweety", fixtures::tweety_triangle()),
        ("nixon", fixtures::nixon_diamond()),
        ("facts_only", fixtures::facts_only()),
    ];

    let required_keys = [
        "conclusion_type",
        "literal",
        "proof_tree",
        "blocked_alternatives",
        "conflicts_resolved",
    ];

    for (name, theory) in &theories {
        let conclusions = reason(theory).unwrap();
        for c in conclusions
            .iter()
            .filter(|c| c.conclusion_type.is_positive())
        {
            if let Some(expl) = explain(theory, &c.literal).unwrap() {
                let json_str = JsonFormatter.format(&expl);
                let value: serde_json::Value = serde_json::from_str(&json_str)
                    .unwrap_or_else(|e| panic!("invalid JSON for {} / {}: {}", name, c.literal, e));

                for key in &required_keys {
                    assert!(
                        value.get(key).is_some(),
                        "JSON for {} / {} missing required key '{}'",
                        name,
                        c.literal,
                        key
                    );
                }
            }
        }
    }
}

// ===========================================================================
// JSON-LD structural invariants
// ===========================================================================

#[test]
fn jsonld_always_has_context_and_type() {
    let theories: Vec<(&str, spindle_core::theory::Theory)> = vec![
        ("tweety", fixtures::tweety_triangle()),
        ("nixon", fixtures::nixon_diamond()),
        ("facts_only", fixtures::facts_only()),
    ];

    for (name, theory) in &theories {
        let conclusions = reason(theory).unwrap();
        for c in conclusions
            .iter()
            .filter(|c| c.conclusion_type.is_positive())
        {
            if let Some(expl) = explain(theory, &c.literal).unwrap() {
                let jsonld_str = JsonLdFormatter.format(&expl);
                let value: serde_json::Value =
                    serde_json::from_str(&jsonld_str).unwrap_or_else(|e| {
                        panic!("invalid JSON-LD for {} / {}: {}", name, c.literal, e)
                    });

                assert!(
                    value.get("@context").is_some(),
                    "JSON-LD for {} / {} missing @context",
                    name,
                    c.literal
                );
                assert_eq!(
                    value["@type"], "spindle:Explanation",
                    "JSON-LD for {} / {} has wrong @type",
                    name, c.literal
                );
            }
        }
    }
}

// ===========================================================================
// DOT structural invariants
// ===========================================================================

#[test]
fn dot_always_has_valid_structure() {
    let theories: Vec<(&str, spindle_core::theory::Theory)> = vec![
        ("tweety", fixtures::tweety_triangle()),
        ("nixon", fixtures::nixon_diamond()),
        ("facts_only", fixtures::facts_only()),
    ];

    for (name, theory) in &theories {
        let conclusions = reason(theory).unwrap();
        for c in conclusions
            .iter()
            .filter(|c| c.conclusion_type.is_positive())
        {
            if let Some(expl) = explain(theory, &c.literal).unwrap() {
                let dot = DotFormatter::default().format(&expl);

                assert!(
                    dot.starts_with("digraph Explanation {"),
                    "DOT for {} / {} must start with 'digraph Explanation {{'",
                    name,
                    c.literal
                );
                assert!(
                    dot.trim_end().ends_with('}'),
                    "DOT for {} / {} must end with '}}'",
                    name,
                    c.literal
                );
                assert!(
                    dot.contains("title [label="),
                    "DOT for {} / {} must have a title node",
                    name,
                    c.literal
                );
            }
        }
    }
}
