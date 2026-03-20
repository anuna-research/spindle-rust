import SpindleLean.Closure.Delta

/-!
# SpindleLean.Closure.Lambda

Lambda closure: defeasible provability (+∂ / −∂) with ambiguity blocking.

## Overview

The defeasible closure computes which literals are *defeasibly provable* (`+d`)
and which are *defeasibly not provable* (`-d`) in a defeasible theory, given the
definite closure results (`+D` / `-D`).

## Algorithm

A literal `q` is defeasibly provable (`+d q`) when all three conditions hold:
  1. ∃r ∈ Rsd[q]: r is applicable (all body literals are in `+d`)
  2. `-D ~q` (the complement is not definitely provable)
  3. ∀s ∈ R[~q]: s is discarded, OR ∃t ∈ Rsd[q] with t applicable AND t > s

A literal `q` is defeasibly not provable (`-d q`) when any of:
  1. ∀r ∈ Rsd[q]: r is discarded
  2. `+D ~q` (the complement is definitely provable)
  3. ∃s ∈ R[~q]: s is applicable AND ∀t ∈ Rsd[q]: t is discarded OR ¬(t > s)

This module uses ambiguity blocking: when two opposing defeasible rules
are both applicable and neither is superior, both literals get `-d`.

## Rust correspondence

This corresponds to Phase 2 of `crates/spindle-core/src/reason/defeasible.rs`:
`resolve_defeasible`, which uses a worklist-driven approach. The Lean version
uses fuel-bounded iteration instead, which is equivalent for finite theories
but easier to reason about in proofs.
-/

namespace Lambda

/-- Check whether a rule `s` (an attacker for `~q`) is countered:
    either `s` is discarded, or some applicable rule `t ∈ Rsd[q]`
    is superior to `s`. -/
def attackerCountered (t : Theory) (plusd : List Literal) (minusd : List Literal)
    (s : Rule) : Bool :=
  -- Attacker is discarded if any body literal is in -d
  s.discarded minusd ||
  -- Or some applicable supporter is superior
  (t.Rsd s.head.complement).any fun supporter =>
    supporter.applicable plusd && t.sup supporter s

/-- Check whether literal `q` satisfies all three `+d` conditions:
    (1) ∃r ∈ Rsd[q] applicable, (2) -D ~q, (3) all attackers countered. -/
def canProve (t : Theory) (plusD : List Literal) (plusd minusd : List Literal)
    (q : Literal) : Bool :=
  -- Condition (1): some strict/defeasible rule for q is applicable
  let supporters := t.Rsd q
  let hasApplicable := supporters.any fun r => r.applicable plusd
  -- Condition (2): complement is not definitely provable
  let compNotDefinite := !plusD.contains q.complement
  -- Condition (3): every attacker for ~q is countered
  let attackers := t.R q.complement
  let allCountered := attackers.all fun s =>
    attackerCountered t plusd minusd s
  hasApplicable && compNotDefinite && allCountered

/-- Check whether literal `q` satisfies any `-d` disjunct:
    (1) all Rsd[q] discarded, (2) +D ~q, (3) undefeated applicable attacker. -/
def canDisprove (t : Theory) (plusD : List Literal) (plusd minusd : List Literal)
    (q : Literal) : Bool :=
  -- Disjunct (1): all strict/defeasible rules for q are discarded
  let supporters := t.Rsd q
  let allDiscarded := supporters.all fun r => r.discarded minusd
  -- Disjunct (2): complement is definitely provable
  let compDefinite := plusD.contains q.complement
  -- Disjunct (3): ∃ applicable attacker that no supporter can beat
  let attackers := t.R q.complement
  let hasUndefeatedAttacker := attackers.any fun s =>
    s.applicable plusd &&
    supporters.all fun supporter =>
      supporter.discarded minusd || !t.sup supporter s
  allDiscarded || compDefinite || hasUndefeatedAttacker

