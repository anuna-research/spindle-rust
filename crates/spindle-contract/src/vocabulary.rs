//! Structured serialization DTOs for the predicate vocabulary (SPEC-024
//! CON-007, REQ-014).
//!
//! These DTOs are additive to the typed V2 contract. Functor and arity are
//! always separate structured fields; a predicate-indicator string is never a
//! machine key. Enum strings are lowercase kebab case; unknown enum strings and
//! inconsistent arities are rejected rather than preserved (SPEC-024 CON-007).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use spindle_core::vocabulary::{
    ArgumentDecl, DeclarationOrigin, DeclarationState, MetaTarget, ObservedArgumentKind,
    OccurrenceRole, PredicateOrigin, PredicateSignature, PredicateSymbol, PredicateSymbolError,
    PrimitiveSort, VocabularyDiagnostic, VocabularyEntry, VocabularyReport,
};

/// The schema identifier for a serialized vocabulary (SPEC-024 CON-007).
pub const VOCABULARY_SCHEMA: &str = "spindle.vocabulary/1";

/// Errors raised when validating a deserialized vocabulary DTO.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum VocabularyDtoError {
    /// The schema field did not match the expected version.
    #[error("unknown vocabulary schema {found:?}, expected {VOCABULARY_SCHEMA:?}")]
    UnknownSchema {
        /// The schema string found in the payload.
        found: String,
    },
    /// A primitive-sort string was not recognized.
    #[error("unknown primitive sort {found:?}")]
    UnknownSort {
        /// The unrecognized sort string.
        found: String,
    },
    /// An observed-kind string was not recognized.
    #[error("unknown observed argument kind {found:?}")]
    UnknownKind {
        /// The unrecognized kind string.
        found: String,
    },
    /// An occurrence-role string was not recognized.
    #[error("unknown occurrence role {found:?}")]
    UnknownRole {
        /// The unrecognized role string.
        found: String,
    },
    /// A profile or signature had a length inconsistent with the symbol arity.
    #[error(
        "entry for {functor:?}/{arity} has an inconsistent array length {found} (expected {arity})"
    )]
    InconsistentArity {
        /// The entry's functor.
        functor: String,
        /// The declared arity.
        arity: u32,
        /// The offending array length.
        found: usize,
    },
    /// An entry's declaration fields describe a state no core
    /// [`DeclarationState`] can represent, such as both a coherent signature
    /// and a conflict payload.
    #[error("entry for {functor:?}/{arity} has contradictory declaration fields")]
    ContradictoryDeclaration {
        /// The entry's functor.
        functor: String,
        /// The declared arity.
        arity: u32,
    },
    /// A signature argument name was empty.
    #[error("entry for {functor:?}/{arity} has an empty argument name at position {position}")]
    EmptyArgumentName {
        /// The entry's functor.
        functor: String,
        /// The declared arity.
        arity: u32,
        /// The zero-based offending position.
        position: usize,
    },
    /// Two signature arguments share a name.
    #[error(
        "entry for {functor:?}/{arity} duplicates argument name {name:?} at positions {first} and {second}"
    )]
    DuplicateArgumentName {
        /// The entry's functor.
        functor: String,
        /// The declared arity.
        arity: u32,
        /// The duplicated name.
        name: String,
        /// The first position holding the name.
        first: usize,
        /// The second position holding the name.
        second: usize,
    },
    /// An arity could not be represented.
    #[error("arity {found} exceeds u32::MAX")]
    ArityOverflow {
        /// The offending arity.
        found: usize,
    },
    /// A predicate functor was empty.
    #[error("predicate functor is empty")]
    EmptyFunctor,
    /// A predicate functor contained a control character.
    #[error("predicate functor contains a control character {ch:?}")]
    ControlFunctor {
        /// The offending character.
        ch: char,
    },
}

/// A predicate symbol in structured form (SPEC-024 CON-002).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PredicateSymbolDto {
    /// The functor, possibly containing `/`.
    pub functor: String,
    /// The argument count.
    pub arity: u32,
}

impl From<PredicateSymbol> for PredicateSymbolDto {
    fn from(symbol: PredicateSymbol) -> Self {
        Self {
            functor: symbol.functor().resolve().to_string(),
            arity: symbol.arity(),
        }
    }
}

