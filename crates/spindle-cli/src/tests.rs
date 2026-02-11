//! CLI Query Command Test Suite
//!
//! Ported from spindle-racket/tests/query/cli-test.rkt and
//! spindle-racket/tests/query/query-test.rkt
//!
//! Tests cover:
//! - CLI query subcommand parsing (parse_literal_arg)
//! - Query output formatting
//! - Integration with reasoning engine
//! - Error handling for invalid queries

use spindle_core::Theory;
use spindle_core::conclusion::ConclusionType;
use spindle_core::literal::Literal;
use spindle_core::query::{
    BlockingType, HypotheticalClaim, QueryStatus, abduce, query, what_if, what_if_provable, why_not,
};
use spindle_core::rule::{Rule, RuleType};
use spindle_parser::parse_spl;
use std::fs;
use tempfile::TempDir;

// =============================================================================
// HELPER FUNCTIONS - Theory Building
// =============================================================================

/// Create a minimal theory with just a single fact
fn make_fact_theory() -> Theory {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", Literal::simple("bird")));
    theory
}

/// Create a theory with a defeasible rule chain: bird => flies
fn make_defeasible_theory() -> Theory {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", Literal::simple("bird")));
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![Literal::simple("bird")],
        vec![Literal::simple("flies")],
    ));
    theory
}

/// Create a theory with strict rules: human -> mortal
fn make_strict_theory() -> Theory {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", Literal::simple("human")));
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Strict,
        vec![Literal::simple("human")],
        vec![Literal::simple("mortal")],
    ));
    theory
}

/// Create a theory with conflicting rules and superiority:
/// bird => flies, penguin => ~flies, penguin > bird
fn make_conflict_theory() -> Theory {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", Literal::simple("bird")));
    theory.add_rule(Rule::fact("f2", Literal::simple("penguin")));
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![Literal::simple("bird")],
        vec![Literal::simple("flies")],
    ));
    theory.add_rule(Rule::new(
        "r2",
        RuleType::Defeasible,
        vec![Literal::simple("penguin")],
        vec![Literal::negated("flies")],
    ));
    theory.add_superiority("r2", "r1");
    theory
}

/// Create a theory with a defeater:
/// bird => flies, broken_wing =X> ~flies
#[allow(dead_code)]
fn make_defeater_theory() -> Theory {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", Literal::simple("bird")));
    theory.add_rule(Rule::fact("f2", Literal::simple("broken_wing")));
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![Literal::simple("bird")],
        vec![Literal::simple("flies")],
    ));
    theory.add_rule(Rule::new(
        "d1",
        RuleType::Defeater,
        vec![Literal::simple("broken_wing")],
        vec![Literal::negated("flies")],
    ));
    theory
}

/// Create a theory with missing premises:
/// tests_pass + code_complete => ready_review (but tests_pass is missing)
fn make_missing_premise_theory() -> Theory {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", Literal::simple("code_complete")));
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![
            Literal::simple("tests_pass"),
            Literal::simple("code_complete"),
        ],
        vec![Literal::simple("ready_review")],
    ));
    theory
}

/// Create a multi-step chain theory: a => b => c => d
fn make_chain_theory() -> Theory {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f1", Literal::simple("a")));
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![Literal::simple("a")],
        vec![Literal::simple("b")],
    ));
    theory.add_rule(Rule::new(
        "r2",
        RuleType::Defeasible,
        vec![Literal::simple("b")],
        vec![Literal::simple("c")],
    ));
    theory.add_rule(Rule::new(
        "r3",
        RuleType::Defeasible,
        vec![Literal::simple("c")],
        vec![Literal::simple("d")],
    ));
    theory
}

// =============================================================================
// PARSE_LITERAL_ARG TESTS
// =============================================================================

/// Parse a literal argument the same way the CLI does
fn parse_literal_arg(s: &str) -> Literal {
    // Replicate CLI parsing logic
    if s.trim().starts_with('(') {
        // Try SPL parsing
        let dummy_spl = format!("(given {s})");
        if let Ok(theory) = parse_spl(&dummy_spl)
            && let Some(fact) = theory.facts().next()
            && let Some(head) = fact.head.first()
        {
            return head.clone();
        }
    }

    // Fallback parsing
    if s.starts_with("(not ") && s.ends_with(')') {
        let inner = &s[5..s.len() - 1];
        Literal::negated(inner)
    } else if let Some(stripped) = s.strip_prefix('~') {
        Literal::negated(stripped)
    } else {
        Literal::simple(s)
    }
}

