//! Temporal bridge pipeline stage.
//!
//! Implements [`TemporalBridge`], which scans rule heads for temporal literals
//! and generates strict bridging rules that connect temporal atoms to their
//! atemporal base forms:
//!
//! - **Positive bridge:** `q[s,e] → base(q)` — a temporal fact implies its base.
//! - **Negation bridge:** `~q[s,e] → ~base(q)` — coupled negation propagation.
//!
//! Bridge rules are strict and fire during Phase 1 (definite closure) of SDL,
//! requiring zero modifications to the defeasible reasoning module.

use std::collections::HashSet;

use smallvec::smallvec;

use super::{Diagnostic, PipelineContext, PipelineStage, Severity};
use crate::body::{BodyArg, BodyLiteral, BodyLogicLiteral};
use crate::error::Result;
use crate::literal::Literal;
use crate::rule::Rule;
use crate::temporal::Temporal;
use crate::theory::Theory;

/// Pipeline stage that generates strict bridging rules from temporal head
/// literals to their atemporal base forms.
///
/// For each distinct temporal head literal `q[s,e]` found across all rules,
/// two strict rules are added:
///
/// 1. `q[s,e] → q` (positive bridge)
/// 2. `~q[s,e] → ~q` (negation bridge)
///
/// Deduplication is by bridge label, which encodes the literal's SPL rendering.
///
/// # Insertion point
///
/// After grounding, before `TemporalVarValidation`:
///
/// ```text
/// Validate → WildcardRewrite → Ground → TemporalBridge → TemporalVarValidation
/// ```
#[derive(Debug, Clone, Default)]
pub struct TemporalBridge;

