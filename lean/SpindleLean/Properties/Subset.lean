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

/-- The gated seed is a subset of delta. -/
theorem gatedDelta_subset (delta : List Literal) :
    ∀ l, l ∈ Closure.gatedDelta delta → l ∈ delta := by
  intro l hl
  exact (List.mem_filter.mp hl).1

/-- Membership in the gated seed, from delta-membership plus consistency. -/
theorem mem_gatedDelta (delta : List Literal) (l : Literal)
    (hl : l ∈ delta) (hcons : l.complement ∉ delta) :
    l ∈ Closure.gatedDelta delta := by
  simp only [Closure.gatedDelta, List.mem_filter]
  refine ⟨hl, ?_⟩
  simp only [Bool.not_eq_true']
  cases hc : delta.contains l.complement
  · rfl
  · exact absurd (List.contains_iff_mem.mp hc) hcons

/-- Every delta literal whose complement is not also in delta is in partial.
    Proof: partialClose starts from the gated delta seed
    (go t delta lambda (gatedDelta delta) fuel), so any delta-consistent
    definite literal is preserved through iteration.

    NOTE: the delta-consistency hypothesis is required — when +D l and
    +D ~l both hold, the engine deliberately withholds +d from both
    (spec condition (2); see lean/DIVERGENCES.md class 1). -/
theorem delta_subset_partial (t : Theory) :
    ∀ l, l ∈ Closure.deltaClose t →
      l.complement ∉ Closure.deltaClose t →
      l ∈ Closure.partialClose t (Closure.deltaClose t)
            (Closure.lambdaClose t (Closure.deltaClose t)) := by
  intro l hl hcons
  simp only [Closure.partialClose]
  exact mem_partialClose_go_of_mem t _ _ _ l _ (mem_gatedDelta _ l hl hcons)

/-- Delta is a subset of lambda.
    Proof: lambdaClose starts from delta as initial set. -/
theorem delta_subset_lambda (t : Theory) :
    ∀ l, l ∈ Closure.deltaClose t → l ∈ Closure.lambdaClose t (Closure.deltaClose t) := by
  intro l hl
  simp only [Closure.lambdaClose]
  exact mem_lambdaClose_go_of_mem t _ _ l _ hl

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
    and starts from gatedDelta delta ⊆ delta ⊆ lambda. By induction, every
    element added to partial was drawn from lambda. -/
theorem partial_subset_lambda (t : Theory) :
    ∀ l, l ∈ Closure.partialClose t (Closure.deltaClose t) (Closure.lambdaClose t (Closure.deltaClose t))
       → l ∈ Closure.lambdaClose t (Closure.deltaClose t) := by
  intro l hl
  simp only [Closure.partialClose] at hl
  exact partial_go_subset_lambda t _ _ _ l _
    (fun x hx => delta_subset_lambda t x (gatedDelta_subset _ x hx)) hl

end Properties