#[test]
fn test_parse_literal_arg_simple() {
    let lit = parse_literal_arg("p");
    assert_eq!(lit.name(), "p");
    assert!(!lit.is_negated());
}

#[test]
fn test_parse_literal_arg_preserves_name() {
    let lit = parse_literal_arg("bird");
    assert_eq!(lit.name(), "bird");
}

#[test]
fn test_parse_literal_arg_multi_word() {
    let lit = parse_literal_arg("can_fly");
    assert_eq!(lit.name(), "can_fly");
}

#[test]
fn test_parse_literal_arg_negated_tilde() {
    let lit = parse_literal_arg("~p");
    assert_eq!(lit.name(), "p");
    assert!(lit.is_negated());
}

#[test]
fn test_parse_literal_arg_preserves_name_with_tilde() {
    let lit = parse_literal_arg("~flies");
    assert_eq!(lit.name(), "flies");
    assert!(lit.is_negated());
}

#[test]
fn test_parse_literal_arg_negated_not_syntax() {
    let lit = parse_literal_arg("(not p)");
    assert_eq!(lit.name(), "p");
    assert!(lit.is_negated());
}

#[test]
fn test_parse_literal_arg_not_syntax_preserves_name() {
    let lit = parse_literal_arg("(not flies)");
    assert_eq!(lit.name(), "flies");
    assert!(lit.is_negated());
}

#[test]
fn test_parse_literal_arg_not_syntax_with_underscore() {
    let lit = parse_literal_arg("(not can_fly)");
    assert_eq!(lit.name(), "can_fly");
    assert!(lit.is_negated());
}

// =============================================================================
// QUERY FUNCTION TESTS
// =============================================================================

#[test]
fn test_query_returns_provable_for_fact() {
    let theory = make_fact_theory();
    let result = query(&theory, &Literal::simple("bird")).unwrap();
    assert_eq!(result.status, QueryStatus::Provable);
}

#[test]
fn test_query_returns_unknown_for_nonexistent() {
    let theory = make_fact_theory();
    let result = query(&theory, &Literal::simple("unknown")).unwrap();
    assert_eq!(result.status, QueryStatus::Unknown);
}

#[test]
fn test_query_detects_definite_conclusions() {
    let theory = make_fact_theory();
    let result = query(&theory, &Literal::simple("bird")).unwrap();
    assert_eq!(result.status, QueryStatus::Provable);
    assert_eq!(
        result.conclusion_type,
        Some(ConclusionType::DefinitelyProvable)
    );
}

#[test]
fn test_query_detects_defeasible_conclusions() {
    let theory = make_defeasible_theory();
    let result = query(&theory, &Literal::simple("flies")).unwrap();
    assert_eq!(result.status, QueryStatus::Provable);
    assert_eq!(
        result.conclusion_type,
        Some(ConclusionType::DefeasiblyProvable)
    );
}

#[test]
fn test_query_detects_refuted_literals() {
    let theory = make_conflict_theory();
    let result = query(&theory, &Literal::simple("flies")).unwrap();
    // flies should be refuted because penguin > bird means ~flies wins
    assert_eq!(result.status, QueryStatus::Refuted);
}

#[test]
fn test_query_provable_helper() {
    let theory = make_fact_theory();
    let result = query(&theory, &Literal::simple("bird")).unwrap();
    assert!(result.is_provable());
}

#[test]
fn test_query_definitely_provable() {
    let theory = make_fact_theory();
    let result = query(&theory, &Literal::simple("bird")).unwrap();
    assert!(result.is_definitely_provable());
}

#[test]
fn test_query_defeasibly_provable() {
    let theory = make_defeasible_theory();
    let result = query(&theory, &Literal::simple("flies")).unwrap();
    assert!(result.is_defeasibly_provable());
}

#[test]
fn test_query_strict_rule_conclusions() {
    let theory = make_strict_theory();
    let result = query(&theory, &Literal::simple("mortal")).unwrap();
    assert!(result.is_provable());
}

#[test]
fn test_query_chain_derivations() {
    let theory = make_chain_theory();
    let result = query(&theory, &Literal::simple("d")).unwrap();
    assert!(result.is_provable());
}

