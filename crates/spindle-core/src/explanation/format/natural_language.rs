//! Natural language explanation formatter.
//!
//! Renders an [`Explanation`] as human-readable English text.

use super::ExplanationFormatter;
use crate::conclusion::ConclusionType;
use crate::explanation::types::*;
use crate::rule::RuleType;

/// Renders explanations as human-readable English text.
#[derive(Debug, Clone)]
pub struct NaturalLanguageFormatter {
    /// Indentation string (default: two spaces).
    pub indent: String,
}

impl Default for NaturalLanguageFormatter {
    fn default() -> Self {
        Self {
            indent: "  ".to_string(),
        }
    }
}

impl NaturalLanguageFormatter {
    /// Create a new formatter with the default two-space indent.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ExplanationFormatter for NaturalLanguageFormatter {
    fn format(&self, explanation: &Explanation) -> String {
        let mut output = String::new();

        // Header
        output.push_str(&format!(
            "Explanation for {} {}\n",
            explanation.conclusion_type, explanation.literal
        ));
        output.push_str(&format!(
            "{}\n\n",
            conclusion_type_explanation(explanation.conclusion_type)
        ));

        // Proof tree
        if let Some(ref proof) = explanation.proof_tree {
            output.push_str("Derivation:\n");
            output.push_str(&proof_node_to_natural_language(proof, 1, &self.indent));
        } else if explanation.conclusion_type.is_positive() {
            output.push_str("No derivation found.\n");
        }

        // Blocked alternatives
        if !explanation.blocked_alternatives.is_empty() {
            output.push_str("\nBlocked Alternatives:\n");
            for (i, blocked) in explanation.blocked_alternatives.iter().enumerate() {
                output.push_str(&format!(
                    "  {}. Rule '{}' was blocked due to {}: {}\n",
                    i + 1,
                    blocked.rule_label,
                    blocked.reason,
                    blocked.explanation
                ));
            }
        }

        // Conflict resolutions
        if !explanation.conflicts_resolved.is_empty() {
            output.push_str("\nConflict Resolutions:\n");
            for (i, conflict) in explanation.conflicts_resolved.iter().enumerate() {
                output.push_str(&format!(
                    "  {}. '{}' defeated '{}' via {}\n",
                    i + 1,
                    conflict.winning_rule,
                    conflict.losing_rule,
                    conflict.resolution_type
                ));
            }
        }

        output
    }
}

// ===========================================================================
// Private helpers
// ===========================================================================

/// Get explanation text for conclusion type.
fn conclusion_type_explanation(ct: ConclusionType) -> &'static str {
    match ct {
        ConclusionType::DefinitelyProvable => {
            "This was proven using only strict rules and facts (cannot be defeated)."
        }
        ConclusionType::DefinitelyNotProvable => "This cannot be proven using strict rules alone.",
        ConclusionType::DefeasiblyProvable => {
            "This was proven using defeasible rules and was not defeated by any conflicting rule."
        }
        ConclusionType::DefeasiblyNotProvable => {
            "This could not be proven defeasibly, either because no applicable rule exists or it was defeated."
        }
    }
}

/// Convert proof node to natural language (recursive).
fn proof_node_to_natural_language(node: &ProofNode, num: usize, indent: &str) -> String {
    let mut output = String::new();

    if let Some(ref step) = node.proof_step {
        let derivation_str = match node.derivation_type {
            DerivationType::Definite => {
                if step.rule_type == RuleType::Fact {
                    "established as a fact"
                } else {
                    "derived strictly"
                }
            }
            DerivationType::Defeasible => "derived defeasibly",
        };

        output.push_str(&format!(
            "{}{}. \"{}\" was {}\n",
            indent, num, node.literal, derivation_str
        ));
        output.push_str(&format!(
            "{}   Using {}: {}\n",
            indent,
            rule_type_name(step.rule_type),
            step.rule_label
        ));

        // Annotations
        if let Some(desc) = step.annotations.description() {
            output.push_str(&format!("{indent}   Description: {desc}\n"));
        }
        if let Some(source) = step.annotations.source() {
            output.push_str(&format!("{indent}   Source: {source}\n"));
        }

        // Body proofs (prerequisites)
        if !step.body_proofs.is_empty() {
            output.push_str(&format!("{indent}   Prerequisites:\n"));
            for (i, bp) in step.body_proofs.iter().enumerate() {
                let sub_num = format!("{}.{}", num, i + 1);
                let sub_indent = format!("{indent}     ");
                output.push_str(&proof_node_to_natural_language(
                    bp,
                    sub_num.parse().unwrap_or(1),
                    &sub_indent,
                ));
            }
        }
    }

    output
}

