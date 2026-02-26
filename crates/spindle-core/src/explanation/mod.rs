//! Explanation System for Spindle
//!
//! This module provides explanation generation capabilities for the
//! Spindle defeasible logic reasoner:
//!
//! - Proof/derivation tree structures (in `types`)
//! - Explanation output generation -- natural language, JSON, JSON-LD, DOT (in `format`)
//! - Blocked alternatives tracking
//! - Conflict resolution explanations
//!
//! Data types live in [`types`]. Output formatters live in [`format`].

pub mod format;
pub mod types;

// Re-export everything at the same path as before for backward compatibility.
// All existing import paths (`use crate::explanation::Explanation`, etc.)
// continue to resolve.
pub use types::*;

use std::collections::HashSet;

use crate::conclusion::ConclusionType;
use crate::error::Result;
use crate::grounding::{apply_substitution_to_literal, match_literal};
use crate::literal::Literal;

// ---------------------------------------------------------------------------
// Backward-compatible convenience methods
// ---------------------------------------------------------------------------
//
// These thin wrappers delegate to the new formatter structs so that existing
// call sites (`explanation.to_json()`, `explanation.to_natural_language()`,
// etc.) continue to compile without changes.

impl Explanation {
    /// Generate natural language explanation.
    ///
    /// Convenience wrapper -- delegates to
    /// [`NaturalLanguageFormatter`](format::NaturalLanguageFormatter).
    pub fn to_natural_language(&self) -> String {
        use format::ExplanationFormatter;
        format::NaturalLanguageFormatter::new().format(self)
    }

    /// Convert to JSON representation.
    ///
    /// Convenience wrapper -- delegates to
    /// [`JsonFormatter`](format::JsonFormatter).
    pub fn to_json(&self) -> serde_json::Value {
        format::JsonFormatter.to_value(self)
    }

    /// Convert to JSON-LD representation with semantic annotations.
    ///
    /// Convenience wrapper -- delegates to
    /// [`JsonLdFormatter`](format::JsonLdFormatter).
    pub fn to_jsonld(&self) -> serde_json::Value {
        format::JsonLdFormatter.to_value(self)
    }

    /// Convert to DOT graph format for visualization.
    ///
    /// Convenience wrapper -- delegates to
    /// [`DotFormatter`](format::DotFormatter).
    pub fn to_dot(&self) -> String {
        use format::ExplanationFormatter;
        format::DotFormatter::default().format(self)
    }
}

/// Explain why a conclusion holds (returns Proof Tree)
pub fn explain(theory: &crate::theory::Theory, literal: &Literal) -> Result<Option<Explanation>> {
    let mut visited = HashSet::new();
    explain_inner(theory, literal, &mut visited)
}