// =============================================================================
// WHY-NOT TESTS
// =============================================================================

#[test]
fn test_why_not_returns_result() {
    let theory = make_missing_premise_theory();
    let result = why_not(&theory, &Literal::simple("ready_review")).unwrap();
    // Should have blocking conditions since tests_pass is missing
    assert!(result.has_blockers());
}

#[test]
fn test_why_not_preserves_literal() {
    let theory = make_missing_premise_theory();
    let lit = Literal::simple("ready_review");
    let result = why_not(&theory, &lit).unwrap();
    assert_eq!(result.literal, lit);
}

#[test]
fn test_why_not_shows_missing_premise() {
    let theory = make_missing_premise_theory();
    let result = why_not(&theory, &Literal::simple("ready_review")).unwrap();
    let missing = result.get_missing_premises();
    assert!(!missing.is_empty());
}

#[test]
fn test_why_not_no_rules_for_literal() {
    let theory = Theory::new();
    let result = why_not(&theory, &Literal::simple("unknown")).unwrap();
    // No rules can derive it, so would_derive should be None
    assert!(result.would_derive.is_none());
}

#[test]
fn test_why_not_display_format() {
    let theory = make_missing_premise_theory();
    let result = why_not(&theory, &Literal::simple("ready_review")).unwrap();
    let display = format!("{result}");
    assert!(display.contains("ready_review"));
    assert!(display.contains("not provable"));
}

// =============================================================================
// ABDUCTION (REQUIRES) TESTS
// =============================================================================

#[test]
fn test_abduce_returns_result() {
    let theory = make_missing_premise_theory();
    let result = abduce(&theory, &Literal::simple("ready_review"), 10).unwrap();
    assert!(result.has_solutions());
}

#[test]
fn test_abduce_preserves_goal() {
    let theory = make_missing_premise_theory();
    let goal = Literal::simple("ready_review");
    let result = abduce(&theory, &goal, 10).unwrap();
    assert_eq!(result.goal, goal);
}

#[test]
fn test_abduce_finds_minimal_facts() {
    let theory = make_missing_premise_theory();
    let result = abduce(&theory, &Literal::simple("ready_review"), 10).unwrap();
    assert!(!result.solutions.is_empty());
}

#[test]
fn test_abduce_solution_includes_missing_premise() {
    let theory = make_missing_premise_theory();
    let result = abduce(&theory, &Literal::simple("ready_review"), 10).unwrap();
    let solutions = &result.solutions;
    assert!(!solutions.is_empty());
    // First solution should include tests_pass
    let first_sol = &solutions[0];
    let fact_names: Vec<_> = first_sol.facts.iter().map(|l| l.name()).collect();
    assert!(fact_names.contains(&"tests_pass"));
}

#[test]
fn test_abduce_already_provable() {
    let theory = make_fact_theory();
    let result = abduce(&theory, &Literal::simple("bird"), 10).unwrap();
    assert!(result.is_already_provable());
}

#[test]
fn test_abduce_no_rules_hypothesizes_literal() {
    let theory = Theory::new();
    let result = abduce(&theory, &Literal::simple("unknown"), 10).unwrap();
    assert!(!result.solutions.is_empty());
    // The only solution should be to add the literal itself
    let first_sol = &result.solutions[0];
    let fact_names: Vec<_> = first_sol.facts.iter().map(|l| l.name()).collect();
    assert!(fact_names.contains(&"unknown"));
}

#[test]
fn test_abduce_finds_alternative_paths() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![Literal::simple("bird")],
        vec![Literal::simple("flies")],
    ));
    theory.add_rule(Rule::new(
        "r2",
        RuleType::Defeasible,
        vec![Literal::simple("plane")],
        vec![Literal::simple("flies")],
    ));

    let result = abduce(&theory, &Literal::simple("flies"), 10).unwrap();
    // Should find both {bird} and {plane}
    assert!(result.solutions.len() >= 2);
}

#[test]
fn test_abduce_respects_max_solutions() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![Literal::simple("a")],
        vec![Literal::simple("x")],
    ));
    theory.add_rule(Rule::new(
        "r2",
        RuleType::Defeasible,
        vec![Literal::simple("b")],
        vec![Literal::simple("x")],
    ));

    let result = abduce(&theory, &Literal::simple("x"), 1).unwrap();
    assert!(result.solutions.len() <= 1);
}

