//! Claims block builder for programmatic SPL generation
//!
//! Provides a fluent API for building `(claims ...)` SPL blocks,
//! including canonical normalisation for external signing workflows.
//!
//! # Signing workflow
//!
//! 1. Build a `ClaimsBlock` with content (no signature yet)
//! 2. Call `to_canonical_spl()` to get deterministic bytes
//! 3. Sign the bytes externally
//! 4. Call `with_signature(sig)` to attach the signature
//! 5. Call `to_spl()` for final output with `:sig` included

use crate::literal::{Literal, render_spl_atom};
use crate::rule::Rule;
use crate::superiority::Superiority;
use crate::theory::Theory;

/// A statement within a claims block.
#[derive(Debug, Clone)]
pub enum ClaimsStatement {
    /// A rule (fact, strict, defeasible, or defeater)
    Rule(Box<Rule>),
    /// A superiority relation between two rules
    Superiority(Superiority),
}

/// Builder for `(claims ...)` SPL blocks.
///
/// ```
/// use spindle_core::claims::ClaimsBlock;
/// use spindle_core::literal::Literal;
/// use spindle_core::rule::Rule;
///
/// let mut block = ClaimsBlock::new("agent:alice")
///     .with_timestamp("2026-01-20T09:00:00Z")
///     .with_id("block-1");
///
/// block.add_fact(Literal::simple("bird"));
/// block.add_rule(Rule::defeasible(
///     "r1",
///     vec![Literal::simple("bird")],
///     Literal::simple("flies"),
/// ));
/// block.add_superiority("r1", "r2");
///
/// // Canonical form for signing (deterministic, excludes :sig)
/// let canonical = block.to_canonical_spl();
/// assert!(!canonical.contains(":sig"));
///
/// // Attach signature and render final SPL
/// let signed = block.with_signature("ed25519:abc123");
/// let spl = signed.to_spl();
/// assert!(spl.contains(":sig \"ed25519:abc123\""));
/// ```
#[derive(Debug, Clone)]
pub struct ClaimsBlock {
    source: String,
    timestamp: Option<String>,
    signature: Option<String>,
    id: Option<String>,
    note: Option<String>,
    statements: Vec<ClaimsStatement>,
    fact_counter: usize,
}