impl TryFrom<&PredicateSymbolDto> for PredicateSymbol {
    type Error = VocabularyDtoError;

    fn try_from(dto: &PredicateSymbolDto) -> Result<Self, Self::Error> {
        PredicateSymbol::try_new(dto.functor.as_str().into(), dto.arity as usize)
            .map_err(map_symbol_error)
    }
}

/// Map a core [`PredicateSymbolError`] to its DTO counterpart, preserving the
/// distinction between an arity overflow and a malformed functor rather than
/// reporting every failure as an overflow.
fn map_symbol_error(error: PredicateSymbolError) -> VocabularyDtoError {
    match error {
        PredicateSymbolError::ArityOverflow { actual } => {
            VocabularyDtoError::ArityOverflow { found: actual }
        }
        PredicateSymbolError::EmptyFunctor => VocabularyDtoError::EmptyFunctor,
        PredicateSymbolError::ControlCharacter { ch } => VocabularyDtoError::ControlFunctor { ch },
    }
}

/// A structured metadata target (SPEC-024 CON-007).
///
/// A predicate target serializes as
/// `{"kind":"predicate","symbol":{...}}`, never an indicator string.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum MetaTargetDto {
    /// A rule-label target.
    Label {
        /// The label string.
        label: String,
    },
    /// A predicate-symbol target.
    Predicate {
        /// The predicate symbol.
        symbol: PredicateSymbolDto,
    },
}

impl From<&MetaTarget> for MetaTargetDto {
    fn from(target: &MetaTarget) -> Self {
        match target {
            MetaTarget::Label(l) => MetaTargetDto::Label { label: l.clone() },
            MetaTarget::Predicate(sym) => MetaTargetDto::Predicate {
                symbol: (*sym).into(),
            },
        }
    }
}

/// One argument declaration in structured form.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ArgumentDeclDto {
    /// The argument name.
    pub name: String,
    /// The primitive sort, as a lowercase name.
    pub sort: String,
}

impl From<&ArgumentDecl> for ArgumentDeclDto {
    fn from(decl: &ArgumentDecl) -> Self {
        Self {
            name: decl.name.clone(),
            sort: decl.sort.name().to_string(),
        }
    }
}

/// A predicate signature in structured form.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SignatureDto {
    /// Ordered argument declarations.
    pub arguments: Vec<ArgumentDeclDto>,
}

/// The origin of one predicate declaration in structured form (SPEC-024
/// CON-007). Unknown kinds are rejected at deserialization by the serde tag.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum DeclarationOriginDto {
    /// Parsed from source at the given location.
    Parsed {
        /// Byte offset of the declaration in the cleaned source.
        byte_offset: usize,
        /// One-based line number of the declaration.
        line: usize,
    },
    /// Added programmatically with no source location.
    Programmatic,
}

impl From<DeclarationOrigin> for DeclarationOriginDto {
    fn from(origin: DeclarationOrigin) -> Self {
        match origin {
            DeclarationOrigin::Parsed(loc) => DeclarationOriginDto::Parsed {
                byte_offset: loc.byte_offset,
                line: loc.line,
            },
            DeclarationOrigin::Programmatic => DeclarationOriginDto::Programmatic,
        }
    }
}

/// One conflicting declaration, retaining its signature and origin so consumers
/// can inspect why a symbol's declarations conflict (SPEC-024 CON-007).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConflictingDeclarationDto {
    /// The declared signature.
    pub signature: SignatureDto,
    /// Where the declaration came from.
    pub origin: DeclarationOriginDto,
}

/// An occurrence origin in structured form.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct OriginDto {
    /// The rule label.
    pub rule: String,
    /// The role: `head` or `body`.
    pub role: String,
    /// The zero-based occurrence index.
    pub index: u32,
}

