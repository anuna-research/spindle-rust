/-
  Spindle Cross-Operator Query Soundness

  Proves three cross-operator soundness properties for the query subsystem:

  1. **Abduce→WhatIf soundness**: if abduce returns F for goal q, then
     what_if(T, F) includes q (i.e., q is a genuine new conclusion).
  2. **WhyNot validity**: if q ∉ conclusions, why_not returns a valid
     blocking reason that is consistent with the reasoning state.
  3. **WhatIf consistency**: what_if results are consistent — the reasoning
     operator cannot simultaneously derive both a literal and its complement
     as new conclusions (given a consistent reasoning operator).

  These theorems tie the three query operators (what_if, why_not, abduce)
  into a coherent system and validate their cross-references in the Rust
  implementation (query/what_if.rs, query/why_not.rs, query/abduce.rs).
-/
import Spindle.Arith.Abduce

namespace Spindle.Arith

/-! ## Literal complement -/

/-- The complement of a literal: flips the negation flag.
    Mirrors the concept of `~q` in defeasible logic where `+D ~q` blocks `+d q`. -/
def Literal.complement (l : Literal) : Literal :=
  { l with negation := !l.negation }

/-- Complementing twice is identity. -/
theorem Literal.complement_complement (l : Literal) :
    l.complement.complement = l := by
  simp only [Literal.complement, Bool.not_not]

/-- A literal is not equal to its complement. -/
theorem Literal.ne_complement (l : Literal) :
    l ≠ l.complement := by
  intro h
  simp only [Literal.complement] at h
  have habs := congrArg Literal.negation h
  simp only at habs
  cases l.negation <;> simp at habs

/-! ## Consistent reasoning operator -/

/-- A consistent reasoning operator: extends ReasoningOp with the guarantee
    that it never simultaneously derives a literal and its complement from
    the same theory. This captures the fundamental consistency property of
    defeasible logic where +∂ q and +∂ ~q cannot both hold at the same
    proof tag.

    SCOPE WARNING: this structure demands monotonicity (inherited from
    `ReasoningOp`) AND consistency simultaneously, and NO Spindle
    reasoning level satisfies both. The defeasible level is consistent
    (`Family.twoSided_consistent`, `Properties.partial_consistent` in
    SpindleLean) but not monotone
    (`Properties.defeasible_not_monotone`); the monotone support closure
    (`supportOp`) is not consistent (it derives both `p` and `~p` from
    contradictory facts, by design — it ignores defeat). Theorems below
    that use only the `consistent` field (`whatIf_consistent`,
    `whatIf_augmented_consistent`, `abduce_consistent`) describe the
    defeasible level's behavior; theorems that also consume derivations
    produced via `mono` describe a hypothetical monotone-and-consistent
    reasoner and do not transfer to the engine as stated. -/
structure ConsistentReasoningOp extends ReasoningOp where
  /-- Consistency: no theory derives both l and complement(l). -/
  consistent : ∀ T : Theory, ∀ l : Literal,
    conclusions T l → ¬ conclusions T l.complement

/-! ## Property 1: Abduce→WhatIf Soundness -/

/-- **Cross-operator soundness (abduce → what_if)**:
    If abduce returns a solution F for goal q in theory T, and q is not
    already derivable from T, then q appears as a what_if conclusion
    when using F as hypothetical facts.

    This bridges the two operators: abduce finds *what to assume*,
    and what_if confirms that *assuming it works*. The Rust implementation
    checks this empirically via `requires` (which calls abduce then
    verifies with reasoning); this theorem proves it holds in general. -/
theorem abduce_whatIf_soundness (R : ReasoningOp) (T : Theory) (q : Literal)
    (s : AbductionSolution R T q) (hnotq : ¬ R.conclusions T q) :
    whatIfConclusion R T s.facts q :=
  abduce_implies_whatIf R T q s hnotq

/-- Strengthened form: the abduction solution's facts, when passed to what_if,
    produce q as a new conclusion AND q is derivable from T ∪ F. -/
theorem abduce_whatIf_soundness_strong (R : ReasoningOp) (T : Theory) (q : Literal)
    (s : AbductionSolution R T q) (hnotq : ¬ R.conclusions T q) :
    whatIfConclusion R T s.facts q ∧
    R.conclusions (T ++ s.facts.map Literal.toFact) q :=
  ⟨abduce_implies_whatIf R T q s hnotq, s.valid⟩

