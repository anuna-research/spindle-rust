//! Predicate identity independent of literal occurrence details.

use std::fmt;

use crate::intern::{SymbolId, resolve};

/// A predicate's interned functor and argument count.
///
/// Argument values, negation, modality, and temporal bounds are excluded. This
/// is a vocabulary/index key, not a key for proof states or literal matching.
/// Arity uses `usize` so every stored literal's argument count is representable
/// without truncation or fallible extraction. All interned names are accepted.
/// Unlike [`PredicateSymbol`](crate::vocabulary::PredicateSymbol), this key does
/// not validate names for predicate-indicator parsing. Its display is diagnostic
/// `functor/arity` text, not a serialization format.
///
/// Ordering follows process-local symbol IDs, then arity; it is not lexical.
///
/// ```
/// use spindle_core::{Literal, PredicateKey, intern};
/// let key = Literal::simple("ci-green").predicate_key();
/// assert_eq!(key, PredicateKey::new(intern("ci-green"), 0));
/// assert_eq!(key.to_string(), "ci-green/0");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PredicateKey {
    functor: SymbolId,
    arity: usize,
}

impl PredicateKey {
    /// Construct a key from an interned functor and argument count.
    pub const fn new(functor: SymbolId, arity: usize) -> Self {
        Self { functor, arity }
    }

    /// Return the interned functor ID.
    pub const fn functor_id(self) -> SymbolId {
        self.functor
    }

    /// Resolve the functor name.
    pub fn functor(self) -> &'static str {
        resolve(self.functor)
    }

    /// Return the number of argument positions.
    pub const fn arity(self) -> usize {
        self.arity
    }
}

impl fmt::Display for PredicateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.functor(), self.arity)
    }
}