/// A vocabulary entry in structured form (SPEC-024 CON-007).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VocabularyEntryDto {
    /// The predicate symbol.
    pub symbol: PredicateSymbolDto,
    /// The signature, present only when the symbol has a single coherent
    /// declaration.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signature: Option<SignatureDto>,
    /// Every distinct origin of the coherent declaration, so a coherent
    /// declaration keeps the same provenance a conflicting one does (SPEC-024
    /// CON-007). Serialization always emits it alongside `signature`; a
    /// `spindle.vocabulary/1` document may omit it when declaration provenance
    /// is unknown, but it never appears without `signature`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub declaration_origins: Option<Vec<DeclarationOriginDto>>,
    /// Every distinct conflicting declaration, present only when the symbol's
    /// declarations conflict (SPEC-024 CON-007).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub conflict: Option<Vec<ConflictingDeclarationDto>>,
    /// Observed argument kinds, one lowercase-kebab list per position.
    pub profile: Vec<Vec<String>>,
    /// A joined description, if any.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    /// Sorted occurrence provenance.
    pub origins: Vec<OriginDto>,
}

/// A vocabulary diagnostic in structured form.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum VocabularyDiagnosticDto {
    /// The symbol has incompatible declarations.
    ConflictingDeclaration {
        /// The affected symbol.
        symbol: PredicateSymbolDto,
    },
    /// The symbol is used but not declared.
    UndeclaredPredicate {
        /// The affected symbol.
        symbol: PredicateSymbolDto,
    },
    /// Predicate metadata resolved to no symbol.
    UnresolvedPredicateMetadata {
        /// The affected symbol.
        symbol: PredicateSymbolDto,
    },
    /// A functor that cannot form a valid predicate symbol (empty or containing a
    /// control character), reported rather than dropped.
    MalformedPredicate {
        /// The rejected functor spelling.
        functor: String,
        /// The application arity.
        arity: u64,
        /// Sorted provenance of every occurrence with this functor and arity.
        origins: Vec<OriginDto>,
    },
}

impl From<&VocabularyDiagnostic> for VocabularyDiagnosticDto {
    fn from(diagnostic: &VocabularyDiagnostic) -> Self {
        match diagnostic {
            VocabularyDiagnostic::ConflictingDeclaration { symbol } => {
                VocabularyDiagnosticDto::ConflictingDeclaration {
                    symbol: (*symbol).into(),
                }
            }
            VocabularyDiagnostic::UndeclaredPredicate { symbol } => {
                VocabularyDiagnosticDto::UndeclaredPredicate {
                    symbol: (*symbol).into(),
                }
            }
            VocabularyDiagnostic::UnresolvedPredicateMetadata { symbol } => {
                VocabularyDiagnosticDto::UnresolvedPredicateMetadata {
                    symbol: (*symbol).into(),
                }
            }
            VocabularyDiagnostic::MalformedPredicate {
                functor,
                arity,
                origins,
            } => VocabularyDiagnosticDto::MalformedPredicate {
                functor: functor.clone(),
                arity: *arity as u64,
                origins: origins.iter().map(origin_to_dto).collect(),
            },
        }
    }
}

/// A complete serialized vocabulary report (SPEC-024 CON-007).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VocabularyReportDto {
    /// The schema identifier.
    pub schema: String,
    /// Entries in `(functor code-point, arity)` order.
    pub entries: Vec<VocabularyEntryDto>,
    /// Deterministically ordered diagnostics.
    pub diagnostics: Vec<VocabularyDiagnosticDto>,
}

impl From<&VocabularyReport> for VocabularyReportDto {
    fn from(report: &VocabularyReport) -> Self {
        let entries = report.vocabulary.entries.iter().map(entry_to_dto).collect();
        let diagnostics = report
            .diagnostics
            .iter()
            .map(VocabularyDiagnosticDto::from)
            .collect();
        Self {
            schema: VOCABULARY_SCHEMA.to_string(),
            entries,
            diagnostics,
        }
    }
}