/-- Converse direction: if what_if shows q is a new conclusion under F, then
    reading F back out of the abduction solution recovers F exactly. This is
    the round-trip content — `whatIf_implies_abduce` transports the hypothetical
    fact set unchanged.

    Note `¬ R.conclusions T q` is not a hypothesis: it is already the second
    component of `whatIfConclusion R T F q`. Stating the conclusion as
    `whatIfConclusion R T s.facts q` instead would make this theorem the
    identity function on `hwi`, since `s.facts` reduces to `F`. -/
theorem whatIf_abduce_roundtrip (R : ReasoningOp) (T : Theory) (q : Literal)
    (F : List Literal) (hwi : whatIfConclusion R T F q) :
    (whatIf_implies_abduce R T q F hwi).facts = F :=
  rfl

/-! ## Property 2: WhyNot Validity -/

/-- **Cross-operator soundness (why_not validity)**:
    If q is not in the conclusions of T, then the why_not oracle returns
    a blocking reason that is valid — meaning it identifies a genuine
    obstacle to deriving q.

    This is a composite validity theorem: regardless of which blocking
    reason type is returned, the result is informative and correct.
    - MissingPremise(l): l is in the supporting rule's body AND l is not derived
    - Defeated(attacker): the attacking rule exists in the theory
    - Contradicted(c): the complement c IS derived from the theory -/
theorem whyNot_valid (R : ReasoningOp) (W : WhyNotOracle R)
    (T : Theory) (q : Literal) (hnotq : ¬ R.conclusions T q) :
    let result := W.explain T q hnotq
    -- The result refers to the correct literal
    result.literal = q ∧
    -- The supporting rule is in the theory with matching head
    result.supportingRule ∈ T ∧
    result.supportingRule.head = q ∧
    -- The blocking reason is valid (case analysis)
    match result.reason with
    | .missingPremise l =>
        l ∈ result.supportingRule.body ∧ ¬ R.conclusions T l
    | .defeated attacker =>
        attacker ∈ T
    | .contradicted c =>
        R.conclusions T c := by
  refine ⟨W.explain_literal T q hnotq,
         W.explain_rule_mem T q hnotq,
         W.explain_rule_head T q hnotq, ?_⟩
  -- Case split on the blocking reason
  match hreason : (W.explain T q hnotq).reason with
  | .missingPremise l =>
    exact ⟨W.explain_missing_in_body T q hnotq l hreason,
           W.explain_missing_not_derived T q hnotq l hreason⟩
  | .defeated attacker =>
    exact W.explain_defeated_mem T q hnotq attacker hreason
  | .contradicted c =>
    exact W.explain_contradicted_derived T q hnotq c hreason

/-- **WhyNot→WhatIf bridge**: if why_not says q is blocked by a missing
    premise l, then adding l as a hypothetical is a meaningful step toward
    making q derivable. Combined with abduce, this creates a complete
    diagnostic-to-repair pipeline.

    The first two conjuncts are what tie this to the *oracle*: they are
    discharged from `W`'s own MissingPremise soundness contract, so `hreason`
    is load-bearing. Without them the statement would hold for an arbitrary
    `l` and say nothing about what why_not reported. -/
theorem whyNot_whatIf_bridge (R : ReasoningOp) (W : WhyNotOracle R)
    (T : Theory) (q : Literal) (hnotq : ¬ R.conclusions T q)
    (l : Literal)
    (hreason : (W.explain T q hnotq).reason = .missingPremise l)
    -- If adding l as a fact actually makes q derivable:
    (hderiv : R.conclusions (T ++ [l].map Literal.toFact) q) :
    -- The reported premise is genuinely missing, and genuinely a premise:
    ¬ R.conclusions T l ∧
    l ∈ (W.explain T q hnotq).supportingRule.body ∧
    -- and q is a what-if conclusion under it, with [l] the abduction solution:
    whatIfConclusion R T [l] q ∧
    (abduce_from_whyNot R T q l hderiv).facts = [l] :=
  ⟨W.explain_missing_not_derived T q hnotq l hreason,
   W.explain_missing_in_body T q hnotq l hreason,
   ⟨hderiv, hnotq⟩,
   rfl⟩

/-! ## Property 3: WhatIf Consistency -/

/-- **Cross-operator soundness (what_if consistency)**:
    For a consistent reasoning operator, if l is a what-if conclusion
    under hypotheticals F, then the complement of l is NOT also a what-if
    conclusion under the same hypotheticals.

    This prevents contradictory hypothetical reasoning: you can't have both
    "+d q" and "+d ~q" as new conclusions from the same set of assumptions,
    given that the underlying reasoning operator is consistent.

    In the Rust implementation, this corresponds to the invariant that
    `WhatIfResult.new_conclusions` never contains both a literal and its
    negation (when using the scalable reasoner, which is consistent). -/
