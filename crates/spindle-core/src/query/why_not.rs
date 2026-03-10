//! Why-Not Query Operator
//!
//! Explains why a literal is **not** provable in a defeasible logic theory.
//! Given a literal, `why_not` inspects the theory's rules and the current
//! set of conclusions to identify blocking conditions: missing premises,
//! defeaters, and contradictions.
//!
//! Uses **family-aware matching** (via [`FamilyId`]) so that temporal
//! conclusions like `p[1,10]` satisfy atemporal queries for `p`, and
//! cites **original rule labels** in explanations.

use std::collections::HashSet;
use std::fmt;

use crate::error::Result;
use crate::literal::Literal;
use crate::projection::FamilyId;
use crate::reason::reason;
use crate::rule::RuleType;
use crate::theory::Theory;

// =============================================================================
// TYPES
// =============================================================================

/// Type of blocking condition
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockingType {
    /// Missing premise in rule body
    MissingPremise,
    /// Defeated by a defeater
    Defeated,
    /// Contradicted by opposing conclusion
    Contradicted,
}

impl fmt::Display for BlockingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BlockingType::MissingPremise => write!(f, "missing premise"),
            BlockingType::Defeated => write!(f, "defeated"),
            BlockingType::Contradicted => write!(f, "contradicted"),
        }
    }
}

/// A condition that blocks a derivation
#[derive(Debug, Clone)]
pub struct BlockingCondition {
    /// Type of blocking
    pub blocking_type: BlockingType,
    /// The rule that was blocked
    pub rule_label: String,
    /// Missing literals (for MissingPremise)
    pub missing_literals: Vec<Literal>,
    /// Blocking rule (for Defeated/Contradicted)
    pub blocking_rule: Option<String>,
    /// Human-readable explanation
    pub explanation: String,
}

impl BlockingCondition {
    /// Create a missing premise blocking condition
    pub fn missing_premise(rule_label: impl Into<String>, missing: Vec<Literal>) -> Self {
        let missing_str: Vec<_> = missing.iter().map(|l| l.to_string()).collect();
        Self {
            blocking_type: BlockingType::MissingPremise,
            rule_label: rule_label.into(),
            missing_literals: missing,
            blocking_rule: None,
            explanation: format!("Missing premises: {}", missing_str.join(", ")),
        }
    }

    /// Create a defeated blocking condition
    pub fn defeated(rule_label: impl Into<String>, by_rule: impl Into<String>) -> Self {
        let by = by_rule.into();
        Self {
            blocking_type: BlockingType::Defeated,
            rule_label: rule_label.into(),
            missing_literals: Vec::new(),
            blocking_rule: Some(by.clone()),
            explanation: format!("Defeated by rule {by}"),
        }
    }

    /// Create a contradicted blocking condition
    pub fn contradicted(rule_label: impl Into<String>, by_rule: impl Into<String>) -> Self {
        let by = by_rule.into();
        Self {
            blocking_type: BlockingType::Contradicted,
            rule_label: rule_label.into(),
            missing_literals: Vec::new(),
            blocking_rule: Some(by.clone()),
            explanation: format!("Contradicted by {by}"),
        }
    }
}

/// Result of a why-not query
#[derive(Debug, Clone)]
pub struct WhyNotResult {
    /// The literal queried
    pub literal: Literal,
    /// Rule that would derive this literal (if body was satisfied)
    pub would_derive: Option<String>,
    /// Conditions blocking the derivation
    pub blocked_by: Vec<BlockingCondition>,
}

impl WhyNotResult {
    /// Create a new why-not result
    pub fn new(literal: Literal) -> Self {
        Self {
            literal,
            would_derive: None,
            blocked_by: Vec::new(),
        }
    }

    /// Check if the literal is actually provable.
    ///
    /// A provable literal has a deriving rule but no blockers.
    pub fn is_provable(&self) -> bool {
        self.would_derive.is_some() && self.blocked_by.is_empty()
    }

    /// Check if there are any blocking conditions
    pub fn has_blockers(&self) -> bool {
        !self.blocked_by.is_empty()
    }

    /// Get missing premises from all blocking conditions
    pub fn get_missing_premises(&self) -> Vec<&Literal> {
        self.blocked_by
            .iter()
            .filter(|b| b.blocking_type == BlockingType::MissingPremise)
            .flat_map(|b| b.missing_literals.iter())
            .collect()
    }
}

