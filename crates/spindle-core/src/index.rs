//! Theory indexing for O(1) rule lookup
//!
//! Indexed theories provide fast lookup of rules by head or body literals.
//! Uses a local atom interner to ensure correct identity for predicates with
//! arguments and temporal windows.

use std::cmp::Ordering;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::error::{Result, SpindleError};
use crate::intern::{SymbolId, intern, resolve};
use crate::literal::Literal;
use crate::projection::{ExactLitId, FamilyId};
use crate::rule::{Rule, RuleLabel};
use crate::temporal::Temporal;
use crate::term::Term;
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

/// Key used for interning atoms in the main reasoning index.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct AtomKey {
    functor: SymbolId,
    mode: (SymbolId, bool), // name_id, negated
    args: Vec<Term>,
    temporal: Temporal,
}

/// Key used for interning exact literals (includes temporal bounds).
/// Structurally identical to `AtomKey` but kept as a type alias to
/// separate the two interning namespaces.
type ExactAtomKey = AtomKey;

/// An indexed theory for efficient rule lookup.
///
/// Holds a reference to the theory to avoid deep cloning during reasoning.
/// Interns atoms locally to ensure `p(a) != p(b)` and `p[1,10] != p[20,30]`.
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
    /// Rules indexed by exact temporal body literals.
    body_index: FxHashMap<LitId, Vec<RuleLabel>>,
    /// Rules indexed by atemporal body families.
    family_body_index: FxHashMap<FamilyId, Vec<RuleLabel>>,
    /// Set of all literal IDs in the theory
    literal_set: FxHashSet<LitId>,
    /// Map from (ExactAtomKey, negated) to ExactLitId
    exact_map: FxHashMap<(ExactAtomKey, bool), ExactLitId>,
    /// Reverse map from exact atom ID to exact key for safe reconstruction.
    exact_atoms: Vec<ExactAtomKey>,
    /// Reverse map from ExactLitId to FamilyId
    exact_to_family: FxHashMap<ExactLitId, FamilyId>,
    /// Map from FamilyId to the sorted ExactLitIds belonging to that family
    family_to_exact: FxHashMap<FamilyId, Vec<ExactLitId>>,
    /// Counter for assigning ExactLitId values
    next_exact_id: u32,
}

impl<'a> IndexedTheory<'a> {
    /// Build an indexed theory from a theory reference.
    ///
    /// Build an indexed theory, panicking on ID-space exhaustion.
    ///
    /// # Panics
    /// Panics if exact-literal interning exhausts the 31-bit ID space.
    /// Prefer [`try_build`](Self::try_build) for fallible construction.
    pub fn build(theory: &'a Theory) -> Self {
        Self::try_build(theory)
            .expect("exact literal capacity exceeded while building theory index")
    }

    /// Build an indexed theory from a theory reference, returning an error if
    /// exact-literal interning exhausts the available ID space.
    pub fn try_build(theory: &'a Theory) -> Result<Self> {
        let mut idx = Self {
            theory,
            atom_map: FxHashMap::default(),
            atoms: Vec::new(),
            head_index: FxHashMap::default(),
            body_index: FxHashMap::default(),
            family_body_index: FxHashMap::default(),
            literal_set: FxHashSet::default(),
            exact_map: FxHashMap::default(),
            exact_atoms: Vec::new(),
            exact_to_family: FxHashMap::default(),
            family_to_exact: FxHashMap::default(),
            next_exact_id: 0,
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
                idx.try_intern_exact(head_lit)?;
            }

            for body_bl in &rule.body {
                // Only logic body literals are indexed; arithmetic constraints are skipped.
                if let Some(logic_lit) = body_bl.as_logic() {
                    let as_lit = logic_lit.to_literal();
                    let lit_id = idx.intern_literal(&as_lit);
                    if as_lit.is_temporal() {
                        idx.body_index
                            .entry(lit_id)
                            .or_default()
                            .push(rule.label.clone());
                    } else {
                        idx.family_body_index
                            .entry(FamilyId::from(&as_lit))
                            .or_default()
                            .push(rule.label.clone());
                    }
                    idx.literal_set.insert(lit_id);
                    idx.try_intern_exact(&as_lit)?;
                }
            }
        }