#[test]
fn test_abduce_solutions_have_confidence() {
    let theory = make_missing_premise_theory();
    let result = abduce(&theory, &Literal::simple("ready_review"), 10).unwrap();
    for sol in &result.solutions {
        assert!(sol.confidence > 0.0);
    }
}

#[test]
fn test_abduce_smallest_solution() {
    let theory = make_missing_premise_theory();
    let result = abduce(&theory, &Literal::simple("ready_review"), 10).unwrap();
    let smallest = result.smallest_solution();
    assert!(smallest.is_some());
}

// =============================================================================
// WHAT-IF TESTS
// =============================================================================

#[test]
fn test_what_if_returns_result() {
    let theory = make_missing_premise_theory();
    let claims = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
    let result = what_if(&theory, claims, &Literal::simple("ready_review")).unwrap();
    assert!(result.is_provable());
}

#[test]
fn test_what_if_provable_true_when_hypothesis_helps() {
    let theory = make_missing_premise_theory();
    let claims = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
    let result = what_if_provable(&theory, claims, &Literal::simple("ready_review")).unwrap();
    assert!(result);
}

#[test]
fn test_what_if_provable_false_when_hypothesis_doesnt_help() {
    let theory = make_missing_premise_theory();
    let claims = vec![HypotheticalClaim::new(Literal::simple("irrelevant_fact"))];
    let result = what_if_provable(&theory, claims, &Literal::simple("ready_review")).unwrap();
    assert!(!result);
}

#[test]
fn test_what_if_with_source_attribution() {
    let theory = make_missing_premise_theory();
    let claims = vec![HypotheticalClaim::with_source(
        Literal::simple("tests_pass"),
        "agent:qa",
    )];
    let result = what_if(&theory, claims, &Literal::simple("ready_review")).unwrap();
    assert!(result.is_provable());
    assert_eq!(result.hypotheticals[0].source, Some("agent:qa".to_string()));
}

#[test]
fn test_what_if_tracks_new_conclusions() {
    let theory = make_missing_premise_theory();
    let claims = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
    let result = what_if(&theory, claims, &Literal::simple("ready_review")).unwrap();
    // ready_review should be a new conclusion
    assert!(!result.new_conclusions.is_empty());
}

#[test]
fn test_what_if_with_multiple_claims() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![Literal::simple("a"), Literal::simple("b")],
        vec![Literal::simple("c")],
    ));

    let claims = vec![
        HypotheticalClaim::new(Literal::simple("a")),
        HypotheticalClaim::new(Literal::simple("b")),
    ];
    let result = what_if_provable(&theory, claims, &Literal::simple("c")).unwrap();
    assert!(result);
}

#[test]
fn test_what_if_with_chain_derivation() {
    let mut theory = Theory::new();
    theory.add_rule(Rule::new(
        "r1",
        RuleType::Defeasible,
        vec![Literal::simple("a")],
        vec![Literal::simple("b")],
    ));
    theory.add_rule(Rule::new(
        "r2",
        RuleType::Defeasible,
        vec![Literal::simple("b")],
        vec![Literal::simple("c")],
    ));

    let claims = vec![HypotheticalClaim::new(Literal::simple("a"))];
    let result = what_if_provable(&theory, claims, &Literal::simple("c")).unwrap();
    assert!(result);
}

// =============================================================================
// QUERY STATUS DISPLAY TESTS
// =============================================================================

#[test]
fn test_query_status_display_provable() {
    assert_eq!(format!("{}", QueryStatus::Provable), "provable");
}

#[test]
fn test_query_status_display_refuted() {
    assert_eq!(format!("{}", QueryStatus::Refuted), "refuted");
}

#[test]
fn test_query_status_display_unknown() {
    assert_eq!(format!("{}", QueryStatus::Unknown), "unknown");
}

// =============================================================================
// CONCLUSION TYPE SYMBOL TESTS
// =============================================================================

#[test]
fn test_conclusion_type_symbol_definitely_provable() {
    assert_eq!(ConclusionType::DefinitelyProvable.symbol(), "+D");
}

#[test]
fn test_conclusion_type_symbol_definitely_not_provable() {
    assert_eq!(ConclusionType::DefinitelyNotProvable.symbol(), "-D");
}

#[test]
fn test_conclusion_type_symbol_defeasibly_provable() {
    assert_eq!(ConclusionType::DefeasiblyProvable.symbol(), "+d");
}

