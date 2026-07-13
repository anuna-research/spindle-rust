//! Provenanced vocabulary derivation (SPEC-024 CON-005, REQ-011, OBS-001).

use std::collections::BTreeMap;

use super::declaration::MetaTarget;
use super::profile::ArgumentProfile;
use super::shape::Shape;
use super::sort::ObservedArgumentKind;
use super::symbol::{HasPredicateSymbol, PredicateSymbol};
use super::theory_signature::{DeclarationState, derive_declaration_states};
use crate::body::BodyLiteral;
use crate::rule::RuleLabel;
use crate::theory::{MetaValue, Theory};

/// Whether an observed occurrence is a rule head or a rule body element
/// (SPEC-024 CON-005).
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OccurrenceRole {
    /// A rule head literal at the given zero-based index.
    Head {
        /// Zero-based position within the rule head.
        index: u32,
    },
    /// A rule body logical literal at the given zero-based index.
    Body {
        /// Zero-based position within the rule body.
        index: u32,
    },
}

/// The provenance of one observed predicate occurrence (SPEC-024 CON-005).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateOrigin {
    /// The rule label the occurrence belongs to.
    pub rule: RuleLabel,
    /// Whether it is a head or body occurrence, and at what index.
    pub role: OccurrenceRole,
}

/// One observed logical occurrence, with everything needed to build profiles,
/// origins, and shape counts in a single pass (SPEC-024 NFR-002).
pub(crate) struct ObservedOccurrence {
    pub symbol: PredicateSymbol,
    pub origin: PredicateOrigin,
    pub kinds: Vec<ObservedArgumentKind>,
}

/// Traverse the theory once, visiting each logical occurrence in a rule head or
/// logical rule body exactly once (SPEC-024 NFR-002). Rules are visited in
/// stable [`RuleLabel`] order; head and body positions use source order.
pub(crate) fn observed(theory: &Theory) -> Vec<ObservedOccurrence> {
    let mut rules: Vec<_> = theory.rules_with_labels().collect();
    rules.sort_by_key(|(label, _)| *label);

    let mut occurrences = Vec::new();
    for (label, rule) in rules {
        for (index, head) in rule.head.iter().enumerate() {
            if let Ok(symbol) = head.predicate_symbol() {
                let kinds = head
                    .predicate_args()
                    .iter()
                    .map(ObservedArgumentKind::of_term)
                    .collect();
                occurrences.push(ObservedOccurrence {
                    symbol,
                    origin: PredicateOrigin {
                        rule: label.clone(),
                        role: OccurrenceRole::Head {
                            index: index as u32,
                        },
                    },
                    kinds,
                });
            }
        }

        for (index, body) in rule.body.iter().enumerate() {
            // Arithmetic constraints are not logical predicate applications and
            // contribute no predicate symbol (SPEC-024 REQ-009).
            if let BodyLiteral::Logic(logic) = body
                && let Ok(symbol) = logic.predicate_symbol()
            {
                let kinds = logic
                    .predicate_args()
                    .iter()
                    .map(ObservedArgumentKind::of_body_arg)
                    .collect();
                occurrences.push(ObservedOccurrence {
                    symbol,
                    origin: PredicateOrigin {
                        rule: label.clone(),
                        role: OccurrenceRole::Body {
                            index: index as u32,
                        },
                    },
                    kinds,
                });
            }
        }
    }
    occurrences
}

/// Collect `(symbol, origin)` pairs for every observed logical occurrence.
pub(crate) fn collect_occurrences(theory: &Theory) -> Vec<(PredicateSymbol, PredicateOrigin)> {
    observed(theory)
        .into_iter()
        .map(|o| (o.symbol, o.origin))
        .collect()
}

/// A vocabulary catalogue entry (SPEC-024 CON-005).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VocabularyEntry {
    /// The predicate symbol this entry describes.
    pub symbol: PredicateSymbol,
    /// The declaration state, if the symbol is declared.
    pub declaration: Option<DeclarationState>,
    /// Observed argument kinds by position.
    pub profile: ArgumentProfile,
    /// A joined description drawn from predicate metadata, if any.
    pub description: Option<String>,
    /// Sorted occurrence provenance.
    pub origins: Box<[PredicateOrigin]>,
}

