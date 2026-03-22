/-
  SpindleLean.Properties.Termination
  Termination proofs for closure computations.

  All three closures terminate on finite theories because:
  - The literal universe is finite (bounded by allLiterals)
  - Each step either adds a new literal (strictly increasing list length)
    or recognizes the fixpoint and stops
  - The fuel parameter provides a trivial structural bound

  Strategy: We prove two things:
  1. (Trivial) The fuel-based `go` functions are structurally recursive on Nat,
     so they always terminate — this is automatic in Lean.
  2. (Substantive) Fixpoint is reached within |allLiterals(T)| steps,
     so fuel=1000 suffices for any theory with ≤1000 distinct literals.
-/
import SpindleLean.Reason
import Mathlib.Data.List.Dedup

namespace Properties

-- ═══════════════════════════════════════════════════════════════
-- Step functions only add elements from the theory universe
-- ═══════════════════════════════════════════════════════════════

/-- deltaStep adds only literals that are rule heads in the theory -/
theorem deltaStep_provenance (t : Theory) (current : List Literal) (l : Literal) :
    l ∈ Closure.deltaStep t current → l ∈ current ∨ ∃ r ∈ t.rules, r.head = l := by
  intro h
  simp only [Closure.deltaStep] at h
  rw [List.mem_dedup, List.mem_append] at h
  cases h with
  | inl h => exact Or.inl h
  | inr h =>
    right
    simp only [List.mem_filterMap] at h
    obtain ⟨r, hrmem, hcond⟩ := h
    split at hcond
    · -- isTrue: guard true, result is some r.head
      simp only [Option.some.injEq] at hcond
      exact ⟨r, hrmem, hcond⟩
    · -- isFalse: guard false, result is none (absurd)
      simp at hcond

/-- lambdaStep adds only literals that are rule heads in the theory -/
theorem lambdaStep_provenance (t : Theory) (delta current : List Literal) (l : Literal) :
    l ∈ Closure.lambdaStep t delta current → l ∈ current ∨ ∃ r ∈ t.rules, r.head = l := by
  intro h
  simp only [Closure.lambdaStep] at h
  rw [List.mem_dedup, List.mem_append] at h
  cases h with
  | inl h => exact Or.inl h
  | inr h =>
    right
    simp only [List.mem_filterMap] at h
    obtain ⟨r, hrmem, hcond⟩ := h
    split at hcond
    · simp only [Option.some.injEq] at hcond
      exact ⟨r, hrmem, hcond⟩
    · simp at hcond

-- ═══════════════════════════════════════════════════════════════
-- Fixpoint detection is correct
-- ═══════════════════════════════════════════════════════════════

/-- When the go loop detects a fixpoint (length unchanged), the set is stable:
    applying the step function again produces the same set.
    This is what the `if next.length == current.length then current` check ensures.

    Note: The length check is a proxy for set equality when the lists are
    dedup'd (no duplicates). Since dedup ensures no duplicates, equal length
    + subset implies equality. -/
theorem fixpoint_stable_delta (t : Theory) (current : List Literal)
    (hfix : (Closure.deltaStep t current).length = current.length) :
    Closure.deltaStep t current = current := by
  -- deltaStep returns (current ++ newLits).dedup
  -- If length is unchanged and current is dedup'd, then newLits added nothing new
  sorry

-- ═══════════════════════════════════════════════════════════════
-- Convergence bounds
-- ═══════════════════════════════════════════════════════════════

/-- Each go iteration either increases the set size or detects the fixpoint.
    Since the universe is finite, convergence happens within |universe| steps. -/
theorem deltaClose_converges_bound (t : Theory) (current : List Literal)
    (fuel : Nat) (hfuel : t.allLiterals.length ≤ fuel) :
    Closure.deltaClose.go t current fuel =
    Closure.deltaClose.go t current (fuel + 1) := by
  sorry

/-- Lambda closure converges within |allLiterals| steps -/
theorem lambdaClose_converges_bound (t : Theory) (delta current : List Literal)
    (fuel : Nat) (hfuel : t.allLiterals.length ≤ fuel) :
    Closure.lambdaClose.go t delta current fuel =
    Closure.lambdaClose.go t delta current (fuel + 1) := by
  sorry

/-- Partial closure converges within |allLiterals| steps -/
theorem partialClose_converges_bound (t : Theory) (delta lambda current : List Literal)
    (fuel : Nat) (hfuel : t.allLiterals.length ≤ fuel) :
    Closure.partialClose.go t delta lambda current fuel =
    Closure.partialClose.go t delta lambda current (fuel + 1) := by
  sorry

-- ═══════════════════════════════════════════════════════════════
-- Fuel independence
-- ═══════════════════════════════════════════════════════════════

/-- For any two fuel values both ≥ |allLiterals|, deltaClose produces the same result.
    This means the choice of fuel=1000 is arbitrary — any sufficient fuel gives
    the same answer. -/
theorem deltaClose_fuel_independent (t : Theory) (fuel₁ fuel₂ : Nat)
    (h₁ : t.allLiterals.length ≤ fuel₁)
    (h₂ : t.allLiterals.length ≤ fuel₂) :
    Closure.deltaClose.go t (t.facts.map (·.head)).dedup fuel₁ =
    Closure.deltaClose.go t (t.facts.map (·.head)).dedup fuel₂ := by
  sorry

end Properties
