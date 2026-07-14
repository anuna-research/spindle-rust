//! Literal phase types (SPEC-024 CON-003).
//!
//! [`GroundLiteral`] and [`LiteralPattern`] are checked wrappers around the
//! existing [`Literal`] AST that make illegal phase transitions
//! unrepresentable. They are additive: existing consumers keep using `Literal`
//! directly (SPEC-024 ADR-003).

use crate::grounding::{Substitution, apply_substitution_to_literal};
use crate::intern::{SymbolId, resolve};
use crate::literal::Literal;
use crate::temporal::{TemporalExpr, TimeExpr};

/// Bindings supplied to [`LiteralPattern::ground`].
///
/// This is the existing grounding [`Substitution`]; the alias names its role at
/// the phase boundary (SPEC-024 CON-003).
pub type Bindings = Substitution;

/// The single shared recognizer for the lexical variable marker (SPEC-024
/// CON-003). A `Term::Symbol` whose resolved spelling begins with `?` is a
/// variable.
#[inline]
pub fn is_variable_symbol(id: SymbolId) -> bool {
    resolve(id).starts_with('?')
}

/// Whether a temporal expression still carries an unresolved (variable)
/// endpoint.
fn temporal_expr_is_unresolved(expr: &TemporalExpr) -> bool {
    matches!(expr.start, TimeExpr::Var(_)) || matches!(expr.end, TimeExpr::Var(_))
}

/// Error returned when a [`Literal`] cannot be wrapped as a [`GroundLiteral`]
/// (SPEC-024 §3.10, REQ-005).
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GroundnessError {
    /// The functor name is a variable symbol.
    #[error("literal functor is a variable symbol")]
    VariableFunctor,
    /// The argument at `position` is a variable symbol.
    #[error("argument at position {position} is a variable symbol")]
    VariableArgument {
        /// The zero-based offending position.
        position: u32,
    },
    /// The literal carries an unresolved interval variable.
    #[error("literal carries an unresolved interval variable")]
    IntervalVariable,
    /// The literal carries an unresolved temporal endpoint.
    #[error("literal carries an unresolved temporal endpoint")]
    UnresolvedTemporalEndpoint,
}

/// Error returned when grounding a [`LiteralPattern`] fails (SPEC-024 REQ-006).
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum GroundingError {
    /// The argument at `position` had no binding after substitution.
    #[error("argument at position {position} ({variable}) has no binding")]
    MissingBinding {
        /// The zero-based offending position.
        position: u32,
        /// The unbound variable's spelling.
        variable: String,
    },
    /// The functor variable had no binding after substitution.
    #[error("functor variable ({variable}) has no binding")]
    MissingFunctorBinding {
        /// The unbound functor variable's spelling.
        variable: String,
    },
    /// The functor variable was bound, but to a term that cannot serve as a
    /// functor (only a symbol term can), so substitution left it unchanged.
    #[error("functor variable ({variable}) is bound to a non-symbol term")]
    IncompatibleFunctorBinding {
        /// The functor variable's spelling.
        variable: String,
    },
    /// A temporal endpoint remained unresolved after substitution.
    #[error("a temporal endpoint remained unresolved after grounding")]
    UnresolvedTemporal,
    /// An interval variable remained unbound after substitution.
    #[error("an interval variable remained unbound after grounding")]
    UnboundInterval,
}

/// Determine the [`GroundnessError`], if any, for a literal.
fn groundness_error(lit: &Literal) -> Option<GroundnessError> {
    if is_variable_symbol(lit.name_id()) {
        return Some(GroundnessError::VariableFunctor);
    }
    for (position, arg) in lit.predicate_args().iter().enumerate() {
        if let crate::term::Term::Symbol(id) = arg
            && is_variable_symbol(*id)
        {
            return Some(GroundnessError::VariableArgument {
                position: position as u32,
            });
        }
    }
    if lit.interval_var.is_some() {
        return Some(GroundnessError::IntervalVariable);
    }
    if let Some(expr) = &lit.temporal_expr
        && temporal_expr_is_unresolved(expr)
    {
        return Some(GroundnessError::UnresolvedTemporalEndpoint);
    }
    None
}

/// A ground literal: no variable symbols, arithmetic argument expressions,
/// interval variable, or unresolved temporal endpoints (SPEC-024 §3.10).
///
/// The wrapper proves this invariant at construction time. (A [`Literal`]
/// cannot hold arithmetic arguments; those live only in
/// [`BodyLogicLiteral`](crate::body::BodyLogicLiteral).)
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GroundLiteral(Literal);

impl GroundLiteral {
    /// Borrow the underlying literal.
    #[inline]
    pub fn as_literal(&self) -> &Literal {
        &self.0
    }

    /// Consume the wrapper, returning the underlying literal.
    #[inline]
    pub fn into_literal(self) -> Literal {
        self.0
    }
}

impl TryFrom<Literal> for GroundLiteral {
    type Error = GroundnessError;

    fn try_from(lit: Literal) -> Result<Self, Self::Error> {
        match groundness_error(&lit) {
            Some(err) => Err(err),
            None => Ok(GroundLiteral(lit)),
        }
    }
}

/// A literal pattern: an application that still requires grounding (SPEC-024
/// §3.9). It contains at least one variable or unresolved temporal endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteralPattern(Literal);

impl LiteralPattern {
    /// Borrow the underlying literal.
    #[inline]
    pub fn as_literal(&self) -> &Literal {
        &self.0
    }

