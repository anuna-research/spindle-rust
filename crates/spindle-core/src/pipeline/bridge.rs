//! Temporal bridge pipeline stage.
//!
//! Implements [`TemporalBridge`], which scans rule heads for temporal literals
//! and generates bridging rules that connect temporal atoms to their atemporal
//! base forms while preserving the originating rule strength:
//!
//! - **Positive bridge:** `q[s,e] → base(q)` — a temporal fact implies its base.
//! - **Negation bridge:** `~q[s,e] → ~base(q)` — coupled negation propagation.

use std::collections::{HashMap, HashSet};

use smallvec::smallvec;

use super::{Diagnostic, PipelineContext, PipelineStage, Severity};
use crate::body::{BodyArg, BodyLiteral, BodyLogicLiteral};
use crate::error::Result;
use crate::literal::Literal;
use crate::rule::{Rule, RuleType};
use crate::temporal::Temporal;
use crate::theory::Theory;

/// Pipeline stage that generates strict bridging rules from temporal head
/// literals to their atemporal base forms.
///
/// For each distinct temporal head literal `q[s,e]` found across all rules,
/// two bridge rules are added:
///
/// 1. `q[s,e] → q` (positive bridge)
/// 2. `~q[s,e] → ~q` (negation bridge)
///
/// Each synthesized bridge preserves the originating rule strength:
/// facts/strict rules produce strict bridges, defeasible rules produce
/// defeasible bridges, and defeaters produce defeater bridges.
///
/// Deduplication is by bridge structure (temporal body + matching atemporal head)
/// keyed by the positive temporal literal's SPL rendering.
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

#[derive(Debug, Clone, Copy, Default)]
struct BridgePresence {
    positive: bool,
    negative: bool,
}

#[derive(Debug, Clone)]
struct BridgeOrigin {
    template_label: String,
    rule_type: RuleType,
}

#[derive(Debug, Clone)]
struct BridgeSeed {
    positive: Literal,
    positive_origin: Option<BridgeOrigin>,
    negative_origin: Option<BridgeOrigin>,
}

/// Return canonical bridge key + polarity for structurally valid bridge rules.
///
/// A valid temporal bridge has:
/// - a single body literal and a single head literal
/// - exactly one logic body literal with non-empty temporal bounds
/// - exactly one atemporal head literal with same atom/mode/args and same polarity
fn existing_bridge_signature(rule: &Rule) -> Option<(String, bool)> {
    if rule.body.len() != 1 || rule.head.len() != 1 {
        return None;
    }

    let body = rule.body[0].as_logic()?;
    if body.temporal.is_empty() {
        return None;
    }

    let body_terms: Vec<_> = body
        .predicate_args()
        .iter()
        .map(|arg| match arg {
            BodyArg::Term(term) => Some(term.clone()),
            BodyArg::Arith(_) => None,
        })
        .collect::<Option<Vec<_>>>()?;

    let head = &rule.head[0];
    if !head.temporal.is_empty()
        || head.name_id() != body.name_id()
        || head.negation != body.negation
        || head.mode != body.mode
        || head.predicate_args() != body_terms.as_slice()
    {
        return None;
    }

    let body_lit = body.to_literal();
    let positive = if body_lit.negation {
        body_lit.complement()
    } else {
        body_lit
    };
    Some((positive.to_spl(), body.negation))
}

fn template_label_priority(label: &str, theory: &Theory) -> (bool, bool, bool) {
    let Some(meta) = theory.get_meta(label) else {
        return (false, false, false);
    };

    (
        meta.properties.contains_key("source"),
        meta.properties.contains_key("timestamp"),
        !meta.properties.is_empty(),
    )
}

fn should_replace_template_label(current: Option<&str>, candidate: &str, theory: &Theory) -> bool {
    let Some(current) = current else {
        return true;
    };

    let candidate_priority = template_label_priority(candidate, theory);
    let current_priority = template_label_priority(current, theory);
    candidate_priority > current_priority
        || (candidate_priority == current_priority && candidate < current)
}

