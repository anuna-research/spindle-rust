import SpindleLean.Closure.Delta
import SpindleLean.Closure.Lambda

/-!
# SpindleLean.Reason

Top-level `reason` function that composes definite and defeasible closures.

## Overview

`reason(theory)` returns the full list of conclusions (`+D`, `-D`, `+d`, `-d`)
for every literal mentioned in the theory. This mirrors the Rust `reason()`
entry point in `crates/spindle-core/src/reason.rs` and serves as the executable
specification for cross-validation via differential testing.

## Algorithm

1. Compute the **definite closure** (Delta): `+D` / `-D` conclusions via
   strict rules and facts.
2. Compute the **defeasible closure** (Lambda): `+d` / `-d` conclusions via
   defeasible rules, using `+D` results and the superiority relation.
3. Return the concatenation of both closures.

## Rust correspondence

This corresponds to the top-level `reason()` function in
`crates/spindle-core/src/reason.rs`, which calls `forward_chain_strict`
(our Delta) followed by `resolve_defeasible` (our Lambda) and collects
both sets of conclusions into a single `Vec<Conclusion>`.
-/

/-- Compute all conclusions for a defeasible theory by composing definite
    and defeasible closures.

    Returns a list containing:
    - `+D` / `-D` conclusions from the definite closure (strict rules + facts)
    - `+d` / `-d` conclusions from the defeasible closure (defeasible rules + superiority)

    The optional `fuel` parameter bounds the fixed-point iterations for both
    closures. The default (`t.rules.length`) is sufficient for any theory
    since each iteration must add at least one new conclusion. -/
def reason (t : Theory) (fuel : Nat := t.rules.length) : List Conclusion :=
  let definite := Delta.definiteClosure t fuel
  let defeasible := Lambda.defeasibleClosure t fuel
  definite ++ defeasible

namespace Reason

/-- Every conclusion produced by `reason` came from either the definite
    or defeasible closure. -/
theorem reason_mem_iff (t : Theory) (fuel : Nat) (c : Conclusion) :
    c ∈ reason t fuel ↔
    c ∈ Delta.definiteClosure t fuel ∨ c ∈ Lambda.defeasibleClosure t fuel := by
  simp [reason, List.mem_append]

/-- Every literal mentioned in the theory appears in the output of `reason`
    with at least one definite and one defeasible conclusion. -/
theorem reason_covers_definite (t : Theory) (fuel : Nat) (l : Literal)
    (hl : l ∈ Delta.allLiterals t) :
    (l ∈ (Delta.computePlusD t fuel)) ∨ (l ∈ (Delta.computeMinusD t fuel)) := by
  exact Delta.plusD_minusD_cover t fuel l hl

/-- `reason` includes all definite closure conclusions. -/
theorem definiteClosure_subset_reason (t : Theory) (fuel : Nat) (c : Conclusion)
    (h : c ∈ Delta.definiteClosure t fuel) :
    c ∈ reason t fuel := by
  exact (reason_mem_iff t fuel c).mpr (Or.inl h)

/-- `reason` includes all defeasible closure conclusions. -/
theorem defeasibleClosure_subset_reason (t : Theory) (fuel : Nat) (c : Conclusion)
    (h : c ∈ Lambda.defeasibleClosure t fuel) :
    c ∈ reason t fuel := by
  exact (reason_mem_iff t fuel c).mpr (Or.inr h)

/-- Facts yield `+D` conclusions in the output of `reason`. -/
theorem facts_in_reason (t : Theory) (r : Rule)
    (hr : r ∈ t.rules) (hfact : r.isFact = true)
    (fuel : Nat) (hfuel : fuel ≥ 1) :
    { conclusionType := .plusD, literal := r.head : Conclusion } ∈ reason t fuel := by
  apply definiteClosure_subset_reason
  simp only [Delta.definiteClosure]
  apply List.mem_append_left
  apply List.mem_map.mpr
  exact ⟨r.head, Delta.facts_in_plusD t r hr hfact fuel hfuel, rfl⟩

end Reason
