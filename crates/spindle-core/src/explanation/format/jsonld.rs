//! JSON-LD explanation formatter.
//!
//! Renders an [`Explanation`] as a JSON-LD document with semantic annotations.

use super::ExplanationFormatter;
use crate::explanation::types::*;
use crate::rule::RuleType;

/// Renders explanations as JSON-LD with semantic annotations.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonLdFormatter;

impl ExplanationFormatter for JsonLdFormatter {
    fn format(&self, explanation: &Explanation) -> String {
        let value = self.to_value(explanation);
        serde_json::to_string_pretty(&value).expect("Explanation JSON-LD serialization cannot fail")
    }
}

impl JsonLdFormatter {
    /// Return the [`serde_json::Value`] for callers that need structured data.
    pub fn to_value(&self, explanation: &Explanation) -> serde_json::Value {
        let context = serde_json::json!({
            "spindle": "https://spindle.dev/ontology#",
            "prov": "http://www.w3.org/ns/prov#",
            "rdfs": "http://www.w3.org/2000/01/rdf-schema#",
            "xsd": "http://www.w3.org/2001/XMLSchema#",
            "conclusionType": "spindle:conclusionType",
            "literal": "spindle:literal",
            "proofTree": "spindle:proofTree",
            "derivationType": "spindle:derivationType",
            "ruleLabel": "spindle:ruleLabel",
            "ruleType": "spindle:ruleType",
            "blockedAlternatives": "spindle:blockedAlternatives",
            "conflictsResolved": "spindle:conflictsResolved",
            "winningRule": "spindle:winningRule",
            "losingRule": "spindle:losingRule",
            "resolutionType": "spindle:resolutionType",
            "blockReason": "spindle:blockReason",
            "wasGeneratedBy": "prov:wasGeneratedBy",
            "wasAttributedTo": "prov:wasAttributedTo"
        });

        let mut doc = serde_json::json!({
            "@context": context,
            "@type": "spindle:Explanation",
            "conclusionType": explanation.conclusion_type.symbol(),
            "literal": explanation.literal.to_string(),
        });

        // Add proof tree with semantic annotations
        if let Some(ref proof) = explanation.proof_tree {
            doc["proofTree"] = proof_node_to_jsonld(proof);
        }

        // Add blocked alternatives
        if !explanation.blocked_alternatives.is_empty() {
            doc["blockedAlternatives"] = explanation
                .blocked_alternatives
                .iter()
                .map(blocked_proof_to_jsonld)
                .collect::<Vec<_>>()
                .into();
        }

        // Add conflict resolutions
        if !explanation.conflicts_resolved.is_empty() {
            doc["conflictsResolved"] = explanation
                .conflicts_resolved
                .iter()
                .map(conflict_to_jsonld)
                .collect::<Vec<_>>()
                .into();
        }

        doc
    }
}

// ===========================================================================
// Private helpers
// ===========================================================================

/// Convert proof node to JSON-LD with semantic annotations.
fn proof_node_to_jsonld(node: &ProofNode) -> serde_json::Value {
    let mut json = serde_json::json!({
        "@type": "spindle:ProofNode",
        "literal": node.literal.to_string(),
        "derivationType": match node.derivation_type {
            DerivationType::Definite => "definite",
            DerivationType::Defeasible => "defeasible",
        },
    });

    if let Some(ref step) = node.proof_step {
        let mut step_json = serde_json::json!({
            "@type": "spindle:ProofStep",
            "ruleLabel": step.rule_label,
            "ruleType": match step.rule_type {
                RuleType::Fact => "fact",
                RuleType::Strict => "strict",
                RuleType::Defeasible => "defeasible",
                RuleType::Defeater => "defeater",
            },
            "ruleText": step.rule_text,
        });

        // Add annotations with provenance
        if !step.annotations.is_empty() {
            if let Some(source) = step.annotations.source() {
                step_json["wasAttributedTo"] = serde_json::json!(source);
            }
            if let Some(desc) = step.annotations.description() {
                step_json["rdfs:comment"] = serde_json::json!(desc);
            }
            if let Some(conf) = step.annotations.confidence() {
                step_json["spindle:confidence"] = serde_json::json!(conf);
            }
            if let Some(id) = &step.annotations.id {
                step_json["@id"] = serde_json::json!(id);
            }
        }

        // Recursively add body proofs
        if !step.body_proofs.is_empty() {
            step_json["bodyProofs"] = step
                .body_proofs
                .iter()
                .map(proof_node_to_jsonld)
                .collect::<Vec<_>>()
                .into();
        }

        json["proofStep"] = step_json;
    }

    json
}

