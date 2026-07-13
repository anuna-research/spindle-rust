//! Deterministic theory-signature derivation (SPEC-024 CON-005, REQ-009).

use std::collections::{BTreeMap, BTreeSet};

use super::declaration::{DeclarationOrigin, PredicateDeclaration};
use super::signature::PredicateSignature;
use super::symbol::PredicateSymbol;
use super::vocab::collect_occurrences;
use crate::theory::Theory;

/// The declaration status of a predicate symbol (SPEC-024 CON-005).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationState {
    /// One coherent signature, with every distinct origin that declared it.
    Declared {
        /// The agreed signature.
        signature: PredicateSignature,
        /// Every distinct origin that declared this signature.
        origins: Box<[DeclarationOrigin]>,
    },
    /// Incompatible signatures for the same symbol, retaining every distinct
    /// source-provenanced declaration.
    Conflict(Box<[PredicateDeclaration]>),
}

/// The set of predicate symbols available to a theory, plus their declaration
/// states (SPEC-024 §3.6).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TheorySignature {
    /// Every distinct predicate symbol available to the theory.
    pub symbols: BTreeSet<PredicateSymbol>,
    /// Declaration states, keyed by predicate symbol.
    pub declarations: BTreeMap<PredicateSymbol, DeclarationState>,
}

impl TheorySignature {
    /// Derive the theory signature from declarations and rule traversal
    /// (SPEC-024 REQ-009).
    ///
    /// The symbol set is the union of observed applications (rule heads and
    /// logical rule bodies) and declarations owned by the theory. Availability
    /// never uses naming conventions.
    pub fn derive(theory: &Theory) -> Self {
        let mut symbols = BTreeSet::new();

        // Observed logical occurrences (heads and logical bodies).
        for (symbol, _) in collect_occurrences(theory) {
            symbols.insert(symbol);
        }

        let declarations = derive_declaration_states(theory);
        // Declared symbols are available even when no rule uses them.
        for symbol in declarations.keys() {
            symbols.insert(*symbol);
        }

        Self {
            symbols,
            declarations,
        }
    }
}

/// Group raw declarations by symbol and resolve each to a [`DeclarationState`]
/// (SPEC-024 REQ-010).
pub(crate) fn derive_declaration_states(
    theory: &Theory,
) -> BTreeMap<PredicateSymbol, DeclarationState> {
    // Preserve source/insertion order within each symbol group.
    let mut grouped: BTreeMap<PredicateSymbol, Vec<PredicateDeclaration>> = BTreeMap::new();
    for decl in theory.predicate_declarations() {
        grouped.entry(decl.symbol()).or_default().push(decl.clone());
    }

    grouped
        .into_iter()
        .map(|(symbol, decls)| (symbol, resolve_declaration_group(decls)))
        .collect()
}

/// Resolve one symbol's declarations into a [`DeclarationState`].
fn resolve_declaration_group(decls: Vec<PredicateDeclaration>) -> DeclarationState {
    // Dedup exact-identical declarations (same signature and origin).
    let mut distinct: Vec<PredicateDeclaration> = Vec::new();
    for decl in decls {
        if !distinct.contains(&decl) {
            distinct.push(decl);
        }
    }

    // Impose a canonical `(signature, origin)` order so both the conflict payload
    // and the merged origin list are independent of insertion/traversal order
    // (SPEC-024 NFR-001, REQ-010).
    distinct.sort();

    let first_signature = &distinct[0].signature;
    let all_agree = distinct.iter().all(|d| &d.signature == first_signature);

    if all_agree {
        // Origins are already sorted and exact-deduped; drop consecutive repeats
        // defensively.
        let mut origins: Vec<DeclarationOrigin> = distinct.iter().map(|d| d.origin).collect();
        origins.dedup();
        DeclarationState::Declared {
            signature: first_signature.clone(),
            origins: origins.into_boxed_slice(),
        }
    } else {
        DeclarationState::Conflict(distinct.into_boxed_slice())
    }
}
