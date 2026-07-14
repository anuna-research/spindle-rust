//! Checked predicate signatures (SPEC-024 CON-004, REQ-008).

use super::sort::PrimitiveSort;
use super::symbol::PredicateSymbol;

/// Error returned when a [`PredicateSignature`] cannot be constructed
/// (SPEC-024 REQ-008). Every variant names the predicate and the offending
/// position or duplicate name.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PredicateSignatureError {
    /// The number of argument declarations does not equal the predicate arity.
    #[error("predicate {} declares {declared} arguments but has arity {arity}", .symbol.indicator())]
    ArityMismatch {
        /// The predicate symbol.
        symbol: PredicateSymbol,
        /// The declared argument count.
        declared: usize,
        /// The predicate arity.
        arity: u32,
    },
    /// An argument name is empty.
    #[error("predicate {} has an empty argument name at position {position}", .symbol.indicator())]
    EmptyArgumentName {
        /// The predicate symbol.
        symbol: PredicateSymbol,
        /// The zero-based offending position.
        position: usize,
    },
    /// Two argument declarations share a name.
    #[error("predicate {} has duplicate argument name {name:?} at positions {first} and {second}", .symbol.indicator())]
    DuplicateArgumentName {
        /// The predicate symbol.
        symbol: PredicateSymbol,
        /// The duplicated name.
        name: String,
        /// The first position holding the name.
        first: usize,
        /// The second position holding the name.
        second: usize,
    },
}

/// One ordered argument declaration: a stable name and a primitive sort.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ArgumentDecl {
    /// The stable argument name.
    pub name: String,
    /// The declared primitive sort.
    pub sort: PrimitiveSort,
}

impl ArgumentDecl {
    /// Construct an argument declaration.
    pub fn new(name: impl Into<String>, sort: PrimitiveSort) -> Self {
        Self {
            name: name.into(),
            sort,
        }
    }
}

/// A checked predicate signature (SPEC-024 §3.3).
///
/// The declaration count always equals the symbol arity, and argument names are
/// unique and non-empty. These invariants are enforced by [`Self::try_new`].
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PredicateSignature {
    /// The predicate symbol.
    pub symbol: PredicateSymbol,
    /// One ordered declaration per argument position.
    pub arguments: Box<[ArgumentDecl]>,
}

impl PredicateSignature {
    /// Construct a checked signature, enforcing SPEC-024 REQ-008.
    ///
    /// The declaration count must equal the predicate arity, and argument names
    /// must be unique and non-empty.
    pub fn try_new(
        symbol: PredicateSymbol,
        arguments: Vec<ArgumentDecl>,
    ) -> Result<Self, PredicateSignatureError> {
        if arguments.len() as u64 != u64::from(symbol.arity()) {
            return Err(PredicateSignatureError::ArityMismatch {
                symbol,
                declared: arguments.len(),
                arity: symbol.arity(),
            });
        }

        for (position, arg) in arguments.iter().enumerate() {
            if arg.name.is_empty() {
                return Err(PredicateSignatureError::EmptyArgumentName { symbol, position });
            }
            if let Some(first) = arguments[..position]
                .iter()
                .position(|a| a.name == arg.name)
            {
                return Err(PredicateSignatureError::DuplicateArgumentName {
                    symbol,
                    name: arg.name.clone(),
                    first,
                    second: position,
                });
            }
        }

        Ok(Self {
            symbol,
            arguments: arguments.into_boxed_slice(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal::InternedLiteralName;

    fn sym(name: &str, arity: usize) -> PredicateSymbol {
        PredicateSymbol::try_new(InternedLiteralName::intern(name), arity).unwrap()
    }

    #[test]
    fn valid_binary_signature() {
        let sig = PredicateSignature::try_new(
            sym("assign-to", 2),
            vec![
                ArgumentDecl::new("task", PrimitiveSort::Symbol),
                ArgumentDecl::new("agent", PrimitiveSort::Symbol),
            ],
        )
        .unwrap();
        assert_eq!(sig.arguments.len(), 2);
    }

    #[test]
    fn wrong_argument_count_rejected() {
        let err = PredicateSignature::try_new(
            sym("p", 2),
            vec![ArgumentDecl::new("only", PrimitiveSort::Any)],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            PredicateSignatureError::ArityMismatch {
                declared: 1,
                arity: 2,
                ..
            }
        ));
    }

    #[test]
    fn empty_name_rejected() {
        let err = PredicateSignature::try_new(
            sym("p", 1),
            vec![ArgumentDecl::new("", PrimitiveSort::Any)],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            PredicateSignatureError::EmptyArgumentName { position: 0, .. }
        ));
    }

    #[test]
    fn duplicate_name_rejected() {
        let err = PredicateSignature::try_new(
            sym("p", 2),
            vec![
                ArgumentDecl::new("x", PrimitiveSort::Any),
                ArgumentDecl::new("x", PrimitiveSort::Any),
            ],
        )
        .unwrap_err();
        assert!(matches!(
            err,
            PredicateSignatureError::DuplicateArgumentName {
                first: 0,
                second: 1,
                ..
            }
        ));
    }

    #[test]
    fn zero_arity_signature_ok() {
        let sig = PredicateSignature::try_new(sym("emergency", 0), vec![]).unwrap();
        assert_eq!(sig.arguments.len(), 0);
    }
}
