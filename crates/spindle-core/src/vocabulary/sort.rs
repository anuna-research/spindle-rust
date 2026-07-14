//! Primitive sorts and observed argument kinds (SPEC-024 CON-004).
//!
//! [`PrimitiveSort`] is a *prescriptive* declaration-level sort; it says what an
//! argument position is allowed to hold. [`ObservedArgumentKind`] is an
//! *observational* classification of what a concrete argument actually is.
//! Observation is kept strictly more precise than the compatibility sorts, so
//! `number` never collapses the distinct numeric observations.

use std::fmt;

use crate::body::BodyArg;
use crate::term::Term;

/// A primitive argument sort (SPEC-024 §3.11).
///
/// Primitive sorts do not assign domain meaning. `number` accepts all three
/// numeric term variants under existing numeric compatibility; `any` accepts
/// any ground [`Term`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PrimitiveSort {
    /// An interned symbolic name (`Term::Symbol`).
    Symbol,
    /// A 64-bit signed integer (`Term::Integer`).
    Integer,
    /// An arbitrary-precision decimal (`Term::Decimal`).
    Decimal,
    /// A canonicalized finite float (`Term::Float`).
    Float,
    /// Any numeric term (integer, decimal, or float).
    Number,
    /// Any ground term.
    Any,
}

impl PrimitiveSort {
    /// Parse a primitive sort from its canonical lowercase name.
    ///
    /// Returns `None` for any unknown spelling (SPEC-024 CON-008).
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "symbol" => Some(PrimitiveSort::Symbol),
            "integer" => Some(PrimitiveSort::Integer),
            "decimal" => Some(PrimitiveSort::Decimal),
            "float" => Some(PrimitiveSort::Float),
            "number" => Some(PrimitiveSort::Number),
            "any" => Some(PrimitiveSort::Any),
            _ => None,
        }
    }

    /// The canonical lowercase name of this sort.
    pub fn name(self) -> &'static str {
        match self {
            PrimitiveSort::Symbol => "symbol",
            PrimitiveSort::Integer => "integer",
            PrimitiveSort::Decimal => "decimal",
            PrimitiveSort::Float => "float",
            PrimitiveSort::Number => "number",
            PrimitiveSort::Any => "any",
        }
    }

    /// Whether a ground [`Term`] satisfies this sort (SPEC-024 REQ-012).
    pub fn accepts_term(self, term: &Term) -> bool {
        match self {
            PrimitiveSort::Symbol => matches!(term, Term::Symbol(_)),
            PrimitiveSort::Integer => matches!(term, Term::Integer(_)),
            PrimitiveSort::Decimal => matches!(term, Term::Decimal(_)),
            PrimitiveSort::Float => matches!(term, Term::Float(_)),
            PrimitiveSort::Number => term.is_numeric(),
            PrimitiveSort::Any => true,
        }
    }

    /// Whether an [`ObservedArgumentKind`] is compatible with this sort.
    ///
    /// Variables and arithmetic expressions are never a match here; the caller
    /// reports them as deferred rather than as type matches (SPEC-024 REQ-012).
    pub fn accepts_kind(self, kind: ObservedArgumentKind) -> bool {
        match kind {
            ObservedArgumentKind::Variable | ObservedArgumentKind::ArithmeticExpression => false,
            ObservedArgumentKind::Symbol => {
                matches!(self, PrimitiveSort::Symbol | PrimitiveSort::Any)
            }
            ObservedArgumentKind::Integer => matches!(
                self,
                PrimitiveSort::Integer | PrimitiveSort::Number | PrimitiveSort::Any
            ),
            ObservedArgumentKind::Decimal => matches!(
                self,
                PrimitiveSort::Decimal | PrimitiveSort::Number | PrimitiveSort::Any
            ),
            ObservedArgumentKind::Float => matches!(
                self,
                PrimitiveSort::Float | PrimitiveSort::Number | PrimitiveSort::Any
            ),
        }
    }
}

impl fmt::Display for PrimitiveSort {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// An observed argument kind (SPEC-024 §3.12).
///
/// These are the kinds that a concrete argument occurrence can exhibit. They
/// are strictly finer-grained than [`PrimitiveSort`]: numeric kinds remain
/// distinct even though numeric matching is compatible.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObservedArgumentKind {
    /// A variable symbol (a `Term::Symbol` whose spelling begins with `?`).
    Variable,
    /// A non-variable symbol.
    Symbol,
    /// An integer term.
    Integer,
    /// A decimal term.
    Decimal,
    /// A float term.
    Float,
    /// An arithmetic expression argument (body literals only).
    ArithmeticExpression,
}

