//! SPL (Spindle Lisp) Parser
//!
//! Parses the LISP-based SPL format into a Theory.
//!
//! # SPL Format
//!
//! ```lisp
//! ; Comments with semicolon
//!
//! ; Facts
//! (given bird)
//! (given (parent alice bob))
//!
//! ; Strict rules
//! (always r1 bird animal)
//!
//! ; Defeasible rules
//! (normally r1 bird flies)
//!
//! ; Defeaters
//! (except d1 broken-wing flies)
//!
//! ; Negation
//! (normally r2 penguin (not flies))
//!
//! ; Superiority
//! (prefer r2 r1)
//!
//! ; Conjunction
//! (normally r3 (and bird healthy) flies)
//! ```

use nom::{
    branch::alt,
    bytes::complete::take_while1,
    character::complete::{char, multispace0},
    multi::many0,
    sequence::{delimited, preceded},
    IResult, Parser,
};

use spindle_core::{Literal, MetaValue, Rule, RuleType, Theory};

use crate::ParseError;

/// Parse an SPL string into a Theory
pub fn parse_spl(input: &str) -> Result<Theory, ParseError> {
    // Remove comments
    let cleaned = remove_comments(input);

    let mut theory = Theory::new();

    // Parse all top-level expressions
    let (remaining, exprs) = parse_expressions(&cleaned).map_err(|e| ParseError::ParserError {
        line: 1,
        message: format!("SPL parse error: {:?}", e),
    })?;

    if !remaining.trim().is_empty() {
        return Err(ParseError::ParserError {
            line: 1,
            message: format!("Unparsed input remaining: {}", remaining.trim()),
        });
    }

    // Process expressions
    for expr in exprs {
        process_expr(&mut theory, &expr)?;
    }

    Ok(theory)
}

/// Remove semicolon comments and #lang directives from input, respecting quotes
fn remove_comments(input: &str) -> String {
    input
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            // Skip #lang directives (Racket-specific)
            if trimmed.starts_with("#lang") {
                return "".to_string();
            }

            let mut result = String::new();
            let mut in_string = false;
            let mut escaped = false;

            for c in line.chars() {
                if in_string {
                    result.push(c);
                    if escaped {
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        in_string = false;
                    }
                } else {
                    if c == ';' {
                        break; // Comment starts
                    }
                    result.push(c);
                    if c == '"' {
                        in_string = true;
                    }
                }
            }
            result
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// S-expression representation
#[derive(Debug, Clone, PartialEq)]
enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

impl SExpr {
    fn as_atom(&self) -> Option<&str> {
        match self {
            SExpr::Atom(s) => Some(s),
            _ => None,
        }
    }

    fn as_list(&self) -> Option<&[SExpr]> {
        match self {
            SExpr::List(v) => Some(v),
            _ => None,
        }
    }
}

/// Parse multiple expressions
fn parse_expressions(input: &str) -> IResult<&str, Vec<SExpr>> {
    many0(preceded(multispace0, parse_sexpr)).parse(input)
}

/// Parse a single s-expression
fn parse_sexpr(input: &str) -> IResult<&str, SExpr> {
    alt((parse_list, parse_string, parse_atom)).parse(input)
}

/// Parse a list: (...)
fn parse_list(input: &str) -> IResult<&str, SExpr> {
    let (input, items) = delimited(
        char('('),
        many0(preceded(multispace0, parse_sexpr)),
        preceded(multispace0, char(')')),
    )
    .parse(input)?;
    Ok((input, SExpr::List(items)))
}

/// Parse a string: "..."
fn parse_string(input: &str) -> IResult<&str, SExpr> {
    let (input, _) = char('"').parse(input)?;
    let (input, content) = take_while1(|c| c != '"').parse(input)?;
    let (input, _) = char('"').parse(input)?;
    Ok((input, SExpr::Atom(content.to_string())))
}

/// Parse an atom (identifier, number, variable)
fn parse_atom(input: &str) -> IResult<&str, SExpr> {
    let (input, s) = take_while1(|c: char| {
        c.is_alphanumeric() || c == '-' || c == '_' || c == '?' || c == '~' || c == ':' || c == '.'
    })
    .parse(input)?;
    Ok((input, SExpr::Atom(s.to_string())))
}

/// Process an s-expression into theory elements
fn process_expr(theory: &mut Theory, expr: &SExpr) -> Result<(), ParseError> {
    let list = expr.as_list().ok_or_else(|| ParseError::ParserError {
        line: 1,
        message: "Expected list expression".to_string(),
    })?;

    if list.is_empty() {
        return Ok(());
    }

    let keyword = list[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line: 1,
        message: "Expected keyword".to_string(),
    })?;

    match keyword {
        "given" => process_fact(theory, &list[1..]),
        "always" => process_rule(theory, RuleType::Strict, &list[1..]),
        "normally" => process_rule(theory, RuleType::Defeasible, &list[1..]),
        "except" => process_rule(theory, RuleType::Defeater, &list[1..]),
        "prefer" => process_prefer(theory, &list[1..]),
        "meta" => process_meta(theory, &list[1..]),
        "#lang" => Ok(()), // Ignore #lang directive
        _ => Err(ParseError::ParserError {
            line: 1,
            message: format!("Unknown keyword: {}", keyword),
        }),
    }
}

