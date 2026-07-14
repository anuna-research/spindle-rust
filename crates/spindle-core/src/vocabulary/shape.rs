//! Non-semantic shape validation (SPEC-024 CON-006, REQ-012).
//!
//! A [`Shape`] is compiled from a [`PredicateSignature`] and validates arity and
//! statically known argument sorts. Shape validation is entirely separate from
//! inference: the reasoner never consults a shape (SPEC-024 ADR-004).

use super::signature::PredicateSignature;
use super::sort::{ObservedArgumentKind, PrimitiveSort};
use super::symbol::{HasPredicateSymbol, PredicateSymbol, PredicateSymbolError};
use crate::body::BodyLogicLiteral;
use crate::literal::Literal;

/// A single shape-validation finding (SPEC-024 CON-006).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShapeDiagnostic {
    /// The application's predicate symbol does not match the shape's predicate
    /// (a functor or arity mismatch).
    PredicateMismatch {
        /// The shape's predicate symbol.
        expected: PredicateSymbol,
        /// The application's predicate symbol.
        actual: PredicateSymbol,
    },
    /// The argument at `position` has a kind incompatible with its declared sort.
    SortMismatch {
        /// The zero-based argument position.
        position: u32,
        /// The declared sort.
        expected: PrimitiveSort,
        /// The observed argument kind.
        actual: ObservedArgumentKind,
    },
    /// The argument at `position` is unresolved (a variable or arithmetic
    /// expression) and is deferred rather than reported as a type match.
    Deferred {
        /// The zero-based argument position.
        position: u32,
        /// The observed argument kind.
        actual: ObservedArgumentKind,
    },
}

/// A collection of shape-validation findings (SPEC-024 CON-006).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShapeReport {
    /// The findings, in ascending argument position.
    pub diagnostics: Vec<ShapeDiagnostic>,
}

impl ShapeReport {
    /// Whether the report contains no diagnostics.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// The number of genuine mismatches — predicate or sort — excluding
    /// [`ShapeDiagnostic::Deferred`] positions, which are not problems but
    /// arguments (variables, arithmetic expressions) that cannot be validated
    /// until grounding (SPEC-024 OBS-001). Use this for problem counts so an
    /// ordinary rule with variable arguments is not reported as diagnostic noise.
    pub fn mismatch_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| !matches!(d, ShapeDiagnostic::Deferred { .. }))
            .count()
    }
}

/// A set of validation constraints compiled from a predicate signature
/// (SPEC-024 §3.13).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shape {
    /// The predicate this shape validates.
    pub predicate: PredicateSymbol,
    /// The declared sort for each argument position, in order.
    pub arguments: Box<[PrimitiveSort]>,
}

impl Shape {
    /// Validate a [`Literal`] application against this shape (SPEC-024 REQ-012).
    pub fn validate(&self, literal: &Literal) -> Result<ShapeReport, PredicateSymbolError> {
        let actual = literal.predicate_symbol()?;
        let kinds: Vec<ObservedArgumentKind> = literal
            .predicate_args()
            .iter()
            .map(ObservedArgumentKind::of_term)
            .collect();
        Ok(self.validate_kinds(actual, &kinds))
    }

    /// Validate a [`BodyLogicLiteral`] application, which may carry arithmetic
    /// argument expressions (reported as deferred).
    pub fn validate_body(
        &self,
        literal: &BodyLogicLiteral,
    ) -> Result<ShapeReport, PredicateSymbolError> {
        let actual = literal.predicate_symbol()?;
        let kinds: Vec<ObservedArgumentKind> = literal
            .predicate_args()
            .iter()
            .map(ObservedArgumentKind::of_body_arg)
            .collect();
        Ok(self.validate_kinds(actual, &kinds))
    }

    /// Validate an application described by its actual symbol and per-position
    /// observed kinds (SPEC-024 REQ-012). Shared by [`Self::validate`],
    /// [`Self::validate_body`], and vocabulary shape counting.
    pub(crate) fn validate_kinds(
        &self,
        actual: PredicateSymbol,
        kinds: &[ObservedArgumentKind],
    ) -> ShapeReport {
        // Arity is part of the symbol, so a wrong arity is a predicate mismatch.
        if actual != self.predicate {
            return ShapeReport {
                diagnostics: vec![ShapeDiagnostic::PredicateMismatch {
                    expected: self.predicate,
                    actual,
                }],
            };
        }

        let mut diagnostics = Vec::new();
        for (position, (&expected, &actual_kind)) in
            self.arguments.iter().zip(kinds.iter()).enumerate()
        {
            let position = position as u32;
            match actual_kind {
                ObservedArgumentKind::Variable | ObservedArgumentKind::ArithmeticExpression => {
                    diagnostics.push(ShapeDiagnostic::Deferred {
                        position,
                        actual: actual_kind,
                    });
                }
                _ if expected.accepts_kind(actual_kind) => {}
                _ => diagnostics.push(ShapeDiagnostic::SortMismatch {
                    position,
                    expected,
                    actual: actual_kind,
                }),
            }
        }
        ShapeReport { diagnostics }
    }
}