/// Internal recursive helper with cycle detection via `visited` set.
fn explain_inner(
    theory: &crate::theory::Theory,
    literal: &Literal,
    visited: &mut HashSet<String>,
) -> Result<Option<Explanation>> {
    use crate::reason::reason;

    let literal_key = literal.to_string();

    // Cycle detection: if we are already in the process of explaining this
    // literal, return None to break the cycle instead of recursing forever.
    if visited.contains(&literal_key) {
        return Ok(None);
    }
    visited.insert(literal_key.clone());

    let conclusions = reason(theory)?;

    // Find the conclusion for the target literal
    let conclusion = match conclusions
        .iter()
        .find(|c| c.literal == *literal && c.conclusion_type.is_positive())
    {
        Some(c) => c,
        None => {
            visited.remove(&literal_key);
            return Ok(None);
        }
    };

    let mut explanation = Explanation::new(conclusion.conclusion_type, conclusion.literal.clone());

    if let Some(rule_label) = &conclusion.rule_label
        && let Some(rule) = theory.get_rule(rule_label)
    {
        let mut step = ProofStep::new(
            rule.label.clone(),
            rule.rule_type,
            rule.to_spl(), // SPL format includes temporal, Allen constraints, state queries
        );

        // Determine substitution by matching rule head against conclusion literal
        // If rule has multiple heads, find the one that matches
        let head_pattern = rule
            .head
            .iter()
            .find(|h| match_literal(h, literal).is_some())
            .unwrap_or(&rule.head[0]); // Fallback, shouldn't happen if logic correct

        let subst = match_literal(head_pattern, literal).unwrap_or_default();

        // Recursively build proofs for body, applying substitution
        let mut body_proofs = Vec::new();
        for body_bl in &rule.body {
            // Only logic body literals participate in proof trees
            let body_lit = match body_bl.as_logic() {
                Some(lit) => lit.to_literal(),
                None => continue, // skip arithmetic constraints
            };
            let ground_body_lit = apply_substitution_to_literal(&body_lit, &subst);

            if let Some(body_expl) = explain_inner(theory, &ground_body_lit, visited)? {
                if let Some(body_tree) = body_expl.proof_tree {
                    body_proofs.push(body_tree);
                }
            } else {
                // If exact ground literal not found, try to find a matching one
                // (Handle existential cases like matter_seen where body var isn't in head)
                // We need to iterate carefully to propagate errors from explain_inner
                for c in &conclusions {
                    if c.conclusion_type.is_positive()
                        && match_literal(&body_lit, &c.literal).is_some()
                        && let Some(body_expl) = explain_inner(theory, &c.literal, visited)?
                        && let Some(body_tree) = body_expl.proof_tree
                    {
                        body_proofs.push(body_tree);
                        break;
                    }
                }
            }
        }
        step.body_proofs = body_proofs;

        // Annotations
        // (Assuming rule has annotations or we mock them)
        // step.annotations = ...

        let proof_node = ProofNode::new(
            literal.clone(),
            match conclusion.conclusion_type {
                ConclusionType::DefinitelyProvable => DerivationType::Definite,
                _ => DerivationType::Defeasible,
            },
        )
        .with_proof_step(step);

        explanation.proof_tree = Some(proof_node);
    }

    visited.remove(&literal_key);
    Ok(Some(explanation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explain_simple_fact() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("bird");

        let result = explain(&theory, &Literal::simple("bird")).unwrap();
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert_eq!(
            explanation.conclusion_type,
            ConclusionType::DefinitelyProvable
        );
        assert_eq!(explanation.literal.name(), "bird");
    }

    #[test]
    fn test_explain_defeasible_chain() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "flies");

        let result = explain(&theory, &Literal::simple("flies")).unwrap();
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert_eq!(
            explanation.conclusion_type,
            ConclusionType::DefeasiblyProvable
        );
        assert_eq!(explanation.literal.name(), "flies");
        // Should have proof tree
        assert!(explanation.proof_tree.is_some());
    }

    #[test]
    fn test_explain_not_provable() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("bird");

        // Try to explain something that isn't provable
        let result = explain(&theory, &Literal::simple("nonexistent")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_explain_strict_rule_chain() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");
        theory.add_strict_rule(&["q"], "r");

        let result = explain(&theory, &Literal::simple("r")).unwrap();
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert_eq!(
            explanation.conclusion_type,
            ConclusionType::DefinitelyProvable
        );
    }

    #[test]
    fn test_explain_negated_literal() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("~guilty");

        let result = explain(&theory, &Literal::negated("guilty")).unwrap();
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert!(explanation.literal.is_negated());
    }

    #[test]
    fn test_explain_with_body_proofs() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_fact("b");
        theory.add_defeasible_rule(&["a", "b"], "c");

        let result = explain(&theory, &Literal::simple("c")).unwrap();
        assert!(result.is_some());
        let explanation = result.unwrap();
        assert!(explanation.proof_tree.is_some());
        let tree = explanation.proof_tree.unwrap();
        // Should have proof step with body proofs
        if let Some(step) = &tree.proof_step {
            // Body proofs should have a and b
            assert!(!step.body_proofs.is_empty());
        }
    }

    #[test]
    fn test_dot_output_with_strict_rule() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_strict_rule(&["a"], "b");

        let result = explain(&theory, &Literal::simple("b")).unwrap();
        assert!(result.is_some());
        let explanation = result.unwrap();

        // Convert to DOT format
        let dot = explanation.to_dot();
        assert!(dot.contains("strict"));
        assert!(dot.contains("digraph"));
    }

    #[test]
    fn test_dot_output_with_body_proofs() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("x");
        theory.add_fact("y");
        theory.add_defeasible_rule(&["x", "y"], "z");

        let result = explain(&theory, &Literal::simple("z")).unwrap();
        assert!(result.is_some());
        let explanation = result.unwrap();

        // Should have body proofs shown as edges in DOT
        let dot = explanation.to_dot();
        assert!(dot.contains("->"));
    }

    #[test]
    fn test_json_output_with_strict_rule() {
        use crate::theory::Theory;

        let mut theory = Theory::new();
        theory.add_fact("p");
        theory.add_strict_rule(&["p"], "q");

        let result = explain(&theory, &Literal::simple("q")).unwrap();
        assert!(result.is_some());
        let explanation = result.unwrap();

        let json = explanation.to_json();
        // Proof tree should exist
        assert!(json.get("proof_tree").is_some());
    }

    #[test]
    fn test_explain_cyclic_rules_no_stack_overflow() {
        use crate::theory::Theory;

        // Create a theory with cyclic rule dependencies: a -> b, b -> c, c -> a
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");
        theory.add_defeasible_rule(&["b"], "c");
        theory.add_defeasible_rule(&["c"], "a"); // cycle back to a

        // Calling explain on "c" should NOT stack overflow.
        // It should return Ok (either Some or None) without panicking.
        let result = explain(&theory, &Literal::simple("c"));
        assert!(result.is_ok(), "explain() should not panic on cyclic rules");
    }
}