theorem whatIf_consistent (R : ConsistentReasoningOp) (T : Theory)
    (F : List Literal) (l : Literal)
    (hwi : whatIfConclusion R.toReasoningOp T F l) :
    ¬ whatIfConclusion R.toReasoningOp T F l.complement := by
  intro ⟨hderiv_comp, _⟩
  exact R.consistent (T ++ F.map Literal.toFact) l hwi.1 hderiv_comp

/-- Consistency extends to the full augmented theory: if the consistent
    reasoning operator derives l from T ∪ F, it does not derive complement(l). -/
theorem whatIf_augmented_consistent (R : ConsistentReasoningOp) (T : Theory)
    (F : List Literal) (l : Literal)
    (hderiv : R.conclusions (T ++ F.map Literal.toFact) l) :
    ¬ R.conclusions (T ++ F.map Literal.toFact) l.complement :=
  R.consistent (T ++ F.map Literal.toFact) l hderiv

/-- **Abduce consistency**: if F is a valid abduction solution for q,
    and the reasoning operator is consistent, then the complement of q
    is not derivable from T ∪ F. This ensures abduction solutions don't
    lead to contradictory theories. -/
theorem abduce_consistent (R : ConsistentReasoningOp) (T : Theory) (q : Literal)
    (s : AbductionSolution R.toReasoningOp T q) :
    ¬ R.conclusions (T ++ s.facts.map Literal.toFact) q.complement :=
  R.consistent (T ++ s.facts.map Literal.toFact) q s.valid

/-- **Full cross-operator consistency**: if abduce returns F for q,
    then what_if(T, F) includes q but NOT complement(q).
    This is the strongest form of cross-operator soundness, combining
    all three properties. -/
theorem cross_operator_soundness (R : ConsistentReasoningOp)
    (T : Theory) (q : Literal)
    (s : AbductionSolution R.toReasoningOp T q)
    (hnotq : ¬ R.conclusions T q) :
    -- 1. q is a what-if conclusion
    whatIfConclusion R.toReasoningOp T s.facts q ∧
    -- 2. complement(q) is NOT a what-if conclusion
    ¬ whatIfConclusion R.toReasoningOp T s.facts q.complement := by
  constructor
  · exact abduce_implies_whatIf R.toReasoningOp T q s hnotq
  · intro ⟨hderiv_comp, _⟩
    exact R.consistent (T ++ s.facts.map Literal.toFact) q s.valid hderiv_comp

/-! ## Integration: diagnostic-to-repair pipeline -/

/-- **Pipeline theorem**: the complete diagnostic-to-repair pipeline is sound.
    Given:
    - q is not derived from T
    - why_not identifies missing premise l
    - adding l makes q derivable (verified by requires/abduce)
    Then:
    - [l] is a valid abduction solution
    - q is a what-if conclusion under [l]
    - complement(q) is NOT a what-if conclusion under [l] (with consistent R)

    This validates the Rust code path:
    query(q) = NotProvable → why_not(q) = MissingPremise(l) →
    requires(q) = {l} → what_if(T, {l}) includes q -/
theorem pipeline_soundness (R : ConsistentReasoningOp) (W : WhyNotOracle R.toReasoningOp)
    (T : Theory) (q l : Literal)
    (hnotq : ¬ R.conclusions T q)
    (hreason : (W.explain T q hnotq).reason = .missingPremise l)
    (hderiv : R.conclusions (T ++ [l].map Literal.toFact) q) :
    -- l is genuinely missing
    ¬ R.conclusions T l ∧
    -- [l] is a valid abduction solution
    R.conclusions (T ++ [l].map Literal.toFact) q ∧
    -- q is a what-if conclusion
    whatIfConclusion R.toReasoningOp T [l] q ∧
    -- complement(q) is not a what-if conclusion
    ¬ whatIfConclusion R.toReasoningOp T [l] q.complement := by
  refine ⟨W.explain_missing_not_derived T q hnotq l hreason,
         hderiv,
         ⟨hderiv, hnotq⟩, ?_⟩
  intro ⟨hderiv_comp, _⟩
  exact R.consistent (T ++ [l].map Literal.toFact) q hderiv hderiv_comp

end Spindle.Arith
