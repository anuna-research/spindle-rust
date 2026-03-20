import SpindleLean.Conclusion
import SpindleLean.Theory

/-!
# SpindleLean.Closure.Delta

Delta closure: definite provability (+Δ / −Δ).

## Overview

The definite closure computes which literals are *definitely provable* (`+D`)
and which are *definitely not provable* (`-D`) in a defeasible theory, using
only facts and strict rules.

## Algorithm

Starting from facts (strict rules with empty body), we iteratively fire strict
rules whose body literals are all in the current `+D` set, adding their head to
`+D`. This repeats until no new conclusions are produced (fixed point) or fuel
is exhausted.

A literal `q` gets `-D` when there is no strict rule for `q` whose body is
entirely contained in the `+D` set.

## Rust correspondence

This corresponds to Phase 1 of `crates/spindle-core/src/reason/definite.rs`:
`forward_chain_strict`, which drains a worklist of strict-rule conclusions.
The Lean version uses fuel-bounded iteration instead of a worklist, which is
equivalent for finite theories but easier to reason about in proofs.
-/

namespace Delta

/-- Collect all literals mentioned in a theory (in rule bodies and heads). -/
def allLiterals (t : Theory) : List Literal :=
  t.rules.flatMap fun r => r.head :: r.body

/-- One step of definite closure: given the current set of definitely-proved
    literals, find all strict rules whose bodies are fully satisfied and
    collect their heads. Returns the union of the current set with any new
    conclusions. -/
def deltaStep (t : Theory) (proved : List Literal) : List Literal :=
  let newHeads := (t.strictRules.filter fun r => r.applicable proved).map Rule.head
  proved ++ newHeads.filter fun l => !proved.contains l

/-- Fuel-bounded iteration of `deltaStep` to compute the `+D` fixed point.

    - `fuel = 0`: return current accumulator (timeout).
    - Otherwise: compute one step; if nothing changed, return (fixed point reached);
      else recurse with decremented fuel. -/
def deltaLoop (t : Theory) (proved : List Literal) (fuel : Nat) : List Literal :=
  match fuel with
  | 0 => proved
  | fuel + 1 =>
    let next := deltaStep t proved
    if next == proved then proved
    else deltaLoop t next fuel

/-- Compute the set of definitely provable (`+D`) literals.

    Seeds with fact heads, then iterates `deltaStep` up to `fuel` times.
    The default fuel is the number of rules in the theory, which is sufficient
    since each iteration must add at least one new literal to make progress. -/
def computePlusD (t : Theory) (fuel : Nat := t.rules.length) : List Literal :=
  let seeds := (t.facts.map Rule.head)
  deltaLoop t seeds fuel

/-- Compute the set of definitely not provable (`-D`) literals.

    A literal `q` mentioned in the theory gets `-D` when `q ∉ +D`. -/
def computeMinusD (t : Theory) (fuel : Nat := t.rules.length) : List Literal :=
  let plusD := computePlusD t fuel
  (allLiterals t).filter fun q => !plusD.contains q

/-- The full definite closure: returns a list of `Conclusion` values
    containing both `+D` and `-D` for every literal mentioned in the theory. -/
def definiteClosure (t : Theory) (fuel : Nat := t.rules.length) : List Conclusion :=
  let plusD := computePlusD t fuel
  let minusD := computeMinusD t fuel
  let pos := plusD.map fun l => { conclusionType := .plusD, literal := l : Conclusion }
  let neg := minusD.map fun l => { conclusionType := .minusD, literal := l : Conclusion }
  pos ++ neg

-- ============================================================================
-- Properties
-- ============================================================================

/-- `deltaStep` is extensive: the output always contains the input. -/
theorem deltaStep_extensive (t : Theory) (proved : List Literal) (l : Literal)
    (h : l ∈ proved) : l ∈ deltaStep t proved := by
  simp only [deltaStep]
  exact List.mem_append_left _ h

/-- `deltaLoop` preserves membership: if `l ∈ proved`, then `l ∈ deltaLoop t proved fuel`. -/
theorem deltaLoop_preserves (t : Theory) (proved : List Literal) (fuel : Nat)
    (l : Literal) (h : l ∈ proved) : l ∈ deltaLoop t proved fuel := by
  induction fuel generalizing proved with
  | zero => exact h
  | succ n ih =>
    simp only [deltaLoop]
    split
    · exact h
    · exact ih _ (deltaStep_extensive t proved l h)

/-- Facts are always in `+D` (when fuel ≥ 1). -/
theorem facts_in_plusD (t : Theory) (r : Rule)
    (hr : r ∈ t.rules) (hfact : r.isFact = true)
    (fuel : Nat) (_hfuel : fuel ≥ 1) :
    r.head ∈ computePlusD t fuel := by
  simp only [computePlusD]
  have hseed : r.head ∈ (t.facts.map Rule.head) := by
    exact List.mem_map.mpr ⟨r, List.mem_filter.mpr ⟨hr, hfact⟩, rfl⟩
  exact deltaLoop_preserves t _ fuel _ hseed

/-- `+D` and `-D` cover the literals of the theory: every literal mentioned
    in the theory is in `+D` or `-D` (or both, in the case of duplicates in
    `allLiterals`). -/
theorem plusD_minusD_cover (t : Theory) (fuel : Nat) (l : Literal)
    (hl : l ∈ allLiterals t) :
    (l ∈ computePlusD t fuel) ∨ (l ∈ computeMinusD t fuel) := by
  by_cases h : (computePlusD t fuel).contains l
  · left
    exact List.contains_iff_mem.mp h
  · right
    simp only [computeMinusD]
    exact List.mem_filter.mpr ⟨hl, by
      simp only [Bool.not_eq_true']
      exact Bool.eq_false_iff.mpr h⟩

end Delta
