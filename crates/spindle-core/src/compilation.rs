//! Rule body compilation into explicit match keys.
//!
//! Compiles each rule's body literals into [`BodyMatchKey`] values that
//! encode the evidence strategy the reasoner should use:
//!
//! - **Temporal** body literals (concrete temporal bounds) require *exact*
//!   evidence — a fact matching the precise temporal window ([`ExactLitId`]).
//! - **Atemporal** body literals (no temporal bounds) require *family*
//!   support — any temporal variant of the literal's family suffices
//!   ([`FamilyId`]).
//! - **Arithmetic** constraints are evaluated directly against the
//!   substitution and carry no match key.

use smallvec::SmallVec;

use crate::index::IndexedTheory;
use crate::projection::{ExactLitId, FamilyId};
use crate::rule::{Rule, RuleLabel};

/// How a single body element should be matched during reasoning.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BodyMatchKey {
    /// Temporal body literal: requires exact evidence matching the specific
    /// temporal window.
    Exact(ExactLitId),
    /// Atemporal body literal: any member of the family provides sufficient
    /// support.
    Family(FamilyId),
    /// Arithmetic constraint: evaluated against the current substitution,
    /// no fact-matching required.
    Arithmetic,
}

impl BodyMatchKey {
    /// Returns `true` if this key requires exact temporal evidence.
    #[inline]
    pub fn is_exact(&self) -> bool {
        matches!(self, BodyMatchKey::Exact(_))
    }

    /// Returns `true` if this key requires family-level support.
    #[inline]
    pub fn is_family(&self) -> bool {
        matches!(self, BodyMatchKey::Family(_))
    }

    /// Returns `true` if this key is an arithmetic constraint.
    #[inline]
    pub fn is_arithmetic(&self) -> bool {
        matches!(self, BodyMatchKey::Arithmetic)
    }
}

/// A compiled rule body with pre-computed match keys.
///
/// Each element in [`keys`](CompiledBody::keys) corresponds positionally to
/// the element at the same index in the original rule's body. The reasoner
/// can inspect the match key to decide whether to perform exact or family
/// lookup without re-examining the body literal's temporal status.
#[derive(Debug, Clone)]
pub struct CompiledBody {
    /// The rule label this compiled body belongs to.
    rule_label: RuleLabel,
    /// Match keys for each body element, in source order.
    keys: SmallVec<[BodyMatchKey; 4]>,
}

impl CompiledBody {
    /// The rule label this compiled body belongs to.
    #[inline]
    pub fn rule_label(&self) -> &str {
        &self.rule_label
    }

    /// Match keys for each body element, in source order.
    #[inline]
    pub fn keys(&self) -> &[BodyMatchKey] {
        &self.keys
    }

    /// Number of body elements.
    #[inline]
    pub fn len(&self) -> usize {
        self.keys.len()
    }

    /// Returns `true` if the body is empty (i.e. a fact).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Returns `true` if every logic key is [`BodyMatchKey::Family`].
    ///
    /// A purely atemporal body can be satisfied by family-level evidence
    /// alone, which may enable faster bulk checking.
    pub fn is_all_family(&self) -> bool {
        self.keys
            .iter()
            .all(|k| matches!(k, BodyMatchKey::Family(_) | BodyMatchKey::Arithmetic))
    }

    /// Returns `true` if any logic key is [`BodyMatchKey::Exact`].
    pub fn has_exact(&self) -> bool {
        self.keys.iter().any(|k| k.is_exact())
    }

    /// Count of logic (non-arithmetic) keys.
    pub fn logic_key_count(&self) -> usize {
        self.keys.iter().filter(|k| !k.is_arithmetic()).count()
    }
}

