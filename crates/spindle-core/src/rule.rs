//! Rules in defeasible logic
//!
//! Four rule types are supported:
//! - Facts (>>): Unconditional truths
//! - Strict rules (->): Must hold if antecedent is true
//! - Defeasible rules (=>): Normally hold unless defeated
//! - Defeaters (~>): Block conclusions without proving anything
//!
//! # Performance
//!
//! Uses `SmallVec` for body and head literals to avoid heap allocation
//! for typical rules (1-4 body literals, 1 head literal).

use std::fmt;

use smallvec::SmallVec;

use crate::literal::Literal;
use crate::mode::Mode;
use crate::temporal::Temporal;

/// Type alias for rule body - most rules have 1-4 body literals
pub type RuleBody = SmallVec<[Literal; 4]>;

/// Type alias for rule head - most rules have 1 head literal
pub type RuleHead = SmallVec<[Literal; 1]>;

/// Type alias for rule labels
pub type RuleLabel = String;

/// The type of a rule, determining its strength
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleType {
    /// Fact (>>): Unconditional truth, no antecedents
    Fact,
    /// Strict rule (->): If antecedent then consequent (cannot be defeated)
    Strict,
    /// Defeasible rule (=>): If antecedent then typically consequent
    Defeasible,
    /// Defeater (~>): Blocks conclusions but doesn't prove anything
    Defeater,
}

impl RuleType {
    /// Get the arrow symbol for this rule type
    pub fn arrow(&self) -> &'static str {
        match self {
            RuleType::Fact => ">>",
            RuleType::Strict => "->",
            RuleType::Defeasible => "=>",
            RuleType::Defeater => "~>",
        }
    }

    /// Check if this rule type can produce definite conclusions
    pub fn is_definite(&self) -> bool {
        matches!(self, RuleType::Fact | RuleType::Strict)
    }

    /// Check if this rule type can produce defeasible conclusions
    pub fn is_defeasible(&self) -> bool {
        matches!(self, RuleType::Defeasible)
    }

    /// Check if this is a defeater (blocks but doesn't prove)
    pub fn is_defeater(&self) -> bool {
        matches!(self, RuleType::Defeater)
    }
}

impl fmt::Display for RuleType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.arrow())
    }
}

/// A rule in defeasible logic
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Rule {
    /// Unique label for this rule
    pub label: RuleLabel,
    /// Type of rule (fact, strict, defeasible, defeater)
    pub rule_type: RuleType,
    /// Modal operator (if any)
    pub mode: Mode,
    /// Temporal bounds (if any)
    pub temporal: Temporal,
    /// Body literals (antecedents/premises)
    /// Uses SmallVec to avoid heap allocation for typical 1-4 body literals
    pub body: RuleBody,
    /// Head literals (consequents/conclusions)
    /// Uses SmallVec to avoid heap allocation for typical single head
    pub head: RuleHead,
}

impl Rule {
    /// Create a new rule
    pub fn new(
        label: impl Into<String>,
        rule_type: RuleType,
        body: impl Into<RuleBody>,
        head: impl Into<RuleHead>,
    ) -> Self {
        Self {
            label: label.into(),
            rule_type,
            mode: Mode::empty(),
            temporal: Temporal::empty(),
            body: body.into(),
            head: head.into(),
        }
    }

    /// Create a fact (no body)
    pub fn fact(label: impl Into<String>, head: Literal) -> Self {
        Self::new(
            label,
            RuleType::Fact,
            SmallVec::new(),
            smallvec::smallvec![head],
        )
    }

    /// Create a strict rule
    pub fn strict(label: impl Into<String>, body: impl Into<RuleBody>, head: Literal) -> Self {
        Self::new(label, RuleType::Strict, body, smallvec::smallvec![head])
    }

    /// Create a defeasible rule
    pub fn defeasible(label: impl Into<String>, body: impl Into<RuleBody>, head: Literal) -> Self {
        Self::new(label, RuleType::Defeasible, body, smallvec::smallvec![head])
    }