impl PipelineStage for TemporalBridge {
    fn name(&self) -> &'static str {
        "temporal_bridge"
    }

    fn apply(&self, mut theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        // Collect distinct temporal head literals. We use the SPL rendering
        // as the deduplication key since it encodes functor, args, mode,
        // negation, and temporal bounds.
        let mut seen: HashSet<String> = HashSet::new();
        let mut bridges: Vec<Rule> = Vec::new();

        // Pre-seed seen set from pre-existing __bridge:: labels so we never
        // regenerate a bridge that already exists in the theory.
        for rule in theory.rules() {
            if let Some(spl_key) = rule.label.strip_prefix("__bridge::") {
                // Strip the optional "neg::" prefix to recover the canonical key
                let canonical = spl_key.strip_prefix("neg::").unwrap_or(spl_key);
                seen.insert(canonical.to_string());
            }
        }

        for rule in theory.rules() {
            for head_lit in rule.head.iter() {
                if head_lit.temporal.is_empty() {
                    continue;
                }

                // Build a canonical key from the *positive* form so that we
                // generate one pair (positive + negation) per distinct atom,
                // regardless of whether the head was negated.
                let positive = if head_lit.negation {
                    head_lit.complement()
                } else {
                    head_lit.clone()
                };
                let key = positive.to_spl();

                if !seen.insert(key) {
                    continue;
                }

                // --- Positive bridge: q[s,e] → q ---
                let pos_label = format!("__bridge::{}", positive.to_spl());
                let pos_body = smallvec![BodyLiteral::Logic(BodyLogicLiteral::from_ids(
                    positive.interned_name(),
                    false,
                    positive.mode.clone(),
                    positive.temporal.clone(),
                    positive
                        .predicate_args()
                        .iter()
                        .map(|t| BodyArg::Term(t.clone()))
                        .collect(),
                ))];
                let pos_head = Literal::from_ids(
                    positive.interned_name(),
                    false,
                    positive.mode.clone(),
                    Temporal::empty(),
                    positive.predicate_args().to_vec(),
                );
                bridges.push(Rule::strict(pos_label, pos_body, pos_head));

                // --- Negation bridge: ~q[s,e] → ~q ---
                let neg_label = format!("__bridge::neg::{}", positive.to_spl());
                let neg_body = smallvec![BodyLiteral::Logic(BodyLogicLiteral::from_ids(
                    positive.interned_name(),
                    true,
                    positive.mode.clone(),
                    positive.temporal.clone(),
                    positive
                        .predicate_args()
                        .iter()
                        .map(|t| BodyArg::Term(t.clone()))
                        .collect(),
                ))];
                let neg_head = Literal::from_ids(
                    positive.interned_name(),
                    true,
                    positive.mode.clone(),
                    Temporal::empty(),
                    positive.predicate_args().to_vec(),
                );
                bridges.push(Rule::strict(neg_label, neg_body, neg_head));
            }
        }

        let count = bridges.len();
        for rule in bridges {
            theory.add_rule(rule);
        }

        if count > 0 {
            ctx.diagnostics.push(Diagnostic {
                severity: Severity::Info,
                stage: self.name(),
                message: format!("generated {count} bridging rules ({} pairs)", count / 2),
            });
        }

        Ok(theory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::PipelineContext;
    use crate::rule::RuleType;
    use crate::temporal::TimePoint;

    #[test]
    fn no_op_for_non_temporal_theory() {
        let mut theory = Theory::new();
        theory.add_fact("a");
        theory.add_defeasible_rule(&["a"], "b");

        let mut ctx = PipelineContext::default();
        let result = TemporalBridge.apply(theory.clone(), &mut ctx).unwrap();

        // No bridges should be generated
        assert_eq!(result.rule_count(), theory.rule_count());
    }

    #[test]
    fn generates_bridge_pair_for_temporal_head() {
        let mut theory = Theory::new();

        // Add a fact with temporal bounds: p[100, 200]
        let head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(100), TimePoint::from_millis(200)),
            vec![],
        );
        theory.add_rule(Rule::new(
            "f1",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![head],
        ));

        let original_count = theory.rule_count();
        let mut ctx = PipelineContext::default();
        let result = TemporalBridge.apply(theory, &mut ctx).unwrap();

        // Should have 2 extra rules (positive + negation bridge)
        assert_eq!(result.rule_count(), original_count + 2);

        // Check positive bridge exists and is strict
        let pos_bridge = result
            .rules()
            .find(|r| r.label.starts_with("__bridge::") && !r.label.contains("neg"))
            .expect("positive bridge should exist");
        assert_eq!(pos_bridge.rule_type, RuleType::Strict);
        assert_eq!(pos_bridge.head.len(), 1);
        assert!(pos_bridge.head[0].temporal.is_empty()); // head is atemporal
        assert!(!pos_bridge.head[0].negation);

        // Check negation bridge exists and is strict
        let neg_bridge = result
            .rules()
            .find(|r| r.label.contains("__bridge::neg::"))
            .expect("negation bridge should exist");
        assert_eq!(neg_bridge.rule_type, RuleType::Strict);
        assert_eq!(neg_bridge.head.len(), 1);
        assert!(neg_bridge.head[0].temporal.is_empty());
        assert!(neg_bridge.head[0].negation); // head is negated
    }

    #[test]
    fn deduplicates_same_temporal_head() {
        let mut theory = Theory::new();

        let head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("q"),
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(0), TimePoint::from_millis(50)),
            vec![],
        );

        // Two rules with the same temporal head
        theory.add_rule(Rule::new(
            "r1",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![head.clone()],
        ));
        theory.add_rule(Rule::new(
            "r2",
            RuleType::Defeasible,
            vec![Literal::from_ids(
                crate::literal::InternedLiteralName::intern("a"),
                false,
                crate::mode::Mode::empty(),
                Temporal::empty(),
                vec![],
            )],
            smallvec::smallvec![head],
        ));

        let original_count = theory.rule_count();
        let mut ctx = PipelineContext::default();
        let result = TemporalBridge.apply(theory, &mut ctx).unwrap();

        // Only one pair of bridges despite two rules with same temporal head
        assert_eq!(result.rule_count(), original_count + 2);
    }

    #[test]
    fn different_temporal_intervals_get_separate_bridges() {
        let mut theory = Theory::new();

        let h1 = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(0), TimePoint::from_millis(100)),
            vec![],
        );
        let h2 = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(200), TimePoint::from_millis(300)),
            vec![],
        );

        theory.add_rule(Rule::new(
            "f1",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![h1],
        ));
        theory.add_rule(Rule::new(
            "f2",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![h2],
        ));

        let original_count = theory.rule_count();
        let mut ctx = PipelineContext::default();
        let result = TemporalBridge.apply(theory, &mut ctx).unwrap();

        // Two distinct temporal intervals → 2 pairs = 4 bridges
        assert_eq!(result.rule_count(), original_count + 4);
    }

    #[test]
    fn skips_generation_when_bridge_labels_already_exist() {
        let mut theory = Theory::new();

        let head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(100), TimePoint::from_millis(200)),
            vec![],
        );
        let spl_key = head.to_spl();

        // Add a temporal fact
        theory.add_rule(Rule::new(
            "f1",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![head.clone()],
        ));

        // Pre-add a bridge rule with the expected label
        let pos_label = format!("__bridge::{spl_key}");
        let dummy_head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::empty(),
            vec![],
        );
        theory.add_rule(Rule::strict(
            pos_label,
            smallvec![],
            dummy_head.clone(),
        ));
        let neg_label = format!("__bridge::neg::{spl_key}");
        let neg_head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            true,
            crate::mode::Mode::empty(),
            Temporal::empty(),
            vec![],
        );
        theory.add_rule(Rule::strict(neg_label, smallvec![], neg_head));

        let original_count = theory.rule_count();
        let mut ctx = PipelineContext::default();
        let result = TemporalBridge.apply(theory, &mut ctx).unwrap();

        // No new bridges should be generated since they already exist
        assert_eq!(result.rule_count(), original_count);
    }
}