impl VocabularyReportDto {
    /// Validate a deserialized report, rejecting unknown enum strings,
    /// inconsistent arities, and invalid schema versions (SPEC-024 REQ-014).
    pub fn validate(&self) -> Result<(), VocabularyDtoError> {
        if self.schema != VOCABULARY_SCHEMA {
            return Err(VocabularyDtoError::UnknownSchema {
                found: self.schema.clone(),
            });
        }
        for entry in &self.entries {
            // Reject a functor the core constructor would reject — empty or
            // containing a control character — so validation never accepts a
            // core-invalid symbol. The string invariant is checked directly so a
            // long-running validator never interns (and thus never permanently
            // leaks) transient DTO functors (SPEC-024 CON-001).
            PredicateSymbol::validate_functor(&entry.symbol.functor).map_err(map_symbol_error)?;
            let arity = entry.symbol.arity;
            if entry.profile.len() as u64 != u64::from(arity) {
                return Err(VocabularyDtoError::InconsistentArity {
                    functor: entry.symbol.functor.clone(),
                    arity,
                    found: entry.profile.len(),
                });
            }
            // No core `DeclarationState` is simultaneously coherent and
            // conflicting, and declaration origins belong to a coherent
            // signature; reject field combinations no core state represents.
            // A signature without `declaration_origins` stays valid: the
            // documented `spindle.vocabulary/1` contract (SPEC-024 CON-007)
            // lets a producer omit provenance it does not have.
            if (entry.signature.is_some() && entry.conflict.is_some())
                || (entry.declaration_origins.is_some() && entry.signature.is_none())
            {
                return Err(VocabularyDtoError::ContradictoryDeclaration {
                    functor: entry.symbol.functor.clone(),
                    arity,
                });
            }
            if let Some(sig) = &entry.signature {
                validate_signature(sig, &entry.symbol)?;
            }
            for conflict in entry.conflict.iter().flatten() {
                validate_signature(&conflict.signature, &entry.symbol)?;
            }
            for position in &entry.profile {
                for kind in position {
                    ObservedArgumentKind::from_name(kind).ok_or_else(|| {
                        VocabularyDtoError::UnknownKind {
                            found: kind.clone(),
                        }
                    })?;
                }
            }
            validate_origins(&entry.origins)?;
        }
        for diagnostic in &self.diagnostics {
            match diagnostic {
                VocabularyDiagnosticDto::ConflictingDeclaration { symbol }
                | VocabularyDiagnosticDto::UndeclaredPredicate { symbol }
                | VocabularyDiagnosticDto::UnresolvedPredicateMetadata { symbol } => {
                    PredicateSymbol::validate_functor(&symbol.functor).map_err(map_symbol_error)?;
                }
                // A malformed-predicate functor is invalid by definition — that
                // is the diagnostic's point — so only its origins are checked.
                VocabularyDiagnosticDto::MalformedPredicate { origins, .. } => {
                    validate_origins(origins)?;
                }
            }
        }
        Ok(())
    }
}

/// Check a signature's arity consistency, sort names, and argument-name
/// invariants against its symbol.
///
/// Argument names are checked exactly as [`PredicateSignature::try_new`] checks
/// them — non-empty and unique — so a validated DTO never contains a signature
/// the core model cannot represent (SPEC-024 REQ-008).
fn validate_signature(
    sig: &SignatureDto,
    symbol: &PredicateSymbolDto,
) -> Result<(), VocabularyDtoError> {
    if sig.arguments.len() as u64 != u64::from(symbol.arity) {
        return Err(VocabularyDtoError::InconsistentArity {
            functor: symbol.functor.clone(),
            arity: symbol.arity,
            found: sig.arguments.len(),
        });
    }
    // Track each name's first position so duplicate detection stays linear;
    // this untrusted path has no argument-count bound the SPL parser has, so a
    // rescan of preceding arguments per position would be Θ(n²) on crafted
    // payloads.
    let mut seen: HashMap<&str, usize> = HashMap::with_capacity(sig.arguments.len());
    for (position, arg) in sig.arguments.iter().enumerate() {
        PrimitiveSort::from_name(&arg.sort).ok_or_else(|| VocabularyDtoError::UnknownSort {
            found: arg.sort.clone(),
        })?;
        if arg.name.is_empty() {
            return Err(VocabularyDtoError::EmptyArgumentName {
                functor: symbol.functor.clone(),
                arity: symbol.arity,
                position,
            });
        }
        if let Some(&first) = seen.get(arg.name.as_str()) {
            return Err(VocabularyDtoError::DuplicateArgumentName {
                functor: symbol.functor.clone(),
                arity: symbol.arity,
                name: arg.name.clone(),
                first,
                second: position,
            });
        }
        seen.insert(arg.name.as_str(), position);
    }
    Ok(())
}

/// Check that every occurrence origin carries a recognized role.
fn validate_origins(origins: &[OriginDto]) -> Result<(), VocabularyDtoError> {
    for origin in origins {
        if origin.role != "head" && origin.role != "body" {
            return Err(VocabularyDtoError::UnknownRole {
                found: origin.role.clone(),
            });
        }
    }
    Ok(())
}

