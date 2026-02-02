//! Modal operators for deontic logic
//!
//! Supports obligation [O], permission [P], and forbidden [F] modalities.

use std::fmt;

/// Modal operator for deontic logic
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Mode {
    /// Mode name (e.g., "O" for obligation)
    pub name: Option<String>,
    /// Whether the mode is negated
    pub negation: bool,
}

impl Mode {
    /// Create an empty (no modal) mode
    pub const fn empty() -> Self {
        Self {
            name: None,
            negation: false,
        }
    }

    /// Create a new mode with the given name
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            negation: false,
        }
    }

    /// Create obligation mode [O]
    pub fn obligation() -> Self {
        Self::new("O")
    }

    /// Create permission mode [P]
    pub fn permission() -> Self {
        Self::new("P")
    }

    /// Create forbidden mode [F]
    pub fn forbidden() -> Self {
        Self::new("F")
    }

    /// Check if this is an empty (non-modal) mode
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
    }

    /// Return the complement of this mode
    pub fn complement(&self) -> Self {
        Self {
            name: self.name.clone(),
            negation: !self.negation,
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.name {
            Some(name) => {
                if self.negation {
                    write!(f, "[-{}]", name)
                } else {
                    write!(f, "[{}]", name)
                }
            }
            None => Ok(()),
        }
    }
}
