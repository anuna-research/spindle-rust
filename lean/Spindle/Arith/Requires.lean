/-
  Spindle Requires Specification

  Formalizes the verified `requires` operator
  (`crates/spindle-core/src/query/requires.rs`, IMPL-011): take raw
  abduction candidates, VERIFY each by injecting the candidate facts into
  the theory and re-running full reasoning, and return exactly the
  candidates under which the goal becomes positively provable.

    "requires_with_options verifies each raw abduction candidate by
     injecting candidate facts and re-running full reasoning. Only
     candidates that make the goal positively provable ... are returned."

  The verification step is modeled as a `Verifier`: a decision procedure
  for the reasoning operator, with a correctness proof tying its boolean
  answer to derivability. In Rust the verifier IS `reason()` itself.

  Main results:
  - `requiresVerify_facts_mem`: the acceptance contract — a raw candidate
    is returned iff injecting it makes the goal provable
  - `requiresVerify_sound`: every returned solution is drawn from the raw
    candidate pool and derives the goal (the search filters, never invents)
  - `requiresVerify_rejected`: rejected candidates genuinely fail
  - `requires_already_provable`: an already-provable goal admits the
    empty solution
  - `requires_whatIf`: a verified solution for a not-yet-provable goal is
    exactly a what-if scenario producing the goal as a NEW conclusion
    (cross-operator consistency with `whatIfConclusion`)

  Note: "every returned solution derives the goal" is NOT stated on its own.
  It holds for any `AbductionSolution` by its `valid` field, so as a theorem
  about `requiresVerify` it would be vacuous. It appears only as a conjunct
  alongside `s.facts ∈ raw`, which is the part that constrains the search.
-/
import Spindle.Arith.Abduce

namespace Spindle.Arith

/-! ## Verifiers -/

/-- A decision procedure for a reasoning operator: a boolean check with a
    correctness proof. In the Rust implementation this is `reason()` plus
    `has_positive_match`. -/
structure Verifier (R : ReasoningOp) where
  /-- Decide whether a literal is a conclusion of a theory. -/
  check : Theory → Literal → Bool
  /-- The decision agrees with derivability. -/
  correct : ∀ T q, check T q = true ↔ R.conclusions T q

/-! ## Verified requires search -/

/-- Verify raw abduction candidates: keep exactly those whose injection
    makes the goal provable, packaging each survivor with its validity
    proof. Mirrors the verification loop of `requires_with_options`. -/
def requiresVerify (R : ReasoningOp) (V : Verifier R) (T : Theory)
    (q : Literal) (raw : List (List Literal)) :
    List (AbductionSolution R T q) :=
  raw.filterMap fun F =>
    if h : V.check (T ++ F.map Literal.toFact) q = true then
      some ⟨F, (V.correct _ _).mp h⟩
    else none

/-- **The acceptance contract**: a fact set appears among the verified
    solutions iff it was a raw candidate AND injecting it makes the goal
    provable. This is `requires.rs`'s accepted/rejected split, stated
    exactly. -/
theorem requiresVerify_facts_mem (R : ReasoningOp) (V : Verifier R)
    (T : Theory) (q : Literal) (raw : List (List Literal)) (F : List Literal) :
    (∃ s ∈ requiresVerify R V T q raw, s.facts = F) ↔
      F ∈ raw ∧ R.conclusions (T ++ F.map Literal.toFact) q := by
  constructor
  · rintro ⟨s, hs, rfl⟩
    simp only [requiresVerify, List.mem_filterMap] at hs
    obtain ⟨F', hF', hopt⟩ := hs
    split at hopt
    · rename_i h
      cases hopt
      exact ⟨hF', (V.correct _ _).mp h⟩
    · cases hopt
  · rintro ⟨hraw, hval⟩
    refine ⟨⟨F, hval⟩, ?_, rfl⟩
    simp only [requiresVerify, List.mem_filterMap]
    refine ⟨F, hraw, ?_⟩
    rw [dif_pos ((V.correct _ _).mpr hval)]

/-- **Soundness**: every solution returned by the verified search was drawn
    from the raw candidate pool, and derives the goal when injected.

    The `s.facts ∈ raw` conjunct is the load-bearing one: `requiresVerify`
    filters, it never invents. Deriving the goal, on its own, would follow
    from `s.valid` for *any* solution whatsoever and would say nothing about
    the search — see the note on oracle soundness in `Abduce.lean`. -/
theorem requiresVerify_sound (R : ReasoningOp) (V : Verifier R) (T : Theory)
    (q : Literal) (raw : List (List Literal))
    (s : AbductionSolution R T q) (hs : s ∈ requiresVerify R V T q raw) :
    s.facts ∈ raw ∧ R.conclusions (T ++ s.facts.map Literal.toFact) q :=
  (requiresVerify_facts_mem R V T q raw s.facts).mp ⟨s, hs, rfl⟩

/-- **Rejection soundness**: a raw candidate that does not appear among
    the verified solutions genuinely fails — injecting it does not make
    the goal provable. -/
theorem requiresVerify_rejected (R : ReasoningOp) (V : Verifier R)
    (T : Theory) (q : Literal) (raw : List (List Literal)) (F : List Literal)
    (hraw : F ∈ raw)
    (hout : ¬ ∃ s ∈ requiresVerify R V T q raw, s.facts = F) :
    ¬ R.conclusions (T ++ F.map Literal.toFact) q := by
  intro hval
  exact hout ((requiresVerify_facts_mem R V T q raw F).mpr ⟨hraw, hval⟩)

/-! ## Already-provable case -/

/-- If the goal is already provable in the base theory, the empty fact set
    is a verified solution — mirroring the `already_provable` short-circuit
    in `requires_with_options`. -/
def requires_already_provable (R : ReasoningOp) (T : Theory) (q : Literal)
    (hq : R.conclusions T q) : AbductionSolution R T q :=
  abduce_trivial R T q hq

/-! ## Cross-operator consistency -/

/-- A verified solution for a goal that is NOT already provable is exactly
    a what-if scenario in which the goal appears as a new conclusion:
    requires and what_if agree on the semantics of hypothetical facts.

    The `s.facts ∈ raw` conjunct keeps `hs` load-bearing. The
    `whatIfConclusion` half alone would follow from `s.valid` and `hnot` for
    any solution, verified or not. -/
theorem requires_whatIf (R : ReasoningOp) (V : Verifier R) (T : Theory)
    (q : Literal) (raw : List (List Literal)) (s : AbductionSolution R T q)
    (hs : s ∈ requiresVerify R V T q raw)
    (hnot : ¬ R.conclusions T q) :
    s.facts ∈ raw ∧ whatIfConclusion R T s.facts q :=
  let ⟨hraw, hval⟩ := requiresVerify_sound R V T q raw s hs
  ⟨hraw, hval, hnot⟩

end Spindle.Arith