/// Compile a single rule's body into match keys.
///
/// For each body element:
/// - `BodyLiteral::Logic` with non-empty temporal bounds → [`BodyMatchKey::Exact`]
/// - `BodyLiteral::Logic` with empty temporal bounds → [`BodyMatchKey::Family`]
/// - `BodyLiteral::Arithmetic` → [`BodyMatchKey::Arithmetic`]
pub fn compile_rule(rule: &Rule, index: &mut IndexedTheory<'_>) -> CompiledBody {
    let keys = rule
        .body
        .iter()
        .map(|bl| match bl {
            crate::body::BodyLiteral::Logic(logic) => {
                let lit = logic.to_literal();
                if lit.is_temporal() {
                    BodyMatchKey::Exact(index.exact_lit_id(&lit))
                } else {
                    BodyMatchKey::Family(FamilyId::from(&lit))
                }
            }
            crate::body::BodyLiteral::Arithmetic(_) => BodyMatchKey::Arithmetic,
        })
        .collect();

    CompiledBody {
        rule_label: rule.label.clone(),
        keys,
    }
}

/// Compile all rules in an indexed theory, returning a map from rule label
/// to compiled body.
pub fn compile_theory(
    index: &mut IndexedTheory<'_>,
) -> rustc_hash::FxHashMap<RuleLabel, CompiledBody> {
    let rules: Vec<Rule> = index.theory().rules().cloned().collect();
    rules
        .iter()
        .map(|rule| {
            let compiled = compile_rule(rule, index);
            (rule.label.clone(), compiled)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::literal::Literal;
    use crate::mode::Mode;
    use crate::rule::Rule;
    use crate::temporal::{Temporal, TimePoint};
    use crate::theory::Theory;

    fn build_theory_and_index(theory: &Theory) -> IndexedTheory<'_> {
        IndexedTheory::build(theory)
    }

    #[test]
    fn fact_produces_empty_compiled_body() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("bird")));

        let mut idx = build_theory_and_index(&theory);
        let rule = theory.get_rule("f1").unwrap();
        let compiled = compile_rule(rule, &mut idx);

        assert!(compiled.is_empty());
        assert_eq!(compiled.rule_label(), "f1");
        assert_eq!(compiled.logic_key_count(), 0);
    }

    #[test]
    fn atemporal_body_produces_family_key() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        ));

        let mut idx = build_theory_and_index(&theory);
        let rule = theory.get_rule("r1").unwrap();
        let compiled = compile_rule(rule, &mut idx);

        assert_eq!(compiled.len(), 1);
        assert!(compiled.is_all_family());
        assert!(!compiled.has_exact());

        let expected_family = FamilyId::from(&Literal::simple("bird"));
        assert_eq!(compiled.keys()[0], BodyMatchKey::Family(expected_family));
    }

    #[test]
    fn temporal_body_produces_exact_key() {
        let temporal_lit = Literal::new(
            "sensor",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let mut theory = Theory::new();
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![temporal_lit.clone()],
            Literal::simple("alert"),
        ));

        let mut idx = build_theory_and_index(&theory);
        let rule = theory.get_rule("r1").unwrap();
        let compiled = compile_rule(rule, &mut idx);

        assert_eq!(compiled.len(), 1);
        assert!(compiled.has_exact());
        assert!(!compiled.is_all_family());
        assert!(compiled.keys()[0].is_exact());
    }

    #[test]
    fn mixed_body_produces_mixed_keys() {
        let temporal_lit = Literal::new(
            "sensor",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );
        let atemporal_lit = Literal::simple("enabled");

        let mut theory = Theory::new();
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![temporal_lit.clone(), atemporal_lit.clone()],
            Literal::simple("alert"),
        ));

        let mut idx = build_theory_and_index(&theory);
        let rule = theory.get_rule("r1").unwrap();
        let compiled = compile_rule(rule, &mut idx);

        assert_eq!(compiled.len(), 2);
        assert!(compiled.has_exact());
        assert!(!compiled.is_all_family());

        assert!(compiled.keys()[0].is_exact());
        assert!(compiled.keys()[1].is_family());
    }

    #[test]
    fn arithmetic_body_produces_arithmetic_key() {
        use crate::arith::{ArithConstraint, ArithExpr};
        use crate::body::BodyLiteral;
        use crate::intern::intern;
        use crate::term::NumericValue;

        let bind = ArithConstraint::Bind {
            var: intern("?x"),
            expr: ArithExpr::Lit(NumericValue::Integer(42)),
        };

        let mut theory = Theory::new();
        theory.add_rule(Rule::new(
            "r1",
            crate::rule::RuleType::Defeasible,
            smallvec::smallvec![BodyLiteral::simple("data"), BodyLiteral::Arithmetic(bind),],
            smallvec::smallvec![Literal::simple("result")],
        ));

        let mut idx = build_theory_and_index(&theory);
        let rule = theory.get_rule("r1").unwrap();
        let compiled = compile_rule(rule, &mut idx);

        assert_eq!(compiled.len(), 2);
        assert!(compiled.keys()[0].is_family());
        assert!(compiled.keys()[1].is_arithmetic());
        assert_eq!(compiled.logic_key_count(), 1);
        assert!(compiled.is_all_family()); // arithmetic doesn't break all_family
    }

    #[test]
    fn compile_theory_compiles_all_rules() {
        let temporal_lit = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );

        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("bird")));
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::simple("bird")],
            Literal::simple("flies"),
        ));
        theory.add_rule(Rule::strict("s1", vec![temporal_lit], Literal::simple("q")));

        let mut idx = build_theory_and_index(&theory);
        let compiled = compile_theory(&mut idx);

        assert_eq!(compiled.len(), 3);
        assert!(compiled.contains_key("f1"));
        assert!(compiled.contains_key("r1"));
        assert!(compiled.contains_key("s1"));

        assert!(compiled["f1"].is_empty());
        assert!(compiled["r1"].is_all_family());
        assert!(compiled["s1"].has_exact());
    }

    #[test]
    fn negated_temporal_body_produces_exact_key() {
        let lit = Literal::new(
            "alarm",
            true,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(5), TimePoint::Moment(15)),
            vec![],
        );

        let mut theory = Theory::new();
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![lit.clone()],
            Literal::simple("safe"),
        ));

        let mut idx = build_theory_and_index(&theory);
        let rule = theory.get_rule("r1").unwrap();
        let compiled = compile_rule(rule, &mut idx);

        assert_eq!(compiled.len(), 1);
        assert!(compiled.keys()[0].is_exact());
    }

    #[test]
    fn negated_atemporal_body_produces_family_key() {
        let mut theory = Theory::new();
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![Literal::negated("broken")],
            Literal::simple("works"),
        ));

        let mut idx = build_theory_and_index(&theory);
        let rule = theory.get_rule("r1").unwrap();
        let compiled = compile_rule(rule, &mut idx);

        assert_eq!(compiled.len(), 1);
        assert!(compiled.keys()[0].is_family());

        let expected = FamilyId::from(&Literal::negated("broken"));
        assert_eq!(compiled.keys()[0], BodyMatchKey::Family(expected));
    }

    #[test]
    fn same_temporal_body_gets_same_exact_id() {
        let lit = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(1), TimePoint::Moment(10)),
            vec![],
        );

        let mut theory = Theory::new();
        // Two rules share the same temporal body literal
        theory.add_rule(Rule::defeasible(
            "r1",
            vec![lit.clone()],
            Literal::simple("a"),
        ));
        theory.add_rule(Rule::defeasible(
            "r2",
            vec![lit.clone()],
            Literal::simple("b"),
        ));

        let mut idx = build_theory_and_index(&theory);
        let compiled = compile_theory(&mut idx);

        // Both rules should get the same ExactLitId for the same body literal
        assert_eq!(compiled["r1"].keys()[0], compiled["r2"].keys()[0]);
    }
}