impl ObservedArgumentKind {
    /// The canonical lowercase kebab-case name of this kind (SPEC-024 CON-007).
    pub fn name(self) -> &'static str {
        match self {
            ObservedArgumentKind::Variable => "variable",
            ObservedArgumentKind::Symbol => "symbol",
            ObservedArgumentKind::Integer => "integer",
            ObservedArgumentKind::Decimal => "decimal",
            ObservedArgumentKind::Float => "float",
            ObservedArgumentKind::ArithmeticExpression => "arithmetic-expression",
        }
    }

    /// Parse an observed kind from its canonical kebab-case name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "variable" => Some(ObservedArgumentKind::Variable),
            "symbol" => Some(ObservedArgumentKind::Symbol),
            "integer" => Some(ObservedArgumentKind::Integer),
            "decimal" => Some(ObservedArgumentKind::Decimal),
            "float" => Some(ObservedArgumentKind::Float),
            "arithmetic-expression" => Some(ObservedArgumentKind::ArithmeticExpression),
            _ => None,
        }
    }

    /// Classify a concrete [`Term`] argument.
    ///
    /// A `Term::Symbol` whose resolved spelling begins with `?` is a variable
    /// under the existing lexical encoding (SPEC-024 CON-003).
    pub fn of_term(term: &Term) -> Self {
        match term {
            Term::Symbol(id) => {
                if crate::intern::resolve(*id).starts_with('?') {
                    ObservedArgumentKind::Variable
                } else {
                    ObservedArgumentKind::Symbol
                }
            }
            Term::Integer(_) => ObservedArgumentKind::Integer,
            Term::Decimal(_) => ObservedArgumentKind::Decimal,
            Term::Float(_) => ObservedArgumentKind::Float,
        }
    }

    /// Classify a body-literal argument, which may be an arithmetic expression.
    pub fn of_body_arg(arg: &BodyArg) -> Self {
        match arg {
            BodyArg::Term(t) => ObservedArgumentKind::of_term(t),
            BodyArg::Arith(_) => ObservedArgumentKind::ArithmeticExpression,
        }
    }
}

impl fmt::Display for ObservedArgumentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::intern;
    use rust_decimal::Decimal;

    #[test]
    fn sort_names_roundtrip() {
        for s in [
            PrimitiveSort::Symbol,
            PrimitiveSort::Integer,
            PrimitiveSort::Decimal,
            PrimitiveSort::Float,
            PrimitiveSort::Number,
            PrimitiveSort::Any,
        ] {
            assert_eq!(PrimitiveSort::from_name(s.name()), Some(s));
        }
        assert_eq!(PrimitiveSort::from_name("task"), None);
    }

    #[test]
    fn number_accepts_all_numeric_terms() {
        assert!(PrimitiveSort::Number.accepts_term(&Term::Integer(1)));
        assert!(PrimitiveSort::Number.accepts_term(&Term::Decimal(Decimal::ONE)));
        assert!(!PrimitiveSort::Number.accepts_term(&Term::Symbol(intern("x"))));
    }

    #[test]
    fn exact_numeric_sorts_are_narrow() {
        assert!(PrimitiveSort::Integer.accepts_term(&Term::Integer(1)));
        assert!(!PrimitiveSort::Integer.accepts_term(&Term::Decimal(Decimal::ONE)));
    }

    #[test]
    fn any_accepts_every_ground_term() {
        assert!(PrimitiveSort::Any.accepts_term(&Term::Symbol(intern("x"))));
        assert!(PrimitiveSort::Any.accepts_term(&Term::Integer(1)));
    }

    #[test]
    fn variable_symbol_classified_as_variable() {
        assert_eq!(
            ObservedArgumentKind::of_term(&Term::Symbol(intern("?X"))),
            ObservedArgumentKind::Variable
        );
        assert_eq!(
            ObservedArgumentKind::of_term(&Term::Symbol(intern("alice"))),
            ObservedArgumentKind::Symbol
        );
    }

    #[test]
    fn observed_kind_names_roundtrip() {
        for k in [
            ObservedArgumentKind::Variable,
            ObservedArgumentKind::Symbol,
            ObservedArgumentKind::Integer,
            ObservedArgumentKind::Decimal,
            ObservedArgumentKind::Float,
            ObservedArgumentKind::ArithmeticExpression,
        ] {
            assert_eq!(ObservedArgumentKind::from_name(k.name()), Some(k));
        }
    }
}