/// Convert blocked proof to JSON-LD.
fn blocked_proof_to_jsonld(blocked: &BlockedProof) -> serde_json::Value {
    let mut json = serde_json::json!({
        "@type": "spindle:BlockedProof",
        "literal": blocked.literal.to_string(),
        "ruleLabel": blocked.rule_label,
        "blockReason": blocked.reason.to_string(),
        "rdfs:comment": blocked.explanation,
    });

    if let Some(ref blocking) = blocked.blocking_rule {
        json["blockedBy"] = serde_json::json!(blocking);
    }

    json
}

/// Convert conflict resolution to JSON-LD.
fn conflict_to_jsonld(conflict: &ConflictResolution) -> serde_json::Value {
    let mut json = serde_json::json!({
        "@type": "spindle:ConflictResolution",
        "winningRule": conflict.winning_rule,
        "losingRule": conflict.losing_rule,
        "resolutionType": conflict.resolution_type.to_string(),
    });

    if let Some(ref sup) = conflict.superiority_label {
        json["superiorityLabel"] = serde_json::json!(sup);
    }

    json
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::literal::Literal;

    #[test]
    fn test_jsonld_context() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let jsonld = explanation.to_jsonld();

        // Verify @context exists and has required namespaces
        assert!(jsonld["@context"].is_object());
        assert_eq!(
            jsonld["@context"]["spindle"],
            "https://spindle.dev/ontology#"
        );
        assert_eq!(jsonld["@context"]["prov"], "http://www.w3.org/ns/prov#");
        assert_eq!(
            jsonld["@context"]["rdfs"],
            "http://www.w3.org/2000/01/rdf-schema#"
        );
        assert_eq!(
            jsonld["@context"]["xsd"],
            "http://www.w3.org/2001/XMLSchema#"
        );
    }

    #[test]
    fn test_jsonld_type() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let jsonld = explanation.to_jsonld();
        assert_eq!(jsonld["@type"], "spindle:Explanation");
    }

    #[test]
    fn test_jsonld_basic_fields() {
        let explanation =
            Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("bird"));

        let jsonld = explanation.to_jsonld();
        assert_eq!(jsonld["conclusionType"], "+D");
        assert_eq!(jsonld["literal"], "bird");
    }

    #[test]
    fn test_jsonld_with_proof_tree() {
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let jsonld = explanation.to_jsonld();

        assert!(jsonld["proofTree"].is_object());
        assert_eq!(jsonld["proofTree"]["@type"], "spindle:ProofNode");
        assert_eq!(jsonld["proofTree"]["literal"], "flies");
        assert_eq!(jsonld["proofTree"]["derivationType"], "defeasible");

        // Verify proof step
        assert!(jsonld["proofTree"]["proofStep"].is_object());
        assert_eq!(
            jsonld["proofTree"]["proofStep"]["@type"],
            "spindle:ProofStep"
        );
        assert_eq!(jsonld["proofTree"]["proofStep"]["ruleLabel"], "r1");
        assert_eq!(jsonld["proofTree"]["proofStep"]["ruleType"], "defeasible");
    }

    #[test]
    fn test_jsonld_with_annotations_provenance() {
        let mut annots = Annotations::with_entries(vec![
            ("source", "expert-dr-smith"),
            ("description", "Birds typically fly"),
            ("confidence", "0.9"),
        ]);
        annots.id = Some("https://example.org/rules/r1".to_string());

        let step =
            ProofStep::new("r1", RuleType::Defeasible, "bird => flies").with_annotations(annots);
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let jsonld = explanation.to_jsonld();
        let proof_step = &jsonld["proofTree"]["proofStep"];

        // Verify provenance annotations
        assert_eq!(proof_step["wasAttributedTo"], "expert-dr-smith");
        assert_eq!(proof_step["rdfs:comment"], "Birds typically fly");
        assert_eq!(proof_step["spindle:confidence"], "0.9");
        assert_eq!(proof_step["@id"], "https://example.org/rules/r1");
    }

    #[test]
    fn test_jsonld_with_nested_body_proofs() {
        // Create a two-level proof: flies <- bird <- penguin
        let penguin_step = ProofStep::new("f1", RuleType::Fact, ">> penguin");
        let penguin_proof = ProofNode::new(Literal::simple("penguin"), DerivationType::Definite)
            .with_proof_step(penguin_step);

        let bird_step = ProofStep::new("s1", RuleType::Strict, "penguin -> bird")
            .with_body_proofs(vec![penguin_proof]);
        let bird_proof = ProofNode::new(Literal::simple("bird"), DerivationType::Definite)
            .with_proof_step(bird_step);

        let flies_step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies")
            .with_body_proofs(vec![bird_proof]);
        let flies_proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(flies_step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(flies_proof);

        let jsonld = explanation.to_jsonld();

        // Navigate to nested proofs
        let body_proofs = &jsonld["proofTree"]["proofStep"]["bodyProofs"];
        assert!(body_proofs.is_array());
        assert_eq!(body_proofs[0]["literal"], "bird");
        assert_eq!(body_proofs[0]["@type"], "spindle:ProofNode");

        // Even deeper nesting
        let nested_body = &body_proofs[0]["proofStep"]["bodyProofs"];
        assert!(nested_body.is_array());
        assert_eq!(nested_body[0]["literal"], "penguin");
    }

    #[test]
    fn test_jsonld_with_blocked_alternatives() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Blocked by superior rule r2",
        )
        .with_blocking_rule("r2");

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_blocked(vec![blocked]);

        let jsonld = explanation.to_jsonld();

        assert!(jsonld["blockedAlternatives"].is_array());
        let blocked_alt = &jsonld["blockedAlternatives"][0];
        assert_eq!(blocked_alt["@type"], "spindle:BlockedProof");
        assert_eq!(blocked_alt["literal"], "flies");
        assert_eq!(blocked_alt["ruleLabel"], "r1");
        assert_eq!(blocked_alt["blockReason"], "superiority");
        assert_eq!(blocked_alt["rdfs:comment"], "Blocked by superior rule r2");
        assert_eq!(blocked_alt["blockedBy"], "r2");
    }

    #[test]
    fn test_jsonld_with_conflict_resolutions() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority)
            .with_superiority("sup1");

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_conflicts(vec![conflict]);

        let jsonld = explanation.to_jsonld();

        assert!(jsonld["conflictsResolved"].is_array());
        let resolved = &jsonld["conflictsResolved"][0];
        assert_eq!(resolved["@type"], "spindle:ConflictResolution");
        assert_eq!(resolved["winningRule"], "r2");
        assert_eq!(resolved["losingRule"], "r1");
        assert_eq!(resolved["resolutionType"], "superiority");
        assert_eq!(resolved["superiorityLabel"], "sup1");
    }

    #[test]
    fn test_jsonld_all_rule_types() {
        let fact_step = ProofStep::new("f1", RuleType::Fact, ">> p");
        let fact_proof = ProofNode::new(Literal::simple("p"), DerivationType::Definite)
            .with_proof_step(fact_step);
        let exp1 = Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("p"))
            .with_proof(fact_proof);
        assert_eq!(
            exp1.to_jsonld()["proofTree"]["proofStep"]["ruleType"],
            "fact"
        );

        let strict_step = ProofStep::new("s1", RuleType::Strict, "p -> q");
        let strict_proof = ProofNode::new(Literal::simple("q"), DerivationType::Definite)
            .with_proof_step(strict_step);
        let exp2 = Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("q"))
            .with_proof(strict_proof);
        assert_eq!(
            exp2.to_jsonld()["proofTree"]["proofStep"]["ruleType"],
            "strict"
        );

        let def_step = ProofStep::new("r1", RuleType::Defeasible, "p => r");
        let def_proof = ProofNode::new(Literal::simple("r"), DerivationType::Defeasible)
            .with_proof_step(def_step);
        let exp3 = Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("r"))
            .with_proof(def_proof);
        assert_eq!(
            exp3.to_jsonld()["proofTree"]["proofStep"]["ruleType"],
            "defeasible"
        );

        let defeater_step = ProofStep::new("d1", RuleType::Defeater, "p ~> s");
        let defeater_proof = ProofNode::new(Literal::simple("s"), DerivationType::Defeasible)
            .with_proof_step(defeater_step);
        let exp4 = Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("s"))
            .with_proof(defeater_proof);
        assert_eq!(
            exp4.to_jsonld()["proofTree"]["proofStep"]["ruleType"],
            "defeater"
        );
    }

    #[test]
    fn test_annotation_preservation_multiple_annotations_jsonld() {
        let mut annots = Annotations::with_entries(vec![
            ("source", "expert-opinion"),
            ("description", "Standard medical practice guideline"),
            ("confidence", "0.85"),
        ]);
        annots.id = Some("urn:guideline:med-123".to_string());

        let step = ProofStep::new("med_rule", RuleType::Defeasible, "symptom => diagnosis")
            .with_annotations(annots);
        let proof = ProofNode::new(Literal::simple("diagnosis"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::simple("diagnosis"),
        )
        .with_proof(proof);

        let jsonld = explanation.to_jsonld();
        let step_jsonld = &jsonld["proofTree"]["proofStep"];

        assert_eq!(step_jsonld["@id"], "urn:guideline:med-123");
        assert_eq!(step_jsonld["wasAttributedTo"], "expert-opinion");
        assert_eq!(
            step_jsonld["rdfs:comment"],
            "Standard medical practice guideline"
        );
        assert_eq!(step_jsonld["spindle:confidence"], "0.85");
    }

    #[test]
    fn test_blocked_proof_with_blocking_rule_jsonld() {
        let blocked = BlockedProof::new(
            Literal::simple("x"),
            "r1",
            BlockReason::Superiority,
            "blocked by superior rule",
        )
        .with_blocking_rule("r2");

        assert_eq!(blocked.blocking_rule, Some("r2".to_string()));

        let jsonld = blocked_proof_to_jsonld(&blocked);
        assert_eq!(jsonld["blockedBy"], "r2");
    }

    #[test]
    fn test_annotation_preservation_jsonld_blocked() {
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

        let jsonld = explanation.to_jsonld();
        assert_eq!(
            jsonld["blockedAlternatives"][0]["ruleLabel"],
            "bird_flies_rule"
        );
        assert_eq!(
            jsonld["blockedAlternatives"][0]["blockedBy"],
            "penguin_no_fly"
        );
    }

    #[test]
    fn test_annotation_preservation_jsonld_conflict() {
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

        let jsonld = explanation.to_jsonld();
        assert_eq!(
            jsonld["conflictsResolved"][0]["winningRule"],
            "specific_penguin_rule"
        );
        assert_eq!(
            jsonld["conflictsResolved"][0]["losingRule"],
            "general_bird_rule"
        );
        assert_eq!(
            jsonld["conflictsResolved"][0]["superiorityLabel"],
            "penguin_beats_bird_sup"
        );
    }

    #[test]
    fn test_annotation_preservation_source_tracking_jsonld() {
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

        let jsonld = explanation.to_jsonld();
        assert_eq!(
            jsonld["proofTree"]["proofStep"]["wasAttributedTo"],
            "legal-statute-42-usc-1983"
        );
    }
}
