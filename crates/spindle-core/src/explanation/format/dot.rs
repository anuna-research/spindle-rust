//! DOT graph explanation formatter.
//!
//! Renders an [`Explanation`] as a Graphviz DOT graph.

use super::ExplanationFormatter;
use crate::explanation::types::*;
use crate::rule::RuleType;

/// Renders explanations as Graphviz DOT graphs.
///
/// The generated graph shows:
/// - Proof nodes as boxes (blue for definite, green for defeasible)
/// - Rule applications as edges
/// - Blocked alternatives as red dashed nodes
/// - Conflict resolutions as orange diamond nodes
#[derive(Debug, Clone)]
pub struct DotFormatter {
    /// Graph direction: `"BT"` (bottom-to-top) or `"TB"` (top-to-bottom).
    pub rank_dir: String,
    /// Font for nodes and edges.
    pub font: String,
}

impl Default for DotFormatter {
    fn default() -> Self {
        Self {
            rank_dir: "BT".to_string(),
            font: "Helvetica".to_string(),
        }
    }
}

impl ExplanationFormatter for DotFormatter {
    fn format(&self, explanation: &Explanation) -> String {
        let mut output = String::new();
        let mut node_counter = 0;

        output.push_str("digraph Explanation {\n");
        output.push_str(&format!("  rankdir={};\n", self.rank_dir));
        output.push_str(&format!("  node [fontname=\"{}\"];\n", self.font));
        output.push_str(&format!("  edge [fontname=\"{}\"];\n\n", self.font));

        // Title node
        let escaped_literal = escape_dot_label(&explanation.literal.to_string());
        output.push_str(&format!(
            "  title [label=\"{} {}\" shape=plaintext fontsize=14 fontcolor=black];\n\n",
            explanation.conclusion_type, escaped_literal
        ));

        // Render proof tree
        if let Some(ref proof) = explanation.proof_tree {
            let root_id = render_proof_node_to_dot(proof, &mut output, &mut node_counter);
            output.push_str(&format!("  title -> n{root_id} [style=invis];\n"));
        }

        // Render blocked alternatives
        if !explanation.blocked_alternatives.is_empty() {
            output.push_str("\n  // Blocked alternatives\n");
            output.push_str("  subgraph cluster_blocked {\n");
            output.push_str("    label=\"Blocked Alternatives\";\n");
            output.push_str("    style=dashed;\n");
            output.push_str("    color=red;\n");

            for blocked in &explanation.blocked_alternatives {
                node_counter += 1;
                let escaped_lit = escape_dot_label(&blocked.literal.to_string());
                let escaped_rule = escape_dot_label(&blocked.rule_label);
                let escaped_reason = escape_dot_label(&blocked.reason.to_string());
                output.push_str(&format!(
                    "    b{node_counter} [label=\"{escaped_lit}\\n(rule: {escaped_rule})\\nblocked: {escaped_reason}\" shape=box style=\"dashed,filled\" fillcolor=\"#ffcccc\"];\n"
                ));
            }
            output.push_str("  }\n");
        }

        // Render conflict resolutions
        if !explanation.conflicts_resolved.is_empty() {
            output.push_str("\n  // Conflict resolutions\n");
            output.push_str("  subgraph cluster_conflicts {\n");
            output.push_str("    label=\"Conflict Resolutions\";\n");
            output.push_str("    style=dashed;\n");
            output.push_str("    color=orange;\n");

            for conflict in &explanation.conflicts_resolved {
                node_counter += 1;
                let escaped_winner = escape_dot_label(&conflict.winning_rule);
                let escaped_loser = escape_dot_label(&conflict.losing_rule);
                let escaped_resolution = escape_dot_label(&conflict.resolution_type.to_string());
                output.push_str(&format!(
                    "    c{node_counter} [label=\"{escaped_winner} > {escaped_loser}\\n({escaped_resolution})\" shape=diamond style=filled fillcolor=\"#ffe0b3\"];\n"
                ));
            }
            output.push_str("  }\n");
        }

        output.push_str("}\n");
        output
    }
}

// ===========================================================================
// Private helpers
// ===========================================================================

/// Escape special characters for DOT labels.
pub(crate) fn escape_dot_label(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('<', "\\<")
        .replace('>', "\\>")
        .replace('{', "\\{")
        .replace('}', "\\}")
}