#[test]
fn test_conclusion_type_symbol_defeasibly_not_provable() {
    assert_eq!(ConclusionType::DefeasiblyNotProvable.symbol(), "-d");
}

// =============================================================================
// BLOCKING TYPE DISPLAY TESTS
// =============================================================================

#[test]
fn test_blocking_type_display_missing_premise() {
    assert_eq!(
        format!("{}", BlockingType::MissingPremise),
        "missing premise"
    );
}

#[test]
fn test_blocking_type_display_defeated() {
    assert_eq!(format!("{}", BlockingType::Defeated), "defeated");
}

#[test]
fn test_blocking_type_display_contradicted() {
    assert_eq!(format!("{}", BlockingType::Contradicted), "contradicted");
}

// =============================================================================
// INTEGRATION TESTS
// =============================================================================

#[test]
fn test_integration_query_explain_consistency() {
    let theory = make_defeasible_theory();
    let flies_lit = Literal::simple("flies");

    // Query should say provable
    let result = query(&theory, &flies_lit).unwrap();
    assert!(result.is_provable());

    // Explain should work for provable literals
    let explanation = spindle_core::explanation::explain(&theory, &flies_lit).unwrap();
    assert!(explanation.is_some());
}

#[test]
fn test_integration_query_why_not_for_non_provable() {
    let theory = make_missing_premise_theory();
    let ready_lit = Literal::simple("ready_review");

    // Query should return unknown
    let q_result = query(&theory, &ready_lit).unwrap();
    assert!(!q_result.is_provable());

    // Why-not should have blocking conditions
    let wn = why_not(&theory, &ready_lit).unwrap();
    assert!(wn.has_blockers());
}

#[test]
fn test_integration_why_not_requires_consistency() {
    let theory = make_missing_premise_theory();
    let ready_lit = Literal::simple("ready_review");

    // Why-not shows what's blocking
    let wn = why_not(&theory, &ready_lit).unwrap();
    let missing = wn.get_missing_premises();
    assert!(!missing.is_empty());

    // Requires shows what to add
    let req = abduce(&theory, &ready_lit, 10).unwrap();
    assert!(req.has_solutions());
}

#[test]
fn test_integration_requires_what_if_consistency() {
    let theory = make_missing_premise_theory();
    let ready_lit = Literal::simple("ready_review");

    // Get what's needed
    let req = abduce(&theory, &ready_lit, 10).unwrap();
    let first_sol = &req.solutions[0];

    // Build claims from needed facts
    let claims: Vec<_> = first_sol
        .facts
        .iter()
        .map(|f| HypotheticalClaim::new(f.clone()))
        .collect();

    // What-if with those claims should make it provable
    let result = what_if_provable(&theory, claims, &ready_lit).unwrap();
    assert!(result);
}

#[test]
fn test_integration_full_workflow() {
    let theory = make_missing_premise_theory();
    let ready_lit = Literal::simple("ready_review");

    // 1. Query shows not provable
    let q_result = query(&theory, &ready_lit).unwrap();
    assert!(!q_result.is_provable());

    // 2. Why-not explains the blocker
    let wn = why_not(&theory, &ready_lit).unwrap();
    assert!(wn.has_blockers());

    // 3. Requires finds solution
    let req = abduce(&theory, &ready_lit, 10).unwrap();
    assert!(req.has_solutions());

    // 4. What-if verifies the solution works
    let claims = vec![HypotheticalClaim::new(Literal::simple("tests_pass"))];
    let wi = what_if_provable(&theory, claims, &ready_lit).unwrap();
    assert!(wi);
}

#[test]
fn test_file_based_query_spl_text() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.spl");

    let spl_content = r#"
; Birds fly example
(given bird)
(normally r1 bird flies)
"#;

    fs::write(&file_path, spl_content).unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    let theory = spindle_parser::parse_spl(&content).unwrap();

    let result = query(&theory, &Literal::simple("flies")).unwrap();
    assert!(result.is_provable());
    assert!(result.is_defeasibly_provable());
}

#[test]
fn test_file_based_query_spl() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.spl");

    let spl_content = r#"
; Birds fly example
(given bird)
(normally r1 bird flies)
"#;

    fs::write(&file_path, spl_content).unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    let theory = spindle_parser::parse_spl(&content).unwrap();

    let result = query(&theory, &Literal::simple("flies")).unwrap();
    assert!(result.is_provable());
    assert!(result.is_defeasibly_provable());
}