impl ClaimsBlock {
    /// Create a new claims block with the given source identifier.
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            timestamp: None,
            signature: None,
            id: None,
            note: None,
            statements: Vec::new(),
            fact_counter: 0,
        }
    }

    /// Create a claims block from an existing theory.
    ///
    /// Pulls all rules and superiorities into the block.
    /// Use this to canonicalize existing SPL for signing:
    ///
    /// ```
    /// use spindle_core::claims::ClaimsBlock;
    /// use spindle_core::theory::Theory;
    /// use spindle_core::literal::Literal;
    /// use spindle_core::rule::Rule;
    ///
    /// let mut theory = Theory::new();
    /// theory.add_fact("bird");
    /// theory.add_rule(Rule::defeasible(
    ///     "r1",
    ///     vec![Literal::simple("bird")],
    ///     Literal::simple("flies"),
    /// ));
    ///
    /// let block = ClaimsBlock::from_theory("agent:alice", &theory);
    /// let canonical = block.to_canonical_spl();
    /// assert!(canonical.contains("(given (bird))"));
    /// ```
    pub fn from_theory(source: impl Into<String>, theory: &Theory) -> Self {
        let mut block = Self::new(source);

        for rule in theory.rules() {
            block
                .statements
                .push(ClaimsStatement::Rule(Box::new(rule.clone())));
        }

        for sup in theory.superiorities() {
            block
                .statements
                .push(ClaimsStatement::Superiority(sup.clone()));
        }

        block
    }

    /// Set the timestamp metadata.
    pub fn with_timestamp(mut self, ts: impl Into<String>) -> Self {
        self.timestamp = Some(ts.into());
        self
    }

    /// Attach a signature (typically after calling `to_canonical_spl()`).
    pub fn with_signature(mut self, sig: impl Into<String>) -> Self {
        self.signature = Some(sig.into());
        self
    }

    /// Set the block identifier metadata.
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the note metadata.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Add a fact with an auto-generated label (`_cf1`, `_cf2`, ...).
    pub fn add_fact(&mut self, literal: Literal) -> &mut Self {
        self.fact_counter += 1;
        let label = format!("_cf{}", self.fact_counter);
        self.statements
            .push(ClaimsStatement::Rule(Box::new(Rule::fact(label, literal))));
        self
    }

    /// Add a fact with an explicit label.
    pub fn add_fact_labeled(&mut self, label: impl Into<String>, literal: Literal) -> &mut Self {
        self.statements
            .push(ClaimsStatement::Rule(Box::new(Rule::fact(label, literal))));
        self
    }

    /// Add a pre-built rule (strict, defeasible, or defeater).
    pub fn add_rule(&mut self, rule: Rule) -> &mut Self {
        self.statements.push(ClaimsStatement::Rule(Box::new(rule)));
        self
    }

    /// Add a superiority relation.
    pub fn add_superiority(
        &mut self,
        superior: impl Into<String>,
        inferior: impl Into<String>,
    ) -> &mut Self {
        self.statements
            .push(ClaimsStatement::Superiority(Superiority::new(
                superior, inferior,
            )));
        self
    }

    /// Get the source identifier.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Get the statements in insertion order.
    pub fn statements(&self) -> &[ClaimsStatement] {
        &self.statements
    }

    /// Render as human-readable SPL with 2-space indentation.
    ///
    /// Includes `:sig` if a signature has been attached.
    pub fn to_spl(&self) -> String {
        let mut out = format!("(claims {}", render_spl_atom(&self.source));

        if let Some(ref ts) = self.timestamp {
            out.push_str(&format!("\n  :at {}", escape_spl_metadata(ts)));
        }
        if let Some(ref sig) = self.signature {
            out.push_str(&format!("\n  :sig {}", escape_spl_metadata(sig)));
        }
        if let Some(ref id) = self.id {
            out.push_str(&format!("\n  :id {}", escape_spl_metadata(id)));
        }
        if let Some(ref note) = self.note {
            out.push_str(&format!("\n  :note {}", escape_spl_metadata(note)));
        }

        for stmt in &self.statements {
            out.push_str(&format!("\n  {}", render_statement(stmt)));
        }

        out.push(')');
        out
    }

    /// Render canonical SPL for external signing.
    ///
    /// **Excludes `:sig`** — this is the form you sign before attaching the signature.
    ///
    /// Canonical specification:
    /// - Metadata order: alphabetical (`:at`, `:id`, `:note`; never `:sig`)
    /// - Statement order: facts by head, rules by label, superiorities by `(superior, inferior)`
    /// - No indentation, one statement per line, single spaces
    ///
    /// ```
    /// use spindle_core::claims::ClaimsBlock;
    /// use spindle_core::literal::Literal;
    ///
    /// let mut block = ClaimsBlock::new("agent:test");
    /// block.add_fact(Literal::simple("z_fact"));
    /// block.add_fact(Literal::simple("a_fact"));
    ///
    /// let canonical = block.to_canonical_spl();
    /// // Facts are sorted alphabetically by head
    /// assert!(canonical.find("a_fact").unwrap() < canonical.find("z_fact").unwrap());
    /// ```
    pub fn to_canonical_spl(&self) -> String {
        let mut out = format!("(claims {}", render_spl_atom(&self.source));

        // Metadata in alphabetical order, excluding :sig
        if let Some(ref ts) = self.timestamp {
            out.push_str(&format!("\n:at {}", escape_spl_metadata(ts)));
        }
        if let Some(ref id) = self.id {
            out.push_str(&format!("\n:id {}", escape_spl_metadata(id)));
        }
        if let Some(ref note) = self.note {
            out.push_str(&format!("\n:note {}", escape_spl_metadata(note)));
        }

        // Separate statements by kind
        let mut facts: Vec<&Rule> = Vec::new();
        let mut rules: Vec<&Rule> = Vec::new();
        let mut sups: Vec<&Superiority> = Vec::new();

        for stmt in &self.statements {
            match stmt {
                ClaimsStatement::Rule(r) if r.is_fact() => facts.push(r),
                ClaimsStatement::Rule(r) => rules.push(r),
                ClaimsStatement::Superiority(s) => sups.push(s),
            }
        }

        // Sort facts by head literal SPL form
        facts.sort_by(|a, b| a.head[0].to_spl().cmp(&b.head[0].to_spl()));
        // Sort rules by label
        rules.sort_by(|a, b| a.label.cmp(&b.label));
        // Sort superiorities by (superior, inferior)
        sups.sort_by(|a, b| (&a.superior, &a.inferior).cmp(&(&b.superior, &b.inferior)));

        for fact in facts {
            out.push_str(&format!("\n{}", fact.to_spl()));
        }
        for rule in rules {
            out.push_str(&format!("\n{}", rule.to_spl()));
        }
        for sup in sups {
            out.push_str(&format!(
                "\n(prefer {} {})",
                render_spl_atom(&sup.superior),
                render_spl_atom(&sup.inferior)
            ));
        }

        out.push(')');
        out
    }
}