/// Get human-readable rule type name.
pub(crate) fn rule_type_name(rt: RuleType) -> &'static str {
    match rt {
        RuleType::Fact => "fact",
        RuleType::Strict => "strict rule",
        RuleType::Defeasible => "defeasible rule",
        RuleType::Defeater => "defeater",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::explanation::format::ExplanationFormatter;
    use crate::literal::Literal;

    #[test]
    fn test_explanation_natural_language() {
        let step = ProofStep::new("f1", RuleType::Fact, ">> bird");
        let proof =
            ProofNode::new(Literal::simple("bird"), DerivationType::Definite).with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("bird"))
                .with_proof(proof);

        let formatter = NaturalLanguageFormatter::new();
        let nl = formatter.format(&explanation);
        assert!(nl.contains("bird"));
        assert!(nl.contains("+D")); // Uses symbol
        assert!(nl.contains("fact"));
    }

    #[test]
    fn test_natural_language_defeasible_derivation() {
        let body_step = ProofStep::new("f1", RuleType::Fact, ">> bird");
        let body_proof = ProofNode::new(Literal::simple("bird"), DerivationType::Definite)
            .with_proof_step(body_step);

        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies")
            .with_body_proofs(vec![body_proof]);
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("flies"));
        assert!(nl.contains("+d")); // Defeasibly provable symbol
        assert!(nl.contains("derived defeasibly"));
        assert!(nl.contains("defeasible rule"));
        assert!(nl.contains("Prerequisites"));
        assert!(nl.contains("bird"));
    }

    #[test]
    fn test_natural_language_with_annotations() {
        let annots = Annotations::with_entries(vec![
            ("description", "Birds typically fly"),
            ("source", "ornithology-textbook"),
        ]);
        let step =
            ProofStep::new("r1", RuleType::Defeasible, "bird => flies").with_annotations(annots);
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("Description: Birds typically fly"));
        assert!(nl.contains("Source: ornithology-textbook"));
    }

    #[test]
    fn test_explanation_with_conflict() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority);

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_conflicts(vec![conflict]);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("Conflict Resolutions"));
        assert!(nl.contains("r2"));
        assert!(nl.contains("r1"));
    }

    #[test]
    fn test_natural_language_with_blocked_alternatives() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Rule r2 is superior and concludes ~flies",
        )
        .with_blocking_rule("r2");

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_blocked(vec![blocked]);

        let nl = explanation.to_natural_language();
        assert!(nl.contains("Blocked Alternatives"));
        assert!(nl.contains("r1"));
        assert!(nl.contains("superiority"));
        assert!(nl.contains("Rule r2 is superior"));
    }

    #[test]
    fn test_natural_language_no_derivation() {
        let explanation = Explanation::new(
            ConclusionType::DefeasiblyNotProvable,
            Literal::simple("flies"),
        );

        let nl = explanation.to_natural_language();
        assert!(nl.contains("-d")); // Not provable symbol
        assert!(nl.contains("could not be proven defeasibly"));
    }

    #[test]
    fn test_natural_language_conclusion_type_explanations() {
        let exp_dp = Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("p"));
        assert!(
            exp_dp
                .to_natural_language()
                .contains("strict rules and facts")
        );

        let exp_dnp = Explanation::new(ConclusionType::DefinitelyNotProvable, Literal::simple("p"));
        assert!(
            exp_dnp
                .to_natural_language()
                .contains("cannot be proven using strict rules")
        );

        let exp_dfp = Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("p"));
        assert!(exp_dfp.to_natural_language().contains("defeasible rules"));

        let exp_dfnp =
            Explanation::new(ConclusionType::DefeasiblyNotProvable, Literal::simple("p"));
        assert!(
            exp_dfnp
                .to_natural_language()
                .contains("could not be proven defeasibly")
        );
    }

    #[test]
    fn test_rule_type_name_all_types() {
        assert_eq!(rule_type_name(RuleType::Fact), "fact");
        assert_eq!(rule_type_name(RuleType::Strict), "strict rule");
        assert_eq!(rule_type_name(RuleType::Defeasible), "defeasible rule");
        assert_eq!(rule_type_name(RuleType::Defeater), "defeater");
    }

    #[test]
    fn test_annotation_preservation_source_tracking() {
        let annots = Annotations::with_entries(vec![
            ("source", "legal-statute-42-usc-1983"),
            ("dc:source", "Civil Rights Act"),
        ]);
        let step = ProofStep::new("r1", RuleType::Defeasible, "violation => liable")
            .with_annotations(annots);
        let proof = ProofNode::new(Literal::simple("liable"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::simple("liable"),
        )
        .with_proof(proof);

        // Natural language should show source
        let nl = explanation.to_natural_language();
        assert!(nl.contains("Source: legal-statute-42-usc-1983"));
    }
}
