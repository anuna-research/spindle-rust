//! Theory indexing for O(1) rule lookup
//!
//! Indexed theories provide fast lookup of rules by head or body literals.
//! Uses a local atom interner to ensure correct identity for predicates with arguments.

use rustc_hash::{FxHashMap, FxHashSet};

use crate::intern::{SymbolId, intern, resolve};
use crate::literal::Literal;
use crate::rule::{Rule, RuleLabel};
use crate::theory::Theory;

/// Unique identifier for an atom (predicate + args + mode) within an IndexedTheory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct AtomId(u32);

impl AtomId {
    /// Return the underlying raw identifier.
    pub fn as_raw(self) -> u32 {
        self.0
    }

    /// Create an AtomId from a raw u32 value.
    pub fn from_raw(value: u32) -> Self {
        AtomId(value)
    }
}

/// Unique identifier for a literal (AtomId + negation) within an IndexedTheory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(transparent)]
pub struct LitId(u32);

impl LitId {
    const NEGATION_BIT: u32 = 1 << 31;
    const ATOM_MASK: u32 = !Self::NEGATION_BIT;

    /// Create a LitId from an atom ID and negation flag.
    pub fn new(atom: AtomId, negated: bool) -> Self {
        if negated {
            Self(atom.0 | Self::NEGATION_BIT)
        } else {
            Self(atom.0 & Self::ATOM_MASK)
        }
    }

    /// Return the underlying atom ID.
    pub fn atom(self) -> AtomId {
        AtomId(self.0 & Self::ATOM_MASK)
    }

    /// Return true if this literal is negated.
    pub fn is_negated(self) -> bool {
        (self.0 & Self::NEGATION_BIT) != 0
    }

    /// Return the complement literal (negation flipped).
    pub fn complement(self) -> Self {
        Self(self.0 ^ Self::NEGATION_BIT)
    }

    /// Return the underlying raw identifier.
    pub fn as_raw(self) -> u32 {
        self.0
    }
}

/// Key used for interning atoms.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AtomKey {
    functor: SymbolId,
    mode: (SymbolId, bool), // name_id, negated
    args: Vec<SymbolId>,
}

/// An indexed theory for efficient rule lookup.
///
/// Holds a reference to the theory to avoid deep cloning during reasoning.
/// Interns atoms locally to ensure p(a) != p(b).
#[derive(Debug)]
pub struct IndexedTheory<'a> {
    /// Reference to the underlying theory (avoids deep clone)
    theory: &'a Theory,
    /// Map from AtomKey to AtomId
    atom_map: FxHashMap<AtomKey, AtomId>,
    /// Map from AtomId to AtomKey (for reconstruction)
    atoms: Vec<AtomKey>,
    /// Rules indexed by head literal
    head_index: FxHashMap<LitId, Vec<RuleLabel>>,
    /// Rules indexed by body literal (trigger index)
    body_index: FxHashMap<LitId, Vec<RuleLabel>>,
    /// Set of all literal IDs in the theory
    literal_set: FxHashSet<LitId>,
}

impl<'a> IndexedTheory<'a> {
    /// Build an indexed theory from a theory reference.
    pub fn build(theory: &'a Theory) -> Self {
        let mut idx = Self {
            theory,
            atom_map: FxHashMap::default(),
            atoms: Vec::new(),
            head_index: FxHashMap::default(),
            body_index: FxHashMap::default(),
            literal_set: FxHashSet::default(),
        };

        // Index all rules
        for rule in theory.rules() {
            for head_lit in &rule.head {
                let lit_id = idx.intern_literal(head_lit);
                idx.head_index
                    .entry(lit_id)
                    .or_default()
                    .push(rule.label.clone());
                idx.literal_set.insert(lit_id);
            }

            for body_lit in &rule.body {
                let lit_id = idx.intern_literal(body_lit);
                idx.body_index
                    .entry(lit_id)
                    .or_default()
                    .push(rule.label.clone());
                idx.literal_set.insert(lit_id);
            }
        }

        idx
    }

