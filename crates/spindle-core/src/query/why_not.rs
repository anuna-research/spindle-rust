//! Why-Not Query Operator
//!
//! Explains why a literal is **not** provable in a defeasible logic theory.
//! Given a literal, `why_not` inspects the theory's rules and the current
//! set of conclusions to identify blocking conditions: missing premises,
//! defeaters, and contradictions.

use std::collections::HashSet;
use std::fmt;

use crate::error::Result;
use crate::literal::Literal;
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

/// Explain why a literal is NOT provable
pub fn why_not(theory: &Theory, literal: &Literal) -> Result<WhyNotResult> {
    let conclusions = reason(theory)?;

    // First check if it IS provable (then why-not doesn't apply)
    let is_provable = conclusions
        .iter()
        .any(|c| c.literal == *literal && c.conclusion_type.is_positive());

    if is_provable {
        // Return a result with would_derive taken from the conclusion's rule_label
        let mut result = WhyNotResult::new(literal.clone());
        result.would_derive = conclusions
            .iter()
            .find(|c| c.literal == *literal && c.conclusion_type.is_positive())
            .and_then(|c| c.rule_label.clone());
        return Ok(result);
    }

    // Collect proven literals for checking body satisfaction
    let proven: HashSet<_> = conclusions
        .iter()
        .filter(|c| c.conclusion_type.is_positive())
        .map(|c| c.literal.clone())
        .collect();

    let complement = literal.complement();
    let mut result = WhyNotResult::new(literal.clone());
    let mut found_rule = false;

    // Find rules that could derive this literal and why they don't fire
    for rule in theory.rules() {
        if rule.head_literal() == literal && rule.rule_type != RuleType::Defeater {
            found_rule = true;

            if result.would_derive.is_none() {
                result.would_derive = Some(rule.label.clone());
            }

            // Check which body literals are missing
            let missing: Vec<_> = rule
                .body
                .iter()
                .filter(|b| !proven.contains(*b))
                .cloned()
                .collect();

            if !missing.is_empty() {
                result
                    .blocked_by
                    .push(BlockingCondition::missing_premise(&rule.label, missing));
            } else {
                // Body is fully satisfied but conclusion not proven.
                // Check for defeater blocking.
                let mut blocked = false;
                for attacker in theory.rules() {
                    if attacker.head_literal() == &complement {
                        let attacker_body_satisfied =
                            attacker.body.iter().all(|b| proven.contains(b));
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
        // Check if complement is proven (contradicted)
        if proven.contains(&complement) {
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