/-- One step of defeasible closure: given current `+d` and `-d` sets,
    try to prove or disprove each undecided literal. Returns updated
    `(plusd, minusd)`. -/
def lambdaStep (t : Theory) (plusD : List Literal)
    (plusd minusd : List Literal) : List Literal × List Literal :=
  let allLits := Delta.allLiterals t
  let undecided := allLits.filter fun q =>
    !plusd.contains q && !minusd.contains q
  let newPlusd := undecided.filter fun q => canProve t plusD plusd minusd q
  let newMinusd := undecided.filter fun q =>
    !newPlusd.contains q && canDisprove t plusD plusd minusd q
  let plusd' := plusd ++ newPlusd.filter fun l => !plusd.contains l
  let minusd' := minusd ++ newMinusd.filter fun l => !minusd.contains l
  (plusd', minusd')

/-- Fuel-bounded iteration of `lambdaStep` to compute the `+d`/`-d` fixed point.

    - `fuel = 0`: return current accumulator (timeout).
    - Otherwise: compute one step; if nothing changed, return (fixed point reached);
      else recurse with decremented fuel. -/
def lambdaLoop (t : Theory) (plusD : List Literal)
    (plusd minusd : List Literal) (fuel : Nat) : List Literal × List Literal :=
  match fuel with
  | 0 => (plusd, minusd)
  | fuel + 1 =>
    let (plusd', minusd') := lambdaStep t plusD plusd minusd
    if plusd' == plusd && minusd' == minusd then (plusd, minusd)
    else lambdaLoop t plusD plusd' minusd' fuel

/-- Seed the initial `+d` set from `+D`: `+D q` yields `+d q` when `-D ~q`. -/
def seedPlusd (plusD : List Literal) : List Literal :=
  plusD.filter fun q => !plusD.contains q.complement

/-- Seed the initial `-d` set: literals with `+D ~q` (condition 2 of -d),
    or `+D q` but also `+D ~q` (condition 2 fails for +d). -/
def seedMinusd (t : Theory) (plusD : List Literal) : List Literal :=
  let allLits := Delta.allLiterals t
  allLits.filter fun q =>
    -- +D ~q → -d q
    plusD.contains q.complement &&
    -- But not if already +d (i.e., not in the +D seed set with -D ~q)
    !( plusD.contains q && !plusD.contains q.complement )

/-- Compute `+d` and `-d` sets for a theory.

    Seeds from `+D`/`-D`, then iterates `lambdaStep` up to `fuel` times.
    The default fuel is the number of rules, which suffices since each
    iteration must add at least one literal to make progress. -/
def computeDefeasible (t : Theory) (fuel : Nat := t.rules.length)
    : List Literal × List Literal :=
  let plusD := Delta.computePlusD t fuel
  let initPlusd := seedPlusd plusD
  let initMinusd := seedMinusd t plusD
  lambdaLoop t plusD initPlusd initMinusd fuel

/-- Compute the set of defeasibly provable (`+d`) literals. -/
def computePlusd (t : Theory) (fuel : Nat := t.rules.length) : List Literal :=
  (computeDefeasible t fuel).1

/-- Compute the set of defeasibly not provable (`-d`) literals. -/
def computeMinusd (t : Theory) (fuel : Nat := t.rules.length) : List Literal :=
  (computeDefeasible t fuel).2

/-- The full defeasible closure: returns a list of `Conclusion` values
    containing `+d` and `-d` for every literal mentioned in the theory.
    Undecided literals at timeout get `-d`. -/
def defeasibleClosure (t : Theory) (fuel : Nat := t.rules.length) : List Conclusion :=
  let (plusd, minusd) := computeDefeasible t fuel
  let allLits := Delta.allLiterals t
  -- Any undecided literal gets -d
  let remaining := allLits.filter fun q =>
    !plusd.contains q && !minusd.contains q
  let finalMinusd := minusd ++ remaining
  let pos := plusd.map fun l => { conclusionType := .plusd, literal := l : Conclusion }
  let neg := finalMinusd.map fun l => { conclusionType := .minusd, literal := l : Conclusion }
  pos ++ neg