        Ok(idx)
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
            args: lit.predicate_args().to_vec(),
            temporal: lit.temporal.clone(),
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
            args: lit.predicate_args().to_vec(),
            temporal: lit.temporal.clone(),
        };

        self.atom_map
            .get(&key)
            .map(|&atom_id| LitId::new(atom_id, lit.negation))
    }

    fn mode_from_parts(mode: (SymbolId, bool)) -> crate::mode::Mode {
        let mode_name = if mode.0.is_empty() {
            None
        } else {
            Some(resolve(mode.0).to_string())
        };

        crate::mode::Mode {
            name: mode_name,
            negation: mode.1,
        }
    }

    fn literal_from_atom_key(key: &AtomKey, negated: bool) -> Literal {
        Literal::from_ids(
            key.functor,
            negated,
            Self::mode_from_parts(key.mode),
            key.temporal.clone(),
            key.args.clone(),
        )
    }

    /// Resolve a LitId back to a Literal if it was interned by this index.
    pub(crate) fn try_resolve_literal(&self, lit_id: LitId) -> Option<Literal> {
        let atom_idx = lit_id.atom().0 as usize;
        self.atoms
            .get(atom_idx)
            .map(|key| Self::literal_from_atom_key(key, lit_id.is_negated()))
    }

    /// Resolve a LitId back to a Literal.
    pub(crate) fn resolve_literal(&self, lit_id: LitId) -> Literal {
        self.try_resolve_literal(lit_id)
            .expect("literal id was not interned by this index")
    }

    /// Get the underlying theory.
    ///
    /// Returns a reference with the original `'a` lifetime (not tied to
    /// `&self`), which allows callers to hold this reference while also
    /// mutating the index.
    pub fn theory(&self) -> &'a Theory {
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
        let mut rules = Vec::new();

        if let Some(lit_id) = self.get_lit_id(lit)
            && let Some(labels) = self.body_index.get(&lit_id)
        {
            rules.extend(labels.iter().filter_map(|l| self.theory.get_rule(l)));
        }

        if let Some(labels) = self.family_body_index.get(&FamilyId::from(lit)) {
            rules.extend(labels.iter().filter_map(|l| self.theory.get_rule(l)));
        }

        rules
    }

    /// Get rules with the given literal ID in the body.
    pub fn rules_with_body_id(&self, lit_id: LitId) -> Vec<&Rule> {
        self.try_resolve_literal(lit_id)
            .map(|lit| self.rules_with_body(&lit))
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

    // -- Exact-family indexing API (CON-001) --

    const EXACT_ID_LIMIT: u32 = ExactLitId::NEGATION_BIT;

    /// Intern a literal as an exact identity (including temporal bounds).
    ///
    /// Returns the same `ExactLitId` for structurally identical literals
    /// (same functor, args, mode, negation, and temporal window).
    fn try_intern_exact(&mut self, lit: &Literal) -> Result<ExactLitId> {
        let mode_id = lit
            .mode
            .name
            .as_deref()
            .map(intern)
            .unwrap_or(SymbolId::EMPTY);
        let key = ExactAtomKey {
            functor: lit.name_id(),
            mode: (mode_id, lit.mode.negation),
            args: lit.predicate_args().to_vec(),
            temporal: lit.temporal.clone(),
        };
        let map_key = (key, lit.negation);

        if let Some(&id) = self.exact_map.get(&map_key) {
            return Ok(id);
        }

        // Allocate a new exact-literal ID in the projection-local ID space.
        let atom = self.allocate_exact_atom()?;
        let exact = ExactLitId::new(atom.as_raw(), lit.negation);

        debug_assert_eq!(self.exact_atoms.len(), atom.0 as usize);
        let family = FamilyId::from(lit);
        let (key, negated) = map_key;

        self.exact_atoms.push(key.clone());
        self.exact_map.insert((key, negated), exact);
        self.exact_to_family.insert(exact, family.clone());
        self.insert_family_member(family, exact);

        Ok(exact)
    }

    fn allocate_exact_atom(&mut self) -> Result<AtomId> {
        if self.next_exact_id >= Self::EXACT_ID_LIMIT {
            return Err(SpindleError::ReasoningError(
                "exact literal capacity exhausted; max 2^31 exact literals per index".to_string(),
            ));
        }

        let raw = self.next_exact_id;
        self.next_exact_id = raw.checked_add(1).ok_or_else(|| {
            SpindleError::ReasoningError(
                "exact literal capacity exhausted while allocating exact literal id".to_string(),
            )
        })?;

        Ok(AtomId(raw))
    }

    fn compare_temporal(lhs: &Temporal, rhs: &Temporal) -> Ordering {
        lhs.start
            .cmp(&rhs.start)
            .then_with(|| lhs.end.cmp(&rhs.end))
    }

    fn insert_family_member(&mut self, family: FamilyId, exact: ExactLitId) {
        let exact_atoms = &self.exact_atoms;
        let new_temporal = exact_atoms[exact.atom_index() as usize].temporal.clone();
        let members = self.family_to_exact.entry(family).or_default();
        // Use strict Less so that equal temporal windows preserve insertion order
        // (stable insertion into a sorted sequence).
        let insert_at = members.partition_point(|existing| {
            exact_atoms
                .get(existing.atom_index() as usize)
                .map(|existing_key| {
                    Self::compare_temporal(&existing_key.temporal, &new_temporal) == Ordering::Less
                })
                .unwrap_or(true)
        });
        members.insert(insert_at, exact);
    }

    /// Get the [`ExactLitId`] for a literal, interning it if not yet seen.
    ///
    /// Two literals that differ only in temporal window produce different
    /// `ExactLitId` values.
    pub fn try_exact_lit_id(&mut self, lit: &Literal) -> Result<ExactLitId> {
        self.try_intern_exact(lit)
    }

    /// Get the [`ExactLitId`] for a literal, interning it if not yet seen.
    ///
    /// Panics if exact-literal interning exhausts the available ID space.
    pub fn exact_lit_id(&mut self, lit: &Literal) -> ExactLitId {
        self.try_exact_lit_id(lit)
            .expect("exact literal capacity exceeded while interning literal")
    }

    /// Get the [`FamilyId`] for a literal (atemporal identity).
    pub fn family_id(&self, lit: &Literal) -> FamilyId {
        FamilyId::from(lit)
    }

    /// Get all exact literals belonging to a family.
    ///
    /// Returns an empty slice if the family has no members in this index.
    pub fn family_members(&self, family: &FamilyId) -> &[ExactLitId] {
        self.family_to_exact
            .get(family)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Get the [`FamilyId`] for an exact literal.
    ///
    /// Returns `None` if the `ExactLitId` was not interned by this index.
    pub fn family_for_exact(&self, exact: ExactLitId) -> Option<&FamilyId> {
        self.exact_to_family.get(&exact)
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

    #[test]
    fn test_indexed_theory_temporal_discrimination() {
        let mut theory = Theory::new();
        let early = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        let late = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(20),
                crate::temporal::TimePoint::Moment(30),
            ),
            vec![],
        );
        theory.add_rule(Rule::fact("f1", early.clone()));
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![late.clone()],
            Literal::simple("q"),
        ));

        let mut indexed = IndexedTheory::build(&theory);
        let early_id = indexed.intern_literal(&early);
        let late_id = indexed.intern_literal(&late);

        assert_ne!(
            early_id, late_id,
            "disjoint temporal windows should have different LitIds"
        );
        assert!(!indexed.rules_with_head_id(early_id).is_empty());
        assert!(!indexed.rules_with_body_id(late_id).is_empty());
        assert!(
            indexed.rules_with_body_id(early_id).is_empty(),
            "an early-window fact must not satisfy a late-window body slot"
        );
    }

    #[test]
    fn exact_lit_id_same_literal_returns_same_id() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact(
            "f1",
            Literal::new(
                "p",
                false,
                Default::default(),
                Temporal::new(
                    crate::temporal::TimePoint::Moment(1),
                    crate::temporal::TimePoint::Moment(10),
                ),
                vec![],
            ),
        ));

        let mut idx = IndexedTheory::build(&theory);

        let lit = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );

        let id1 = idx.exact_lit_id(&lit);
        let id2 = idx.exact_lit_id(&lit);
        assert_eq!(id1, id2);
    }

    #[test]
    fn exact_lit_id_temporal_variants_differ() {
        let mut theory = Theory::new();
        let lit_a = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        let lit_b = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(20),
                crate::temporal::TimePoint::Moment(30),
            ),
            vec![],
        );
        theory.add_rule(Rule::fact("f1", lit_a.clone()));
        theory.add_rule(Rule::fact("f2", lit_b.clone()));

        let mut idx = IndexedTheory::build(&theory);

        let id_a = idx.exact_lit_id(&lit_a);
        let id_b = idx.exact_lit_id(&lit_b);
        assert_ne!(id_a, id_b);
    }

    #[test]
    fn family_members_groups_temporal_variants() {
        let mut theory = Theory::new();
        let lit_a = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        let lit_b = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(20),
                crate::temporal::TimePoint::Moment(30),
            ),
            vec![],
        );
        theory.add_rule(Rule::fact("f1", lit_a.clone()));
        theory.add_rule(Rule::fact("f2", lit_b.clone()));

        let idx = IndexedTheory::build(&theory);
        let family = idx.family_id(&lit_a);
        let members = idx.family_members(&family);

        assert_eq!(members.len(), 2);
    }

    #[test]
    fn family_members_are_sorted_by_temporal_window() {
        let early = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        let late = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(20),
                crate::temporal::TimePoint::Moment(30),
            ),
            vec![],
        );

        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f_late", late.clone()));
        theory.add_rule(Rule::fact("f_early", early.clone()));

        let mut idx = IndexedTheory::build(&theory);
        let early_id = idx.exact_lit_id(&early);
        let late_id = idx.exact_lit_id(&late);
        let family = idx.family_id(&early);

        assert_eq!(idx.family_members(&family), &[early_id, late_id]);
    }

    #[test]
    fn family_for_exact_round_trips() {
        let mut theory = Theory::new();
        let lit = Literal::new(
            "q",
            true,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(5),
                crate::temporal::TimePoint::Moment(15),
            ),
            vec![],
        );
        theory.add_rule(Rule::fact("f1", lit.clone()));

        let mut idx = IndexedTheory::build(&theory);
        let exact = idx.exact_lit_id(&lit);
        let family = idx.family_for_exact(exact).unwrap();

        assert_eq!(*family, idx.family_id(&lit));
    }

    #[test]
    fn try_resolve_literal_returns_none_for_unknown_id() {
        let theory = Theory::new();
        let idx = IndexedTheory::build(&theory);

        assert_eq!(
            idx.try_resolve_literal(LitId::new(AtomId::from_raw(999), false)),
            None
        );
    }

    #[test]
    fn family_members_empty_for_unknown_family() {
        let theory = Theory::new();
        let idx = IndexedTheory::build(&theory);
        let unknown = FamilyId::from(&Literal::simple("unknown"));
        assert!(idx.family_members(&unknown).is_empty());
    }

    #[test]
    fn negation_separates_exact_families() {
        let mut theory = Theory::new();
        let pos = Literal::new(
            "p",
            false,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        let neg = Literal::new(
            "p",
            true,
            Default::default(),
            Temporal::new(
                crate::temporal::TimePoint::Moment(1),
                crate::temporal::TimePoint::Moment(10),
            ),
            vec![],
        );
        theory.add_rule(Rule::fact("f1", pos.clone()));
        theory.add_rule(Rule::fact("f2", neg.clone()));

        let mut idx = IndexedTheory::build(&theory);

        let exact_pos = idx.exact_lit_id(&pos);
        let exact_neg = idx.exact_lit_id(&neg);
        assert_ne!(exact_pos, exact_neg);

        let family_pos = idx.family_for_exact(exact_pos).unwrap();
        let family_neg = idx.family_for_exact(exact_neg).unwrap();
        assert_ne!(family_pos, family_neg);
    }

    #[test]
    fn try_exact_lit_id_errors_before_negation_bit_collision() {
        let theory = Theory::new();
        let mut idx = IndexedTheory::build(&theory);
        idx.next_exact_id = LitId::NEGATION_BIT;

        let err = idx
            .try_exact_lit_id(&Literal::simple("overflow"))
            .unwrap_err();
        assert!(err.to_string().contains("exact literal capacity exhausted"));
    }
}