/// A derived, inspectable catalogue of theory terms (SPEC-024 §3.7).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Vocabulary {
    /// Entries in `(functor, arity)` order.
    pub entries: Vec<VocabularyEntry>,
}

impl Vocabulary {
    /// Derive the vocabulary and its diagnostics from a theory (SPEC-024 REQ-011).
    pub fn derive(theory: &Theory) -> VocabularyReport {
        derive_vocabulary(theory)
    }
}

/// A vocabulary derivation diagnostic (SPEC-024 CON-005, OBS-001).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VocabularyDiagnostic {
    /// The symbol has incompatible declarations.
    ConflictingDeclaration {
        /// The affected predicate symbol.
        symbol: PredicateSymbol,
    },
    /// The symbol is used but never declared (reported per caller policy;
    /// SPEC-024 REQ-018).
    UndeclaredPredicate {
        /// The affected predicate symbol.
        symbol: PredicateSymbol,
    },
    /// Predicate metadata targets a symbol with neither a declaration nor an
    /// observed occurrence, so it is not joined by name (SPEC-024 CON-005).
    UnresolvedPredicateMetadata {
        /// The affected predicate symbol.
        symbol: PredicateSymbol,
    },
}

/// Summary counts for a vocabulary derivation (SPEC-024 OBS-001).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DerivationSummary {
    /// Distinct predicate symbols in the vocabulary.
    pub distinct_symbols: usize,
    /// Observed logical occurrences visited.
    pub observed_occurrences: usize,
    /// Raw predicate declarations stored in the theory.
    pub declarations: usize,
    /// Symbols with conflicting declarations.
    pub declaration_conflicts: usize,
    /// Predicate metadata targets that resolved to no symbol.
    pub unresolved_predicate_metadata: usize,
    /// Observed symbols without a declaration.
    pub undeclared_uses: usize,
    /// Shape diagnostics produced by validating observed uses of declared
    /// symbols against their compiled shapes.
    pub shape_diagnostics: usize,
}

/// The full result of vocabulary derivation (SPEC-024 CON-005).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VocabularyReport {
    /// The derived catalogue.
    pub vocabulary: Vocabulary,
    /// Deterministically ordered diagnostics.
    pub diagnostics: Vec<VocabularyDiagnostic>,
    /// Summary counts.
    pub summary: DerivationSummary,
}

