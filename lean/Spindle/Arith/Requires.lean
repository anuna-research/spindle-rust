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
  - `requiresVerify_sound`: every returned solution genuinely derives the
    goal (validity by construction)
  - `requiresVerify_facts_mem`: the acceptance contract — a raw candidate
    is returned iff injecting it makes the goal provable
  - `requiresVerify_rejected`: rejected candidates genuinely fail
  - `requires_already_provable`: an already-provable goal admits the
    empty solution
  - `requires_whatIf`: a verified solution for a not-yet-provable goal is
    exactly a what-if scenario producing the goal as a NEW conclusion
    (cross-operator consistency with `whatIfConclusion`)
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

/-- **Soundness**: every solution returned by the verified search derives
    the goal when injected. Immediate from the validity field — the model
    cannot return an unverified candidate. -/
theorem requiresVerify_sound (R : ReasoningOp) (V : Verifier R) (T : Theory)
    (q : Literal) (raw : List (List Literal))
    (s : AbductionSolution R T q) (_ : s ∈ requiresVerify R V T q raw) :
    R.conclusions (T ++ s.facts.map Literal.toFact) q :=
  s.valid

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
    requires and what_if agree on the semantics of hypothetical facts. -/
theorem requires_whatIf (R : ReasoningOp) (V : Verifier R) (T : Theory)
    (q : Literal) (raw : List (List Literal)) (s : AbductionSolution R T q)
    (hs : s ∈ requiresVerify R V T q raw)
    (hnot : ¬ R.conclusions T q) :
    whatIfConclusion R T s.facts q :=
  ⟨requiresVerify_sound R V T q raw s hs, hnot⟩

/-- Verified solutions ARE abduction solutions (requires refines abduce):
    every output of the verified search satisfies abduce's validity
    contract. Definitional, since both carry the same validity proof. -/
theorem requires_refines_abduce (R : ReasoningOp) (V : Verifier R)
    (T : Theory) (q : Literal) (raw : List (List Literal))
    (s : AbductionSolution R T q) (hs : s ∈ requiresVerify R V T q raw) :
    R.conclusions (T ++ s.facts.map Literal.toFact) q :=
  abduce_solution_valid R T q s

end Spindle.Arith
