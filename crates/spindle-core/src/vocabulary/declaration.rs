//! Predicate declarations, source provenance, and metadata targets
//! (SPEC-024 CON-008).

use super::signature::PredicateSignature;
use super::symbol::PredicateSymbol;

/// A source location within a parsed theory (SPEC-024 CON-008).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SourceLocation {
    /// Byte offset of the declaration in the cleaned source.
    pub byte_offset: usize,
    /// One-based line number of the declaration.
    pub line: usize,
}

impl SourceLocation {
    /// Construct a source location.
    pub fn new(byte_offset: usize, line: usize) -> Self {
        Self { byte_offset, line }
    }
}

/// The origin of a [`PredicateDeclaration`] (SPEC-024 CON-008).
///
/// Distinguishes parser source provenance from a programmatic declaration with
/// no source location.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DeclarationOrigin {
    /// Parsed from source at the given location.
    Parsed(SourceLocation),
    /// Added programmatically with no source location.
    Programmatic,
}

/// An origin-bearing predicate declaration (SPEC-024 §3.4).
///
/// A declaration declares structure — not a fact or rule — and therefore does
/// not itself establish or derive any literal.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct PredicateDeclaration {
    /// The checked predicate signature.
    pub signature: PredicateSignature,
    /// Where the declaration came from.
    pub origin: DeclarationOrigin,
}

impl PredicateDeclaration {
    /// Construct a predicate declaration.
    pub fn new(signature: PredicateSignature, origin: DeclarationOrigin) -> Self {
        Self { signature, origin }
    }

    /// The predicate symbol this declaration describes.
    #[inline]
    pub fn symbol(&self) -> PredicateSymbol {
        self.signature.symbol
    }
}

/// A structured metadata target (SPEC-024 §3.5, CON-008).
///
/// `Label` preserves existing rule-label metadata; `Predicate` attaches
/// metadata to a predicate symbol without overloading a rule label or parsing a
/// predicate-indicator string.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum MetaTarget {
    /// Metadata keyed by a rule label (the pre-SPEC-024 behavior).
    Label(String),
    /// Metadata keyed by a predicate symbol.
    Predicate(PredicateSymbol),
}

impl MetaTarget {
    /// Borrow the label string, if this is a label target.
    pub fn as_label(&self) -> Option<&str> {
        match self {
            MetaTarget::Label(s) => Some(s),
            MetaTarget::Predicate(_) => None,
        }
    }

    /// Copy the predicate symbol, if this is a predicate target.
    pub fn as_predicate(&self) -> Option<PredicateSymbol> {
        match self {
            MetaTarget::Predicate(sym) => Some(*sym),
            MetaTarget::Label(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal::InternedLiteralName;
    use crate::vocabulary::signature::{ArgumentDecl, PredicateSignature};
    use crate::vocabulary::sort::PrimitiveSort;

    fn sym(name: &str, arity: usize) -> PredicateSymbol {
        PredicateSymbol::try_new(InternedLiteralName::intern(name), arity).unwrap()
    }

    #[test]
    fn declaration_reports_symbol() {
        let sig = PredicateSignature::try_new(
            sym("assign-to", 2),
            vec![
                ArgumentDecl::new("task", PrimitiveSort::Symbol),
                ArgumentDecl::new("agent", PrimitiveSort::Symbol),
            ],
        )
        .unwrap();
        let decl = PredicateDeclaration::new(sig, DeclarationOrigin::Programmatic);
        assert_eq!(decl.symbol(), sym("assign-to", 2));
    }

    #[test]
    fn meta_targets_distinct_by_arity_and_kind() {
        let a = MetaTarget::Predicate(sym("assign-to", 2));
        let b = MetaTarget::Predicate(sym("assign-to", 1));
        let c = MetaTarget::Label("assign-to".to_string());
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.as_predicate(), Some(sym("assign-to", 2)));
        assert_eq!(c.as_label(), Some("assign-to"));
    }
}
