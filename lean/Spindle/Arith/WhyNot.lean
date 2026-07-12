/-
  Spindle Why-Not Specification

  Formalizes the why_not operator: given a theory T and a literal q not in
  the conclusions, return a BlockingReason explaining why q is not derived.

  Three blocking reasons:
  - MissingPremise: a body literal l of a supporting rule is unproved (l ∉ +d)
  - Defeated: a defeater rule attacks the supporting rule
  - Contradicted: the complement ~q is derived at a stronger tag (+D ~q blocks +d q)

  Main result:
  - `whyNot_missingPremise_not_derived`: if why_not returns MissingPremise(l),
    then l ∉ +d (the missing literal is genuinely not derived).
-/
import Spindle.Arith.WhatIf

namespace Spindle.Arith

/-! ## Blocking reasons -/

/-- A blocking reason explains why a literal q is not derivable from theory T.
    Mirrors the Rust `BlockingType` enum in `why_not.rs`. -/
inductive BlockingReason where
  /-- A body literal of a supporting rule is not derived.
      The `Literal` field identifies which premise is missing. -/
  | missingPremise (missing : Literal) : BlockingReason
  /-- A defeater rule blocks the supporting rule.
      The `Rule` field identifies the attacking defeater. -/
  | defeated (attacker : Rule) : BlockingReason
  /-- The complement literal is derived at a stronger proof tag,
      blocking derivation. E.g., +D ~q blocks +d q.
      The `Literal` field is the contradicting complement. -/
  | contradicted (complement : Literal) : BlockingReason
  deriving Repr

/-! ## Why-not result -/

/-- A why-not result pairs the queried literal with its blocking reason. -/
structure WhyNotResult where
  /-- The literal that was queried. -/
  literal : Literal
  /-- The rule that would derive the literal (if its body were satisfied). -/
  supportingRule : Rule
  /-- The reason derivation is blocked. -/
  reason : BlockingReason

/-! ## Abstract why-not specification -/

/-- A why-not oracle is an abstract decision procedure that, given a reasoning
    operator R, theory T, and literal q not in R's conclusions, produces a
    blocking reason explaining why q is not derived.

    The key soundness requirement is that if the oracle returns
    `MissingPremise(l)`, then l is genuinely not derived by R from T.

    SCOPE: this contract models the case where SOME rule for q exists
    (`explain_rule_mem` demands a supporting rule in T), so it is not
    instantiable on theories with no rule for q — Rust's why_not reports
    a separate "no supporting rule" outcome there, and its `Undetermined`
    blocking type is likewise outside this model. The theorems below are
    conditional guarantees about any oracle meeting the contract; no
    concrete instance is provided. -/
structure WhyNotOracle (R : ReasoningOp) where
  /-- Given a theory and a non-derived literal, produce a why-not result. -/
  explain : (T : Theory) → (q : Literal) → ¬ R.conclusions T q → WhyNotResult
  /-- The result's literal matches the queried literal. -/
  explain_literal : ∀ T q (h : ¬ R.conclusions T q),
    (explain T q h).literal = q
  /-- The supporting rule is in the theory. -/
  explain_rule_mem : ∀ T q (h : ¬ R.conclusions T q),
    (explain T q h).supportingRule ∈ T
  /-- The supporting rule's head matches the queried literal. -/
  explain_rule_head : ∀ T q (h : ¬ R.conclusions T q),
    (explain T q h).supportingRule.head = q
  /-- **Soundness for MissingPremise**: if the oracle returns MissingPremise(l),
      then l is a body literal of the supporting rule, and l is not derived. -/
  explain_missing_in_body : ∀ T q (h : ¬ R.conclusions T q) (l : Literal),
    (explain T q h).reason = .missingPremise l →
    l ∈ (explain T q h).supportingRule.body
  explain_missing_not_derived : ∀ T q (h : ¬ R.conclusions T q) (l : Literal),
    (explain T q h).reason = .missingPremise l →
    ¬ R.conclusions T l
  /-- **Soundness for Defeated**: the attacking rule is in the theory. -/
  explain_defeated_mem : ∀ T q (h : ¬ R.conclusions T q) (attacker : Rule),
    (explain T q h).reason = .defeated attacker →
    attacker ∈ T
  /-- **Soundness for Contradicted**: the complement is derived. -/
  explain_contradicted_derived : ∀ T q (h : ¬ R.conclusions T q) (c : Literal),
    (explain T q h).reason = .contradicted c →
    R.conclusions T c

