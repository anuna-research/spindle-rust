import SpindleLean.Closure.Partial
import SpindleLean.Reason

/-!
# SpindleLean.AmbiguityBlocking

Ambiguity blocking semantics for defeasible reasoning.

## Overview

In ambiguity blocking, when two opposing defeasible rules are both applicable
and neither is superior to the other, both literals get `-d` (defeasibly not
provable). This prevents ambiguous conclusions from propagating.

## Ambiguity Locality

The key property formalized here is **Ambiguity Locality**: if a literal `q`
has a supporting rule whose body does not mention any ambiguous literals, and
no rule concludes `~q`, then `q` is defeasibly provable (`+d`) regardless of
any ambiguity elsewhere in the theory.

## Test Theory

The canonical test demonstrates this with:
- `r1: => p` (defeasible, empty body)
- `r2: => ~p` (defeasible, empty body)
- `r3: => q` (defeasible, empty body)

Here `p` and `~p` are both `-d` (ambiguous), but `q` is `+d` because `r3`
has no body mentioning `p` or `~p`, and no rule concludes `~q`.
-/

namespace AmbiguityBlocking

-- ============================================================================
-- Test Theory: r1: => p, r2: => ~p, r3: => q
-- ============================================================================

/-- The canonical ambiguity blocking test theory:
    - `r1: => p` and `r2: => ~p` create an ambiguity in `p`
    - `r3: => q` should be unaffected by the `p`/`~p` conflict -/
def testTheory : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "p"),
      Rule.mkDefeasible "r2" [] (Literal.neg "p"),
      Rule.mkDefeasible "r3" [] (Literal.pos "q")
    ],
    superiority := [] }

private def hasConcl (cs : List Conclusion) (ct : ConclusionType) (l : Literal) : Bool :=
  cs.any fun c => c.conclusionType == ct && c.literal == l

-- q is +d (isolated from the p/~p ambiguity)
#guard hasConcl (reason testTheory) .plusd (Literal.pos "q")

-- p and ~p are both -d (ambiguous)
#guard hasConcl (reason testTheory) .minusd (Literal.pos "p")
#guard hasConcl (reason testTheory) .minusd (Literal.neg "p")

-- p and ~p are NOT +d
#guard !hasConcl (reason testTheory) .plusd (Literal.pos "p")
#guard !hasConcl (reason testTheory) .plusd (Literal.neg "p")

-- q is NOT -d
#guard !hasConcl (reason testTheory) .minusd (Literal.pos "q")

-- ============================================================================
-- Supporting Lemmas
-- ============================================================================

/-- A rule with an empty body is always applicable, regardless of what
    has been proved so far. -/
theorem empty_body_applicable (r : Rule) (h : r.body = []) (proved : List Literal) :
    r.applicable proved = true := by
  simp [Rule.applicable, h]

/-- If some rule in a list has an empty body, the list has an applicable rule. -/
theorem any_applicable_of_empty_body (rules : List Rule) (r : Rule)
    (hr : r ∈ rules) (hbody : r.body = []) (proved : List Literal) :
    rules.any (fun s => s.applicable proved) = true := by
  rw [List.any_eq_true]
  exact ⟨r, hr, empty_body_applicable r hbody proved⟩

-- ============================================================================
-- Core Lemma: canProve with no attackers
-- ============================================================================

/-- When there are no rules concluding `~q`, `canProve` succeeds given an
    applicable supporter and `-D ~q`. This is the core of ambiguity locality:
    ambiguity in unrelated literals cannot block `q`. -/
theorem canProve_no_attackers (t : Theory) (plusD plusd minusd : List Literal)
    (q : Literal)
    (h_applicable : (t.Rsd q).any (fun r => r.applicable plusd) = true)
    (h_compNotDef : plusD.contains q.complement = false)
    (h_no_attackers : t.R q.complement = []) :
    Lambda.canProve t plusD plusd minusd q = true := by
  simp only [Lambda.canProve, h_no_attackers, h_applicable, h_compNotDef,
    List.all_nil, Bool.not_false, Bool.and_true]

-- ============================================================================
-- Lambda step adds provable literals
-- ============================================================================

/-- If `q` is in `allLiterals`, undecided (not in `plusd` or `minusd`), and
    `canProve` returns `true`, then `q` appears in the `+d` output of
    `lambdaStep`. -/
