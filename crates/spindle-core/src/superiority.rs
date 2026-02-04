//! Superiority relations for conflict resolution
//!
//! When two rules produce conflicting conclusions, superiority
//! relations determine which rule wins.
//!
//! # Performance
//!
//! The `SuperiorityIndex` provides O(1) lookup for superiority checks,
//! compared to O(N) linear scan of a Vec<Superiority>.

use rustc_hash::{FxHashMap, FxHashSet};
use std::fmt;

use crate::rule::RuleLabel;

/// A superiority relation between two rules
///
/// Indicates that the superior rule takes precedence over the
/// inferior rule when they produce conflicting conclusions.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Superiority {
    /// The label of the superior (winning) rule
    pub superior: RuleLabel,
    /// The label of the inferior (losing) rule
    pub inferior: RuleLabel,
}

impl Superiority {
    /// Create a new superiority relation
    pub fn new(superior: impl Into<String>, inferior: impl Into<String>) -> Self {
        Self {
            superior: superior.into(),
            inferior: inferior.into(),
        }
    }
}

impl fmt::Display for Superiority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} > {}", self.superior, self.inferior)
    }
}

/// An indexed structure for O(1) superiority lookups.
///
/// This replaces linear scans through Vec<Superiority> with constant-time
/// hash lookups. It maintains both forward (who beats who) and reverse
/// (who is beaten by who) indices.
///
/// # Example
///
/// ```rust
/// use spindle_core::superiority::{Superiority, SuperiorityIndex};
///
/// let sups = vec![
///     Superiority::new("r2", "r1"),
///     Superiority::new("r3", "r1"),
///     Superiority::new("r3", "r2"),
/// ];
/// let index = SuperiorityIndex::build(&sups);
///
/// assert!(index.is_superior("r2", "r1"));
/// assert!(index.is_superior("r3", "r1"));
/// assert!(!index.is_superior("r1", "r2"));
///
/// // r3 beats both r1 and r2
/// assert_eq!(index.inferiors_of("r3").len(), 2);
///
/// // r1 is beaten by both r2 and r3
/// assert_eq!(index.superiors_of("r1").len(), 2);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SuperiorityIndex {
    /// Set of (superior, inferior) pairs for O(1) lookup
    pairs: FxHashSet<(RuleLabel, RuleLabel)>,
    /// Forward index: rule -> rules it beats
    inferiors: FxHashMap<RuleLabel, Vec<RuleLabel>>,
    /// Reverse index: rule -> rules that beat it
    superiors: FxHashMap<RuleLabel, Vec<RuleLabel>>,
}

impl SuperiorityIndex {
    /// Create an empty superiority index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an index from a slice of superiority relations.
    ///
    /// This is O(N) where N is the number of superiority relations.
    pub fn build(superiorities: &[Superiority]) -> Self {
        let mut index = Self {
            pairs: FxHashSet::with_capacity_and_hasher(superiorities.len(), Default::default()),
            inferiors: FxHashMap::default(),
            superiors: FxHashMap::default(),
        };

        for sup in superiorities {
            index.add(sup.superior.clone(), sup.inferior.clone());
        }

        index
    }

    /// Add a superiority relation to the index.
    pub fn add(&mut self, superior: RuleLabel, inferior: RuleLabel) {
        // Add to pair set
        self.pairs.insert((superior.clone(), inferior.clone()));

        // Add to forward index (superior -> inferior)
        self.inferiors
            .entry(superior.clone())
            .or_default()
            .push(inferior.clone());

        // Add to reverse index (inferior -> superior)
        self.superiors.entry(inferior).or_default().push(superior);
    }

    /// Check if `superior` is superior to `inferior`.
    ///
    /// This is O(1) average case.
    #[inline]
    pub fn is_superior(&self, superior: &str, inferior: &str) -> bool {
        self.pairs
            .contains(&(superior.to_owned(), inferior.to_owned()))
    }

    /// Check if `superior` is superior to `inferior` (owned version).
    ///
    /// This is O(1) average case and avoids allocation when keys are already owned.
    #[inline]
    pub fn is_superior_owned(&self, superior: &RuleLabel, inferior: &RuleLabel) -> bool {
        self.pairs.contains(&(superior.clone(), inferior.clone()))
    }

