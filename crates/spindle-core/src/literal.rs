//! Literals - the fundamental units of defeasible reasoning
//!
//! A literal represents a proposition that can be true or false,
//! optionally with modal operators and temporal bounds.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::mode::Mode;
use crate::temporal::Temporal;

/// Type alias for literal names
pub type LiteralName = String;

/// A literal in defeasible logic
///
/// Literals are the atomic propositions that rules reason about.
/// They can be negated, have modal operators, temporal bounds,
/// and predicate arguments.
#[derive(Debug, Clone, Default)]
pub struct Literal {
    /// The name of the literal (e.g., "flies", "bird")
    pub name: LiteralName,
    /// Whether this literal is negated
    pub negation: bool,
    /// Modal operator (if any)
    pub mode: Mode,
    /// Temporal bounds (if any)
    pub temporal: Temporal,
    /// Predicate arguments (e.g., for parent(alice, bob))
    pub predicates: Vec<String>,
}

impl Literal {
    /// Create a simple positive literal
    pub fn simple(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            negation: false,
            mode: Mode::empty(),
            temporal: Temporal::empty(),
            predicates: Vec::new(),
        }
    }

    /// Create a negated literal
    pub fn negated(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            negation: true,
            mode: Mode::empty(),
            temporal: Temporal::empty(),
            predicates: Vec::new(),
        }
    }

    /// Create a literal with full specification
    pub fn new(
        name: impl Into<String>,
        negation: bool,
        mode: Mode,
        temporal: Temporal,
        predicates: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            negation,
            mode,
            temporal,
            predicates,
        }
    }

    /// Return the complement (negation flipped) of this literal
    pub fn complement(&self) -> Self {
        Self {
            name: self.name.clone(),
            negation: !self.negation,
            mode: self.mode.clone(),
            temporal: self.temporal.clone(),
            predicates: self.predicates.clone(),
        }
    }

    /// Check if this literal is positive (not negated)
    pub fn is_positive(&self) -> bool {
        !self.negation
    }

    /// Check if this literal is negated
    pub fn is_negated(&self) -> bool {
        self.negation
    }

    /// Check if this literal has modal operators
    pub fn is_modal(&self) -> bool {
        !self.mode.is_empty()
    }

    /// Check if this literal has temporal bounds
    pub fn is_temporal(&self) -> bool {
        !self.temporal.is_empty()
    }

    /// Check if this literal has predicate arguments
    pub fn is_predicate(&self) -> bool {
        !self.predicates.is_empty()
    }

    /// Get the canonical name for indexing (includes negation)
    pub fn canonical_name(&self) -> String {
        if self.negation {
            format!("~{}", self.name)
        } else {
            self.name.clone()
        }
    }
}

impl PartialEq for Literal {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.negation == other.negation
            && self.mode == other.mode
            && self.predicates == other.predicates
        // Note: temporal is not included in equality for reasoning purposes
    }
}

impl Eq for Literal {}

impl Hash for Literal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.negation.hash(state);
        self.mode.name.hash(state);
        self.mode.negation.hash(state);
        self.predicates.hash(state);
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Mode prefix
        if !self.mode.is_empty() {
            write!(f, "{}", self.mode)?;
        }

        // Negation
        if self.negation {
            write!(f, "~")?;
        }

        // Name
        write!(f, "{}", self.name)?;

        // Predicates
        if !self.predicates.is_empty() {
            write!(f, "({})", self.predicates.join(", "))?;
        }

        // Temporal
        if !self.temporal.is_empty() {
            write!(f, "{}", self.temporal)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_literal() {
        let lit = Literal::simple("bird");
        assert_eq!(lit.name, "bird");
        assert!(!lit.negation);
        assert!(lit.mode.is_empty());
    }

    #[test]
    fn test_negated_literal() {
        let lit = Literal::negated("flies");
        assert_eq!(lit.name, "flies");
        assert!(lit.negation);
    }

    #[test]
    fn test_complement() {
        let lit = Literal::simple("bird");
        let comp = lit.complement();
        assert_eq!(comp.name, "bird");
        assert!(comp.negation);

        let comp2 = comp.complement();
        assert!(!comp2.negation);
    }

    #[test]
    fn test_display() {
        let lit = Literal::simple("bird");
        assert_eq!(format!("{}", lit), "bird");

        let neg = Literal::negated("flies");
        assert_eq!(format!("{}", neg), "~flies");
    }
}
