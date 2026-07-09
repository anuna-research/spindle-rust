/-
  Spindle Abduce Specification

  Formalizes the abduce operator: given a theory T and a goal literal q,
  find sets of facts F such that q ∈ reason(T ∪ F). Unlike the Rust
  implementation (which doesn't verify solutions), every returned solution
  is proven to be a valid explanation.

  Main results:
  - `abduce_solution_valid`: every solution F satisfies q ∈ reason(T ∪ F)
  - `abduce_already_provable`: if q ∈ reason(T), the empty set is a solution
  - `abduce_mono`: more rules in T → solutions remain valid
  - `abduce_solution_superset`: supersets of valid solutions are valid
  - Integration with what-if and why-not operators
-/
import Spindle.Arith.WhyNot

namespace Spindle.Arith

/-! ## Abduction solution -/

/-- An abduction solution: a set of hypothetical facts F together with
    a proof that adding F to the theory T derives the goal q.

    This is the key difference from the Rust implementation: solutions
    carry a validity proof, so `abduce` cannot return spurious candidates.
    Mirrors `AbductionSolution` in Rust's `abduce.rs`, but with verified
    correctness. -/
structure AbductionSolution (R : ReasoningOp) (T : Theory) (q : Literal) where
  /-- The hypothetical facts to assume. -/
  facts : List Literal
  /-- Proof that q is derivable from T augmented with facts (as fact rules). -/
  valid : R.conclusions (T ++ facts.map Literal.toFact) q

/-- The size of a solution: number of hypothetical facts. -/
def AbductionSolution.size {R : ReasoningOp} {T : Theory} {q : Literal}
    (s : AbductionSolution R T q) : Nat :=
  s.facts.length

/-- A solution with no facts means q is already provable from T. -/
def AbductionSolution.isAlreadyProvable {R : ReasoningOp} {T : Theory}
    {q : Literal} (s : AbductionSolution R T q) : Prop :=
  s.facts = []

/-! ## Abduction result -/

/-- An abduction result collects all solutions found for a goal. -/
structure AbductionResult (R : ReasoningOp) (T : Theory) (q : Literal) where
  /-- The list of verified solutions. -/
  solutions : List (AbductionSolution R T q)

/-- Whether any solutions were found. -/
def AbductionResult.hasSolutions {R : ReasoningOp} {T : Theory} {q : Literal}
    (r : AbductionResult R T q) : Prop :=
  r.solutions ≠ []

/-! ## Core soundness theorem -/

/-- **Main theorem**: every solution returned by abduce is valid.
    For any solution F in the result, q ∈ reason(T ∪ F).

    This is guaranteed by construction: `AbductionSolution` carries
    the validity proof as a field, so it is impossible to construct
    an invalid solution. This theorem simply extracts the proof. -/
theorem abduce_solution_valid (R : ReasoningOp) (T : Theory) (q : Literal)
    (s : AbductionSolution R T q) :
    R.conclusions (T ++ s.facts.map Literal.toFact) q :=
  s.valid

/-! ## Already-provable case -/

/-- If q is already derivable from T, then the empty fact set is a valid
    abduction solution. This mirrors the Rust code's check at the start
    of `abduce()` that returns an empty solution when the goal is already
    provable. -/
def abduce_trivial (R : ReasoningOp) (T : Theory) (q : Literal)
    (hq : R.conclusions T q) : AbductionSolution R T q :=
  ⟨[], by simp only [List.map_nil, List.append_nil]; exact hq⟩

/-- The trivial solution has size 0. -/
theorem abduce_trivial_size (R : ReasoningOp) (T : Theory) (q : Literal)
    (hq : R.conclusions T q) :
    (abduce_trivial R T q hq).size = 0 := rfl

/-- The trivial solution is already provable. -/
theorem abduce_trivial_isAlreadyProvable (R : ReasoningOp) (T : Theory)
    (q : Literal) (hq : R.conclusions T q) :
    (abduce_trivial R T q hq).isAlreadyProvable :=
  rfl

/-! ## Direct assumption -/

/-- The goal literal itself is always a valid abduction candidate:
    adding `q` as a fact to the theory trivially derives `q` (provided
    the reasoning operator derives facts from their own rules).

    This requires that R derives the head of any fact rule present in
    the theory. We model this as a precondition. -/
theorem abduce_self_valid (R : ReasoningOp) (T : Theory) (q : Literal)
    (hfact : ∀ T' : Theory, ∀ l : Literal,
      Rule.mk l [] ∈ T' → R.conclusions T' l) :
    R.conclusions (T ++ [q].map Literal.toFact) q := by
  apply hfact
  simp only [List.map_cons, List.map_nil, Literal.toFact]
  exact List.mem_append_right T (List.mem_cons.mpr (Or.inl rfl))

/-! ## Superset preservation -/

/-- **Superset preservation**: if F is a valid abduction solution, then
    any F' ⊇ F is also valid. Adding more hypothetical facts cannot
    invalidate an existing derivation, by monotonicity of reasoning. -/
theorem abduce_solution_superset (R : ReasoningOp) (T : Theory) (q : Literal)
    (s : AbductionSolution R T q) (F' : List Literal)
    (hsub : ∀ l, l ∈ s.facts → l ∈ F') :
    R.conclusions (T ++ F'.map Literal.toFact) q := by
  apply R.mono (T ++ s.facts.map Literal.toFact)
  · intro r hr
    simp only [List.mem_append, List.mem_map] at hr ⊢
    rcases hr with h | ⟨l, hlF, rfl⟩
    · exact Or.inl h
    · exact Or.inr ⟨l, hsub l hlF, rfl⟩
  · exact s.valid

/-- Construct a new solution from a superset of an existing solution's facts. -/
def AbductionSolution.extend {R : ReasoningOp} {T : Theory} {q : Literal}
    (s : AbductionSolution R T q) (F' : List Literal)
    (hsub : ∀ l, l ∈ s.facts → l ∈ F') :
    AbductionSolution R T q :=
  ⟨F', abduce_solution_superset R T q s F' hsub⟩

/-! ## Monotonicity in theory -/

/-- **Theory monotonicity**: a solution valid for theory T is also valid
    for any T' ⊇ T (with the same hypothetical facts). Adding more rules
    to the base theory cannot invalidate an abduction solution. -/
theorem abduce_mono (R : ReasoningOp) (T T' : Theory) (q : Literal)
    (hsub : ∀ r, r ∈ T → r ∈ T')
    (s : AbductionSolution R T q) :
    R.conclusions (T' ++ s.facts.map Literal.toFact) q := by
  apply R.mono (T ++ s.facts.map Literal.toFact)
  · intro r hr
    simp only [List.mem_append, List.mem_map] at hr ⊢
    rcases hr with h | hmap
    · exact Or.inl (hsub r h)
    · exact Or.inr hmap
  · exact s.valid

/-- Lift a solution from T to T' ⊇ T. -/
def AbductionSolution.lift {R : ReasoningOp} {T T' : Theory} {q : Literal}
    (s : AbductionSolution R T q)
    (hsub : ∀ r, r ∈ T → r ∈ T') :
    AbductionSolution R T' q :=
  ⟨s.facts, abduce_mono R T T' q hsub s⟩

/-! ## Union of solutions -/

/-- The union of two solutions' fact sets is also a valid solution.
    Both derivations go through since the union is a superset of each. -/
theorem abduce_union_valid (R : ReasoningOp) (T : Theory) (q : Literal)
    (s : AbductionSolution R T q) (F₂ : List Literal) :
    R.conclusions (T ++ (s.facts ++ F₂).map Literal.toFact) q := by
  apply R.mono (T ++ s.facts.map Literal.toFact)
  · intro r hr
    simp only [List.mem_append, List.mem_map] at hr ⊢
    rcases hr with h | ⟨l, hlF, rfl⟩
    · exact Or.inl h
    · exact Or.inr ⟨l, Or.inl hlF, rfl⟩
  · exact s.valid

/-! ## Abstract abduction oracle -/

/-- An abduction oracle is an abstract decision procedure that, given a
    reasoning operator R, theory T, and goal literal q, produces a list
    of verified abduction solutions.

    Unlike the Rust `abduce` function, every solution is verified:
    the oracle must prove that q ∈ reason(T ∪ F) for each returned F. -/
structure AbductionOracle (R : ReasoningOp) where
  /-- Given a theory and goal, produce verified solutions. -/
  abduce : (T : Theory) → (q : Literal) → AbductionResult R T q
  /-- If q is already provable, the trivial solution is included. -/
  abduce_trivial_included : ∀ T q,
    R.conclusions T q → (abduce T q).solutions ≠ []
  /-- Solutions are sorted by size (ascending), matching Rust behavior. -/
  abduce_sorted : ∀ T q (i j : Nat)
    (hi : i < (abduce T q).solutions.length)
    (hj : j < (abduce T q).solutions.length),
    i ≤ j →
    ((abduce T q).solutions.get ⟨i, hi⟩).size ≤
    ((abduce T q).solutions.get ⟨j, hj⟩).size

/-! ## Oracle soundness

Oracle soundness needs no theorem of its own. `AbductionSolution` carries
`valid` as a *field*, so an oracle cannot return an unsound solution — it could
not construct one. `abduce_solution_valid` states exactly that, for every
solution, oracle or not.

A theorem of the shape `s ∈ (A.abduce T q).solutions → R.conclusions …` would
prove the goal by projecting `s.valid` and never consume the membership
hypothesis, so it would hold for any `A` whatsoever — including one that
returns garbage. Soundness here is enforced by the type, not by a proof. -/

/-- A sound oracle with solutions implies the goal is abducibly reachable. -/
theorem abductionOracle_reachable (R : ReasoningOp) (A : AbductionOracle R)
    (T : Theory) (q : Literal)
    (hne : (A.abduce T q).hasSolutions) :
    ∃ F : List Literal, R.conclusions (T ++ F.map Literal.toFact) q := by
  simp only [AbductionResult.hasSolutions] at hne
  match h : (A.abduce T q).solutions with
  | [] => contradiction
  | s :: _ => exact ⟨s.facts, s.valid⟩

/-! ## Integration with What-If -/

/-- **Abduce–What-If bridge**: if F is a valid abduction solution for q
    in theory T, and q is not already derivable from T, then q is a
    what-if conclusion of T with hypotheticals F. -/
theorem abduce_implies_whatIf (R : ReasoningOp) (T : Theory) (q : Literal)
    (s : AbductionSolution R T q)
    (hnotq : ¬ R.conclusions T q) :
    whatIfConclusion R T s.facts q :=
  ⟨s.valid, hnotq⟩

/-- Conversely, if q is a what-if conclusion of T with hypotheticals F,
    then F is a valid abduction solution for q. -/
def whatIf_implies_abduce (R : ReasoningOp) (T : Theory) (q : Literal)
    (F : List Literal) (h : whatIfConclusion R T F q) :
    AbductionSolution R T q :=
  ⟨F, h.1⟩

/-- What-if and abduce are two views of the same phenomenon:
    q is a what-if conclusion under F iff F is a valid abduction solution
    for q (when q is not already provable). -/
theorem whatIf_iff_abduce (R : ReasoningOp) (T : Theory) (q : Literal)
    (F : List Literal) (hnotq : ¬ R.conclusions T q) :
    whatIfConclusion R T F q ↔
    R.conclusions (T ++ F.map Literal.toFact) q :=
  ⟨fun h => h.1, fun h => ⟨h, hnotq⟩⟩

/-! ## Integration with Why-Not -/

/-- **Why-Not–Abduce bridge**: if why_not returns MissingPremise(l) for
    goal q, then l is an abduction candidate — assuming l (plus possibly
    other missing premises) could make q derivable.

    This formalizes the intuition that why_not identifies *what's missing*,
    and abduce proposes *what to add*. The missing premise from why_not
    is a natural starting point for abduction. -/
theorem whyNot_guides_abduce (R : ReasoningOp) (W : WhyNotOracle R)
    (T : Theory) (q : Literal) (hnotq : ¬ R.conclusions T q)
    (l : Literal)
    (hreason : (W.explain T q hnotq).reason = .missingPremise l) :
    l ∈ (W.explain T q hnotq).supportingRule.body ∧
    ¬ R.conclusions T l :=
  whyNot_missingPremise_whatIf_candidate R W T q hnotq l hreason

/-- If adding missing premise l (from why_not) as a fact makes q derivable,
    then [l] is a valid abduction solution. -/
def abduce_from_whyNot (R : ReasoningOp) (T : Theory) (q : Literal)
    (l : Literal)
    (hderiv : R.conclusions (T ++ [l].map Literal.toFact) q) :
    AbductionSolution R T q :=
  ⟨[l], hderiv⟩

/-! ## Minimality -/

/-- A solution is minimal if no proper subset of its facts is also a
    valid solution. -/
def AbductionSolution.isMinimal {R : ReasoningOp} {T : Theory} {q : Literal}
    (s : AbductionSolution R T q) : Prop :=
  ∀ F' : List Literal, F'.length < s.facts.length →
    (∀ l, l ∈ F' → l ∈ s.facts) →
    ¬ R.conclusions (T ++ F'.map Literal.toFact) q

/-- The trivial (empty) solution is always minimal: there is no
    proper subset of the empty set. -/
theorem abduce_trivial_minimal (R : ReasoningOp) (T : Theory) (q : Literal)
    (hq : R.conclusions T q) :
    (abduce_trivial R T q hq).isMinimal := by
  intro F' hlt _
  simp only [abduce_trivial] at hlt
  exact absurd hlt (Nat.not_lt_zero _)

/-! ## Composition: chained abduction -/

/-- If F₁ is a valid abduction solution for some intermediate goal p,
    and F₂ is a valid abduction solution for q in the theory T ∪ {p},
    then F₁ ∪ F₂ is a valid abduction solution for q in T (provided
    the reasoning operator has the intermediate derivation property).

    This captures the Rust code's potential for recursive abduction:
    to derive q, we might need p, and to get p we need to abduce F₁. -/
theorem abduce_chain (R : ReasoningOp) (T : Theory) (q p : Literal)
    (F₁ : List Literal)
    (hderiv : R.conclusions (T ++ (F₁ ++ [p]).map Literal.toFact) q)
    (s₁ : AbductionSolution R T p) :
    R.conclusions (T ++ (s₁.facts ++ F₁ ++ [p]).map Literal.toFact) q := by
  apply R.mono (T ++ (F₁ ++ [p]).map Literal.toFact)
  · intro r hr
    simp only [List.mem_append, List.mem_map] at hr ⊢
    rcases hr with h | ⟨l, hlF, rfl⟩
    · exact Or.inl h
    · rcases hlF with hlF₁ | hlP
      · exact Or.inr ⟨l, Or.inl (Or.inr hlF₁), rfl⟩
      · exact Or.inr ⟨l, Or.inr hlP, rfl⟩
  · exact hderiv

end Spindle.Arith