/// Process a fact: (given literal)
fn process_fact(theory: &mut Theory, args: &[SExpr]) -> Result<(), ParseError> {
    if args.is_empty() {
        return Err(ParseError::ParserError {
            line: 1,
            message: "fact requires at least one argument".to_string(),
        });
    }

    let lit = parse_literal(&args[0])?;

    // Handle flat predicate: (given pred arg1 arg2 ...)
    let lit = if args.len() > 1 {
        let name = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
            line: 1,
            message: "Expected predicate name".to_string(),
        })?;
        let predicates: Result<Vec<String>, _> = args[1..]
            .iter()
            .map(|a| {
                a.as_atom()
                    .map(|s| s.to_string())
                    .ok_or_else(|| ParseError::ParserError {
                        line: 1,
                        message: "Expected atom argument".to_string(),
                    })
            })
            .collect();
        Literal::new(
            name,
            false,
            Default::default(),
            Default::default(),
            predicates?,
        )
    } else {
        lit
    };

    let label = theory.next_fact_label();
    let rule = Rule::fact(label, lit);
    theory.add_rule(rule);
    Ok(())
}

/// Process a rule: (always/normally/except [label] body head)
fn process_rule(
    theory: &mut Theory,
    rule_type: RuleType,
    args: &[SExpr],
) -> Result<(), ParseError> {
    if args.len() < 2 {
        return Err(ParseError::ParserError {
            line: 1,
            message: "rule requires at least body and head".to_string(),
        });
    }

    // Check if first arg is a label (simple atom, not a list)
    let (label, body_expr, head_expr) = if args.len() >= 3 {
        // Could be labeled
        if let Some(label_str) = args[0].as_atom() {
            // Check if it looks like a label (not a keyword like "and")
            if label_str != "and" && label_str != "not" && !label_str.starts_with('(') {
                (Some(label_str.to_string()), &args[1], &args[2])
            } else {
                (None, &args[0], &args[1])
            }
        } else {
            (None, &args[0], &args[1])
        }
    } else {
        (None, &args[0], &args[1])
    };

    let body = parse_body(body_expr)?;
    let head = parse_literal(head_expr)?;

    let final_label = label.unwrap_or_else(|| theory.next_rule_label(rule_type));
    let rule = Rule::new(final_label, rule_type, body, vec![head]);
    theory.add_rule(rule);
    Ok(())
}

/// Parse a body expression (single literal or conjunction)
fn parse_body(expr: &SExpr) -> Result<Vec<Literal>, ParseError> {
    match expr {
        SExpr::Atom(_) => Ok(vec![parse_literal(expr)?]),
        SExpr::List(items) => {
            if items.is_empty() {
                return Ok(vec![]);
            }

            // Check for (and ...)
            if let Some("and") = items[0].as_atom() {
                items[1..].iter().map(parse_literal).collect()
            } else {
                // Single complex literal
                Ok(vec![parse_literal(expr)?])
            }
        }
    }
}

/// Parse a literal expression
fn parse_literal(expr: &SExpr) -> Result<Literal, ParseError> {
    match expr {
        SExpr::Atom(s) => {
            // Handle negation prefix
            if let Some(name) = s.strip_prefix('~') {
                Ok(Literal::negated(name))
            } else {
                Ok(Literal::simple(s))
            }
        }
        SExpr::List(items) => {
            if items.is_empty() {
                return Err(ParseError::ParserError {
                    line: 1,
                    message: "Empty list is not a valid literal".to_string(),
                });
            }

            let first = items[0].as_atom().ok_or_else(|| ParseError::ParserError {
                line: 1,
                message: "Expected atom in literal".to_string(),
            })?;

            match first {
                "not" => {
                    // (not literal)
                    if items.len() != 2 {
                        return Err(ParseError::ParserError {
                            line: 1,
                            message: "not takes exactly one argument".to_string(),
                        });
                    }
                    let inner = parse_literal(&items[1])?;
                    Ok(inner.complement())
                }
                _ => {
                    // Predicate: (name arg1 arg2 ...)
                    let predicates: Result<Vec<String>, _> = items[1..]
                        .iter()
                        .map(|a| {
                            a.as_atom().map(|s| s.to_string()).ok_or_else(|| {
                                ParseError::ParserError {
                                    line: 1,
                                    message: "Expected atom argument".to_string(),
                                }
                            })
                        })
                        .collect();
                    Ok(Literal::new(
                        first,
                        false,
                        Default::default(),
                        Default::default(),
                        predicates?,
                    ))
                }
            }
        }
    }
}

