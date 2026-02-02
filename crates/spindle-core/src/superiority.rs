//! Superiority relations for conflict resolution
//!
//! When two rules produce conflicting conclusions, superiority
//! relations determine which rule wins.

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
}
