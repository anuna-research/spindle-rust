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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Conclusion {
    /// The type of conclusion
    pub conclusion_type: ConclusionType,
    /// The literal this conclusion is about
    pub literal: Literal,
}

impl Conclusion {
    /// Create a new conclusion
    pub fn new(conclusion_type: ConclusionType, literal: Literal) -> Self {
        Self {
            conclusion_type,
            literal,
        }
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
    fn test_conclusion_display() {
        let c = Conclusion::definitely_provable(Literal::simple("bird"));
        assert_eq!(format!("{}", c), "+D bird");
    }
}
