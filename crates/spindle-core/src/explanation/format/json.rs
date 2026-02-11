//! JSON explanation formatter.
//!
//! Renders an [`Explanation`] as a flat JSON structure.

use super::ExplanationFormatter;
use crate::explanation::types::*;
use crate::rule::RuleType;

/// Renders explanations as a flat JSON structure.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonFormatter;

impl ExplanationFormatter for JsonFormatter {
    fn format(&self, explanation: &Explanation) -> String {
        let value = self.to_value(explanation);
        serde_json::to_string_pretty(&value).expect("Explanation JSON serialization cannot fail")
    }
}

impl JsonFormatter {
    /// Return the [`serde_json::Value`] for callers that need structured data
    /// (e.g., the CLI embedding it in a larger JSON envelope).
    pub fn to_value(&self, explanation: &Explanation) -> serde_json::Value {
        serde_json::json!({
            "conclusion_type": explanation.conclusion_type.symbol(),
            "literal": explanation.literal.to_string(),
            "proof_tree": explanation.proof_tree.as_ref().map(proof_node_to_json),
            "blocked_alternatives": explanation.blocked_alternatives.iter()
                .map(blocked_proof_to_json)
                .collect::<Vec<_>>(),
            "conflicts_resolved": explanation.conflicts_resolved.iter()
                .map(conflict_to_json)
                .collect::<Vec<_>>(),
        })
    }
}

// ===========================================================================
// Private helpers
// ===========================================================================

/// Convert proof node to JSON.
fn proof_node_to_json(node: &ProofNode) -> serde_json::Value {
    serde_json::json!({
        "literal": node.literal.to_string(),
        "derivation_type": match node.derivation_type {
            DerivationType::Definite => "definite",
            DerivationType::Defeasible => "defeasible",
        },
        "proof_step": node.proof_step.as_ref().map(|step| serde_json::json!({
            "rule_label": step.rule_label,
            "rule_type": match step.rule_type {
                RuleType::Fact => "fact",
                RuleType::Strict => "strict",
                RuleType::Defeasible => "defeasible",
                RuleType::Defeater => "defeater",
            },
            "rule_text": step.rule_text,
            "body_proofs": step.body_proofs.iter()
                .map(proof_node_to_json)
                .collect::<Vec<_>>(),
        })),
    })
}

/// Convert blocked proof to JSON.
fn blocked_proof_to_json(blocked: &BlockedProof) -> serde_json::Value {
    serde_json::json!({
        "literal": blocked.literal.to_string(),
        "rule_label": blocked.rule_label,
        "reason": blocked.reason.to_string(),
        "blocking_rule": blocked.blocking_rule,
        "explanation": blocked.explanation,
    })
}

/// Convert conflict resolution to JSON.
fn conflict_to_json(conflict: &ConflictResolution) -> serde_json::Value {
    serde_json::json!({
        "winning_rule": conflict.winning_rule,
        "losing_rule": conflict.losing_rule,
        "resolution_type": conflict.resolution_type.to_string(),
        "superiority_label": conflict.superiority_label,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::literal::Literal;

    #[test]
    fn test_explanation_json() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let json = explanation.to_json();
        assert_eq!(json["conclusion_type"], "+d");
        assert_eq!(json["literal"], "flies");
    }

    #[test]
    fn test_json_with_proof_tree() {
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let json = explanation.to_json();
        assert!(json["proof_tree"].is_object());
        assert_eq!(json["proof_tree"]["literal"], "flies");
        assert_eq!(json["proof_tree"]["derivation_type"], "defeasible");
        assert_eq!(json["proof_tree"]["proof_step"]["rule_label"], "r1");
        assert_eq!(json["proof_tree"]["proof_step"]["rule_type"], "defeasible");
    }

    #[test]
    fn test_json_with_blocked_alternatives() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Blocked by r2",
        )
        .with_blocking_rule("r2");

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_blocked(vec![blocked]);

        let json = explanation.to_json();
        assert!(json["blocked_alternatives"].is_array());
        assert_eq!(json["blocked_alternatives"][0]["rule_label"], "r1");
        assert_eq!(json["blocked_alternatives"][0]["reason"], "superiority");
        assert_eq!(json["blocked_alternatives"][0]["blocking_rule"], "r2");
    }

    #[test]
    fn test_json_with_conflicts() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority)
            .with_superiority("sup1");

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_conflicts(vec![conflict]);

        let json = explanation.to_json();
        assert!(json["conflicts_resolved"].is_array());
        assert_eq!(json["conflicts_resolved"][0]["winning_rule"], "r2");
        assert_eq!(json["conflicts_resolved"][0]["losing_rule"], "r1");
        assert_eq!(
            json["conflicts_resolved"][0]["resolution_type"],
            "superiority"
        );
        assert_eq!(json["conflicts_resolved"][0]["superiority_label"], "sup1");
    }

    #[test]
    fn test_proof_node_to_json_with_defeater() {
        let step = ProofStep {
            rule_label: "d1".to_string(),
            rule_type: RuleType::Defeater,
            rule_text: "d1: p ~> q".to_string(),
            body_proofs: vec![],
            annotations: Default::default(),
        };

        let node =
            ProofNode::new(Literal::simple("q"), DerivationType::Defeasible).with_proof_step(step);

        let json = proof_node_to_json(&node);
        assert_eq!(json["proof_step"]["rule_type"], "defeater");
    }

    #[test]
    fn test_annotation_preservation_rule_labels_json() {
        let step = ProofStep::new("bird_flies_rule", RuleType::Defeasible, "bird => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let json = explanation.to_json();
        assert_eq!(
            json["proof_tree"]["proof_step"]["rule_label"],
            "bird_flies_rule"
        );
    }

    #[test]
    fn test_annotation_preservation_blocked_proof_info_json() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "bird_flies_rule",
            BlockReason::Superiority,
            "The penguin exception rule (penguin_no_fly) is superior",
        )
        .with_blocking_rule("penguin_no_fly");

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_blocked(vec![blocked]);

        let json = explanation.to_json();
        assert_eq!(
            json["blocked_alternatives"][0]["rule_label"],
            "bird_flies_rule"
        );
        assert_eq!(
            json["blocked_alternatives"][0]["blocking_rule"],
            "penguin_no_fly"
        );
        assert!(
            json["blocked_alternatives"][0]["explanation"]
                .as_str()
                .unwrap()
                .contains("penguin exception")
        );
    }

    #[test]
    fn test_annotation_preservation_conflict_info_json() {
        let conflict = ConflictResolution::new(
            "specific_penguin_rule",
            "general_bird_rule",
            ResolutionType::Superiority,
        )
        .with_superiority("penguin_beats_bird_sup");

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_conflicts(vec![conflict]);

        let json = explanation.to_json();
        assert_eq!(
            json["conflicts_resolved"][0]["winning_rule"],
            "specific_penguin_rule"
        );
        assert_eq!(
            json["conflicts_resolved"][0]["losing_rule"],
            "general_bird_rule"
        );
        assert_eq!(
            json["conflicts_resolved"][0]["superiority_label"],
            "penguin_beats_bird_sup"
        );
    }
}