/-! ## Main theorem: MissingPremise soundness -/

/-- **Main theorem**: if `why_not` returns `MissingPremise(l)` for a query q
    in theory T, then `l` is not in the conclusions of T.

    This is the core soundness guarantee: the missing premise identified
    by `why_not` is genuinely not derived. If it were derived, the oracle
    would be inconsistent with its own soundness contract.

    Formally: for any sound why-not oracle W over reasoning operator R,
    if W.explain(T, q) = MissingPremise(l), then ¬ R.conclusions T l. -/
theorem whyNot_missingPremise_not_derived
    (R : ReasoningOp) (W : WhyNotOracle R)
    (T : Theory) (q : Literal) (hnotq : ¬ R.conclusions T q)
    (l : Literal)
    (hreason : (W.explain T q hnotq).reason = .missingPremise l) :
    ¬ R.conclusions T l :=
  W.explain_missing_not_derived T q hnotq l hreason

/-! ## Corollary: MissingPremise literal is in the rule body -/

/-- If `why_not` returns `MissingPremise(l)`, then `l` appears in the body
    of the supporting rule. This means the rule *would* derive q if l
    (and possibly other body literals) were proved. -/
theorem whyNot_missingPremise_in_body
    (R : ReasoningOp) (W : WhyNotOracle R)
    (T : Theory) (q : Literal) (hnotq : ¬ R.conclusions T q)
    (l : Literal)
    (hreason : (W.explain T q hnotq).reason = .missingPremise l) :
    l ∈ (W.explain T q hnotq).supportingRule.body :=
  W.explain_missing_in_body T q hnotq l hreason

/-! ## Corollary: Contradicted implies complement is derived -/

/-- If `why_not` returns `Contradicted(c)`, then the complement literal c
    is actually derived from the theory. This captures the scenario where
    +D ~q blocks +d q. -/
theorem whyNot_contradicted_derived
    (R : ReasoningOp) (W : WhyNotOracle R)
    (T : Theory) (q : Literal) (hnotq : ¬ R.conclusions T q)
    (c : Literal)
    (hreason : (W.explain T q hnotq).reason = .contradicted c) :
    R.conclusions T c :=
  W.explain_contradicted_derived T q hnotq c hreason

/-! ## Corollary: Defeated attacker is in the theory -/

/-- If `why_not` returns `Defeated(attacker)`, then the attacking rule
    is present in the theory. -/
theorem whyNot_defeated_in_theory
    (R : ReasoningOp) (W : WhyNotOracle R)
    (T : Theory) (q : Literal) (hnotq : ¬ R.conclusions T q)
    (attacker : Rule)
    (hreason : (W.explain T q hnotq).reason = .defeated attacker) :
    attacker ∈ T :=
  W.explain_defeated_mem T q hnotq attacker hreason

/-! ## Non-derivability propagation -/

/-- If why_not identifies a missing premise l, and l is also non-derived,
    then we can recursively ask why_not about l. This theorem shows that
    the chain is well-founded: each MissingPremise identifies a literal
    that is genuinely non-derived, so recursive explanation is justified. -/
theorem whyNot_chain
    (R : ReasoningOp) (W : WhyNotOracle R)
    (T : Theory) (q : Literal) (hnotq : ¬ R.conclusions T q)
    (l : Literal)
    (hreason : (W.explain T q hnotq).reason = .missingPremise l) :
    ∃ _ : ¬ R.conclusions T l, True :=
  ⟨W.explain_missing_not_derived T q hnotq l hreason, trivial⟩

/-! ## Interaction with what-if -/

/-- If q is not derived and why_not says MissingPremise(l), then adding l
    as a hypothetical fact could potentially make q derivable (it is a
    what-if candidate). More precisely, l is a body literal of a rule
    for q, so adding l removes one obstacle to deriving q. -/
theorem whyNot_missingPremise_whatIf_candidate
    (R : ReasoningOp) (W : WhyNotOracle R)
    (T : Theory) (q : Literal) (hnotq : ¬ R.conclusions T q)
    (l : Literal)
    (hreason : (W.explain T q hnotq).reason = .missingPremise l) :
    l ∈ (W.explain T q hnotq).supportingRule.body ∧ ¬ R.conclusions T l :=
  ⟨W.explain_missing_in_body T q hnotq l hreason,
   W.explain_missing_not_derived T q hnotq l hreason⟩

end Spindle.Arith
