//! Literals - the fundamental units of defeasible reasoning
//!
//! A literal represents a proposition that can be true or false,
//! optionally with modal operators and temporal bounds.
//!
//! # Performance
//!
//! Literal names are stored as interned `SymbolId` values (4 bytes)
//! instead of heap-allocated `String`s (24 bytes). This eliminates
//! allocations in hot paths of the reasoning engine.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::intern::{intern, resolve, LiteralId, SymbolId};
use crate::mode::Mode;
use crate::temporal::Temporal;

/// Type alias for literal names (for backward compatibility)
pub type LiteralName = String;

/// A literal in defeasible logic
///
/// Literals are the atomic propositions that rules reason about.
/// They can be negated, have modal operators, temporal bounds,
/// and predicate arguments.
///
/// # Performance
///
/// The name is stored as an interned `SymbolId` (4 bytes, Copy) instead
/// of a `String` (24 bytes, heap-allocated). Use `literal_id()` for
/// O(1) HashSet/HashMap operations instead of `canonical_name()`.
#[derive(Debug, Clone, Default)]
pub struct Literal {
    /// The interned name of the literal (e.g., "flies", "bird")
    name_id: SymbolId,
    /// Whether this literal is negated
    pub negation: bool,
    /// Modal operator (if any)
    pub mode: Mode,
    /// Temporal bounds (if any)
    pub temporal: Temporal,
    /// Predicate arguments (e.g., for parent(alice, bob))
    /// Stored as interned SymbolIds for efficiency
    predicate_ids: Vec<SymbolId>,
}

impl Literal {
    /// Create a simple positive literal
    pub fn simple(name: impl AsRef<str>) -> Self {
        Self {
            name_id: intern(name.as_ref()),
            negation: false,
            mode: Mode::empty(),
            temporal: Temporal::empty(),
            predicate_ids: Vec::new(),
        }
    }

    /// Create a negated literal
    pub fn negated(name: impl AsRef<str>) -> Self {
        Self {
            name_id: intern(name.as_ref()),
            negation: true,
            mode: Mode::empty(),
            temporal: Temporal::empty(),
            predicate_ids: Vec::new(),
        }
    }

    /// Create a literal with full specification
    pub fn new(
        name: impl AsRef<str>,
        negation: bool,
        mode: Mode,
        temporal: Temporal,
        predicates: Vec<String>,
    ) -> Self {
        Self {
            name_id: intern(name.as_ref()),
            negation,
            mode,
            temporal,
            predicate_ids: predicates.iter().map(|s| intern(s)).collect(),
        }
    }

    /// Create a literal directly from interned SymbolIds (zero allocation)
    ///
    /// This is an optimized constructor for use when SymbolIds are already
    /// available, such as during grounding operations.
    #[inline]
    pub fn from_ids(
        name_id: SymbolId,
        negation: bool,
        mode: Mode,
        temporal: Temporal,
        predicate_ids: Vec<SymbolId>,
    ) -> Self {
        Self {
            name_id,
            negation,
            mode,
            temporal,
            predicate_ids,
        }
    }

    /// Return the complement (negation flipped) of this literal
    ///
    /// This is a cheap operation as it only flips a boolean and
    /// copies the SymbolId (no heap allocation).
    pub fn complement(&self) -> Self {
        Self {
            name_id: self.name_id,
            negation: !self.negation,
            mode: self.mode.clone(),
            temporal: self.temporal.clone(),
            predicate_ids: self.predicate_ids.clone(),
        }
    }

