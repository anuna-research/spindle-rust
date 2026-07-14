//! Positional argument profiles (SPEC-024 CON-004, REQ-007).

use std::collections::BTreeSet;

use super::sort::ObservedArgumentKind;
use super::symbol::PredicateSymbol;

/// Observational evidence about the argument kinds found for one predicate
/// symbol, grouped by argument position (SPEC-024 §3.12).
///
/// A profile does not claim that future occurrences are constrained to the
/// observed kinds. The position count is fixed by the symbol arity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentProfile {
    /// The predicate symbol this profile describes.
    pub symbol: PredicateSymbol,
    /// One observed-kind set per argument position, in order.
    pub positions: Box<[BTreeSet<ObservedArgumentKind>]>,
}

impl ArgumentProfile {
    /// Create an empty profile with one empty position set per argument.
    ///
    /// The position count is derived from the symbol arity and cannot be
    /// supplied independently (SPEC-024 CON-004).
    pub fn empty(symbol: PredicateSymbol) -> Self {
        let positions = (0..symbol.arity())
            .map(|_| BTreeSet::new())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { symbol, positions }
    }

    /// Record one observed kind at the given argument position.
    ///
    /// Out-of-range positions are ignored; callers observe applications whose
    /// arity matches the symbol by construction.
    pub fn observe(&mut self, position: usize, kind: ObservedArgumentKind) {
        if let Some(set) = self.positions.get_mut(position) {
            set.insert(kind);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal::InternedLiteralName;

    fn sym(name: &str, arity: usize) -> PredicateSymbol {
        PredicateSymbol::try_new(InternedLiteralName::intern(name), arity).unwrap()
    }

    #[test]
    fn positions_kept_separate_and_numeric_kinds_distinct() {
        // Observe cost(item, 2) and cost(other, 2.5).
        let mut profile = ArgumentProfile::empty(sym("cost", 2));
        profile.observe(0, ObservedArgumentKind::Symbol);
        profile.observe(1, ObservedArgumentKind::Integer);
        profile.observe(0, ObservedArgumentKind::Symbol);
        profile.observe(1, ObservedArgumentKind::Decimal);

        assert_eq!(
            profile.positions[0].iter().copied().collect::<Vec<_>>(),
            vec![ObservedArgumentKind::Symbol]
        );
        assert_eq!(
            profile.positions[1].iter().copied().collect::<Vec<_>>(),
            vec![ObservedArgumentKind::Integer, ObservedArgumentKind::Decimal]
        );
    }
}