theorem lambdaStep_adds_provable (t : Theory) (plusD plusd minusd : List Literal)
    (q : Literal)
    (h_in : q ∈ Delta.allLiterals t)
    (h_not_plusd : plusd.contains q = false)
    (h_not_minusd : minusd.contains q = false)
    (h_prove : Lambda.canProve t plusD plusd minusd q = true) :
    q ∈ (Lambda.lambdaStep t plusD plusd minusd).1 := by
  simp only [Lambda.lambdaStep]
  apply List.mem_append_right
  simp only [List.mem_filter, Bool.and_eq_true, Bool.not_eq_true']
  exact ⟨⟨⟨h_in, h_not_plusd, h_not_minusd⟩, h_prove⟩, h_not_plusd⟩

/-- If `q` is in the `+d` output of `lambdaStep`, then it is in the `+d`
    output of `lambdaLoop` with at least one fuel step. -/
theorem lambdaLoop_of_step_mem (t : Theory) (plusD plusd minusd : List Literal)
    (fuel : Nat) (q : Literal)
    (hq : q ∈ (Lambda.lambdaStep t plusD plusd minusd).1) :
    q ∈ (Lambda.lambdaLoop t plusD plusd minusd (fuel + 1)).1 := by
  simp only [Lambda.lambdaLoop]
  split
  · -- Fixed point: step result equals input
    -- step.1 == plusd means step.1 = plusd, so q ∈ plusd
    rename_i h
    simp only [Bool.and_eq_true] at h
    have h_eq := eq_of_beq h.1
    rw [h_eq] at hq
    exact hq
  · -- Not fixed point: recurse, and q is preserved
    exact Lambda.lambdaLoop_preserves_plusd _ _ _ _ fuel q hq

-- ============================================================================
-- Ambiguity Locality Theorem
-- ============================================================================

/-- **Ambiguity Locality**: if literal `q` has a supporting rule with empty
    body in `Rsd[q]`, the complement `~q` is not definitely provable, and
    no rule in the theory concludes `~q`, then `q` is defeasibly provable
    (`+d q`).

    This captures the key property of ambiguity blocking: conflicting literals
    (like an ambiguous `p`/`~p` pair) do not affect the provability of
    independent literals whose rules do not reference the conflicting atoms.

    The proof works by showing that:
    1. `canProve` succeeds for `q` in any state (empty body ⇒ always applicable)
    2. Either `q` is in the initial `+d` seed (from `+D`), or
    3. `q` gets added in the first `lambdaStep` iteration -/
theorem ambiguity_locality (t : Theory) (fuel : Nat) (q : Literal)
    (hfuel : fuel ≥ 1)
    (h_in_theory : q ∈ Delta.allLiterals t)
    (h_rule : ∃ r, r ∈ t.Rsd q ∧ r.body = [])
    (h_compNotDef : (Delta.computePlusD t fuel).contains q.complement = false)
    (h_no_attackers : t.R q.complement = []) :
    q ∈ Lambda.computePlusd t fuel := by
  simp only [Lambda.computePlusd, Lambda.computeDefeasible]
  obtain ⟨r, hr_mem, hr_body⟩ := h_rule
  -- Abbreviate the seed sets for readability
  let plusD := Delta.computePlusD t fuel
  let initPlusd := Lambda.seedPlusd plusD
  let initMinusd := Lambda.seedMinusd t plusD
  -- Case split: is q already in the initial +d seed?
  by_cases hq_seed : q ∈ initPlusd
  · -- q is in the seed (it was +D with -D ~q), so it persists through iteration
    exact Lambda.lambdaLoop_preserves_plusd _ _ _ _ fuel q hq_seed
  · -- q is not in the seed; show it gets added in the first lambdaStep
    -- First, q is also not in the initial -d seed (requires +D ~q, which is false)
    have hq_not_minusd : initMinusd.contains q = false := by
      simp only [initMinusd, Lambda.seedMinusd]
      rw [Bool.eq_false_iff]
      intro h_mem
      rw [List.contains_iff_mem] at h_mem
      simp only [List.mem_filter, Bool.and_eq_true] at h_mem
      rw [h_compNotDef] at h_mem
      simp at h_mem
    -- q is not in the initial +d seed
    have hq_not_plusd : initPlusd.contains q = false := by
      rw [Bool.eq_false_iff]
      exact mt List.contains_iff_mem.mp hq_seed
    -- canProve succeeds for q with any plusd (empty body is always applicable)
    have h_can_prove : Lambda.canProve t plusD initPlusd initMinusd q = true :=
      canProve_no_attackers t plusD initPlusd initMinusd q
        (any_applicable_of_empty_body _ r hr_mem hr_body initPlusd)
        h_compNotDef h_no_attackers
    -- q gets added in the first lambdaStep
    have h_in_step : q ∈ (Lambda.lambdaStep t plusD initPlusd initMinusd).1 :=
      lambdaStep_adds_provable t plusD initPlusd initMinusd q
        h_in_theory hq_not_plusd hq_not_minusd h_can_prove
    -- With fuel ≥ 1, lambdaLoop runs at least one step and preserves q
    obtain ⟨n, rfl⟩ := Nat.exists_eq_succ_of_ne_zero (by omega : fuel ≠ 0)
    exact lambdaLoop_of_step_mem t plusD initPlusd initMinusd n q h_in_step

end AmbiguityBlocking