fn derive_vocabulary(theory: &Theory) -> VocabularyReport {
    let occurrences = observed(theory);
    let declaration_states = derive_declaration_states(theory);

    // Group observations by symbol (BTreeMap keeps (functor, arity) order).
    let mut per_symbol: BTreeMap<PredicateSymbol, SymbolAccumulator> = BTreeMap::new();
    for occ in &occurrences {
        let acc = per_symbol
            .entry(occ.symbol)
            .or_insert_with(|| SymbolAccumulator::new(occ.symbol));
        acc.observe(occ);
    }
    // Ensure declared-but-unobserved symbols still get an entry.
    for symbol in declaration_states.keys() {
        per_symbol
            .entry(*symbol)
            .or_insert_with(|| SymbolAccumulator::new(*symbol));
    }

    // Predicate descriptions, keyed by symbol.
    let descriptions = predicate_descriptions(theory);

    let mut entries = Vec::with_capacity(per_symbol.len());
    let mut diagnostics = Vec::new();
    let mut declaration_conflicts = 0usize;
    let mut undeclared_uses = 0usize;
    let mut shape_diagnostics = 0usize;

    for (symbol, mut acc) in per_symbol {
        acc.origins.sort();
        let declaration = declaration_states.get(&symbol).cloned();

        match &declaration {
            Some(DeclarationState::Conflict(_)) => {
                declaration_conflicts += 1;
                diagnostics.push(VocabularyDiagnostic::ConflictingDeclaration { symbol });
            }
            Some(DeclarationState::Declared { signature, .. }) => {
                let shape = Shape::from(signature);
                for kinds in &acc.observed_kinds {
                    shape_diagnostics += shape.validate_kinds(symbol, kinds).diagnostics.len();
                }
            }
            None => {
                // Observed but undeclared (declared-only symbols are always in
                // declaration_states, so a None here means it was observed).
                undeclared_uses += 1;
                diagnostics.push(VocabularyDiagnostic::UndeclaredPredicate { symbol });
            }
        }

        entries.push(VocabularyEntry {
            symbol,
            declaration,
            profile: acc.profile,
            description: descriptions.get(&symbol).cloned(),
            origins: acc.origins.into_boxed_slice(),
        });
    }

    // Predicate metadata whose symbol is neither declared nor observed.
    let mut unresolved_predicate_metadata = 0usize;
    for target in theory.metadata().keys() {
        if let MetaTarget::Predicate(symbol) = target
            && !entries.iter().any(|e| e.symbol == *symbol)
        {
            unresolved_predicate_metadata += 1;
            diagnostics.push(VocabularyDiagnostic::UnresolvedPredicateMetadata { symbol: *symbol });
        }
    }

    // Deterministic diagnostic order independent of HashMap iteration.
    diagnostics.sort_by_key(diagnostic_key);

    let summary = DerivationSummary {
        distinct_symbols: entries.len(),
        observed_occurrences: occurrences.len(),
        declarations: theory.predicate_declarations().len(),
        declaration_conflicts,
        unresolved_predicate_metadata,
        undeclared_uses,
        shape_diagnostics,
    };

    VocabularyReport {
        vocabulary: Vocabulary { entries },
        diagnostics,
        summary,
    }
}

/// A sort key giving diagnostics a total, deterministic order.
fn diagnostic_key(d: &VocabularyDiagnostic) -> (u8, PredicateSymbol) {
    match d {
        VocabularyDiagnostic::ConflictingDeclaration { symbol } => (0, *symbol),
        VocabularyDiagnostic::UndeclaredPredicate { symbol } => (1, *symbol),
        VocabularyDiagnostic::UnresolvedPredicateMetadata { symbol } => (2, *symbol),
    }
}

/// Read predicate descriptions from `MetaTarget::Predicate` entries.
fn predicate_descriptions(theory: &Theory) -> BTreeMap<PredicateSymbol, String> {
    let mut out = BTreeMap::new();
    for (target, meta) in theory.metadata() {
        if let MetaTarget::Predicate(symbol) = target
            && let Some(value) = meta.properties.get("description")
        {
            let text = match value {
                MetaValue::String(s) => s.clone(),
                MetaValue::List(items) => items.join(" "),
            };
            out.insert(*symbol, text);
        }
    }
    out
}

/// Per-symbol accumulator building a profile and origin list in one pass.
struct SymbolAccumulator {
    profile: ArgumentProfile,
    origins: Vec<PredicateOrigin>,
    observed_kinds: Vec<Vec<ObservedArgumentKind>>,
}

impl SymbolAccumulator {
    fn new(symbol: PredicateSymbol) -> Self {
        Self {
            profile: ArgumentProfile::empty(symbol),
            origins: Vec::new(),
            observed_kinds: Vec::new(),
        }
    }

    fn observe(&mut self, occ: &ObservedOccurrence) {
        for (position, kind) in occ.kinds.iter().enumerate() {
            self.profile.observe(position, *kind);
        }
        self.origins.push(occ.origin.clone());
        self.observed_kinds.push(occ.kinds.clone());
    }
}

impl Ord for PredicateOrigin {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort by rule label, then role (Head before Body), then index (CON-005).
        self.rule
            .cmp(&other.rule)
            .then_with(|| self.role.cmp(&other.role))
    }
}

impl PartialOrd for PredicateOrigin {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
