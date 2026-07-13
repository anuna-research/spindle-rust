//! Predicate model, theory signatures, and vocabulary (SPEC-024).
//!
//! This module adds a structural predicate-level identity ([`PredicateSymbol`])
//! and the derived tooling projections built on top of it — signatures,
//! declarations, argument profiles, theory signatures, vocabularies, and
//! non-semantic shape validation. None of it changes defeasible inference; the
//! reasoner ignores declarations and predicate metadata during rule firing
//! (SPEC-024 ADR-004, REQ-013).

pub mod declaration;
pub mod phase;
pub mod profile;
pub mod shape;
pub mod signature;
pub mod sort;
pub mod symbol;
pub mod theory_signature;
pub mod vocab;

pub use declaration::{DeclarationOrigin, MetaTarget, PredicateDeclaration, SourceLocation};
pub use phase::{
    Bindings, ClassifiedLiteral, GroundLiteral, GroundingError, GroundnessError, LiteralPattern,
    is_variable_symbol,
};
pub use profile::ArgumentProfile;
pub use shape::{Shape, ShapeDiagnostic, ShapeReport};
pub use signature::{ArgumentDecl, PredicateSignature, PredicateSignatureError};
pub use sort::{ObservedArgumentKind, PrimitiveSort};
pub use symbol::{
    HasPredicateSymbol, PredicateIndicatorDisplay, PredicateSymbol, PredicateSymbolError,
};
pub use theory_signature::{DeclarationState, TheorySignature};
pub use vocab::{
    DerivationSummary, OccurrenceRole, PredicateOrigin, Vocabulary, VocabularyDiagnostic,
    VocabularyEntry, VocabularyReport,
};