    /// Get all rules that `rule` is superior to (rules it beats).
    ///
    /// Returns an empty slice if `rule` has no inferiors.
    #[inline]
    pub fn inferiors_of(&self, rule: &str) -> &[RuleLabel] {
        self.inferiors
            .get(rule)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get all rules that are superior to `rule` (rules that beat it).
    ///
    /// Returns an empty slice if `rule` has no superiors.
    #[inline]
    pub fn superiors_of(&self, rule: &str) -> &[RuleLabel] {
        self.superiors
            .get(rule)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Check if there are any superiority relations defined.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Get the number of superiority relations.
    #[inline]
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Iterate over all superiority pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&RuleLabel, &RuleLabel)> {
        self.pairs.iter().map(|(sup, inf)| (sup, inf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_superiority() {
        let sup = Superiority::new("r2", "r1");
        assert_eq!(sup.superior, "r2");
        assert_eq!(sup.inferior, "r1");
        assert_eq!(format!("{}", sup), "r2 > r1");
    }

    #[test]
    fn test_index_empty() {
        let index = SuperiorityIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
        assert!(!index.is_superior("r1", "r2"));
    }

    #[test]
    fn test_index_build() {
        let sups = vec![
            Superiority::new("r2", "r1"),
            Superiority::new("r3", "r1"),
            Superiority::new("r3", "r2"),
        ];
        let index = SuperiorityIndex::build(&sups);

        assert_eq!(index.len(), 3);
        assert!(!index.is_empty());
    }

    #[test]
    fn test_index_is_superior() {
        let sups = vec![Superiority::new("r2", "r1"), Superiority::new("r3", "r2")];
        let index = SuperiorityIndex::build(&sups);

        // Direct superiorities
        assert!(index.is_superior("r2", "r1"));
        assert!(index.is_superior("r3", "r2"));

        // Not superior
        assert!(!index.is_superior("r1", "r2"));
        assert!(!index.is_superior("r2", "r3"));

        // Transitivity is NOT implied (r3 > r2 > r1 doesn't mean r3 > r1)
        assert!(!index.is_superior("r3", "r1"));
    }

    #[test]
    fn test_index_inferiors_of() {
        let sups = vec![Superiority::new("r3", "r1"), Superiority::new("r3", "r2")];
        let index = SuperiorityIndex::build(&sups);

        let inf = index.inferiors_of("r3");
        assert_eq!(inf.len(), 2);
        assert!(inf.contains(&"r1".to_string()));
        assert!(inf.contains(&"r2".to_string()));

        // r1 beats nobody
        assert!(index.inferiors_of("r1").is_empty());
    }

    #[test]
    fn test_index_superiors_of() {
        let sups = vec![Superiority::new("r2", "r1"), Superiority::new("r3", "r1")];
        let index = SuperiorityIndex::build(&sups);

        let sup = index.superiors_of("r1");
        assert_eq!(sup.len(), 2);
        assert!(sup.contains(&"r2".to_string()));
        assert!(sup.contains(&"r3".to_string()));

        // r3 is beaten by nobody
        assert!(index.superiors_of("r3").is_empty());
    }

    #[test]
    fn test_index_add() {
        let mut index = SuperiorityIndex::new();
        index.add("r2".to_string(), "r1".to_string());

        assert!(index.is_superior("r2", "r1"));
        assert!(!index.is_superior("r1", "r2"));
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn test_index_is_superior_owned() {
        let sups = vec![Superiority::new("r2", "r1")];
        let index = SuperiorityIndex::build(&sups);

        let r2 = "r2".to_string();
        let r1 = "r1".to_string();
        assert!(index.is_superior_owned(&r2, &r1));
        assert!(!index.is_superior_owned(&r1, &r2));
    }

    #[test]
    fn test_index_iter() {
        let sups = vec![Superiority::new("r2", "r1"), Superiority::new("r3", "r1")];
        let index = SuperiorityIndex::build(&sups);

        let pairs: Vec<_> = index.iter().collect();
        assert_eq!(pairs.len(), 2);
    }
}