    /// Intern a literal into the local atom store.
    pub fn intern_literal(&mut self, lit: &Literal) -> LitId {
        let mode_id = lit
            .mode
            .name
            .as_deref()
            .map(intern)
            .unwrap_or(SymbolId::EMPTY);
        let key = AtomKey {
            functor: lit.name_id(),
            mode: (mode_id, lit.mode.negation),
            args: lit.predicate_ids().to_vec(),
        };

        let atom_id = if let Some(&id) = self.atom_map.get(&key) {
            id
        } else {
            let id = AtomId(self.atoms.len() as u32);
            self.atom_map.insert(key.clone(), id);
            self.atoms.push(key);
            id
        };

        LitId::new(atom_id, lit.negation)
    }

    /// Lookup a literal ID without interning new atoms.
    pub fn get_lit_id(&self, lit: &Literal) -> Option<LitId> {
        let mode_id = lit
            .mode
            .name
            .as_deref()
            .map(intern)
            .unwrap_or(SymbolId::EMPTY);
        let key = AtomKey {
            functor: lit.name_id(),
            mode: (mode_id, lit.mode.negation),
            args: lit.predicate_ids().to_vec(),
        };

        self.atom_map
            .get(&key)
            .map(|&atom_id| LitId::new(atom_id, lit.negation))
    }

    /// Resolve a LitId back to a Literal.
    pub fn resolve_literal(&self, lit_id: LitId) -> Literal {
        let atom_id = lit_id.atom();
        let key = &self.atoms[atom_id.0 as usize];

        // Construct mode from key
        let mode_name_id = key.mode.0;
        let mode_name = if mode_name_id.is_empty() {
            None
        } else {
            Some(resolve(mode_name_id).to_string())
        };

        let mode = crate::mode::Mode {
            name: mode_name,
            negation: key.mode.1,
        };

        Literal::from_ids(
            key.functor,
            lit_id.is_negated(),
            mode,
            crate::temporal::Temporal::empty(), // Temporal lost in identity, which is correct for reasoning
            key.args.clone(),
        )
    }

    /// Get the underlying theory
    pub fn theory(&self) -> &Theory {
        self.theory
    }

    /// Get rules with the given literal in the head.
    pub fn rules_with_head(&self, lit: &Literal) -> Vec<&Rule> {
        if let Some(lit_id) = self.get_lit_id(lit) {
            self.rules_with_head_id(lit_id)
        } else {
            Vec::new()
        }
    }

    /// Get rules with the given literal ID in the head.
    pub fn rules_with_head_id(&self, lit_id: LitId) -> Vec<&Rule> {
        self.head_index
            .get(&lit_id)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|l| self.theory.get_rule(l))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get rules with the given literal in the body.
    pub fn rules_with_body(&self, lit: &Literal) -> Vec<&Rule> {
        if let Some(lit_id) = self.get_lit_id(lit) {
            self.rules_with_body_id(lit_id)
        } else {
            Vec::new()
        }
    }

    /// Get rules with the given literal ID in the body.
    pub fn rules_with_body_id(&self, lit_id: LitId) -> Vec<&Rule> {
        self.body_index
            .get(&lit_id)
            .map(|labels| {
                labels
                    .iter()
                    .filter_map(|l| self.theory.get_rule(l))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get total number of distinct atoms interned.
    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    /// Get all literal IDs in the theory.
    pub fn all_literal_ids(&self) -> impl Iterator<Item = &LitId> {
        self.literal_set.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexed_theory_arg_discrimination() {
        let mut theory = Theory::new();
        // p(a)
        theory.add_rule(Rule::fact(
            "f1",
            Literal::new(
                "p",
                false,
                Default::default(),
                Default::default(),
                vec!["a".to_string()],
            ),
        ));
        // p(b) => q
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::new(
                "p",
                false,
                Default::default(),
                Default::default(),
                vec!["b".to_string()],
            )],
            Literal::simple("q"),
        ));

        let mut indexed = IndexedTheory::build(&theory);

        let p_a = Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["a".to_string()],
        );
        let p_b = Literal::new(
            "p",
            false,
            Default::default(),
            Default::default(),
            vec!["b".to_string()],
        );

        let id_a = indexed.intern_literal(&p_a);
        let id_b = indexed.intern_literal(&p_b);

        assert_ne!(id_a, id_b, "p(a) and p(b) should have different LitIds");

        // p(a) is a fact head
        assert!(!indexed.rules_with_head_id(id_a).is_empty());
        // p(b) is a rule body
        assert!(!indexed.rules_with_body_id(id_b).is_empty());

        // p(a) is NOT a rule body
        assert!(indexed.rules_with_body_id(id_a).is_empty());
    }
}
