/-
  SpindleLean.Properties.Subset
  Proof that Delta ⊆ Partial ⊆ Lambda.

  These are the fundamental closure containment properties.
  Delta (definite) is always a subset of Partial (defeasible),
  which is always a subset of Lambda (over-approximation).

  Strategy: prove that each closure's iteration step preserves membership
  of its input, then show containment follows from initialization.
-/
import SpindleLean.Reason
import Mathlib.Data.List.Dedup

namespace Properties

-- ═══════════════════════════════════════════════════════════════
-- Helper: membership preserved through append + dedup
-- ═══════════════════════════════════════════════════════════════

theorem mem_append_dedup_of_mem_left {α : Type} [DecidableEq α]
    {l₁ l₂ : List α} {a : α} (h : a ∈ l₁) :
    a ∈ (l₁ ++ l₂).dedup := by
  rw [List.mem_dedup]
  exact List.mem_append_left l₂ h

-- ═══════════════════════════════════════════════════════════════
-- Delta step preserves membership
-- ═══════════════════════════════════════════════════════════════

/-- deltaStep only adds elements; it never removes existing ones -/
theorem mem_deltaStep_of_mem (t : Theory) (current : List Literal) (l : Literal)
    (h : l ∈ current) : l ∈ Closure.deltaStep t current := by
  simp only [Closure.deltaStep]
  exact mem_append_dedup_of_mem_left h

/-- deltaClose.go preserves all elements of its input -/
theorem mem_deltaClose_go_of_mem (t : Theory) (current : List Literal)
    (l : Literal) (fuel : Nat) (h : l ∈ current) :
    l ∈ Closure.deltaClose.go t current fuel := by
  induction fuel generalizing current with
  | zero => exact h
  | succ n ih =>
    simp only [Closure.deltaClose.go]
    split
    · exact h
    · exact ih _ (mem_deltaStep_of_mem t current l h)

-- ═══════════════════════════════════════════════════════════════
-- Lambda step preserves membership
-- ═══════════════════════════════════════════════════════════════

/-- lambdaStep only adds elements; it never removes existing ones -/
theorem mem_lambdaStep_of_mem (t : Theory) (delta current : List Literal)
    (l : Literal) (h : l ∈ current) :
    l ∈ Closure.lambdaStep t delta current := by
  simp only [Closure.lambdaStep]
  exact mem_append_dedup_of_mem_left h

/-- lambdaClose.go preserves all elements of its input -/
theorem mem_lambdaClose_go_of_mem (t : Theory) (delta current : List Literal)
    (l : Literal) (fuel : Nat) (h : l ∈ current) :
    l ∈ Closure.lambdaClose.go t delta current fuel := by
  induction fuel generalizing current with
  | zero => exact h
  | succ n ih =>
    simp only [Closure.lambdaClose.go]
    split
    · exact h
    · exact ih _ (mem_lambdaStep_of_mem t delta current l h)

-- ═══════════════════════════════════════════════════════════════
-- Partial step preserves membership
-- ═══════════════════════════════════════════════════════════════

/-- partialStep only adds elements; it never removes existing ones -/
theorem mem_partialStep_of_mem (t : Theory) (delta lambda current : List Literal)
    (l : Literal) (h : l ∈ current) :
    l ∈ Closure.partialStep t delta lambda current := by
  simp only [Closure.partialStep]
  exact mem_append_dedup_of_mem_left h

/-- partialClose.go preserves all elements of its input -/
theorem mem_partialClose_go_of_mem (t : Theory) (delta lambda current : List Literal)
    (l : Literal) (fuel : Nat) (h : l ∈ current) :
    l ∈ Closure.partialClose.go t delta lambda current fuel := by
  induction fuel generalizing current with
  | zero => exact h
  | succ n ih =>
    simp only [Closure.partialClose.go]
    split
    · exact h
    · exact ih _ (mem_partialStep_of_mem t delta lambda current l h)

-- ═══════════════════════════════════════════════════════════════
-- Main theorems
-- ═══════════════════════════════════════════════════════════════

/-- Every literal in delta is also in partial.
    Proof: partialClose starts from delta as initial set (go t delta lambda delta fuel),
    so any element in delta is preserved through iteration. -/
theorem delta_subset_partial (t : Theory) :
    ∀ l, l ∈ Closure.deltaClose t → l ∈ Closure.partialClose t (Closure.deltaClose t) (Closure.lambdaClose t (Closure.deltaClose t)) := by
  intro l hl
  simp only [Closure.partialClose]
  exact mem_partialClose_go_of_mem t _ _ _ l 1000 hl

/-- Delta is a subset of lambda.
    Proof: lambdaClose starts from delta as initial set. -/
theorem delta_subset_lambda (t : Theory) :
    ∀ l, l ∈ Closure.deltaClose t → l ∈ Closure.lambdaClose t (Closure.deltaClose t) := by
  intro l hl
  simp only [Closure.lambdaClose]
  exact mem_lambdaClose_go_of_mem t _ _ l 1000 hl

/-- partialClose.go only contains elements from lambda, given the invariant
    that the initial current ⊆ lambda -/
theorem partial_go_subset_lambda (t : Theory) (delta lambda current : List Literal)
    (l : Literal) (fuel : Nat)
    (hinv : ∀ (x : Literal), x ∈ current → x ∈ lambda)
    (hl : l ∈ Closure.partialClose.go t delta lambda current fuel) :
    l ∈ lambda := by
  induction fuel generalizing current with
  | zero =>
    simp only [Closure.partialClose.go] at hl
    exact hinv l hl
  | succ n ih =>
    simp only [Closure.partialClose.go] at hl
    split at hl
    · exact hinv l hl
    · apply ih _ _ hl
      intro x hx
      simp only [Closure.partialStep] at hx
      rw [List.mem_dedup, List.mem_append] at hx
      cases hx with
      | inl h => exact hinv x h
      | inr h =>
        simp only [List.mem_filter] at h
        exact h.1

/-- Every literal in partial is also in lambda.
    Proof: partialStep only adds candidates from lambda (it filters lambda),
    and starts from delta ⊆ lambda. By induction, every element added
    to partial was drawn from lambda. -/
theorem partial_subset_lambda (t : Theory) :
    ∀ l, l ∈ Closure.partialClose t (Closure.deltaClose t) (Closure.lambdaClose t (Closure.deltaClose t))
       → l ∈ Closure.lambdaClose t (Closure.deltaClose t) := by
  intro l hl
  simp only [Closure.partialClose] at hl
  exact partial_go_subset_lambda t _ _ _ l 1000 (delta_subset_lambda t) hl

end Properties