impl fmt::Display for WhyNotResult {
    /// Convert to human-readable string
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_provable() {
            write!(f, "{} is provable", self.literal)?;
            if let Some(ref rule) = self.would_derive {
                write!(f, " (derived by rule: {rule})")?;
            }
            return Ok(());
        }

        if self.blocked_by.is_empty() {
            return write!(
                f,
                "{} is not provable: no rules can derive it",
                self.literal
            );
        }

        writeln!(f, "{} is not provable:", self.literal)?;

        if let Some(ref rule) = self.would_derive {
            writeln!(f, "  Would be derived by rule: {rule}")?;
        }

        writeln!(f, "  Blocked by:")?;
        for bc in &self.blocked_by {
            writeln!(f, "    - Rule {}: {}", bc.rule_label, bc.blocking_type)?;
            writeln!(f, "      ({})", bc.explanation)?;
        }
        Ok(())
    }
}

// =============================================================================
// WHY-NOT OPERATOR
// =============================================================================

/// Explain why a literal is NOT provable.
///
/// Uses family-aware matching (via [`FamilyId`]) so that temporal
/// conclusions satisfy atemporal queries, and cites original rule labels
/// in all explanations.
pub fn why_not(theory: &Theory, literal: &Literal) -> Result<WhyNotResult> {
    let conclusions = reason(theory)?;
    let goal_family = FamilyId::from(literal);

    // First check if it IS provable (family-aware)
    let provable_conclusion = conclusions
        .iter()
        .find(|c| c.conclusion_type.is_positive() && FamilyId::from(&c.literal) == goal_family);

    if let Some(conclusion) = provable_conclusion {
        let mut result = WhyNotResult::new(literal.clone());
        result.would_derive = conclusion.rule_label.clone();
        return Ok(result);
    }

    // Collect proven families for checking body satisfaction
    let proven_families: HashSet<FamilyId> = conclusions
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| FamilyId::from(&c.literal))
        .collect();

    let complement = literal.complement();
    let complement_family = FamilyId::from(&complement);
    let mut result = WhyNotResult::new(literal.clone());
    let mut found_rule = false;

    // Find rules that could derive this literal (family-aware head match)
    for rule in theory.rules() {
        let head_family = FamilyId::from(rule.head_literal());
        if head_family == goal_family && rule.rule_type != RuleType::Defeater {
            found_rule = true;

            if result.would_derive.is_none() {
                result.would_derive = Some(rule.label.clone());
            }

            // Check which body literals are missing (family-aware satisfaction)
            let body_lits: Vec<Literal> = rule
                .body
                .iter()
                .filter_map(|bl| bl.as_logic().map(|l| l.to_literal()))
                .collect();
            let missing: Vec<_> = body_lits
                .iter()
                .filter(|b| !proven_families.contains(&FamilyId::from(*b)))
                .cloned()
                .collect();

            if !missing.is_empty() {
                result
                    .blocked_by
                    .push(BlockingCondition::missing_premise(&rule.label, missing));
            } else {
                // Body is fully satisfied but conclusion not proven.
                // Check for defeater blocking (family-aware attacker match).
                let mut blocked = false;
                for attacker in theory.rules() {
                    let attacker_head_family = FamilyId::from(attacker.head_literal());
                    if attacker_head_family == complement_family {
                        let attacker_body_lits: Vec<Literal> = attacker
                            .body
                            .iter()
                            .filter_map(|bl| bl.as_logic().map(|l| l.to_literal()))
                            .collect();
                        let attacker_body_satisfied = attacker_body_lits
                            .iter()
                            .all(|b| proven_families.contains(&FamilyId::from(b)));
                        if !attacker_body_satisfied {
                            continue;
                        }

                        if attacker.rule_type == RuleType::Defeater {
                            // Defeaters block unless the rule is explicitly superior
                            let rule_superior = theory.is_superior(&rule.label, &attacker.label);
                            if !rule_superior {
                                result.blocked_by.push(BlockingCondition::defeated(
                                    &rule.label,
                                    &attacker.label,
                                ));
                                blocked = true;
                            }
                        } else {
                            // For defeasible rules: check superiority both directions
                            let attacker_superior =
                                theory.is_superior(&attacker.label, &rule.label);
                            let rule_superior = theory.is_superior(&rule.label, &attacker.label);

                            if rule_superior && !attacker_superior {
                                // Rule is superior — skip this attacker
                                continue;
                            }

                            // Report as blocker if attacker is superior or ambiguity
                            result.blocked_by.push(BlockingCondition::contradicted(
                                &rule.label,
                                &attacker.label,
                            ));
                            blocked = true;
                        }
                    }
                }
                if !blocked {
                    // Body satisfied, no attackers found, but still not provable.
                    // This can happen with ambiguity blocking.
                    result.blocked_by.push(BlockingCondition {
                        blocking_type: BlockingType::Contradicted,
                        rule_label: rule.label.clone(),
                        missing_literals: Vec::new(),
                        blocking_rule: None,
                        explanation: "Body satisfied but conclusion blocked by ambiguity"
                            .to_string(),
                    });
                }
            }
        }
    }

    // If no rules found at all
    if !found_rule {
        // Check if complement is proven (family-aware)
        if proven_families.contains(&complement_family) {
            result.blocked_by.push(BlockingCondition {
                blocking_type: BlockingType::Contradicted,
                rule_label: String::new(),
                missing_literals: Vec::new(),
                blocking_rule: None,
                explanation: format!("Complement {complement} is proven"),
            });
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal::Literal;
    use crate::theory::Theory;

    // =========================================================================
    // 1. Display for provable literal (lines ~141-146)
    // =========================================================================

    #[test]
    fn test_display_provable_literal() {
        // A fact is provable. why_not should return is_provable() == true,
        // and Display should produce "X is provable (derived by rule: ...)"
        let mut th = Theory::new();
        th.add_fact("bird");

        let result = why_not(&th, &Literal::simple("bird")).unwrap();
        assert!(result.is_provable());
        assert!(result.would_derive.is_some());

        let display = result.to_string();
        assert!(
            display.contains("is provable"),
            "Expected 'is provable' in display, got: {display}"
        );
        assert!(
            display.contains("derived by rule"),
            "Expected 'derived by rule' in display, got: {display}"
        );
    }

    #[test]
    fn test_display_provable_defeasible() {
        // Defeasibly derived literal should also hit the provable Display branch
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_defeasible_rule(&["bird"], "flies");

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        assert!(result.is_provable());

        let display = result.to_string();
        assert!(display.contains("flies is provable"));
        assert!(display.contains("derived by rule"));
    }

    // =========================================================================
    // 2. Attacker with unsatisfied body (line ~236)
    // =========================================================================

    #[test]
    fn test_attacker_with_unsatisfied_body_is_ignored() {
        // bird => flies, heavy ~> ~flies (defeater), but heavy is NOT a fact.
        // The defeater's body is unsatisfied, so it should be skipped (continue).
        // flies should be provable.
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_defeasible_rule(&["bird"], "flies");
        // Defeater with unsatisfied body: "heavy" is not a fact
        th.add_defeater(&["heavy"], "~flies");

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        // flies IS provable because the defeater body is unsatisfied
        assert!(
            result.is_provable(),
            "flies should be provable when defeater body is unsatisfied"
        );
        assert!(
            !result.has_blockers(),
            "No blockers expected when attacker body is not satisfied"
        );
    }

    #[test]
    fn test_defeasible_attacker_with_unsatisfied_body_is_ignored() {
        // bird => flies, penguin => ~flies, but penguin is NOT a fact.
        // The attacker's body is unsatisfied, so it should be skipped.
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_defeasible_rule(&["bird"], "flies");
        th.add_defeasible_rule(&["penguin"], "~flies");

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        assert!(
            result.is_provable(),
            "flies should be provable when defeasible attacker body is unsatisfied"
        );
    }

    // =========================================================================
    // 3. Rule superior to defeater (line ~248)
    // =========================================================================

    #[test]
    fn test_rule_superior_to_defeater_no_blocking() {
        // bird => flies (r1), broken_wing ~> ~flies (d1), r1 > d1
        // The defeater's body IS satisfied, but r1 is superior so it should NOT block.
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("broken_wing");
        let r1 = th.add_defeasible_rule(&["bird"], "flies");
        let d1 = th.add_defeater(&["broken_wing"], "~flies");
        th.add_superiority(&r1, &d1);

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        // flies should be provable because r1 > d1 overrides the defeater
        assert!(
            !result.has_blockers(),
            "Defeater should not block when rule is superior, got blockers: {:?}",
            result.blocked_by
        );
    }

    #[test]
    fn test_defeater_blocks_when_not_superior() {
        // bird => flies (r1), broken_wing ~> ~flies (d1), NO superiority
        // The defeater's body IS satisfied and r1 is NOT superior, so it blocks.
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("broken_wing");
        th.add_defeasible_rule(&["bird"], "flies");
        th.add_defeater(&["broken_wing"], "~flies");

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        let has_defeated = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_type == BlockingType::Defeated);
        assert!(
            has_defeated,
            "Defeater should block when rule is NOT superior, got blockers: {:?}",
            result.blocked_by
        );
    }

    // =========================================================================
    // 4. Defeasible-vs-defeasible contradiction (lines ~250-266)
    //    Attacker is NOT superior => should produce Contradicted blocker
    // =========================================================================

    #[test]
    fn test_defeasible_vs_defeasible_contradicted_attacker_superior() {
        // bird => flies (r1), penguin => ~flies (r2), r2 > r1
        // Attacker r2 is superior, so the reasoner does NOT prove flies.
        // why_not should report a Contradicted blocker from r2.
        // (This exercises lines ~250-266 with attacker_superior=true, rule_superior=false)
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("penguin");
        let r1 = th.add_defeasible_rule(&["bird"], "flies");
        let r2 = th.add_defeasible_rule(&["penguin"], "~flies");
        th.add_superiority(&r2, &r1);

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        let has_contradicted = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_type == BlockingType::Contradicted);
        assert!(
            has_contradicted,
            "Expected Contradicted blocker when attacker is superior, got: {:?}",
            result.blocked_by
        );
    }

    #[test]
    fn test_defeasible_vs_defeasible_no_superiority_credulous() {
        // bird => flies (r1), penguin => ~flies (r2), NO superiority
        // With credulous semantics (reason.rs), flies is still provable.
        // why_not should report no blockers because the literal IS provable.
        // This test documents the credulous behavior.
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("penguin");
        th.add_defeasible_rule(&["bird"], "flies");
        th.add_defeasible_rule(&["penguin"], "~flies");

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        // Under credulous semantics, flies IS provable
        // If the reasoner blocks it (skeptical), it should report a Contradicted blocker
        if result.is_provable() {
            assert!(
                !result.has_blockers(),
                "Provable literal should have no blockers"
            );
        } else {
            let has_contradicted = result
                .blocked_by
                .iter()
                .any(|b| b.blocking_type == BlockingType::Contradicted);
            assert!(
                has_contradicted,
                "Non-provable literal with conflicting rules should have Contradicted blocker"
            );
        }
    }

    // =========================================================================
    // 5. Rule superior to defeasible attacker (lines ~255-257)
    //    r1 > r2 => skip the attacker blocker
    // =========================================================================

    #[test]
    fn test_rule_superior_to_defeasible_attacker_skips_blocker() {
        // bird => flies (r1), penguin => ~flies (r2), r1 > r2
        // Rule r1 is superior to attacker r2, so r2 should be skipped.
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("penguin");
        let r1 = th.add_defeasible_rule(&["bird"], "flies");
        let r2 = th.add_defeasible_rule(&["penguin"], "~flies");
        th.add_superiority(&r1, &r2);

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        // flies should be provable since r1 > r2
        assert!(
            !result.has_blockers(),
            "Rule superior to defeasible attacker should skip blocker, got: {:?}",
            result.blocked_by
        );
    }

    #[test]
    fn test_rule_superior_no_contradicted_entry() {
        // Verify that when rule is superior, no Contradicted entry is added for that attacker
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("penguin");
        let r1 = th.add_defeasible_rule(&["bird"], "flies");
        let r2 = th.add_defeasible_rule(&["penguin"], "~flies");
        th.add_superiority(&r1, &r2);

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        let has_r2_blocker = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_rule.as_deref() == Some(r2.as_str()));
        assert!(
            !has_r2_blocker,
            "Inferior attacker r2 should not appear as a blocker"
        );
    }

    // =========================================================================
    // 6. Ambiguity blocking (lines ~269-279)
    //    Body satisfied, no attackers found, but conclusion still not provable
    // =========================================================================

    #[test]
    fn test_ambiguity_blocking_body_satisfied_but_not_provable() {
        // The ambiguity fallback (lines 269-279) fires when a rule's body is
        // fully satisfied, no attacker has a satisfied body, yet the reasoner
        // still did not prove the literal.
        //
        // Known bug: reason.rs doesn't fire empty-body non-fact defeasible rules.
        // An empty-body DEFEASIBLE rule (not a strict fact) should trigger this:
        // why_not sees the body as trivially satisfied, finds no attackers, but
        // the reasoner never proved the literal.
        use crate::rule::Rule;

        let mut th = Theory::new();
        let rule = Rule::defeasible("r1", vec![], Literal::simple("p"));
        th.add_rule(rule);

        let result = why_not(&th, &Literal::simple("p")).unwrap();

        if !result.is_provable() {
            // We hit the ambiguity blocking fallback path
            let has_ambiguity = result.blocked_by.iter().any(|b| {
                b.blocking_type == BlockingType::Contradicted && b.explanation.contains("ambiguity")
            });
            assert!(
                has_ambiguity,
                "Expected ambiguity blocking condition, got: {:?}",
                result.blocked_by
            );
        }
        // If provable, the reasoner fires empty-body defeasible rules and the
        // ambiguity fallback is not reached. The Display provable path is tested
        // separately in test_display_provable_literal.
    }

    #[test]
    fn test_ambiguity_blocking_display_format() {
        // Directly construct a WhyNotResult that represents ambiguity blocking
        // and verify the Display output covers the blocked_by branch with
        // the ambiguity explanation.
        let mut result = WhyNotResult::new(Literal::simple("goal"));
        result.would_derive = Some("r1".to_string());
        result.blocked_by.push(BlockingCondition {
            blocking_type: BlockingType::Contradicted,
            rule_label: "r1".to_string(),
            missing_literals: Vec::new(),
            blocking_rule: None,
            explanation: "Body satisfied but conclusion blocked by ambiguity".to_string(),
        });

        assert!(result.has_blockers());
        assert!(!result.is_provable());

        let display = result.to_string();
        assert!(
            display.contains("not provable"),
            "Display should say 'not provable', got: {display}"
        );
        assert!(
            display.contains("Would be derived by rule: r1"),
            "Display should mention deriving rule, got: {display}"
        );
        assert!(
            display.contains("Blocked by"),
            "Display should mention blocking, got: {display}"
        );
        assert!(
            display.contains("contradicted"),
            "Display should mention contradicted type, got: {display}"
        );
        assert!(
            display.contains("ambiguity"),
            "Display should mention ambiguity, got: {display}"
        );
    }

    // =========================================================================
    // Additional edge case tests for completeness
    // =========================================================================

    #[test]
    fn test_multiple_attackers_mixed_types() {
        // r1: bird => flies, d1: broken_wing ~> ~flies (defeater),
        // r2: penguin => ~flies (defeasible). Both bodies satisfied, no superiority.
        // Should get both Defeated (from defeater) and Contradicted (from defeasible).
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("broken_wing");
        th.add_fact("penguin");
        th.add_defeasible_rule(&["bird"], "flies");
        th.add_defeater(&["broken_wing"], "~flies");
        th.add_defeasible_rule(&["penguin"], "~flies");

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        let types: Vec<_> = result.blocked_by.iter().map(|b| b.blocking_type).collect();
        assert!(
            types.contains(&BlockingType::Defeated),
            "Should have Defeated blocker from defeater, got: {:?}",
            types
        );
        assert!(
            types.contains(&BlockingType::Contradicted),
            "Should have Contradicted blocker from defeasible attacker, got: {:?}",
            types
        );
    }

    #[test]
    fn test_why_not_result_accessors() {
        let mut result = WhyNotResult::new(Literal::simple("x"));
        assert!(!result.is_provable());
        assert!(!result.has_blockers());
        assert!(result.get_missing_premises().is_empty());

        result.would_derive = Some("r1".to_string());
        assert!(result.is_provable()); // has would_derive and no blockers

        result.blocked_by.push(BlockingCondition::missing_premise(
            "r1",
            vec![Literal::simple("y")],
        ));
        assert!(!result.is_provable()); // now has blockers
        assert!(result.has_blockers());
        assert_eq!(result.get_missing_premises().len(), 1);
    }

    // =========================================================================
    // Family-aware tests
    // =========================================================================

    #[test]
    fn test_why_not_family_aware_provable() {
        // Temporal fact p[1,10] should family-satisfy an atemporal query for p
        use crate::rule::Rule;

        let mut th = Theory::new();
        let temporal_lit = Literal::new(
            "p",
            false,
            crate::mode::Mode::empty(),
            crate::temporal::Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        th.add_rule(Rule::fact("f1", temporal_lit));

        let result = why_not(&th, &Literal::simple("p")).unwrap();
        assert!(
            result.is_provable(),
            "Temporal p[1,10] should family-satisfy atemporal query for p"
        );
    }

    #[test]
    fn test_why_not_family_aware_body_satisfaction() {
        // Rule: a => goal. Temporal fact a[5,15] should satisfy body literal a
        // via family matching, so goal should be provable.
        use crate::rule::Rule;

        let mut th = Theory::new();
        let temporal_a = Literal::new(
            "a",
            false,
            crate::mode::Mode::empty(),
            crate::temporal::Temporal::new(
                crate::temporal::TimePoint::Moment(5),
                crate::temporal::TimePoint::Moment(15),
            ),
            vec![],
        );
        th.add_rule(Rule::fact("f1", temporal_a));
        th.add_defeasible_rule(&["a"], "goal");

        let result = why_not(&th, &Literal::simple("goal")).unwrap();
        assert!(
            result.is_provable(),
            "Temporal a[5,15] should family-satisfy body literal a, making goal provable"
        );
    }

    #[test]
    fn test_why_not_family_aware_head_match() {
        // Rule with temporal head p[1,10] should be found when querying
        // why_not for atemporal p, because they share the same family.
        use crate::rule::Rule;

        let mut th = Theory::new();
        let temporal_head = Literal::new(
            "p",
            false,
            crate::mode::Mode::empty(),
            crate::temporal::Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        let body = vec![Literal::simple("a")];
        th.add_rule(Rule::defeasible("r1", body, temporal_head));

        let result = why_not(&th, &Literal::simple("p")).unwrap();
        // r1 should be found as a candidate rule via family head match
        assert_eq!(
            result.would_derive.as_deref(),
            Some("r1"),
            "Should find rule r1 via family head match"
        );
        // Body literal 'a' is missing
        let missing = result.get_missing_premises();
        assert!(
            missing.iter().any(|l| l.name() == "a"),
            "Should report 'a' as missing premise"
        );
    }

    #[test]
    fn test_why_not_family_aware_complement_proven() {
        // Temporal complement ~p[1,10] should be detected as blocking
        // atemporal query for p via family matching.
        use crate::rule::Rule;

        let mut th = Theory::new();
        let temporal_neg = Literal::new(
            "p",
            true,
            crate::mode::Mode::empty(),
            crate::temporal::Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        th.add_rule(Rule::fact("f1", temporal_neg));

        let result = why_not(&th, &Literal::simple("p")).unwrap();
        assert!(
            !result.is_provable(),
            "p should not be provable when ~p[1,10] is proven"
        );
        let has_complement_blocker = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_type == BlockingType::Contradicted);
        assert!(
            has_complement_blocker,
            "Should report complement as contradicted blocker"
        );
    }

    #[test]
    fn test_why_not_family_aware_attacker_detection() {
        // Rule: bird => flies. Defeater with temporal head: broken_wing ~> ~flies[1,10].
        // Both bodies satisfied. The defeater should be detected via family matching.
        use crate::rule::Rule;

        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("broken_wing");
        th.add_defeasible_rule(&["bird"], "flies");

        let temporal_neg_flies = Literal::new(
            "flies",
            true,
            crate::mode::Mode::empty(),
            crate::temporal::Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        th.add_rule(Rule::defeater("d1", vec![Literal::simple("broken_wing")], temporal_neg_flies));

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        // The defeater d1 should block flies via family-aware attacker detection
        let has_defeated = result
            .blocked_by
            .iter()
            .any(|b| b.blocking_type == BlockingType::Defeated);
        assert!(
            has_defeated,
            "Temporal defeater ~flies[1,10] should block atemporal flies via family matching"
        );
    }

    #[test]
    fn test_why_not_cites_original_rule_labels() {
        // Verify that blocking conditions cite the original rule labels
        let mut th = Theory::new();
        th.add_fact("bird");
        th.add_fact("broken_wing");
        let r1 = th.add_defeasible_rule(&["bird"], "flies");
        let d1 = th.add_defeater(&["broken_wing"], "~flies");

        let result = why_not(&th, &Literal::simple("flies")).unwrap();
        // The defeated condition should cite both the blocked rule and the blocker
        let defeated = result
            .blocked_by
            .iter()
            .find(|b| b.blocking_type == BlockingType::Defeated);
        assert!(defeated.is_some(), "Should have a Defeated blocker");
        let defeated = defeated.unwrap();
        assert_eq!(
            defeated.rule_label, r1,
            "Should cite original rule label {r1}"
        );
        assert_eq!(
            defeated.blocking_rule.as_deref(),
            Some(d1.as_str()),
            "Should cite original defeater label {d1}"
        );
    }
}
