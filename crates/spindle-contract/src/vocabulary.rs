//! Structured serialization DTOs for the predicate vocabulary (SPEC-024
//! CON-007, REQ-014).
//!
//! These DTOs are additive to the typed V2 contract. Functor and arity are
//! always separate structured fields; a predicate-indicator string is never a
//! machine key. Enum strings are lowercase kebab case; unknown enum strings and
//! inconsistent arities are rejected rather than preserved (SPEC-024 CON-007).

use serde::{Deserialize, Serialize};

use spindle_core::vocabulary::{
    ArgumentDecl, DeclarationState, MetaTarget, ObservedArgumentKind, OccurrenceRole,
    PredicateOrigin, PredicateSymbol, PredicateSymbolError, PrimitiveSort, VocabularyDiagnostic,
    VocabularyEntry, VocabularyReport,
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
            if let Some(sig) = &entry.signature {
                if sig.arguments.len() as u64 != u64::from(arity) {
                    return Err(VocabularyDtoError::InconsistentArity {
                        functor: entry.symbol.functor.clone(),
                        arity,
                        found: sig.arguments.len(),
                    });
                }
                for arg in &sig.arguments {
                    PrimitiveSort::from_name(&arg.sort).ok_or_else(|| {
                        VocabularyDtoError::UnknownSort {
                            found: arg.sort.clone(),
                        }
                    })?;
                }
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
            for origin in &entry.origins {
                if origin.role != "head" && origin.role != "body" {
                    return Err(VocabularyDtoError::UnknownRole {
                        found: origin.role.clone(),
                    });
                }
            }
        }
        Ok(())
    }
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

fn entry_to_dto(entry: &VocabularyEntry) -> VocabularyEntryDto {
    let signature = match &entry.declaration {
        Some(DeclarationState::Declared { signature, .. }) => Some(SignatureDto {
            arguments: signature
                .arguments
                .iter()
                .map(ArgumentDeclDto::from)
                .collect(),
        }),
        _ => None,
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