    /// Get the literal name as a string slice
    ///
    /// This resolves the interned SymbolId back to its string.
    #[inline]
    pub fn name(&self) -> &'static str {
        resolve(self.name_id)
    }

    /// Get the interned name ID (for advanced use)
    #[inline]
    pub fn name_id(&self) -> SymbolId {
        self.name_id
    }

    /// Get predicates as strings (for display/serialization)
    pub fn predicates(&self) -> Vec<&'static str> {
        self.predicate_ids.iter().map(|id| resolve(*id)).collect()
    }

    /// Get predicate IDs (for efficient comparison)
    #[inline]
    pub fn predicate_ids(&self) -> &[SymbolId] {
        &self.predicate_ids
    }

    /// Check if this literal is positive (not negated)
    #[inline]
    pub fn is_positive(&self) -> bool {
        !self.negation
    }

    /// Check if this literal is negated
    #[inline]
    pub fn is_negated(&self) -> bool {
        self.negation
    }

    /// Check if this literal has modal operators
    #[inline]
    pub fn is_modal(&self) -> bool {
        !self.mode.is_empty()
    }

    /// Check if this literal has temporal bounds
    #[inline]
    pub fn is_temporal(&self) -> bool {
        !self.temporal.is_empty()
    }

    /// Check if this literal has predicate arguments
    #[inline]
    pub fn is_predicate(&self) -> bool {
        !self.predicate_ids.is_empty()
    }

    /// Get the LiteralId for this literal (name + negation combined).
    ///
    /// This is the preferred method for HashSet/HashMap keys as it's
    /// a 4-byte Copy type with O(1) hashing, compared to `canonical_name()`
    /// which allocates a new String.
    ///
    /// # Example
    ///
    /// ```rust
    /// use spindle_core::prelude::*;
    /// use std::collections::HashSet;
    ///
    /// let mut proven: HashSet<LiteralId> = HashSet::new();
    /// let bird = Literal::simple("bird");
    ///
    /// proven.insert(bird.literal_id());
    /// assert!(proven.contains(&bird.literal_id()));
    /// ```
    #[inline]
    pub fn literal_id(&self) -> LiteralId {
        LiteralId::new(self.name_id, self.negation)
    }

    /// Get the canonical name for indexing (includes negation)
    ///
    /// **Note:** This method allocates a new String. For performance-critical
    /// code, use `literal_id()` instead which returns a Copy type.
    ///
    /// Kept for backward compatibility with existing code.
    pub fn canonical_name(&self) -> String {
        if self.negation {
            format!("~{}", self.name())
        } else {
            self.name().to_owned()
        }
    }
}

impl PartialEq for Literal {
    fn eq(&self, other: &Self) -> bool {
        self.name_id == other.name_id
            && self.negation == other.negation
            && self.mode == other.mode
            && self.predicate_ids == other.predicate_ids
        // Note: temporal is not included in equality for reasoning purposes
    }
}

impl Eq for Literal {}

impl Hash for Literal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name_id.hash(state);
        self.negation.hash(state);
        self.mode.name.hash(state);
        self.mode.negation.hash(state);
        self.predicate_ids.hash(state);
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
        write!(f, "{}", self.name())?;

        // Predicates
        if !self.predicate_ids.is_empty() {
            let preds: Vec<_> = self.predicate_ids.iter().map(|id| resolve(*id)).collect();
            write!(f, "({})", preds.join(", "))?;
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
        assert_eq!(lit.name(), "bird");
        assert!(!lit.negation);
        assert!(lit.mode.is_empty());
    }

    #[test]
    fn test_negated_literal() {
        let lit = Literal::negated("flies");
        assert_eq!(lit.name(), "flies");
        assert!(lit.negation);
    }

    #[test]
    fn test_complement() {
        let lit = Literal::simple("bird");
        let comp = lit.complement();
        assert_eq!(comp.name(), "bird");
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

    #[test]
    fn test_literal_id() {
        let pos = Literal::simple("bird");
        let neg = Literal::negated("bird");

        assert!(!pos.literal_id().is_negated());
        assert!(neg.literal_id().is_negated());
        assert_eq!(pos.literal_id().symbol(), neg.literal_id().symbol());
        assert_eq!(pos.literal_id(), neg.literal_id().complement());
    }

    #[test]
    fn test_literal_id_in_hashset() {
        use std::collections::HashSet;

        let mut set: HashSet<LiteralId> = HashSet::new();
        let bird = Literal::simple("bird");
        let not_bird = Literal::negated("bird");
        let flies = Literal::simple("flies");

        set.insert(bird.literal_id());
        set.insert(flies.literal_id());

        assert!(set.contains(&bird.literal_id()));
        assert!(set.contains(&flies.literal_id()));
        assert!(!set.contains(&not_bird.literal_id()));
    }

    #[test]
    fn test_canonical_name_compat() {
        let pos = Literal::simple("bird");
        let neg = Literal::negated("flies");

        assert_eq!(pos.canonical_name(), "bird");
        assert_eq!(neg.canonical_name(), "~flies");
    }

    #[test]
    fn test_predicates() {
        let lit = Literal::new(
            "parent",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["alice".to_string(), "bob".to_string()],
        );

        let preds = lit.predicates();
        assert_eq!(preds, vec!["alice", "bob"]);
    }
}
