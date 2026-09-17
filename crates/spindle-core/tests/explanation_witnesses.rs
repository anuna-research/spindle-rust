//! Issue #37: proofs on original quantified theories must preserve one witness.
use spindle_core::explanation::{ProofNode, explain};
use spindle_core::grounding::{apply_substitution_to_literal, match_literal};
use spindle_core::literal::Literal;
use spindle_core::reason::reason;
use spindle_core::theory::Theory;
use spindle_parser::parse_spl;

fn atom(spl: &str) -> Literal {
    parse_spl(&format!("(given {spl})"))
        .unwrap()
        .rules()
        .next()
        .unwrap()
        .head[0]
        .clone()
}

// Independently match the head AND all premises to one original source rule.
// Matching each leaf separately would miss the bug this regression guards.
fn validate_tree(theory: &Theory, node: &ProofNode) {
    let step = node.proof_step.as_ref().expect("complete proof step");
    assert!(
        theory.rules().any(|rule| {
            if rule.rule_type != step.rule_type || rule.body.len() != step.body_proofs.len() {
                return false;
            }
            rule.head.iter().any(|head| {
                let Some(mut subst) = match_literal(head, &node.literal) else {
                    return false;
                };
                for (body, proof) in rule.body.iter().zip(&step.body_proofs) {
                    let pattern = apply_substitution_to_literal(
                        &body.as_logic().unwrap().to_literal(),
                        &subst,
                    );
                    let Some(bindings) = match_literal(&pattern, &proof.literal) else {
                        return false;
                    };
                    subst.terms.extend(bindings.terms);
                }
                true
            })
        }),
        "no consistent source rule for {node:?}"
    );
    // The displayed rule must itself be the grounded instance being proved.
    let cited = parse_spl(&step.rule_text).unwrap();
    let rule = cited.rules().next().unwrap();
    assert!(rule.head.contains(&node.literal));
    assert_eq!(rule.body.len(), step.body_proofs.len());
    for (body, child) in rule.body.iter().zip(&step.body_proofs) {
        assert_eq!(body.as_logic().unwrap().to_literal(), child.literal);
    }
    for child in &step.body_proofs {
        validate_tree(theory, child);
    }
}

#[test]
fn issue_37_original_theory_preserves_witnesses_in_different_fact_orders() {
    let source = include_str!("fixtures/issue_37_witness.spl");
    let (facts, rules): (Vec<_>, Vec<_>) =
        source.lines().partition(|line| line.starts_with("(given "));
    for reverse in [false, true] {
        for offset in 0..facts.len() {
            let mut ordered = facts.clone();
            if reverse {
                ordered.reverse();
            }
            ordered.rotate_left(offset);
            let theory =
                parse_spl(&format!("{}\n{}", ordered.join("\n"), rules.join("\n"))).unwrap();
            let conclusions = reason(&theory).unwrap();
            for target in ["(tests-passed rev-b)", "(recommended run-b review)"] {
                let literal = atom(target);
                assert!(
                    conclusions
                        .iter()
                        .any(|c| c.literal == literal && c.conclusion_type.is_positive())
                );
                let explanation = explain(&theory, &literal).unwrap().unwrap();
                validate_tree(
                    &theory,
                    explanation.proof_tree.as_ref().expect("complete proof"),
                );
            }
            let wrong_revision = atom("(tests-passed rev-c)");
            assert!(
                !conclusions
                    .iter()
                    .any(|c| c.literal == wrong_revision && c.conclusion_type.is_positive())
            );
        }
    }
}