/// Render a metadata value as a quoted, escaped SPL string.
///
/// Always wraps in double quotes, escaping `\` and `"` within the value.
fn escape_spl_metadata(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Render a single statement as SPL.
fn render_statement(stmt: &ClaimsStatement) -> String {
    match stmt {
        ClaimsStatement::Rule(rule) => rule.to_spl(),
        ClaimsStatement::Superiority(sup) => {
            format!(
                "(prefer {} {})",
                render_spl_atom(&sup.superior),
                render_spl_atom(&sup.inferior)
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_fields() {
        let block = ClaimsBlock::new("agent:security")
            .with_timestamp("2026-01-20T09:00:00Z")
            .with_signature("abc123")
            .with_id("block-1")
            .with_note("scan results");

        assert_eq!(block.source(), "agent:security");
        assert_eq!(block.timestamp.as_deref(), Some("2026-01-20T09:00:00Z"));
        assert_eq!(block.signature.as_deref(), Some("abc123"));
        assert_eq!(block.id.as_deref(), Some("block-1"));
        assert_eq!(block.note.as_deref(), Some("scan results"));
    }

    #[test]
    fn test_to_spl_basic() {
        let mut block = ClaimsBlock::new("agent:security").with_timestamp("2026-01-20T09:00:00Z");

        block.add_fact(Literal::simple("vulnerability_detected"));
        block.add_rule(Rule::defeasible(
            "sec1",
            vec![Literal::simple("vulnerability_detected")],
            Literal::simple("security_risk"),
        ));
        block.add_superiority("sec1", "dev1");

        let spl = block.to_spl();
        assert!(spl.starts_with("(claims agent:security"));
        assert!(spl.contains(":at \"2026-01-20T09:00:00Z\""));
        assert!(spl.contains("(given (vulnerability_detected))"));
        assert!(spl.contains("(normally sec1 (vulnerability_detected) (security_risk))"));
        assert!(spl.contains("(prefer sec1 dev1)"));
        assert!(spl.ends_with(')'));
    }

    #[test]
    fn test_to_spl_includes_signature() {
        let block = ClaimsBlock::new("agent:a").with_signature("sig123");
        let spl = block.to_spl();
        assert!(spl.contains(":sig \"sig123\""));
    }

    #[test]
    fn test_canonical_determinism() {
        // Build two blocks with same content in different insertion order
        let mut block1 = ClaimsBlock::new("agent:test").with_timestamp("2026-01-01T00:00:00Z");
        block1.add_fact(Literal::simple("b_fact"));
        block1.add_fact(Literal::simple("a_fact"));
        block1.add_rule(Rule::defeasible(
            "r2",
            vec![Literal::simple("b_fact")],
            Literal::simple("result"),
        ));
        block1.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("a_fact")],
            Literal::simple("result"),
        ));
        block1.add_superiority("r2", "r1");
        block1.add_superiority("r1", "r3");

        let mut block2 = ClaimsBlock::new("agent:test").with_timestamp("2026-01-01T00:00:00Z");
        // Reversed insertion order
        block2.add_superiority("r1", "r3");
        block2.add_superiority("r2", "r1");
        block2.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("a_fact")],
            Literal::simple("result"),
        ));
        block2.add_rule(Rule::defeasible(
            "r2",
            vec![Literal::simple("b_fact")],
            Literal::simple("result"),
        ));
        block2.add_fact(Literal::simple("a_fact"));
        block2.add_fact(Literal::simple("b_fact"));

        assert_eq!(block1.to_canonical_spl(), block2.to_canonical_spl());
    }

    #[test]
    fn test_canonical_excludes_sig() {
        let block = ClaimsBlock::new("agent:a").with_signature("should_not_appear");
        let canonical = block.to_canonical_spl();
        assert!(
            !canonical.contains(":sig"),
            "canonical form must exclude :sig"
        );
        assert!(!canonical.contains("should_not_appear"));
    }

    #[test]
    fn test_canonical_metadata_order() {
        let block = ClaimsBlock::new("agent:a")
            .with_note("a note")
            .with_id("an-id")
            .with_timestamp("2026-01-01T00:00:00Z");

        let canonical = block.to_canonical_spl();
        let at_pos = canonical.find(":at").expect(":at present");
        let id_pos = canonical.find(":id").expect(":id present");
        let note_pos = canonical.find(":note").expect(":note present");

        assert!(at_pos < id_pos, ":at should come before :id");
        assert!(id_pos < note_pos, ":id should come before :note");
    }

    #[test]
    fn test_round_trip() {
        let mut block = ClaimsBlock::new("agent:alice")
            .with_timestamp("2026-01-20T09:00:00Z")
            .with_id("block-1")
            .with_note("round trip test");

        block.add_fact(Literal::simple("bird"));
        block.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        ));
        block.add_superiority("r1", "r2");

        let spl = block.to_spl();

        // Parse it back via the parser crate
        let theory = spindle_parser::spl::parse_spl(&spl).expect("should parse generated SPL");

        // Check rules — use format!("{:?}") to compare across crate boundaries
        assert_eq!(theory.rule_count(), 2); // 1 fact + 1 rule

        // Check the defeasible rule exists and has correct structure
        let r1 = theory.get_rule("r1").expect("rule r1 should exist");
        assert_eq!(format!("{}", r1.rule_type), "=>");
        assert_eq!(r1.head[0].name(), "flies");
        assert_eq!(r1.body[0].name(), "bird");

        // Check superiority
        assert_eq!(theory.superiorities().len(), 1);
        assert!(theory.is_superior("r1", "r2"));

        // Check metadata on r1 — use Debug format to avoid cross-crate type mismatch
        let meta = theory.get_meta("r1").expect("r1 should have metadata");
        let source_val = format!("{:?}", meta.properties.get("source"));
        assert!(
            source_val.contains("agent:alice"),
            "source metadata should contain agent:alice, got: {source_val}"
        );
    }

    #[test]
    fn test_from_theory() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        ));
        theory.add_superiority("r1", "r2");

        let block =
            ClaimsBlock::from_theory("agent:alice", &theory).with_timestamp("2026-01-01T00:00:00Z");

        // Should have 2 rules + 1 superiority
        assert_eq!(block.statements().len(), 3);

        let canonical = block.to_canonical_spl();
        assert!(canonical.contains("(given (bird))"));
        assert!(canonical.contains("(normally r1 (bird) (flies))"));
        assert!(canonical.contains("(prefer r1 r2)"));

        // Canonical should be deterministic on repeated calls
        let block2 =
            ClaimsBlock::from_theory("agent:alice", &theory).with_timestamp("2026-01-01T00:00:00Z");
        assert_eq!(canonical, block2.to_canonical_spl());
    }

    #[test]
    fn test_from_theory_sign_workflow() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        ));

        let block = ClaimsBlock::from_theory("agent:alice", &theory);
        let canonical = block.to_canonical_spl();
        assert!(!canonical.contains(":sig"));

        // Simulate external signing
        let signed = block.with_signature("externally-computed-sig");
        let final_spl = signed.to_spl();
        assert!(final_spl.contains(":sig \"externally-computed-sig\""));

        // Final SPL should parse back cleanly
        let reparsed = spindle_parser::spl::parse_spl(&final_spl).expect("signed SPL should parse");
        assert_eq!(reparsed.rule_count(), 2);
    }

    #[test]
    fn test_empty_block() {
        let block = ClaimsBlock::new("agent:empty");
        let spl = block.to_spl();
        assert_eq!(spl, "(claims agent:empty)");

        let canonical = block.to_canonical_spl();
        assert_eq!(canonical, "(claims agent:empty)");
    }

    #[test]
    fn test_metadata_escapes_quotes() {
        let block = ClaimsBlock::new("agent:a")
            .with_note("he said \"hello\"")
            .with_id("id-with-\"quotes\"");

        let spl = block.to_spl();
        assert!(
            spl.contains(r#":note "he said \"hello\""#),
            "to_spl should escape quotes in :note, got: {spl}"
        );
        assert!(
            spl.contains(r#":id "id-with-\"quotes\""#),
            "to_spl should escape quotes in :id, got: {spl}"
        );

        let canonical = block.to_canonical_spl();
        assert!(
            canonical.contains(r#":note "he said \"hello\""#),
            "to_canonical_spl should escape quotes in :note, got: {canonical}"
        );
        assert!(
            canonical.contains(r#":id "id-with-\"quotes\""#),
            "to_canonical_spl should escape quotes in :id, got: {canonical}"
        );
    }

    #[test]
    fn test_metadata_escapes_backslashes() {
        let block = ClaimsBlock::new("agent:a")
            .with_timestamp(r"path\to\file")
            .with_signature(r"sig\with\slashes");

        let spl = block.to_spl();
        assert!(
            spl.contains(r#":at "path\\to\\file"#),
            "to_spl should escape backslashes in :at, got: {spl}"
        );
        assert!(
            spl.contains(r#":sig "sig\\with\\slashes"#),
            "to_spl should escape backslashes in :sig, got: {spl}"
        );
    }

    #[test]
    fn test_metadata_escapes_mixed() {
        let block = ClaimsBlock::new("agent:a").with_note(r#"a "b\" c"#);

        let spl = block.to_spl();
        assert!(
            spl.contains(r#":note "a \"b\\\" c"#),
            "to_spl should escape mixed quotes and backslashes, got: {spl}"
        );

        let canonical = block.to_canonical_spl();
        assert!(
            canonical.contains(r#":note "a \"b\\\" c"#),
            "to_canonical_spl should escape mixed, got: {canonical}"
        );
    }
}
