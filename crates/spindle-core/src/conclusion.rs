//! Conclusions from defeasible reasoning
//!
//! The reasoning engine produces four types of conclusions:
//! - +D (definitely provable): Via facts and strict rules
//! - -D (definitely not provable): Cannot be proven definitely
//! - +d (defeasibly provable): Via defeasible rules, not defeated
//! - -d (defeasibly not provable): Cannot be proven defeasibly

use std::fmt;

use crate::literal::Literal;

/// The type of conclusion
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConclusionType {
    /// Definitely provable (+D)
    DefinitelyProvable,
    /// Definitely not provable (-D)
    DefinitelyNotProvable,
    /// Defeasibly provable (+d)
    DefeasiblyProvable,
    /// Defeasibly not provable (-d)
    DefeasiblyNotProvable,
}

impl ConclusionType {
    /// Get the symbol for this conclusion type
    pub fn symbol(&self) -> &'static str {
        match self {
            ConclusionType::DefinitelyProvable => "+D",
            ConclusionType::DefinitelyNotProvable => "-D",
            ConclusionType::DefeasiblyProvable => "+d",
            ConclusionType::DefeasiblyNotProvable => "-d",
        }
    }

    /// Check if this is a positive conclusion
    pub fn is_positive(&self) -> bool {
        matches!(
            self,
            ConclusionType::DefinitelyProvable | ConclusionType::DefeasiblyProvable
        )
    }

    /// Check if this is a definite conclusion
    pub fn is_definite(&self) -> bool {
        matches!(
            self,
            ConclusionType::DefinitelyProvable | ConclusionType::DefinitelyNotProvable
        )
    }
}

impl fmt::Display for ConclusionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol())
    }
}

/// A conclusion from the reasoning engine
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Conclusion {
    /// The type of conclusion
    pub conclusion_type: ConclusionType,
    /// The literal this conclusion is about
    pub literal: Literal,
    /// The rule that derived this conclusion (if any)
    pub rule_label: Option<String>,
}

impl Conclusion {
    /// Create a new conclusion
    pub fn new(conclusion_type: ConclusionType, literal: Literal) -> Self {
        Self {
            conclusion_type,
            literal,
            rule_label: None,
        }
    }

    /// Create with rule label
    pub fn with_rule(mut self, label: impl Into<String>) -> Self {
        self.rule_label = Some(label.into());
        self
    }

    /// Create a definitely provable conclusion
    pub fn definitely_provable(literal: Literal) -> Self {
        Self::new(ConclusionType::DefinitelyProvable, literal)
    }

    /// Create a defeasibly provable conclusion
    pub fn defeasibly_provable(literal: Literal) -> Self {
        Self::new(ConclusionType::DefeasiblyProvable, literal)
    }

    /// Check if this conclusion is positive (provable)
    pub fn is_positive(&self) -> bool {
        self.conclusion_type.is_positive()
    }

    /// Check if this conclusion is definite
    pub fn is_definite(&self) -> bool {
        self.conclusion_type.is_definite()
    }
}

impl fmt::Display for Conclusion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.conclusion_type, self.literal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conclusion_types() {
        assert_eq!(ConclusionType::DefinitelyProvable.symbol(), "+D");
        assert_eq!(ConclusionType::DefeasiblyProvable.symbol(), "+d");
        assert!(ConclusionType::DefinitelyProvable.is_positive());
        assert!(!ConclusionType::DefinitelyNotProvable.is_positive());
    }

    #[test]
    fn test_conclusion_type_is_definite() {
        assert!(ConclusionType::DefinitelyProvable.is_definite());
        assert!(ConclusionType::DefinitelyNotProvable.is_definite());
        assert!(!ConclusionType::DefeasiblyProvable.is_definite());
        assert!(!ConclusionType::DefeasiblyNotProvable.is_definite());
    }

    #[test]
    fn test_conclusion_display() {
        let c = Conclusion::definitely_provable(Literal::simple("bird"));
        assert_eq!(format!("{}", c), "+D bird");
    }

    #[test]
    fn test_conclusion_is_positive() {
        let c1 = Conclusion::definitely_provable(Literal::simple("bird"));
        assert!(c1.is_positive());

        let c2 = Conclusion::defeasibly_provable(Literal::simple("flies"));
        assert!(c2.is_positive());

        let c3 = Conclusion::new(ConclusionType::DefinitelyNotProvable, Literal::simple("x"));
        assert!(!c3.is_positive());

        let c4 = Conclusion::new(ConclusionType::DefeasiblyNotProvable, Literal::simple("y"));
        assert!(!c4.is_positive());
    }

    #[test]
    fn test_conclusion_is_definite() {
        let c1 = Conclusion::definitely_provable(Literal::simple("bird"));
        assert!(c1.is_definite());

        let c2 = Conclusion::defeasibly_provable(Literal::simple("flies"));
        assert!(!c2.is_definite());

        let c3 = Conclusion::new(ConclusionType::DefinitelyNotProvable, Literal::simple("x"));
        assert!(c3.is_definite());
    }
}