    /// Create a defeater
    pub fn defeater(label: impl Into<String>, body: impl Into<RuleBody>, head: Literal) -> Self {
        Self::new(label, RuleType::Defeater, body, smallvec::smallvec![head])
    }

    /// Check if this rule is a fact
    pub fn is_fact(&self) -> bool {
        self.rule_type == RuleType::Fact
    }

    /// Check if this rule has an empty body
    pub fn has_empty_body(&self) -> bool {
        self.body.is_empty()
    }

    /// Get the single head literal (panics if multiple heads)
    pub fn head_literal(&self) -> &Literal {
        assert_eq!(self.head.len(), 1, "Expected single head literal");
        &self.head[0]
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.label)?;

        if !self.body.is_empty() {
            let body_str: Vec<_> = self.body.iter().map(|l| l.to_string()).collect();
            write!(f, "{} ", body_str.join(", "))?;
        }

        write!(f, "{} ", self.rule_type.arrow())?;

        let head_str: Vec<_> = self.head.iter().map(|l| l.to_string()).collect();
        write!(f, "{}", head_str.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_types() {
        assert_eq!(RuleType::Fact.arrow(), ">>");
        assert_eq!(RuleType::Strict.arrow(), "->");
        assert_eq!(RuleType::Defeasible.arrow(), "=>");
        assert_eq!(RuleType::Defeater.arrow(), "~>");
    }

    #[test]
    fn test_rule_type_is_definite() {
        assert!(RuleType::Fact.is_definite());
        assert!(RuleType::Strict.is_definite());
        assert!(!RuleType::Defeasible.is_definite());
        assert!(!RuleType::Defeater.is_definite());
    }

    #[test]
    fn test_rule_type_is_defeasible() {
        assert!(!RuleType::Fact.is_defeasible());
        assert!(!RuleType::Strict.is_defeasible());
        assert!(RuleType::Defeasible.is_defeasible());
        assert!(!RuleType::Defeater.is_defeasible());
    }

    #[test]
    fn test_rule_type_is_defeater() {
        assert!(!RuleType::Fact.is_defeater());
        assert!(!RuleType::Strict.is_defeater());
        assert!(!RuleType::Defeasible.is_defeater());
        assert!(RuleType::Defeater.is_defeater());
    }

    #[test]
    fn test_rule_type_display() {
        assert_eq!(format!("{}", RuleType::Fact), ">>");
        assert_eq!(format!("{}", RuleType::Strict), "->");
        assert_eq!(format!("{}", RuleType::Defeasible), "=>");
        assert_eq!(format!("{}", RuleType::Defeater), "~>");
    }

    #[test]
    fn test_fact_creation() {
        let fact = Rule::fact("f1", Literal::simple("bird"));
        assert!(fact.is_fact());
        assert!(fact.has_empty_body());
        assert_eq!(fact.head_literal().name(), "bird");
    }

    #[test]
    fn test_defeasible_rule() {
        let rule = Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        );
        assert_eq!(rule.rule_type, RuleType::Defeasible);
        assert_eq!(rule.body.len(), 1);
        assert_eq!(rule.body[0].name(), "bird");
    }

    #[test]
    fn test_strict_rule() {
        let rule = Rule::strict(
            "s1",
            vec![Literal::simple("mammal")],
            Literal::simple("animal"),
        );
        assert_eq!(rule.rule_type, RuleType::Strict);
        assert!(!rule.has_empty_body());
    }

    #[test]
    fn test_defeater_rule() {
        let rule = Rule::defeater(
            "d1",
            vec![Literal::simple("penguin")],
            Literal::negated("flies"),
        );
        assert_eq!(rule.rule_type, RuleType::Defeater);
    }

    #[test]
    fn test_rule_display() {
        let rule = Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        );
        assert_eq!(format!("{}", rule), "r1: bird => flies");
    }

    #[test]
    fn test_rule_display_fact() {
        let fact = Rule::fact("f1", Literal::simple("bird"));
        assert_eq!(format!("{}", fact), "f1: >> bird");
    }
}