-- ============================================================================
-- Properties
-- ============================================================================

/-- `lambdaStep` preserves `+d` membership: proved literals stay proved. -/
theorem lambdaStep_preserves_plusd (t : Theory) (plusD plusd minusd : List Literal)
    (l : Literal) (h : l ∈ plusd) :
    l ∈ (lambdaStep t plusD plusd minusd).1 := by
  simp only [lambdaStep]
  exact List.mem_append_left _ h

/-- `lambdaStep` preserves `-d` membership: disproved literals stay disproved. -/
theorem lambdaStep_preserves_minusd (t : Theory) (plusD plusd minusd : List Literal)
    (l : Literal) (h : l ∈ minusd) :
    l ∈ (lambdaStep t plusD plusd minusd).2 := by
  simp only [lambdaStep]
  exact List.mem_append_left _ h

/-- `lambdaLoop` preserves `+d` membership. -/
theorem lambdaLoop_preserves_plusd (t : Theory) (plusD plusd minusd : List Literal)
    (fuel : Nat) (l : Literal) (h : l ∈ plusd) :
    l ∈ (lambdaLoop t plusD plusd minusd fuel).1 := by
  induction fuel generalizing plusd minusd with
  | zero => exact h
  | succ n ih =>
    simp only [lambdaLoop]
    split
    · exact h
    · have := lambdaStep_preserves_plusd t plusD plusd minusd l h
      exact ih _ _ this

/-- `lambdaLoop` preserves `-d` membership. -/
theorem lambdaLoop_preserves_minusd (t : Theory) (plusD plusd minusd : List Literal)
    (fuel : Nat) (l : Literal) (h : l ∈ minusd) :
    l ∈ (lambdaLoop t plusD plusd minusd fuel).2 := by
  induction fuel generalizing plusd minusd with
  | zero => exact h
  | succ n ih =>
    simp only [lambdaLoop]
    split
    · exact h
    · have := lambdaStep_preserves_minusd t plusD plusd minusd l h
      exact ih _ _ this

/-- `+D q` with `-D ~q` implies `+d q` (subsumption). -/
theorem plusD_subsumes_plusd (t : Theory) (fuel : Nat) (q : Literal)
    (hplusD : q ∈ Delta.computePlusD t fuel)
    (hminusD : (Delta.computePlusD t fuel).contains q.complement = false) :
    q ∈ computePlusd t fuel := by
  simp only [computePlusd, computeDefeasible]
  apply lambdaLoop_preserves_plusd
  simp only [seedPlusd]
  exact List.mem_filter.mpr ⟨hplusD, by simp only [hminusD, Bool.not_false]⟩

/-- `+d` and `-d` cover the literals: after `defeasibleClosure`, every literal
    mentioned in the theory is either `+d` or `-d`. -/
theorem plusd_minusd_cover (t : Theory) (fuel : Nat) (l : Literal)
    (hl : l ∈ Delta.allLiterals t) :
    l ∈ (defeasibleClosure t fuel).map Conclusion.literal := by
  simp only [defeasibleClosure]
  simp only [List.map_append, List.mem_append, List.map_map, Function.comp,
    List.mem_map]
  -- Case split: is l in plusd?
  by_cases hplusd : l ∈ (computeDefeasible t fuel).1
  · left; exact ⟨l, hplusd, rfl⟩
  · right
    by_cases hminusd : l ∈ (computeDefeasible t fuel).2
    · left; exact ⟨l, hminusd, rfl⟩
    · right
      refine ⟨l, ?_, rfl⟩
      simp only [List.mem_filter, Bool.and_eq_true, Bool.not_eq_true']
      exact ⟨hl, Bool.of_not_eq_true (mt List.contains_iff_mem.mp hplusd),
             Bool.of_not_eq_true (mt List.contains_iff_mem.mp hminusd)⟩

end Lambda