impl From<&PredicateSignature> for Shape {
    /// Compile a shape from a checked signature. Infallible because signature
    /// construction has already checked arity (SPEC-024 CON-006).
    fn from(signature: &PredicateSignature) -> Self {
        Shape {
            predicate: signature.symbol,
            arguments: signature
                .arguments
                .iter()
                .map(|a| a.sort)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::intern;
    use crate::literal::InternedLiteralName;
    use crate::mode::Mode;
    use crate::temporal::Temporal;
    use crate::term::Term;
    use crate::vocabulary::signature::ArgumentDecl;

    fn sym(name: &str, arity: usize) -> PredicateSymbol {
        PredicateSymbol::try_new(InternedLiteralName::intern(name), arity).unwrap()
    }

    fn shape(name: &str, sorts: Vec<PrimitiveSort>) -> Shape {
        let args: Vec<ArgumentDecl> = sorts
            .iter()
            .enumerate()
            .map(|(i, s)| ArgumentDecl::new(format!("a{i}"), *s))
            .collect();
        let sig = PredicateSignature::try_new(sym(name, sorts.len()), args).unwrap();
        Shape::from(&sig)
    }

    #[test]
    fn number_accepts_all_numeric() {
        let s = shape("p", vec![PrimitiveSort::Number]);
        let lit = Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Integer(1)],
        );
        assert!(s.validate(&lit).unwrap().is_empty());
    }

    #[test]
    fn exact_sort_mismatch_reported() {
        let s = shape("p", vec![PrimitiveSort::Integer]);
        let lit = Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("x"))],
        );
        let report = s.validate(&lit).unwrap();
        assert_eq!(
            report.diagnostics,
            vec![ShapeDiagnostic::SortMismatch {
                position: 0,
                expected: PrimitiveSort::Integer,
                actual: ObservedArgumentKind::Symbol,
            }]
        );
    }

    #[test]
    fn wrong_arity_is_predicate_mismatch() {
        let s = shape("p", vec![PrimitiveSort::Any]);
        let lit = Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Integer(1), Term::Integer(2)],
        );
        let report = s.validate(&lit).unwrap();
        assert!(matches!(
            report.diagnostics.as_slice(),
            [ShapeDiagnostic::PredicateMismatch { .. }]
        ));
    }

    #[test]
    fn variable_is_deferred_not_matched() {
        let s = shape("p", vec![PrimitiveSort::Integer]);
        let lit = Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?X"))],
        );
        let report = s.validate(&lit).unwrap();
        assert_eq!(
            report.diagnostics,
            vec![ShapeDiagnostic::Deferred {
                position: 0,
                actual: ObservedArgumentKind::Variable,
            }]
        );
    }

    #[test]
    fn mismatch_count_excludes_deferred() {
        // One real sort mismatch and one deferred variable: only the mismatch counts.
        let s = shape("p", vec![PrimitiveSort::Integer, PrimitiveSort::Integer]);
        let lit = Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("x")), Term::Symbol(intern("?Y"))],
        );
        let report = s.validate(&lit).unwrap();
        assert_eq!(report.diagnostics.len(), 2);
        assert_eq!(report.mismatch_count(), 1);
    }

    #[test]
    fn mismatch_count_zero_for_all_variables() {
        let s = shape("p", vec![PrimitiveSort::Integer, PrimitiveSort::Symbol]);
        let lit = Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("?X")), Term::Symbol(intern("?Y"))],
        );
        assert_eq!(s.validate(&lit).unwrap().mismatch_count(), 0);
    }

    #[test]
    fn any_accepts_all_ground() {
        let s = shape("p", vec![PrimitiveSort::Any, PrimitiveSort::Any]);
        let lit = Literal::from_ids(
            intern("p"),
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![Term::Symbol(intern("x")), Term::Integer(1)],
        );
        assert!(s.validate(&lit).unwrap().is_empty());
    }
}
