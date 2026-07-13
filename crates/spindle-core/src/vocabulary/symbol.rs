//! [`PredicateSymbol`] — the structural predicate identity (SPEC-024 CON-001).
//!
//! A predicate symbol is the ordered pair `(functor, arity)`. It deliberately
//! excludes argument values, polarity, modal operator, temporal bounds, rule
//! label, and provenance, so it is suitable as a map key for declarations and
//! documentation but **not** as a proof-state or literal-identity key.

use std::cmp::Ordering;
use std::fmt::{self, Write as _};

use crate::body::BodyLogicLiteral;
use crate::literal::{InternedLiteralName, Literal};

/// Error returned when a predicate symbol cannot be constructed (SPEC-024 CON-001).
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PredicateSymbolError {
    /// The application arity does not fit in a `u32`.
    #[error("predicate arity {actual} exceeds u32::MAX and cannot be represented")]
    ArityOverflow {
        /// The rejected argument count.
        actual: usize,
    },
}

/// The structural identity of a predicate: functor plus arity (SPEC-024 REQ-001).
///
/// # Equality and ordering
///
/// Equality and hashing consider **exactly** the interned functor and the
/// unsigned arity. Polarity, argument values, mode, temporal bounds, labels,
/// and metadata never affect equality.
///
/// Ordering is defined manually by comparing the resolved functor's Unicode
/// scalar sequence and then the numeric arity. It is **not** derived from
/// [`InternedLiteralName`], whose `SymbolId` order reflects process-local
/// interning history rather than lexical order (SPEC-024 CON-001).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PredicateSymbol {
    functor: InternedLiteralName,
    arity: u32,
}

impl PredicateSymbol {
    /// Construct a predicate symbol from a functor and argument count.
    ///
    /// Returns [`PredicateSymbolError::ArityOverflow`] when `arity` cannot be
    /// represented as a `u32`.
    pub fn try_new(
        functor: InternedLiteralName,
        arity: usize,
    ) -> Result<Self, PredicateSymbolError> {
        let arity = u32::try_from(arity)
            .map_err(|_| PredicateSymbolError::ArityOverflow { actual: arity })?;
        Ok(Self { functor, arity })
    }

    /// The interned functor name.
    #[inline]
    pub fn functor(self) -> InternedLiteralName {
        self.functor
    }

    /// The number of argument positions.
    #[inline]
    pub fn arity(self) -> u32 {
        self.arity
    }

    /// A [`Display`](fmt::Display) adapter rendering the Prolog-style indicator
    /// notation `functor/arity` with quoting per SPEC-024 CON-002.
    #[inline]
    pub fn indicator(self) -> PredicateIndicatorDisplay {
        PredicateIndicatorDisplay(self)
    }
}

impl Ord for PredicateSymbol {
    fn cmp(&self, other: &Self) -> Ordering {
        // Compare by resolved functor code points, then numeric arity.
        self.functor
            .resolve()
            .cmp(other.functor.resolve())
            .then_with(|| self.arity.cmp(&other.arity))
    }
}