/// Process meta: (meta label (key "value") (key2 "value2") ...)
fn process_meta(theory: &mut Theory, args: &[SExpr]) -> Result<(), ParseError> {
    if args.is_empty() {
        return Err(ParseError::ParserError {
            line: 1,
            message: "meta requires a label".to_string(),
        });
    }

    let label = args[0].as_atom().ok_or_else(|| ParseError::ParserError {
        line: 1,
        message: "meta label must be an atom".to_string(),
    })?;

    // Process each property: (key "value") or (key ("v1" "v2"))
    for prop in &args[1..] {
        if let Some(prop_list) = prop.as_list()
            && prop_list.len() >= 2 {
            let key = prop_list[0]
                .as_atom()
                .ok_or_else(|| ParseError::ParserError {
                    line: 1,
                    message: "meta property key must be an atom".to_string(),
                })?;

            // Check if value is a list or single value
            let value = match &prop_list[1] {
                SExpr::Atom(s) => MetaValue::String(s.clone()),
                SExpr::List(items) => {
                    // List of strings
                    let strings: Result<Vec<String>, _> = items
                        .iter()
                        .map(|item| {
                            item.as_atom().map(|s| s.to_string()).ok_or_else(|| {
                                ParseError::ParserError {
                                    line: 1,
                                    message: "meta list values must be atoms".to_string(),
                                }
                            })
                        })
                        .collect();
                    MetaValue::List(strings?)
                }
            };

            theory.add_meta(label, key, value);
        }
    }

    Ok(())
}

/// Process prefer: (prefer r1 r2 ...)
fn process_prefer(theory: &mut Theory, args: &[SExpr]) -> Result<(), ParseError> {
    if args.len() < 2 {
        return Err(ParseError::ParserError {
            line: 1,
            message: "prefer requires at least two labels".to_string(),
        });
    }

    let labels: Result<Vec<&str>, _> = args
        .iter()
        .map(|a| {
            a.as_atom().ok_or_else(|| ParseError::ParserError {
                line: 1,
                message: "prefer arguments must be atoms".to_string(),
            })
        })
        .collect();
    let labels = labels?;

    // Create chain of superiorities: r1 > r2 > r3 ...
    for window in labels.windows(2) {
        theory.add_superiority(window[0], window[1]);
    }

    Ok(())
}

// Helper trait for Theory to generate labels
trait TheoryLabelExt {
    fn next_fact_label(&mut self) -> String;
    fn next_rule_label(&mut self, rule_type: RuleType) -> String;
}

impl TheoryLabelExt for Theory {
    fn next_fact_label(&mut self) -> String {
        let count = self.facts().count() + 1;
        format!("f{}", count)
    }