    /// Ground this pattern under `bindings`, returning a checked
    /// [`GroundLiteral`] (SPEC-024 REQ-006).
    ///
    /// Complete bindings produce a ground wrapper; missing bindings produce a
    /// positional error. No success value requires a second groundness scan by
    /// the caller.
    pub fn ground(self, bindings: &Bindings) -> Result<GroundLiteral, GroundingError> {
        let grounded = apply_substitution_to_literal(&self.0, bindings);
        match groundness_error(&grounded) {
            None => Ok(GroundLiteral(grounded)),
            Some(GroundnessError::VariableFunctor) => {
                let variable = resolve(grounded.name_id()).to_string();
                // Substitution replaces a variable functor only with a symbol
                // binding; a binding of any other term kind leaves the functor
                // variable in place. Distinguish that invalid data from an
                // omitted binding (SPEC-024 REQ-006).
                match bindings.terms.get(&grounded.name_id()) {
                    Some(term) if !matches!(term, crate::term::Term::Symbol(_)) => {
                        Err(GroundingError::IncompatibleFunctorBinding { variable })
                    }
                    _ => Err(GroundingError::MissingFunctorBinding { variable }),
                }
            }
            Some(GroundnessError::VariableArgument { position }) => {
                let variable = match grounded.predicate_args().get(position as usize) {
                    Some(crate::term::Term::Symbol(id)) => resolve(*id).to_string(),
                    _ => String::new(),
                };
                Err(GroundingError::MissingBinding { position, variable })
            }
            Some(GroundnessError::IntervalVariable) => Err(GroundingError::UnboundInterval),
            Some(GroundnessError::UnresolvedTemporalEndpoint) => {
                Err(GroundingError::UnresolvedTemporal)
            }
        }
    }
}

/// The result of classifying a [`Literal`] into exactly one phase (SPEC-024
/// REQ-004).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassifiedLiteral {
    /// The literal still requires grounding.
    Pattern(LiteralPattern),
    /// The literal is ground.
    Ground(GroundLiteral),
}

impl Literal {
    /// Classify this literal as ground or a pattern (SPEC-024 REQ-004).
    ///
    /// This is total: every literal lands in exactly one variant.
    pub fn classify(self) -> ClassifiedLiteral {
        match groundness_error(&self) {
            None => ClassifiedLiteral::Ground(GroundLiteral(self)),
            Some(_) => ClassifiedLiteral::Pattern(LiteralPattern(self)),
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

    #[test]
    fn ground_literal_from_symbol_args() {
        let lit = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["a".to_string(), "b".to_string()],
        );
        assert!(GroundLiteral::try_from(lit).is_ok());
    }

    #[test]
    fn variable_argument_rejected() {
        let lit = Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?X"))],
        );
        assert_eq!(
            GroundLiteral::try_from(lit),
            Err(GroundnessError::VariableArgument { position: 0 })
        );
    }

    #[test]
    fn interval_variable_rejected() {
        let mut lit = Literal::simple("p");
        lit.interval_var = Some(intern("?T"));
        assert_eq!(
            GroundLiteral::try_from(lit),
            Err(GroundnessError::IntervalVariable)
        );
    }

    #[test]
    fn classify_is_total_and_exclusive() {
        let ground = Literal::simple("p");
        assert!(matches!(ground.classify(), ClassifiedLiteral::Ground(_)));

        let pattern = Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?X"))],
        );
        assert!(matches!(pattern.classify(), ClassifiedLiteral::Pattern(_)));
    }

    #[test]
    fn ground_resolves_bindings() {
        let pattern = match Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?X"))],
        )
        .classify()
        {
            ClassifiedLiteral::Pattern(p) => p,
            _ => panic!("expected pattern"),
        };

        let mut bindings = Bindings::default();
        bindings
            .terms
            .insert(intern("?X"), Term::Symbol(intern("a")));
        let ground = pattern.ground(&bindings).unwrap();
        assert_eq!(
            ground.as_literal().predicate_args()[0],
            Term::Symbol(intern("a"))
        );
    }

    #[test]
    fn ground_distinguishes_incompatible_functor_binding() {
        let make_pattern = || match Literal::from_ids(
            intern("?P"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![],
        )
        .classify()
        {
            ClassifiedLiteral::Pattern(p) => p,
            _ => panic!("expected pattern"),
        };

        // A binding exists but has the wrong type: incompatible, not missing.
        let mut bindings = Bindings::default();
        bindings.terms.insert(intern("?P"), Term::Integer(1));
        assert_eq!(
            make_pattern().ground(&bindings).unwrap_err(),
            GroundingError::IncompatibleFunctorBinding {
                variable: "?P".to_string()
            }
        );

        // No binding at all: missing.
        assert_eq!(
            make_pattern().ground(&Bindings::default()).unwrap_err(),
            GroundingError::MissingFunctorBinding {
                variable: "?P".to_string()
            }
        );
    }

    #[test]
    fn ground_reports_missing_binding_position() {
        let pattern = match Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("a")), Term::Symbol(intern("?Y"))],
        )
        .classify()
        {
            ClassifiedLiteral::Pattern(p) => p,
            _ => panic!("expected pattern"),
        };

        let bindings = Bindings::default();
        let err = pattern.ground(&bindings).unwrap_err();
        assert_eq!(
            err,
            GroundingError::MissingBinding {
                position: 1,
                variable: "?Y".to_string()
            }
        );
    }
}