impl PartialOrd for PredicateSymbol {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Types that project directly to a [`PredicateSymbol`] (SPEC-024 CON-001).
pub trait HasPredicateSymbol {
    /// Extract the predicate symbol from functor and stored argument count.
    fn predicate_symbol(&self) -> Result<PredicateSymbol, PredicateSymbolError>;
}

impl HasPredicateSymbol for Literal {
    #[inline]
    fn predicate_symbol(&self) -> Result<PredicateSymbol, PredicateSymbolError> {
        PredicateSymbol::try_new(self.interned_name(), self.predicate_args().len())
    }
}

impl HasPredicateSymbol for BodyLogicLiteral {
    /// Counts the stored [`BodyArg`](crate::body::BodyArg) sequence directly, so
    /// arithmetic argument positions are retained in the arity. This does **not**
    /// pass through [`BodyLogicLiteral::to_literal`], which omits arithmetic
    /// arguments (SPEC-024 REQ-002).
    #[inline]
    fn predicate_symbol(&self) -> Result<PredicateSymbol, PredicateSymbolError> {
        PredicateSymbol::try_new(self.interned_name(), self.predicate_args().len())
    }
}

/// Characters permitted in a bare (unquoted) predicate-indicator functor
/// (SPEC-024 CON-002): a Unicode alphabetic scalar, an ASCII digit, or one of
/// the listed punctuation marks. A bare functor deliberately excludes `/`.
///
/// This must stay in lockstep with the recognizer's bare-char predicate in
/// `spindle-parser` so rendering and re-parsing round-trip. `is_alphanumeric`
/// is deliberately avoided: it admits Unicode numeric scalars (e.g. `²`) that
/// the grammar's `digit = "0".."9"` excludes.
pub(crate) fn is_bare_indicator_char(c: char) -> bool {
    c.is_alphabetic()
        || c.is_ascii_digit()
        || matches!(
            c,
            '-' | '_' | '?' | '~' | ':' | '.' | '+' | '*' | '<' | '>' | '=' | '!'
        )
}

/// Returns `true` when `functor` can be rendered as a bare indicator functor.
pub(crate) fn functor_is_bare(functor: &str) -> bool {
    !functor.is_empty() && functor.chars().all(is_bare_indicator_char)
}

/// A [`Display`](fmt::Display) adapter for the predicate-indicator notation.
///
/// Rendering quotes and escapes only when required and never changes functor
/// content (SPEC-024 REQ-003, TEST-003).
#[derive(Clone, Copy, Debug)]
pub struct PredicateIndicatorDisplay(PredicateSymbol);

impl fmt::Display for PredicateIndicatorDisplay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let functor = self.0.functor.resolve();
        if functor_is_bare(functor) {
            write!(f, "{functor}/{}", self.0.arity)
        } else {
            f.write_str("\"")?;
            for c in functor.chars() {
                match c {
                    '"' => f.write_str("\\\"")?,
                    '\\' => f.write_str("\\\\")?,
                    other => f.write_char(other)?,
                }
            }
            write!(f, "\"/{}", self.0.arity)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::intern;
    use crate::mode::Mode;
    use crate::temporal::Temporal;
    use crate::term::Term;

    fn sym(name: &str, arity: usize) -> PredicateSymbol {
        PredicateSymbol::try_new(InternedLiteralName::intern(name), arity).unwrap()
    }

    #[test]
    fn projects_by_functor_and_arity() {
        let p1 = sym("p", 1);
        let p1b = sym("p", 1);
        let p0 = sym("p", 0);
        let p2 = sym("p", 2);
        assert_eq!(p1, p1b);
        assert_ne!(p1, p0);
        assert_ne!(p1, p2);
        assert_ne!(p0, p2);
    }

    #[test]
    fn arity_overflow_rejected() {
        // usize::MAX cannot fit in u32 on 64-bit platforms.
        let err = PredicateSymbol::try_new(InternedLiteralName::intern("p"), usize::MAX);
        assert_eq!(
            err,
            Err(PredicateSymbolError::ArityOverflow { actual: usize::MAX })
        );
    }

    #[test]
    fn literal_and_negation_project_equal() {
        let pos = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".to_string()],
        );
        let neg = Literal::new(
            "p",
            true,
            Mode::empty(),
            Temporal::empty(),
            vec!["b".to_string()],
        );
        assert_eq!(
            pos.predicate_symbol().unwrap(),
            neg.predicate_symbol().unwrap()
        );
        assert_eq!(pos.predicate_symbol().unwrap(), sym("p", 1));
    }

    #[test]
    fn body_arity_counts_arith_args() {
        use crate::arith::ArithExpr;
        use crate::body::BodyArg;

        let bll = BodyLogicLiteral::new(
            "cost",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(intern("item"))),
                BodyArg::Arith(ArithExpr::Var(intern("?total"))),
            ],
        );
        // Direct extraction retains the arithmetic position: arity is 2.
        assert_eq!(bll.predicate_symbol().unwrap(), sym("cost", 2));
        // The lossy to_literal() path would report arity 1.
        assert_eq!(bll.to_literal().predicate_args().len(), 1);
    }

    #[test]
    fn ordering_is_lexical_not_intern_order() {
        // Intern in reverse lexical order so SymbolId order differs from code-point order.
        let zebra = sym("zebra", 0);
        let apple = sym("apple", 0);
        assert!(apple < zebra);
    }

    #[test]
    fn indicator_roundtrip_shapes() {
        assert_eq!(sym("assign-to", 2).indicator().to_string(), "assign-to/2");
        assert_eq!(sym("p", 0).indicator().to_string(), "p/0");
        assert_eq!(
            sym("rate/limit", 2).indicator().to_string(),
            "\"rate/limit\"/2"
        );
    }
}