/// Convert one occurrence origin to its structured form.
fn origin_to_dto(origin: &PredicateOrigin) -> OriginDto {
    let (role, index) = match origin.role {
        OccurrenceRole::Head { index } => ("head", index),
        OccurrenceRole::Body { index } => ("body", index),
    };
    OriginDto {
        rule: origin.rule.clone(),
        role: role.to_string(),
        index,
    }
}

/// Convert a core signature to its structured form.
fn signature_to_dto(signature: &PredicateSignature) -> SignatureDto {
    SignatureDto {
        arguments: signature
            .arguments
            .iter()
            .map(ArgumentDeclDto::from)
            .collect(),
    }
}

fn entry_to_dto(entry: &VocabularyEntry) -> VocabularyEntryDto {
    let (signature, declaration_origins, conflict) = match &entry.declaration {
        Some(DeclarationState::Declared { signature, origins }) => (
            Some(signature_to_dto(signature)),
            Some(
                origins
                    .iter()
                    .map(|o| DeclarationOriginDto::from(*o))
                    .collect(),
            ),
            None,
        ),
        Some(DeclarationState::Conflict(decls)) => (
            None,
            None,
            Some(
                decls
                    .iter()
                    .map(|d| ConflictingDeclarationDto {
                        signature: signature_to_dto(&d.signature),
                        origin: d.origin.into(),
                    })
                    .collect(),
            ),
        ),
        None => (None, None, None),
    };
    let profile = entry
        .profile
        .positions
        .iter()
        .map(|kinds| kinds.iter().map(|k| k.name().to_string()).collect())
        .collect();
    let origins = entry.origins.iter().map(origin_to_dto).collect();
    VocabularyEntryDto {
        symbol: entry.symbol.into(),
        signature,
        declaration_origins,
        conflict,
        profile,
        description: entry.description.clone(),
        origins,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spindle_core::Theory;
    use spindle_core::vocabulary::{
        ArgumentDecl, DeclarationOrigin, MetaTarget, PredicateDeclaration, PredicateSignature,
        PredicateSymbol, PrimitiveSort, Vocabulary,
    };

    fn sym(name: &str, arity: usize) -> PredicateSymbol {
        PredicateSymbol::try_new(name.into(), arity).unwrap()
    }

    fn assign_theory() -> Theory {
        let mut theory = Theory::new();
        // r1: head assign-to(?t, ?a)
        theory.add_defeasible_rule(&["request"], "assign-to");
        let sig = PredicateSignature::try_new(
            sym("assign-to", 2),
            vec![
                ArgumentDecl::new("task", PrimitiveSort::Symbol),
                ArgumentDecl::new("agent", PrimitiveSort::Symbol),
            ],
        )
        .unwrap();
        theory.add_predicate_declaration(PredicateDeclaration::new(
            sig,
            DeclarationOrigin::Programmatic,
        ));
        theory.add_meta_target(
            MetaTarget::Predicate(sym("assign-to", 2)),
            "description",
            spindle_core::MetaValue::String("Assign a task to an agent.".to_string()),
        );
        theory
    }

    #[test]
    fn roundtrips_report_with_slash_functor() {
        let mut theory = Theory::new();
        let sig = PredicateSignature::try_new(
            sym("rate/limit", 1),
            vec![ArgumentDecl::new("key", PrimitiveSort::Symbol)],
        )
        .unwrap();
        theory.add_predicate_declaration(PredicateDeclaration::new(
            sig,
            DeclarationOrigin::Programmatic,
        ));
        let report = Vocabulary::derive(&theory);
        let dto = VocabularyReportDto::from(&report);

        let json = serde_json::to_string(&dto).unwrap();
        let back: VocabularyReportDto = serde_json::from_str(&json).unwrap();
        back.validate().unwrap();
        assert_eq!(dto, back);
        // The slash-bearing functor survives as a structured field, not a key.
        assert_eq!(back.entries[0].symbol.functor, "rate/limit");
    }

    #[test]
    fn serializes_expected_shape() {
        let report = Vocabulary::derive(&assign_theory());
        let dto = VocabularyReportDto::from(&report);
        assert_eq!(dto.schema, VOCABULARY_SCHEMA);
        let entry = dto
            .entries
            .iter()
            .find(|e| e.symbol.functor == "assign-to" && e.symbol.arity == 2)
            .unwrap();
        assert_eq!(entry.symbol.arity, 2);
        assert_eq!(
            entry.description.as_deref(),
            Some("Assign a task to an agent.")
        );
        let sig = entry.signature.as_ref().unwrap();
        assert_eq!(sig.arguments[0].name, "task");
        assert_eq!(sig.arguments[0].sort, "symbol");
    }

    #[test]
    fn meta_target_serializes_as_structured_object() {
        let dto = MetaTargetDto::from(&MetaTarget::Predicate(sym("assign-to", 2)));
        let json = serde_json::to_value(&dto).unwrap();
        assert_eq!(json["kind"], "predicate");
        assert_eq!(json["symbol"]["functor"], "assign-to");
        assert_eq!(json["symbol"]["arity"], 2);
    }

    #[test]
    fn rejects_invalid_schema() {
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&assign_theory()));
        dto.schema = "spindle.vocabulary/999".to_string();
        assert!(matches!(
            dto.validate(),
            Err(VocabularyDtoError::UnknownSchema { .. })
        ));
    }

    #[test]
    fn rejects_unknown_sort() {
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&assign_theory()));
        let entry = dto
            .entries
            .iter_mut()
            .find(|e| e.symbol.functor == "assign-to" && e.symbol.arity == 2)
            .unwrap();
        entry.signature.as_mut().unwrap().arguments[0].sort = "task".to_string();
        assert!(matches!(
            dto.validate(),
            Err(VocabularyDtoError::UnknownSort { .. })
        ));
    }

    #[test]
    fn try_from_maps_empty_functor_not_arity_overflow() {
        let dto = PredicateSymbolDto {
            functor: String::new(),
            arity: 1,
        };
        assert_eq!(
            PredicateSymbol::try_from(&dto),
            Err(VocabularyDtoError::EmptyFunctor)
        );
    }

    #[test]
    fn try_from_maps_control_functor_not_arity_overflow() {
        let dto = PredicateSymbolDto {
            functor: "a\nb".to_string(),
            arity: 1,
        };
        assert_eq!(
            PredicateSymbol::try_from(&dto),
            Err(VocabularyDtoError::ControlFunctor { ch: '\n' })
        );
    }

    #[test]
    fn validate_rejects_core_invalid_symbol() {
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&assign_theory()));
        let entry = dto
            .entries
            .iter_mut()
            .find(|e| e.symbol.functor == "assign-to" && e.symbol.arity == 2)
            .unwrap();
        entry.symbol.functor = String::new();
        assert_eq!(dto.validate(), Err(VocabularyDtoError::EmptyFunctor));
    }

    #[test]
    fn malformed_predicate_dto_carries_origins() {
        let mut theory = Theory::new();
        theory.add_fact(""); // empty functor: no valid predicate symbol
        let dto = VocabularyReportDto::from(&Vocabulary::derive(&theory));
        let json = serde_json::to_value(&dto).unwrap();
        let diag = &json["diagnostics"][0];
        assert_eq!(diag["kind"], "malformed-predicate");
        assert_eq!(diag["functor"], "");
        assert_eq!(diag["arity"], 0);
        assert_eq!(diag["origins"][0]["role"], "head");
        // The report carries no entries, so it round-trips and validates.
        let back: VocabularyReportDto =
            serde_json::from_str(&serde_json::to_string(&dto).unwrap()).unwrap();
        back.validate().unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn coherent_declaration_retains_origins() {
        use spindle_core::vocabulary::SourceLocation;

        let mut theory = Theory::new();
        let sig = PredicateSignature::try_new(
            sym("p", 1),
            vec![ArgumentDecl::new("x", PrimitiveSort::Symbol)],
        )
        .unwrap();
        theory.add_predicate_declaration(PredicateDeclaration::new(
            sig,
            DeclarationOrigin::Parsed(SourceLocation::new(42, 3)),
        ));
        let dto = VocabularyReportDto::from(&Vocabulary::derive(&theory));
        let entry = dto
            .entries
            .iter()
            .find(|e| e.symbol.functor == "p" && e.symbol.arity == 1)
            .unwrap();
        assert!(entry.signature.is_some());
        // The coherent declaration keeps its source provenance, like the
        // conflict path does.
        assert_eq!(
            entry.declaration_origins.as_deref(),
            Some(
                &[DeclarationOriginDto::Parsed {
                    byte_offset: 42,
                    line: 3,
                }][..]
            )
        );

        let json = serde_json::to_string(&dto).unwrap();
        let back: VocabularyReportDto = serde_json::from_str(&json).unwrap();
        back.validate().unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn validate_rejects_signature_alongside_conflict() {
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&assign_theory()));
        let entry = dto
            .entries
            .iter_mut()
            .find(|e| e.symbol.functor == "assign-to" && e.symbol.arity == 2)
            .unwrap();
        assert!(entry.signature.is_some());
        entry.conflict = Some(vec![]);
        assert!(matches!(
            dto.validate(),
            Err(VocabularyDtoError::ContradictoryDeclaration { .. })
        ));
    }

    #[test]
    fn validate_accepts_v1_signature_without_declaration_origins() {
        // The documented spindle.vocabulary/1 example (SPEC-024 CON-007) has a
        // coherent signature and no declaration_origins; that payload must
        // keep validating.
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&assign_theory()));
        let entry = dto
            .entries
            .iter_mut()
            .find(|e| e.symbol.functor == "assign-to" && e.symbol.arity == 2)
            .unwrap();
        assert!(entry.signature.is_some());
        entry.declaration_origins = None;
        dto.validate().unwrap();
    }

    #[test]
    fn validate_rejects_declaration_origins_without_signature() {
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&assign_theory()));
        let entry = dto
            .entries
            .iter_mut()
            .find(|e| e.symbol.functor == "assign-to" && e.symbol.arity == 2)
            .unwrap();
        assert!(entry.declaration_origins.is_some());
        entry.signature = None;
        assert!(matches!(
            dto.validate(),
            Err(VocabularyDtoError::ContradictoryDeclaration { .. })
        ));
    }

    #[test]
    fn validate_rejects_empty_argument_name() {
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&assign_theory()));
        let entry = dto
            .entries
            .iter_mut()
            .find(|e| e.symbol.functor == "assign-to" && e.symbol.arity == 2)
            .unwrap();
        entry.signature.as_mut().unwrap().arguments[1].name = String::new();
        assert!(matches!(
            dto.validate(),
            Err(VocabularyDtoError::EmptyArgumentName { position: 1, .. })
        ));
    }

    #[test]
    fn validate_rejects_duplicate_argument_name() {
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&assign_theory()));
        let entry = dto
            .entries
            .iter_mut()
            .find(|e| e.symbol.functor == "assign-to" && e.symbol.arity == 2)
            .unwrap();
        entry.signature.as_mut().unwrap().arguments[1].name = "task".to_string();
        assert!(matches!(
            dto.validate(),
            Err(VocabularyDtoError::DuplicateArgumentName {
                first: 0,
                second: 1,
                ..
            })
        ));
    }

    #[test]
    fn conflict_entries_retain_declarations_and_origins() {
        let mut theory = Theory::new();
        for sort in [PrimitiveSort::Symbol, PrimitiveSort::Integer] {
            let sig = PredicateSignature::try_new(sym("p", 1), vec![ArgumentDecl::new("x", sort)])
                .unwrap();
            theory.add_predicate_declaration(PredicateDeclaration::new(
                sig,
                DeclarationOrigin::Programmatic,
            ));
        }
        let dto = VocabularyReportDto::from(&Vocabulary::derive(&theory));
        let entry = dto
            .entries
            .iter()
            .find(|e| e.symbol.functor == "p" && e.symbol.arity == 1)
            .unwrap();
        // No agreed signature, but both conflicting declarations survive.
        assert!(entry.signature.is_none());
        let conflict = entry.conflict.as_ref().expect("conflict payload present");
        assert_eq!(conflict.len(), 2);
        let sorts: Vec<_> = conflict
            .iter()
            .map(|d| d.signature.arguments[0].sort.as_str())
            .collect();
        assert!(sorts.contains(&"symbol") && sorts.contains(&"integer"));
        assert!(
            conflict
                .iter()
                .all(|d| d.origin == DeclarationOriginDto::Programmatic)
        );

        // The conflict payload round-trips and validates.
        let json = serde_json::to_string(&dto).unwrap();
        let back: VocabularyReportDto = serde_json::from_str(&json).unwrap();
        back.validate().unwrap();
        assert_eq!(dto, back);
    }

    #[test]
    fn validate_rejects_bad_sort_in_conflict_payload() {
        let mut theory = Theory::new();
        for sort in [PrimitiveSort::Symbol, PrimitiveSort::Integer] {
            let sig = PredicateSignature::try_new(sym("p", 1), vec![ArgumentDecl::new("x", sort)])
                .unwrap();
            theory.add_predicate_declaration(PredicateDeclaration::new(
                sig,
                DeclarationOrigin::Programmatic,
            ));
        }
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&theory));
        dto.entries[0].conflict.as_mut().unwrap()[0]
            .signature
            .arguments[0]
            .sort = "widget".to_string();
        assert!(matches!(
            dto.validate(),
            Err(VocabularyDtoError::UnknownSort { .. })
        ));
    }

    #[test]
    fn validate_rejects_bogus_diagnostic_origin_role() {
        let mut theory = Theory::new();
        theory.add_fact(""); // produces a malformed-predicate diagnostic
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&theory));
        let VocabularyDiagnosticDto::MalformedPredicate { origins, .. } = &mut dto.diagnostics[0]
        else {
            panic!("expected malformed-predicate diagnostic");
        };
        origins[0].role = "bogus".to_string();
        assert!(matches!(
            dto.validate(),
            Err(VocabularyDtoError::UnknownRole { .. })
        ));
    }

    #[test]
    fn validate_rejects_core_invalid_symbol_in_diagnostics() {
        let mut theory = Theory::new();
        theory.add_fact("undeclared");
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&theory));
        let VocabularyDiagnosticDto::UndeclaredPredicate { symbol } = &mut dto.diagnostics[0]
        else {
            panic!("expected undeclared-predicate diagnostic");
        };
        symbol.functor = String::new();
        assert_eq!(dto.validate(), Err(VocabularyDtoError::EmptyFunctor));
    }

    #[test]
    fn rejects_inconsistent_profile_arity() {
        let mut dto = VocabularyReportDto::from(&Vocabulary::derive(&assign_theory()));
        let entry = dto
            .entries
            .iter_mut()
            .find(|e| e.symbol.functor == "assign-to" && e.symbol.arity == 2)
            .unwrap();
        entry.profile.push(vec![]); // now length 3 != arity 2
        assert!(matches!(
            dto.validate(),
            Err(VocabularyDtoError::InconsistentArity { .. })
        ));
    }

    #[test]
    fn serialization_is_byte_identical_across_insertion_orders() {
        // SPEC-024 TEST-016 / NFR-001: structurally equal theories built in
        // different insertion orders serialize byte-identically.
        fn build(reverse: bool) -> Theory {
            let mut theory = Theory::new();
            let mut names = ["apple", "zebra", "mango"];
            if reverse {
                names.reverse();
            }
            for n in names {
                let sig = PredicateSignature::try_new(
                    sym(n, 1),
                    vec![ArgumentDecl::new("x", PrimitiveSort::Symbol)],
                )
                .unwrap();
                theory.add_predicate_declaration(PredicateDeclaration::new(
                    sig,
                    DeclarationOrigin::Programmatic,
                ));
            }
            theory
        }

        let a = serde_json::to_string(&VocabularyReportDto::from(&Vocabulary::derive(&build(
            false,
        ))))
        .unwrap();
        let b = serde_json::to_string(&VocabularyReportDto::from(&Vocabulary::derive(&build(
            true,
        ))))
        .unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn entries_sorted_by_functor_then_arity() {
        let mut theory = Theory::new();
        theory.add_fact("zebra");
        theory.add_fact("apple");
        let report = Vocabulary::derive(&theory);
        let dto = VocabularyReportDto::from(&report);
        let functors: Vec<_> = dto
            .entries
            .iter()
            .map(|e| e.symbol.functor.clone())
            .collect();
        let mut sorted = functors.clone();
        sorted.sort();
        assert_eq!(functors, sorted);
    }
}