fn reserve_bridge_label(reserved: &mut HashSet<String>, preferred: String) -> String {
    if reserved.insert(preferred.clone()) {
        return preferred;
    }

    let mut suffix = 1;
    loop {
        let candidate = format!("{preferred}::auto::{suffix}");
        if reserved.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn bridge_rule_type(source_rule_type: RuleType) -> RuleType {
    match source_rule_type {
        RuleType::Fact | RuleType::Strict => RuleType::Strict,
        RuleType::Defeasible => RuleType::Defeasible,
        RuleType::Defeater => RuleType::Defeater,
    }
}

impl PipelineStage for TemporalBridge {
    fn name(&self) -> &'static str {
        "temporal_bridge"
    }

    fn apply(&self, mut theory: Theory, ctx: &mut PipelineContext) -> Result<Theory> {
        // Track which bridge variants (positive/negative) already exist.
        let mut existing: HashMap<String, BridgePresence> = HashMap::new();
        for rule in theory.rules() {
            if let Some((key, is_negative)) = existing_bridge_signature(rule) {
                let entry = existing.entry(key).or_default();
                if is_negative {
                    entry.negative = true;
                } else {
                    entry.positive = true;
                }
            }
        }

        // Collect one seed per distinct temporal atom, with deterministic
        // template-label choice per polarity (prefer trust-relevant metadata
        // such as source/timestamp, then other metadata, then lexicographically
        // smaller labels).
        let mut seeds: HashMap<String, BridgeSeed> = HashMap::new();
        for rule in theory.rules() {
            let template_label = rule.template_label().to_string();

            for head_lit in &rule.head {
                if head_lit.temporal.is_empty() {
                    continue;
                }

                let positive = if head_lit.negation {
                    head_lit.complement()
                } else {
                    head_lit.clone()
                };
                let key = positive.to_spl();

                let seed = seeds.entry(key).or_insert_with(|| BridgeSeed {
                    positive,
                    positive_origin: None,
                    negative_origin: None,
                });
                let slot = if head_lit.negation {
                    &mut seed.negative_origin
                } else {
                    &mut seed.positive_origin
                };
                if should_replace_template_label(
                    slot.as_ref().map(|origin| origin.template_label.as_str()),
                    &template_label,
                    &theory,
                ) {
                    *slot = Some(BridgeOrigin {
                        template_label: template_label.clone(),
                        rule_type: bridge_rule_type(rule.rule_type),
                    });
                }
            }
        }

        let mut reserved_labels: HashSet<String> =
            theory.rules().map(|rule| rule.label.clone()).collect();
        let mut bridges: Vec<Rule> = Vec::new();
        for (key, seed) in seeds {
            let presence = existing.get(&key).copied().unwrap_or_default();
            let positive = &seed.positive;
            let pos_origin = seed
                .positive_origin
                .clone()
                .or_else(|| seed.negative_origin.clone())
                .expect("bridge seed must have at least one origin");
            let neg_origin = seed
                .negative_origin
                .clone()
                .or_else(|| seed.positive_origin.clone())
                .expect("bridge seed must have at least one origin");

            if !presence.positive {
                // --- Positive bridge: q[s,e] → q ---
                let pos_label =
                    reserve_bridge_label(&mut reserved_labels, format!("__bridge::{key}"));
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
                let mut pos_rule = Rule::new(
                    pos_label,
                    pos_origin.rule_type,
                    pos_body,
                    smallvec![pos_head],
                );
                pos_rule.template_label = Some(pos_origin.template_label.clone());
                bridges.push(pos_rule);
            }

            if !presence.negative {
                // --- Negation bridge: ~q[s,e] → ~q ---
                let neg_label =
                    reserve_bridge_label(&mut reserved_labels, format!("__bridge::neg::{key}"));
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
                let mut neg_rule = Rule::new(
                    neg_label,
                    neg_origin.rule_type,
                    neg_body,
                    smallvec![neg_head],
                );
                neg_rule.template_label = Some(neg_origin.template_label.clone());
                bridges.push(neg_rule);
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
    use crate::conclusion::ConclusionType;
    use crate::pipeline::PipelineContext;
    use crate::pipeline::compute_weighted_conclusions;
    use crate::reason::reason_prepared;
    use crate::rule::RuleType;
    use crate::temporal::TimePoint;
    use crate::trust::{Source, TrustPolicy};

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
    fn skips_generation_when_structural_bridges_already_exist() {
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

        // Pre-add valid bridge rules with the expected labels
        let pos_label = format!("__bridge::{spl_key}");
        let pos_head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::empty(),
            vec![],
        );
        let pos_body = smallvec![BodyLiteral::Logic(BodyLogicLiteral::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            head.temporal.clone(),
            vec![],
        ))];
        theory.add_rule(Rule::strict(pos_label, pos_body, pos_head));

        let neg_label = format!("__bridge::neg::{spl_key}");
        let neg_head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            true,
            crate::mode::Mode::empty(),
            Temporal::empty(),
            vec![],
        );
        let neg_body = smallvec![BodyLiteral::Logic(BodyLogicLiteral::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            true,
            crate::mode::Mode::empty(),
            head.temporal.clone(),
            vec![],
        ))];
        theory.add_rule(Rule::strict(neg_label, neg_body, neg_head));

        let original_count = theory.rule_count();
        let mut ctx = PipelineContext::default();
        let result = TemporalBridge.apply(theory, &mut ctx).unwrap();

        // No new bridges should be generated since they already exist
        assert_eq!(result.rule_count(), original_count);
    }

    #[test]
    fn user_labeled_bridge_prefix_does_not_block_required_generation() {
        let mut theory = Theory::new();

        let temporal_head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(10), TimePoint::from_millis(20)),
            vec![],
        );
        let spl_key = temporal_head.to_spl();
        let expected_base = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::empty(),
            vec![],
        );

        // Real temporal fact that requires bridge generation
        theory.add_rule(Rule::new(
            "f1",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![temporal_head.clone()],
        ));

        // User-authored rule spoofing a bridge label but not bridge structure
        theory.add_rule(Rule::strict(
            format!("__bridge::{spl_key}"),
            smallvec![],
            Literal::simple("sentinel"),
        ));

        let mut ctx = PipelineContext::default();
        let bridged = TemporalBridge.apply(theory, &mut ctx).unwrap();

        assert_eq!(
            bridged.rule_count(),
            4,
            "bridge generation should add two rules without replacing the user-authored collision"
        );
        let sentinel_rule = bridged
            .get_rule(&format!("__bridge::{spl_key}"))
            .expect("original user-authored rule should be preserved");
        assert_eq!(sentinel_rule.head.len(), 1);
        assert_eq!(sentinel_rule.head[0].name(), "sentinel");
        assert!(
            sentinel_rule.body.is_empty(),
            "original colliding rule body should remain unchanged"
        );

        // Ensure a real bridge exists: p[10,20] -> p
        let has_real_bridge = bridged.rules().any(|rule| {
            rule.rule_type == RuleType::Strict
                && rule.head.len() == 1
                && rule.head[0] == expected_base
                && rule.body.len() == 1
                && rule.body[0]
                    .as_logic()
                    .is_some_and(|bl| bl.to_literal() == temporal_head)
        });
        assert!(
            has_real_bridge,
            "user labels with __bridge:: prefix must not suppress actual bridge generation"
        );

        // Runtime behavior check: base literal must be definitely provable
        let conclusions = reason_prepared(&bridged).unwrap();
        let base_proved = conclusions.iter().any(|c| {
            c.literal == expected_base
                && c.conclusion_type == ConclusionType::DefinitelyProvable
                && c.is_positive()
        });
        assert!(
            base_proved,
            "base literal p should be derived through bridge"
        );
    }

    #[test]
    fn synthesized_bridges_preserve_polarity_specific_template_labels_and_trust() {
        let mut theory = Theory::new();

        let positive_head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(100), TimePoint::from_millis(200)),
            vec![],
        );
        let negative_head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            true,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(100), TimePoint::from_millis(200)),
            vec![],
        );
        theory.add_rule(Rule::new(
            "z_pos",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![positive_head],
        ));
        theory.add_rule(Rule::new(
            "a_neg",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![negative_head],
        ));
        theory.add_meta_string("z_pos", "source", "alice");
        theory.add_meta_string("a_neg", "source", "bob");
        *theory.trust_policy_mut() = TrustPolicy::new(0.5)
            .with_trust("alice", 0.9)
            .with_trust("bob", 0.1);

        let mut ctx = PipelineContext::default();
        let bridged = TemporalBridge.apply(theory, &mut ctx).unwrap();

        let pos_bridge = bridged
            .rules()
            .find(|rule| {
                rule.head.len() == 1
                    && !rule.head[0].negation
                    && rule.head[0].name() == "p"
                    && rule.head[0].temporal.is_empty()
                    && rule.body.len() == 1
                    && rule.body[0].as_logic().is_some_and(|bl| !bl.negation)
            })
            .expect("expected synthesized positive bridge");
        assert_eq!(pos_bridge.template_label.as_deref(), Some("z_pos"));

        let neg_bridge = bridged
            .rules()
            .find(|rule| {
                rule.head.len() == 1
                    && rule.head[0].negation
                    && rule.head[0].name() == "p"
                    && rule.head[0].temporal.is_empty()
                    && rule.body.len() == 1
                    && rule.body[0].as_logic().is_some_and(|bl| bl.negation)
            })
            .expect("expected synthesized negative bridge");
        assert_eq!(neg_bridge.template_label.as_deref(), Some("a_neg"));

        let conclusions = reason_prepared(&bridged).unwrap();
        let weighted =
            compute_weighted_conclusions(&conclusions, &bridged, bridged.trust_policy(), None);

        let positive_base = weighted
            .iter()
            .find(|wc| {
                wc.literal.name() == "p"
                    && !wc.literal.negation
                    && wc.literal.temporal.is_empty()
                    && wc.conclusion_type == ConclusionType::DefinitelyProvable
            })
            .expect("expected +D p");
        assert!(
            (positive_base.degree - 0.9).abs() < 1e-10,
            "positive bridge should inherit Alice's trust, got {}",
            positive_base.degree
        );
        assert!(positive_base.sources.contains(&Source::new("alice")));

        let negative_base = weighted
            .iter()
            .find(|wc| {
                wc.literal.name() == "p"
                    && wc.literal.negation
                    && wc.literal.temporal.is_empty()
                    && wc.conclusion_type == ConclusionType::DefinitelyProvable
            })
            .expect("expected +D ~p");
        assert!(
            (negative_base.degree - 0.1).abs() < 1e-10,
            "negative bridge should inherit Bob's trust, got {}",
            negative_base.degree
        );
        assert!(negative_base.sources.contains(&Source::new("bob")));
    }

    #[test]
    fn synthesized_bridge_rules_preserve_source_trust() {
        let mut theory = Theory::new();

        let temporal_head = Literal::from_ids(
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
            smallvec::smallvec![temporal_head],
        ));
        theory.add_meta_string("f1", "source", "alice");
        *theory.trust_policy_mut() = TrustPolicy::new(0.5).with_trust("alice", 0.9);

        let mut ctx = PipelineContext::default();
        let bridged = TemporalBridge.apply(theory, &mut ctx).unwrap();
        let conclusions = reason_prepared(&bridged).unwrap();
        let weighted =
            compute_weighted_conclusions(&conclusions, &bridged, bridged.trust_policy(), None);

        let base = weighted
            .iter()
            .find(|wc| {
                wc.literal.name() == "p"
                    && wc.literal.temporal.is_empty()
                    && wc.conclusion_type == ConclusionType::DefinitelyProvable
            })
            .expect("expected +D base conclusion for p");

        assert!(
            (base.degree - 0.9).abs() < 1e-10,
            "bridged base literal should inherit source trust 0.9, got {}",
            base.degree
        );
        assert!(
            base.sources.contains(&Source::new("alice")),
            "bridged base literal should retain source attribution"
        );
    }

    #[test]
    fn synthesized_bridges_prefer_source_bearing_template_labels() {
        let mut theory = Theory::new();

        let temporal_head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(100), TimePoint::from_millis(200)),
            vec![],
        );
        theory.add_rule(Rule::new(
            "a_meta",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![temporal_head.clone()],
        ));
        theory.add_rule(Rule::new(
            "z_source",
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![temporal_head],
        ));
        theory.add_meta_string("a_meta", "priority", "high");
        theory.add_meta_string("z_source", "source", "alice");
        *theory.trust_policy_mut() = TrustPolicy::new(0.5).with_trust("alice", 0.9);

        let mut ctx = PipelineContext::default();
        let bridged = TemporalBridge.apply(theory, &mut ctx).unwrap();

        let pos_bridge = bridged
            .rules()
            .find(|rule| {
                rule.head.len() == 1
                    && !rule.head[0].negation
                    && rule.head[0].name() == "p"
                    && rule.head[0].temporal.is_empty()
                    && rule.body.len() == 1
                    && rule.body[0].as_logic().is_some_and(|bl| !bl.negation)
            })
            .expect("expected synthesized positive bridge");
        assert_eq!(pos_bridge.template_label.as_deref(), Some("z_source"));

        let conclusions = reason_prepared(&bridged).unwrap();
        let weighted =
            compute_weighted_conclusions(&conclusions, &bridged, bridged.trust_policy(), None);
        let base = weighted
            .iter()
            .find(|wc| {
                wc.literal.name() == "p"
                    && !wc.literal.negation
                    && wc.literal.temporal.is_empty()
                    && wc.conclusion_type == ConclusionType::DefinitelyProvable
            })
            .expect("expected +D p");

        assert!(
            (base.degree - 0.9).abs() < 1e-10,
            "bridge should inherit Alice's trust rather than default trust, got {}",
            base.degree
        );
        assert!(base.sources.contains(&Source::new("alice")));
    }

    /// Helper: create a temporal fact `>> name [start, end].`
    fn temporal_fact(label: &str, name: &str, start: i64, end: i64) -> Rule {
        let head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern(name),
            false,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(start), TimePoint::from_millis(end)),
            vec![],
        );
        Rule::new(
            label,
            RuleType::Fact,
            Vec::<Literal>::new(),
            smallvec::smallvec![head],
        )
    }

    // --- TEST-007: Bridge rule generated for temporal fact ---
    // Trace: REQ-004
    #[test]
    fn bridge_rule_generated_for_temporal_fact() {
        // >> p [1, 10].
        let mut theory = Theory::new();
        theory.add_rule(temporal_fact("f1", "p", 1, 10));

        let mut ctx = PipelineContext::default();
        let bridged = TemporalBridge.apply(theory, &mut ctx).unwrap();

        // Verify positive bridge: body is temporal p[1,10], head is atemporal p
        let has_bridge = bridged.rules().any(|r| {
            r.label.starts_with("__bridge::")
                && !r.label.contains("neg")
                && r.head.iter().any(|h| h.temporal.is_empty() && !h.negation)
                && r.body
                    .iter()
                    .any(|b| b.as_logic().map_or(false, |l| !l.temporal.is_empty()))
        });
        assert!(has_bridge, "Expected bridging rule p[1,10] → p");
    }

    // --- TEST-008: Bridge generated even when base head exists (unconditional) ---
    // Trace: REQ-004
    #[test]
    fn bridge_generated_even_when_base_head_exists() {
        // >> p [1, 10].  >> p.
        let mut theory = Theory::new();
        theory.add_rule(temporal_fact("tf1", "p", 1, 10));
        theory.add_fact("p");

        let mut ctx = PipelineContext::default();
        let bridged = TemporalBridge.apply(theory, &mut ctx).unwrap();

        let bridge_count = bridged
            .rules()
            .filter(|r| r.label.starts_with("__bridge::"))
            .count();
        assert!(
            bridge_count > 0,
            "Bridge generated unconditionally for temporal heads even when base fact exists"
        );
    }

    #[test]
    fn defeasible_temporal_heads_generate_defeasible_bridges() {
        let mut theory = Theory::new();
        let temporal_head = Literal::from_ids(
            crate::literal::InternedLiteralName::intern("p"),
            true,
            crate::mode::Mode::empty(),
            Temporal::new(TimePoint::from_millis(1), TimePoint::from_millis(10)),
            vec![],
        );
        theory.add_rule(Rule::new(
            "r_neg",
            RuleType::Defeasible,
            vec![Literal::simple("b")],
            smallvec::smallvec![temporal_head],
        ));

        let mut ctx = PipelineContext::default();
        let bridged = TemporalBridge.apply(theory, &mut ctx).unwrap();

        let neg_bridge = bridged
            .rules()
            .find(|rule| {
                rule.label.starts_with("__bridge::neg::")
                    && rule.head.len() == 1
                    && rule.head[0].name() == "p"
                    && rule.head[0].negation
                    && rule.head[0].temporal.is_empty()
            })
            .expect("expected synthesized negative bridge");

        assert_eq!(
            neg_bridge.rule_type,
            RuleType::Defeasible,
            "bridges from defeasible temporal heads must remain defeasible"
        );
        assert_eq!(neg_bridge.template_label.as_deref(), Some("r_neg"));
    }

    // --- TEST-009: Negation bridge generated ---
    // Trace: REQ-004
    #[test]
    fn negation_bridge_generated() {
        // >> p [1, 10].
        let mut theory = Theory::new();
        theory.add_rule(temporal_fact("f1", "p", 1, 10));

        let mut ctx = PipelineContext::default();
        let bridged = TemporalBridge.apply(theory, &mut ctx).unwrap();

        let neg_bridge = bridged
            .rules()
            .find(|r| r.label.starts_with("__bridge::neg::"))
            .expect("Expected negation bridging rule ~p[1,10] → ~p");

        assert_eq!(neg_bridge.rule_type, RuleType::Strict);
        assert!(
            neg_bridge.head[0].negation,
            "Negation bridge head must be negated"
        );
        assert!(
            neg_bridge.head[0].temporal.is_empty(),
            "Negation bridge head must be atemporal"
        );
        // Body should be negated and temporal
        let body_lit = neg_bridge.body[0]
            .as_logic()
            .expect("body should be a logic literal");
        assert!(body_lit.negation, "Negation bridge body must be negated");
        assert!(
            !body_lit.temporal.is_empty(),
            "Negation bridge body must be temporal"
        );
    }

    #[test]
    fn inferior_defeasible_temporal_attacker_does_not_become_strict_blocker() {
        let mut theory = Theory::new();

        theory.add_fact("a");
        theory.add_fact("b");
        let r_p = theory.add_defeasible_rule(&["a"], "p");
        theory.add_rule(Rule::new(
            "r_neg",
            RuleType::Defeasible,
            vec![Literal::simple("b")],
            vec![Literal::new(
                "p",
                true,
                crate::mode::Mode::empty(),
                Temporal::from_bounds(1, 10),
                vec![],
            )],
        ));
        theory.add_superiority(&r_p, "r_neg");
        theory.add_defeasible_rule(&["p"], "q");

        let conclusions = crate::reason::reason(&theory).unwrap();

        let has_p = conclusions.iter().any(|conclusion| {
            conclusion.conclusion_type == ConclusionType::DefeasiblyProvable
                && conclusion.literal.name() == "p"
                && !conclusion.literal.negation
                && conclusion.literal.temporal.is_empty()
        });
        let has_q = conclusions.iter().any(|conclusion| {
            conclusion.conclusion_type == ConclusionType::DefeasiblyProvable
                && conclusion.literal.name() == "q"
                && !conclusion.literal.negation
        });

        assert!(has_p, "superior atemporal rule should still derive p");
        assert!(
            has_q,
            "an inferior temporal attacker must not block q through a strict bridge"
        );
    }

    // --- TEST-010: Bridge stage is no-op for non-temporal theory ---
    // Trace: REQ-004, REQ-008
    #[test]
    fn bridge_no_op_for_non_temporal_spl_theory() {
        // >> bird.  bird => can_fly.
        let mut theory = Theory::new();
        theory.add_fact("bird");
        theory.add_defeasible_rule(&["bird"], "can_fly");

        let original_count = theory.rule_count();
        let mut ctx = PipelineContext::default();
        let bridged = TemporalBridge.apply(theory, &mut ctx).unwrap();

        assert_eq!(
            original_count,
            bridged.rule_count(),
            "No bridge rules should be added to a non-temporal theory"
        );
        assert!(
            ctx.diagnostics.is_empty(),
            "No diagnostics expected for non-temporal theory"
        );
    }

    // --- TEST-018: Bridge labels are deterministic ---
    // Trace: REQ-010
    #[test]
    fn bridge_labels_are_deterministic() {
        let bridge = TemporalBridge;

        // First run: >> p [1, 10].
        let mut theory1 = Theory::new();
        theory1.add_rule(temporal_fact("f1", "p", 1, 10));
        let mut ctx1 = PipelineContext::default();
        let bridged1 = bridge.apply(theory1, &mut ctx1).unwrap();
        let mut labels1: Vec<_> = bridged1
            .rules()
            .filter(|r| r.label.starts_with("__bridge::"))
            .map(|r| r.label.clone())
            .collect();
        labels1.sort();

        // Second run: same input
        let mut theory2 = Theory::new();
        theory2.add_rule(temporal_fact("f1", "p", 1, 10));
        let mut ctx2 = PipelineContext::default();
        let bridged2 = bridge.apply(theory2, &mut ctx2).unwrap();
        let mut labels2: Vec<_> = bridged2
            .rules()
            .filter(|r| r.label.starts_with("__bridge::"))
            .map(|r| r.label.clone())
            .collect();
        labels2.sort();

        assert_eq!(labels1, labels2, "Bridge labels must be deterministic");
        assert_eq!(labels1.len(), 2, "Expected exactly 2 bridge labels");
    }
}