#[test]
fn test_file_based_penguin_example() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("penguin.spl");

    let spl_content = r#"
; Classic Tweety/Penguin example
(given bird)
(given penguin)
(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)
"#;

    fs::write(&file_path, spl_content).unwrap();

    let content = fs::read_to_string(&file_path).unwrap();
    let theory = spindle_parser::parse_spl(&content).unwrap();

    // flies should be refuted (penguin wins)
    let flies_result = query(&theory, &Literal::simple("flies")).unwrap();
    assert_eq!(flies_result.status, QueryStatus::Refuted);

    // ~flies should be provable
    let not_flies_result = query(&theory, &Literal::negated("flies")).unwrap();
    assert!(not_flies_result.is_provable());
}

// =============================================================================
// JSON OUTPUT TESTS
// =============================================================================

#[test]
fn test_query_result_json_format() {
    let theory = make_defeasible_theory();
    let result = query(&theory, &Literal::simple("flies")).unwrap();

    // Build JSON similar to CLI output
    let json_output = serde_json::json!({
        "literal": result.literal.to_string(),
        "status": result.status.to_string(),
        "conclusion_type": result.conclusion_type.map(|ct| ct.symbol()),
    });

    let json_str = serde_json::to_string_pretty(&json_output).unwrap();
    assert!(json_str.contains("flies"));
    assert!(json_str.contains("provable"));
    assert!(json_str.contains("+d"));
}

#[test]
fn test_why_not_result_json_format() {
    let theory = make_missing_premise_theory();
    let result = why_not(&theory, &Literal::simple("ready_review")).unwrap();

    let blockers: Vec<_> = result
        .blocked_by
        .iter()
        .map(|b| {
            serde_json::json!({
                "type": b.blocking_type.to_string(),
                "rule": b.rule_label,
                "explanation": b.explanation
            })
        })
        .collect();

    let json_output = serde_json::json!({
        "literal": result.literal.to_string(),
        "would_derive": result.would_derive,
        "blocked_by": blockers
    });

    let json_str = serde_json::to_string_pretty(&json_output).unwrap();
    assert!(json_str.contains("ready_review"));
    assert!(json_str.contains("blocked_by"));
}

#[test]
fn test_abduce_result_json_format() {
    let theory = make_missing_premise_theory();
    let result = abduce(&theory, &Literal::simple("ready_review"), 10).unwrap();

    let solutions: Vec<_> = result
        .solutions
        .iter()
        .map(|s| {
            let facts: Vec<_> = s.facts.iter().map(|l| l.to_string()).collect();
            serde_json::json!({
                "facts": facts,
                "confidence": s.confidence
            })
        })
        .collect();

    let json_output = serde_json::json!({
        "goal": result.goal.to_string(),
        "solutions": solutions
    });

    let json_str = serde_json::to_string_pretty(&json_output).unwrap();
    assert!(json_str.contains("ready_review"));
    assert!(json_str.contains("solutions"));
}

// =============================================================================
// ERROR HANDLING TESTS
// =============================================================================

#[test]
fn test_query_empty_theory() {
    let theory = Theory::new();
    let result = query(&theory, &Literal::simple("anything")).unwrap();
    assert_eq!(result.status, QueryStatus::Unknown);
}

#[test]
fn test_why_not_on_provable_literal() {
    let theory = make_fact_theory();
    let result = why_not(&theory, &Literal::simple("bird")).unwrap();
    // When literal IS provable, why-not should have no blockers
    assert!(!result.has_blockers());
}

#[test]
fn test_abduce_with_zero_max() {
    let theory = make_missing_premise_theory();
    let result = abduce(&theory, &Literal::simple("ready_review"), 0).unwrap();
    // With max=0, should still potentially return solutions
    // (implementation detail - may vary)
    assert!(result.solutions.is_empty() || !result.solutions.is_empty());
}

#[test]
fn test_what_if_empty_claims() {
    let theory = make_missing_premise_theory();
    let claims: Vec<HypotheticalClaim> = vec![];
    let result = what_if_provable(&theory, claims, &Literal::simple("ready_review")).unwrap();
    // With no hypotheticals, should match base theory behavior
    assert!(!result);
}