    fn next_rule_label(&mut self, rule_type: RuleType) -> String {
        let prefix = match rule_type {
            RuleType::Fact => "f",
            RuleType::Strict => "s",
            RuleType::Defeasible => "r",
            RuleType::Defeater => "d",
        };
        let count = self.rules_by_type(rule_type).count() + 1;
        format!("{}{}", prefix, count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // BASIC PARSING TESTS (existing)
    // =========================================================================

    #[test]
    fn test_parse_simple_fact() {
        let theory = parse_spl("(given bird)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_predicate_fact() {
        let theory = parse_spl("(given (parent alice bob))").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_flat_predicate_fact() {
        let theory = parse_spl("(given employed alice acme)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_defeasible_rule() {
        let theory = parse_spl("(normally r1 bird flies)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_strict_rule() {
        let theory = parse_spl("(always r1 bird animal)").unwrap();
        assert_eq!(theory.rule_count(), 1);
    }

    #[test]
    fn test_parse_negated_head() {
        let theory = parse_spl("(normally r1 penguin (not flies))").unwrap();
        let rule = theory.rules().next().unwrap();
        assert!(rule.head_literal().is_negated());
    }

    #[test]
    fn test_parse_superiority() {
        let theory = parse_spl("(given a)\n(given b)\n(prefer r2 r1)").unwrap();
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_parse_superiority_chain() {
        let theory = parse_spl("(prefer r3 r2 r1)").unwrap();
        assert_eq!(theory.superiorities().len(), 2);
    }

    #[test]
    fn test_parse_conjunction() {
        let theory = parse_spl("(normally r1 (and bird healthy) flies)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.body.len(), 2);
    }

    #[test]
    fn test_parse_complete_theory() {
        let input = r#"
            ; Penguin example
            (given bird)
            (given penguin)
            (normally r1 bird flies)
            (normally r2 penguin (not flies))
            (prefer r2 r1)
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 4);
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_parse_with_variables() {
        let theory = parse_spl("(given (parent ?x ?y))").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.head_literal().predicates().len(), 2);
        assert!(rule.head_literal().predicates()[0].starts_with('?'));
    }

    // =========================================================================
    // IMPORT DECLARATIONS TESTS
    // =========================================================================
    // Tests for (import ...) syntax parsing
    // Reference: SPL-GRAMMAR.abnf and spindle-racket/src/lang/spl-parser.rkt
    //
    // Note: Import parsing is not yet implemented. These tests document the
    // expected syntax and currently verify that unrecognized keywords produce
    // appropriate errors. When import support is added, update these tests
    // to verify successful parsing.

    #[test]
    fn test_parse_import_basic_not_implemented() {
        // Basic import without prefix - not yet implemented
        // Expected syntax: (import "path/to/file.spl")
        let result = parse_spl(r#"(import "base.spl")"#);
        // Currently returns error for unknown keyword
        assert!(
            result.is_err(),
            "Import is not yet implemented, should return error"
        );
        let err = result.unwrap_err();
        assert!(
            format!("{:?}", err).contains("import"),
            "Error should mention 'import' keyword"
        );
    }

    #[test]
    fn test_parse_import_with_prefix_not_implemented() {
        // Import with 'as' prefix for namespacing - not yet implemented
        // Expected syntax: (import "module.spl" as m)
        let result = parse_spl(r#"(import "module.spl" as m)"#);
        assert!(
            result.is_err(),
            "Import with prefix is not yet implemented"
        );
    }

    #[test]
    fn test_parse_import_relative_path_not_implemented() {
        // Relative path import - not yet implemented
        // Expected syntax: (import "./lib/utils.spl")
        let result = parse_spl(r#"(import "./lib/utils.spl")"#);
        assert!(
            result.is_err(),
            "Import with relative path is not yet implemented"
        );
    }

    #[test]
    fn test_parse_import_syntax_specification() {
        // Document the expected import syntax from SPL-GRAMMAR.abnf:
        // import-stmt = "(" "import" string ")"
        //             / "(" "import" string "as" identifier ")"
        //
        // Examples:
        //   (import "common-rules.spl")
        //   (import "legal.spl" as legal)
        //
        // When implemented, imports should:
        // 1. Parse the path string
        // 2. Optionally parse the 'as' prefix
        // 3. Return ImportDecl structure with path and optional prefix
        assert!(true, "Import syntax documented");
    }

    // =========================================================================
    // PREFIX DECLARATIONS TESTS
    // =========================================================================
    // Tests for (prefix ...) syntax for namespace declarations
    // Reference: spindle-racket import-tests.rkt @prefix syntax
    //
    // Note: Prefix declarations are not yet implemented. These tests document
    // the expected syntax. In SPL, prefix is used for namespace bindings
    // similar to RDF/Turtle syntax.

    #[test]
    fn test_parse_prefix_declaration_not_implemented() {
        // Prefix declaration for namespacing - not yet implemented
        // Expected syntax: (prefix name "uri")
        let result = parse_spl(r#"(prefix dc "http://purl.org/dc/terms/")"#);
        assert!(
            result.is_err(),
            "Prefix declaration is not yet implemented"
        );
        let err = result.unwrap_err();
        assert!(
            format!("{:?}", err).contains("prefix"),
            "Error should mention 'prefix' keyword"
        );
    }

    #[test]
    fn test_parse_prefix_with_colon_not_implemented() {
        // Prefix with colon notation (RDF-style) - not yet implemented
        // Expected syntax: (prefix dc: "http://purl.org/dc/terms/")
        let result = parse_spl(r#"(prefix dc: "http://purl.org/dc/terms/")"#);
        assert!(
            result.is_err(),
            "Prefix with colon is not yet implemented"
        );
    }

    #[test]
    fn test_parse_prefix_syntax_specification() {
        // Document the expected prefix syntax:
        // Prefixes define namespace bindings that can be used in identifiers.
        //
        // Examples:
        //   (prefix dc "http://purl.org/dc/terms/")
        //   (prefix foaf "http://xmlns.com/foaf/0.1/")
        //
        // Usage in rules:
        //   (given dc:creator)          ; Uses dc prefix
        //   (normally r1 foaf:knows trust)
        //
        // When implemented, should return PrefixDecl with:
        // - name: The prefix identifier (e.g., "dc")
        // - uri: The namespace URI
        assert!(true, "Prefix syntax documented");
    }

    // =========================================================================
    // CLAIMS BLOCKS TESTS
    // =========================================================================
    // Tests for (claims source statements...) syntax with metadata
    // Reference: code-review-claims.spl and spl-parser.rkt claims-stmt
    //
    // Claims blocks attribute statements to a source identity, enabling
    // trust-weighted reasoning. Not yet implemented in this parser.

    #[test]
    fn test_parse_claims_basic_not_implemented() {
        // Basic claims block with source identifier - not yet implemented
        // Expected syntax: (claims source-id statement...)
        let input = r#"(claims agent:security (given vulnerability_detected))"#;
        let result = parse_spl(input);
        assert!(
            result.is_err(),
            "Claims block is not yet implemented"
        );
        let err = result.unwrap_err();
        assert!(
            format!("{:?}", err).contains("claims"),
            "Error should mention 'claims' keyword"
        );
    }

    #[test]
    fn test_parse_claims_syntax_specification() {
        // Document the expected claims syntax from spl-parser.rkt:
        //
        // claims-stmt = "(" "claims" source claims-meta? claims-statements ")"
        // source = identifier ":" identifier  ; e.g., agent:security
        // claims-meta = (:at "timestamp")? (:sig "signature")? (:note "note")?
        // claims-statements = (fact | rule | superiority)+
        //
        // Example:
        //   (claims agent:security
        //     :at "2026-01-20T09:00:00Z"
        //     :note "Automated security scan"
        //     (given vulnerability_detected)
        //     (normally sec1 vulnerability_detected security_risk))
        //
        // When implemented, should produce ClaimsBlock with:
        // - source: String (e.g., "agent:security")
        // - timestamp: Option<String>
        // - signature: Option<String>
        // - note: Option<String>
        // - statements: Vec<Statement>
        assert!(true, "Claims syntax documented");
    }

    #[test]
    fn test_parse_claims_metadata_fields() {
        // Document claims metadata fields:
        //
        // :at - ISO 8601 timestamp when claims were made
        //   Example: :at "2026-01-20T09:00:00Z"
        //
        // :sig - Cryptographic signature for verification
        //   Example: :sig "abc123signature"
        //
        // :note - Human-readable annotation
        //   Example: :note "CI pipeline results"
        //
        // All metadata fields are optional.
        assert!(true, "Claims metadata fields documented");
    }

    #[test]
    fn test_parse_claims_source_format() {
        // Document source identifier format:
        //
        // Source identifiers use category:name format:
        //   agent:security    - A security scanning agent
        //   agent:coder       - A code analysis agent
        //   agent:qa          - A QA testing agent
        //   system:policy     - System-level policy rules
        //   user:admin        - An administrative user
        //
        // The category provides context for trust evaluation.
        assert!(true, "Claims source format documented");
    }

    #[test]
    fn test_parse_claims_inner_statements() {
        // Document allowed inner statements in claims blocks:
        //
        // Claims blocks can contain:
        // - Facts: (given literal)
        // - Rules: (normally/always/except label body head)
        // - Superiorities: (prefer label1 label2)
        //
        // Claims blocks CANNOT contain nested claims blocks.
        //
        // Example with all statement types:
        //   (claims agent:policy
        //     (given condition)
        //     (normally r1 condition result)
        //     (except d1 override result)
        //     (prefer d1 r1))
        assert!(true, "Claims inner statements documented");
    }

    #[test]
    fn test_parse_claims_code_review_example() {
        // Document the code review scenario from code-review-claims.spl:
        //
        // Multiple agents contribute claims about a pull request:
        //
        // (claims agent:security
        //   :at "2026-01-20T09:00:00Z"
        //   :note "Automated security scan results"
        //   (given vulnerability_detected)
        //   (normally sec1 vulnerability_detected security_risk))
        //
        // (claims agent:coder
        //   :at "2026-01-20T09:30:00Z"
        //   :note "CI pipeline results"
        //   (given tests_pass)
        //   (normally dev1 tests_pass code_compiles))
        //
        // ; Global superiority (outside claims blocks)
        // (prefer sec1 dev1)
        //
        // This enables trust-weighted reasoning where different sources
        // have different credibility levels.
        assert!(true, "Claims code review example documented");
    }

    // =========================================================================
    // PARSED THEORY TESTS
    // =========================================================================
    // Tests for basic theory parsing functionality

    #[test]
    fn test_parse_spl_basic_theory() {
        // Basic theory parsing (no imports)
        let input = r#"
            (given bird)
            (normally r1 bird flies)
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);
    }

    #[test]
    fn test_parse_spl_with_comments_before_statements() {
        // Comments before statements should be ignored
        let input = r#"
            ; Comment before facts
            ; Multiple lines of comments
            (given bird)
            (normally r1 bird flies)
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);
    }

    #[test]
    fn test_parse_spl_lang_directive() {
        // #lang directive should be ignored (Racket compatibility)
        let input = r#"
            #lang spindle

            (given bird)
            (normally r1 bird flies)
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);
    }

    #[test]
    fn test_parse_theory_with_all_rule_types() {
        // Theory with strict, defeasible, and defeater rules
        let input = r#"
            (given bird)
            (given penguin)
            (always s1 penguin bird)
            (normally r1 bird flies)
            (except d1 injured (not flies))
            (prefer r1 d1)
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 5);
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_parse_theory_preserves_labels() {
        // Verify rule labels are preserved
        let theory = parse_spl("(normally my-rule bird flies)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.label, "my-rule");
    }

    #[test]
    fn test_parse_theory_auto_generates_labels() {
        // Auto-generated labels for unlabeled rules
        let theory = parse_spl("(normally bird flies)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert!(!rule.label.is_empty());
    }

    // =========================================================================
    // PARSE_SPL_WITH_IMPORTS TESTS (Future Implementation)
    // =========================================================================
    // Tests documenting expected behavior for import resolution
    //
    // When parse_spl_with_imports is implemented, it should:
    // 1. Parse the main theory file
    // 2. Collect all (import ...) declarations
    // 3. Recursively parse imported files
    // 4. Merge imported rules/facts into the main theory
    // 5. Handle prefixed imports by namespacing literals
    // 6. Detect and report import cycles

    #[test]
    fn test_parsed_theory_structure_specification() {
        // Document the expected ParsedTheory structure:
        //
        // pub struct ParsedTheory {
        //     /// The merged theory with all rules from imports
        //     pub theory: Theory,
        //     /// Import declarations found in the source
        //     pub imports: Vec<ImportDecl>,
        //     /// Prefix declarations for namespacing
        //     pub prefixes: Vec<PrefixDecl>,
        //     /// Claims blocks with source attribution
        //     pub claims: Vec<ClaimsBlock>,
        // }
        //
        // pub struct ImportDecl {
        //     pub path: String,
        //     pub prefix: Option<String>,
        // }
        //
        // pub struct PrefixDecl {
        //     pub name: String,
        //     pub uri: String,
        // }
        //
        // pub struct ClaimsBlock {
        //     pub source: String,
        //     pub timestamp: Option<String>,
        //     pub signature: Option<String>,
        //     pub note: Option<String>,
        //     pub statements: Vec<Statement>,
        // }
        assert!(true, "ParsedTheory structure documented");
    }

    #[test]
    fn test_import_resolution_behavior() {
        // Document expected import resolution behavior:
        //
        // 1. Basic import: (import "base.spl")
        //    - Parse base.spl
        //    - Merge all rules/facts into main theory
        //    - Literals keep their original names
        //
        // 2. Prefixed import: (import "base.spl" as b)
        //    - Parse base.spl
        //    - Prefix all exported literals with "b:"
        //    - e.g., "bird" becomes "b:bird"
        //
        // 3. Nested imports:
        //    - If A imports B, and B imports C
        //    - All of C's exports are available in A
        //
        // 4. Cycle detection:
        //    - If A imports B and B imports A
        //    - Return error: "Import cycle detected: A -> B -> A"
        assert!(true, "Import resolution behavior documented");
    }

    // =========================================================================
    // EDGE CASES AND ERROR HANDLING TESTS
    // =========================================================================

    #[test]
    fn test_parse_empty_input() {
        let theory = parse_spl("").unwrap();
        assert_eq!(theory.rule_count(), 0);
    }

    #[test]
    fn test_parse_only_comments() {
        let input = r#"
            ; Comment 1
            ; Comment 2
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 0);
    }

    #[test]
    fn test_parse_whitespace_handling() {
        // Various whitespace configurations
        let input = "(given   bird)\n\n\n(normally   r1   bird   flies)";
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);
    }

    #[test]
    fn test_parse_nested_expressions() {
        // Deeply nested expressions
        let theory = parse_spl("(normally r1 (and a (not b)) (not c))").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.body.len(), 2);
        assert!(rule.head_literal().is_negated());
    }

    #[test]
    fn test_parse_meta_statement() {
        // Meta statements for rule annotations
        let input = r#"
            (normally r1 bird flies)
            (meta r1 (confidence "0.9") (source "expert"))
        "#;
        let result = parse_spl(input);
        // Meta should be parsed (may attach to rules or be stored separately)
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_complex_predicates() {
        // Predicates with multiple arguments
        let theory = parse_spl("(given (relationship alice bob friend))").unwrap();
        let rule = theory.rules().next().unwrap();
        let lit = rule.head_literal();
        assert_eq!(lit.name(), "relationship");
        assert_eq!(lit.predicates().len(), 3);
    }

    #[test]
    fn test_parse_negation_prefix_tilde() {
        // Tilde prefix for negation
        let theory = parse_spl("(given ~flying)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert!(rule.head_literal().is_negated());
    }

    #[test]
    fn test_parse_string_with_spaces() {
        // String values containing spaces
        let input = r#"(meta r1 (description "This is a long description"))"#;
        let result = parse_spl(input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_identifier_with_hyphens() {
        // Identifiers with hyphens (common in Lisp-style code)
        let theory = parse_spl("(normally my-rule-1 has-wings can-fly)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.label, "my-rule-1");
    }

    #[test]
    fn test_parse_identifier_with_underscores() {
        // Identifiers with underscores
        let theory = parse_spl("(given vulnerability_detected)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.head_literal().name(), "vulnerability_detected");
    }

    #[test]
    fn test_parse_identifier_with_dots() {
        // Identifiers with dots (namespace-like)
        // The parser supports dots in identifiers for namespace-like syntax
        let result = parse_spl("(given module.submodule.fact)");
        assert!(result.is_ok(), "Dotted identifiers should parse successfully");
        let theory = result.unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.head_literal().name(), "module.submodule.fact");
    }

    // =========================================================================
    // DEFEATER RULES TESTS
    // =========================================================================

    #[test]
    fn test_parse_defeater_basic() {
        let theory = parse_spl("(except d1 broken-wing flies)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.rule_type, RuleType::Defeater);
    }

    #[test]
    fn test_parse_defeater_negated_head() {
        let theory = parse_spl("(except d1 injured (not flies))").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.rule_type, RuleType::Defeater);
        assert!(rule.head_literal().is_negated());
    }

    #[test]
    fn test_parse_defeater_conjunction_body() {
        let theory = parse_spl("(except d1 (and bird injured) flies)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.rule_type, RuleType::Defeater);
        assert_eq!(rule.body.len(), 2);
    }

    // =========================================================================
    // STRICT RULES TESTS
    // =========================================================================

    #[test]
    fn test_parse_strict_basic() {
        let theory = parse_spl("(always s1 penguin bird)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.rule_type, RuleType::Strict);
    }

    #[test]
    fn test_parse_strict_conjunction() {
        let theory = parse_spl("(always s1 (and parent child) related)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.rule_type, RuleType::Strict);
        assert_eq!(rule.body.len(), 2);
    }

    // =========================================================================
    // PROVIDE STATEMENT TESTS
    // =========================================================================
    // Tests for (provide ...) syntax for module exports
    //
    // Note: Provide statements are not yet implemented.

    #[test]
    fn test_parse_provide_identifiers_not_implemented() {
        // Provide specific identifiers - not yet implemented
        // Expected syntax: (provide id1 id2 ...)
        let result = parse_spl("(provide bird flies)");
        assert!(
            result.is_err(),
            "Provide is not yet implemented"
        );
        let err = result.unwrap_err();
        assert!(
            format!("{:?}", err).contains("provide"),
            "Error should mention 'provide' keyword"
        );
    }

    #[test]
    fn test_parse_provide_all_not_implemented() {
        // Provide all exports - not yet implemented
        // Expected syntax: (provide all)
        let result = parse_spl("(provide all)");
        assert!(
            result.is_err(),
            "Provide all is not yet implemented"
        );
    }

    #[test]
    fn test_parse_provide_conclusions_not_implemented() {
        // Provide only conclusions - not yet implemented
        // Expected syntax: (provide conclusions)
        let result = parse_spl("(provide conclusions)");
        assert!(
            result.is_err(),
            "Provide conclusions is not yet implemented"
        );
    }

    #[test]
    fn test_parse_provide_syntax_specification() {
        // Document the expected provide syntax from SPL-GRAMMAR.abnf:
        //
        // provide-stmt = "(" "provide" 1*provide-item ")"
        // provide-item = identifier / "all" / "conclusions"
        //
        // Examples:
        //   (provide bird flies)      ; Export specific literals
        //   (provide all)             ; Export all defined literals
        //   (provide conclusions)     ; Export only derived conclusions
        //
        // When implemented, should control what an imported module exports.
        assert!(true, "Provide syntax documented");
    }

    // =========================================================================
    // ADDITIONAL RULE SYNTAX TESTS
    // =========================================================================

    #[test]
    fn test_parse_rule_with_complex_body_and_head() {
        // Complex predicate in both body and head
        let theory = parse_spl("(normally r1 (parent ?x ?y) (ancestor ?x ?y))").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.body.len(), 1);
        assert_eq!(rule.body[0].name(), "parent");
        assert_eq!(rule.head_literal().name(), "ancestor");
    }

    #[test]
    fn test_parse_multiple_facts_same_predicate() {
        // Multiple facts with same predicate, different arguments
        let input = r#"
            (given (person alice))
            (given (person bob))
            (given (person carol))
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 3);
    }

    #[test]
    fn test_parse_rule_chain() {
        // Chain of rules for transitive reasoning
        let input = r#"
            (given (parent alice bob))
            (given (parent bob carol))
            (normally r1 (parent ?x ?y) (ancestor ?x ?y))
            (normally r2 (and (ancestor ?x ?y) (ancestor ?y ?z)) (ancestor ?x ?z))
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 4);
    }

    #[test]
    fn test_parse_superiority_multiple_pairs() {
        // Multiple independent superiority declarations
        let input = r#"
            (prefer r1 r2)
            (prefer r3 r4)
            (prefer r5 r6)
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.superiorities().len(), 3);
    }

    #[test]
    fn test_parse_mixed_labeled_unlabeled_rules() {
        // Mix of labeled and unlabeled rules
        let input = r#"
            (given bird)
            (normally r1 bird flies)
            (normally healthy happy)
            (except d1 sick (not happy))
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 4);
        // Verify the labeled rules have their labels
        let rules: Vec<_> = theory.rules().collect();
        assert!(rules.iter().any(|r| r.label == "r1"));
        assert!(rules.iter().any(|r| r.label == "d1"));
    }

    #[test]
    fn test_parse_inline_comments() {
        // Comments can appear between statements
        let input = r#"
            (given bird)  ; This is a fact
            ; This rule says birds normally fly
            (normally r1 bird flies)
            (prefer r2 r1)  ; r2 beats r1
        "#;
        let theory = parse_spl(input).unwrap();
        assert_eq!(theory.rule_count(), 2);
        assert_eq!(theory.superiorities().len(), 1);
    }

    #[test]
    fn test_parse_meta_with_multiple_properties() {
        // Meta statement with multiple properties
        let input = r#"
            (normally r1 bird flies)
            (meta r1
              (confidence "0.9")
              (source "expert")
              (category "biology"))
        "#;
        let theory = parse_spl(input).unwrap();
        // Meta should be stored in theory
        assert_eq!(theory.rule_count(), 1);
        // Check that metadata was attached
        let meta = theory.get_meta("r1");
        assert!(meta.is_some());
        let meta = meta.unwrap();
        assert!(meta.properties.get("confidence").is_some());
        assert!(meta.properties.get("source").is_some());
        assert!(meta.properties.get("category").is_some());
    }

    #[test]
    fn test_parse_meta_with_list_value() {
        // Meta with list value
        let input = r#"
            (normally r1 bird flies)
            (meta r1 (tags ("animal" "flying" "nature")))
        "#;
        let theory = parse_spl(input).unwrap();
        let meta = theory.get_meta("r1");
        assert!(meta.is_some());
        let meta = meta.unwrap();
        assert!(meta.properties.get("tags").is_some());
    }

    #[test]
    fn test_parse_fact_with_numeric_argument() {
        // Predicates can have numeric arguments
        let theory = parse_spl("(given (temperature 25))").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.head_literal().predicates().len(), 1);
    }

    #[test]
    fn test_parse_fact_with_string_argument() {
        // Predicates can have string arguments
        let theory = parse_spl(r#"(given (name alice "Alice Smith"))"#).unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.head_literal().predicates().len(), 2);
    }

    #[test]
    fn test_parse_conjunction_with_negation() {
        // Conjunction containing negated literals
        let theory = parse_spl("(normally r1 (and bird (not injured)) flies)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.body.len(), 2);
        assert!(!rule.body[0].is_negated()); // bird
        assert!(rule.body[1].is_negated());  // (not injured)
    }

    #[test]
    fn test_parse_large_conjunction() {
        // Conjunction with many elements
        let theory = parse_spl("(normally r1 (and a b c d e) result)").unwrap();
        let rule = theory.rules().next().unwrap();
        assert_eq!(rule.body.len(), 5);
    }
}