/// Render a proof node to DOT format, returning the node ID.
fn render_proof_node_to_dot(node: &ProofNode, output: &mut String, counter: &mut usize) -> usize {
    *counter += 1;
    let node_id = *counter;

    let color = match node.derivation_type {
        DerivationType::Definite => "#cce5ff",   // Light blue
        DerivationType::Defeasible => "#d4edda", // Light green
    };

    let escaped_literal = escape_dot_label(&node.literal.to_string());

    // Create node label
    let label = if let Some(ref step) = node.proof_step {
        let rule_type_str = match step.rule_type {
            RuleType::Fact => "fact",
            RuleType::Strict => "strict",
            RuleType::Defeasible => "defeasible",
            RuleType::Defeater => "defeater",
        };
        let escaped_label = escape_dot_label(&step.rule_label);
        format!("{escaped_literal}\\n[{rule_type_str}: {escaped_label}]")
    } else {
        escaped_literal
    };

    output.push_str(&format!(
        "  n{node_id} [label=\"{label}\" shape=box style=filled fillcolor=\"{color}\"];\n"
    ));

    // Render body proofs and add edges
    if let Some(ref step) = node.proof_step {
        for body_proof in &step.body_proofs {
            let child_id = render_proof_node_to_dot(body_proof, output, counter);
            let escaped_rule = escape_dot_label(&step.rule_label);
            output.push_str(&format!(
                "  n{child_id} -> n{node_id} [label=\"{escaped_rule}\"];\n"
            ));
        }
    }

    node_id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conclusion::ConclusionType;
    use crate::explanation::format::ExplanationFormatter;
    use crate::literal::Literal;

    #[test]
    fn test_dot_basic_structure() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let dot = explanation.to_dot();

        // Check basic DOT structure
        assert!(dot.starts_with("digraph Explanation {"));
        assert!(dot.ends_with("}\n"));
        assert!(dot.contains("rankdir=BT")); // Bottom-to-top layout
        assert!(dot.contains("node [fontname=\"Helvetica\"]"));
    }

    #[test]
    fn test_dot_title_node() {
        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"));

        let dot = explanation.to_dot();

        // Title should contain conclusion type and literal
        assert!(dot.contains("title [label=\"+d flies\""));
        assert!(dot.contains("shape=plaintext"));
    }

    #[test]
    fn test_dot_with_proof_node() {
        let step = ProofStep::new("r1", RuleType::Defeasible, "bird => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let dot = explanation.to_dot();

        // Should have a node for the literal
        assert!(dot.contains("flies"));
        assert!(dot.contains("[defeasible: r1]"));
        assert!(dot.contains("shape=box"));
        assert!(dot.contains("style=filled"));
        // Green color for defeasible
        assert!(dot.contains("#d4edda"));
    }

    #[test]
    fn test_dot_definite_derivation_color() {
        let step = ProofStep::new("f1", RuleType::Fact, ">> bird");
        let proof =
            ProofNode::new(Literal::simple("bird"), DerivationType::Definite).with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefinitelyProvable, Literal::simple("bird"))
                .with_proof(proof);

        let dot = explanation.to_dot();

        // Blue color for definite
        assert!(dot.contains("#cce5ff"));
    }

    #[test]
    fn test_dot_with_body_proofs() {
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

        let dot = explanation.to_dot();

        // Should have edge between nodes
        assert!(dot.contains("->")); // Edge notation
        assert!(dot.contains("[label=\"r1\"]")); // Edge label
        assert!(dot.contains("bird"));
        assert!(dot.contains("flies"));
    }

    #[test]
    fn test_dot_with_blocked_alternatives() {
        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Blocked by r2",
        );

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_blocked(vec![blocked]);

        let dot = explanation.to_dot();

        // Check blocked alternatives cluster
        assert!(dot.contains("subgraph cluster_blocked"));
        assert!(dot.contains("label=\"Blocked Alternatives\""));
        assert!(dot.contains("style=dashed"));
        assert!(dot.contains("color=red"));
        assert!(dot.contains("#ffcccc")); // Light red fill
        assert!(dot.contains("blocked: superiority"));
        assert!(dot.contains("(rule: r1)"));
    }

    #[test]
    fn test_dot_with_conflict_resolutions() {
        let conflict = ConflictResolution::new("r2", "r1", ResolutionType::Superiority);

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_conflicts(vec![conflict]);

        let dot = explanation.to_dot();

        // Check conflicts cluster
        assert!(dot.contains("subgraph cluster_conflicts"));
        assert!(dot.contains("label=\"Conflict Resolutions\""));
        assert!(dot.contains("color=orange"));
        assert!(dot.contains("shape=diamond")); // Conflict nodes are diamonds
        assert!(dot.contains("#ffe0b3")); // Orange fill
        assert!(dot.contains("r2 > r1"));
        assert!(dot.contains("(superiority)"));
    }

    #[test]
    fn test_dot_escaping_special_characters() {
        let step = ProofStep::new("r<1>", RuleType::Defeasible, "bird \"tweety\" => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let dot = explanation.to_dot();

        // Special characters should be escaped
        assert!(dot.contains("r\\<1\\>")); // Escaped < and >
    }

    #[test]
    fn test_dot_with_negated_literal() {
        let step = ProofStep::new("r2", RuleType::Defeasible, "penguin => ~flies");
        let proof = ProofNode::new(Literal::negated("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_proof(proof);

        let dot = explanation.to_dot();

        assert!(dot.contains("~flies"));
    }

    #[test]
    fn test_dot_complex_proof_tree() {
        let penguin_step = ProofStep::new("f1", RuleType::Fact, ">> penguin");
        let penguin_proof = ProofNode::new(Literal::simple("penguin"), DerivationType::Definite)
            .with_proof_step(penguin_step);

        let bird_step = ProofStep::new("s1", RuleType::Strict, "penguin -> bird")
            .with_body_proofs(vec![penguin_proof]);
        let bird_proof = ProofNode::new(Literal::simple("bird"), DerivationType::Definite)
            .with_proof_step(bird_step);

        let not_flies_step = ProofStep::new("r2", RuleType::Defeasible, "penguin => ~flies")
            .with_body_proofs(vec![bird_proof.clone()]);
        let not_flies_proof = ProofNode::new(Literal::negated("flies"), DerivationType::Defeasible)
            .with_proof_step(not_flies_step);

        let blocked = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "r2 > r1 via superiority",
        );

        let conflict =
            ConflictResolution::new("r2", "r1", ResolutionType::Superiority).with_superiority("s1");

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_proof(not_flies_proof)
        .with_blocked(vec![blocked])
        .with_conflicts(vec![conflict]);

        let dot = explanation.to_dot();

        // Verify all components are present
        assert!(dot.contains("penguin"));
        assert!(dot.contains("bird"));
        assert!(dot.contains("~flies"));
        assert!(dot.contains("cluster_blocked"));
        assert!(dot.contains("cluster_conflicts"));

        // Verify edges exist
        let edge_count = dot.matches("->").count();
        assert!(
            edge_count >= 2,
            "Expected at least 2 edges, found {edge_count}"
        );
    }

    #[test]
    fn test_dot_multiple_blocked_alternatives() {
        let blocked1 = BlockedProof::new(
            Literal::simple("flies"),
            "r1",
            BlockReason::Superiority,
            "Blocked by r2",
        );
        let blocked2 = BlockedProof::new(
            Literal::simple("flies"),
            "r3",
            BlockReason::Defeater,
            "Blocked by defeater d1",
        );

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_blocked(vec![blocked1, blocked2]);

        let dot = explanation.to_dot();

        // Both blocked alternatives should be present
        assert!(dot.contains("(rule: r1)"));
        assert!(dot.contains("(rule: r3)"));
        assert!(dot.contains("blocked: superiority"));
        assert!(dot.contains("blocked: defeater"));
    }

    #[test]
    fn test_dot_multiple_conflict_resolutions() {
        let conflict1 = ConflictResolution::new("r2", "r1", ResolutionType::Superiority);
        let conflict2 = ConflictResolution::new("r4", "r3", ResolutionType::TeamDefeat);

        let explanation = Explanation::new(
            ConclusionType::DefeasiblyProvable,
            Literal::negated("flies"),
        )
        .with_conflicts(vec![conflict1, conflict2]);

        let dot = explanation.to_dot();

        assert!(dot.contains("r2 > r1"));
        assert!(dot.contains("r4 > r3"));
        assert!(dot.contains("(superiority)"));
        assert!(dot.contains("(team defeat)"));
    }

    #[test]
    fn test_escape_dot_label() {
        assert_eq!(escape_dot_label("simple"), "simple");
        assert_eq!(escape_dot_label("with\"quotes"), "with\\\"quotes");
        assert_eq!(
            escape_dot_label("with<angle>brackets"),
            "with\\<angle\\>brackets"
        );
        assert_eq!(
            escape_dot_label("with{curly}braces"),
            "with\\{curly\\}braces"
        );
        assert_eq!(escape_dot_label("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_dot_label("with\nnewline"), "with\\nnewline");
    }

    #[test]
    fn test_dot_annotation_preservation_rule_labels() {
        let step = ProofStep::new("bird_flies_rule", RuleType::Defeasible, "bird => flies");
        let proof = ProofNode::new(Literal::simple("flies"), DerivationType::Defeasible)
            .with_proof_step(step);

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("flies"))
                .with_proof(proof);

        let dot = explanation.to_dot();
        assert!(dot.contains("bird_flies_rule"));
    }

    #[test]
    fn test_dot_formatter_custom_config() {
        let formatter = DotFormatter {
            rank_dir: "TB".to_string(),
            font: "Courier".to_string(),
        };

        let explanation =
            Explanation::new(ConclusionType::DefeasiblyProvable, Literal::simple("p"));

        let dot = formatter.format(&explanation);
        assert!(dot.contains("rankdir=TB"));
        assert!(dot.contains("fontname=\"Courier\""));
    }
}
