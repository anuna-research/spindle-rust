/-
  SpindleLean.Properties.Util
  Shared proof infrastructure used across multiple property files.
-/
import SpindleLean.Reason
import Mathlib.Data.List.Dedup
import Mathlib.Data.Finset.Dedup
import Mathlib.Data.Finset.Card

namespace Properties

/-- Nodup subset with equal length: if s1 ⊆ s2, both Nodup, same length,
    but a ∈ s2 and a ∉ s1, that's a contradiction. -/
theorem nodup_subset_length_absurd {α : Type*} [DecidableEq α]
    {s1 s2 : List α} (hnd1 : s1.Nodup) (hnd2 : s2.Nodup)
    (hsub : ∀ x, x ∈ s1 → x ∈ s2) (hlen : s2.length = s1.length)
    {a : α} (ha2 : a ∈ s2) (ha1 : a ∉ s1) : False := by
  have h_sub : s1.toFinset ⊆ s2.toFinset :=
    fun x hx => List.mem_toFinset.mpr (hsub x (List.mem_toFinset.mp hx))
  have h_ne : s1.toFinset ≠ s2.toFinset := by
    intro heq
    have : a ∉ s1.toFinset := by rwa [List.mem_toFinset]
    exact this (heq ▸ List.mem_toFinset.mpr ha2)
  have h_ssubset : s1.toFinset ⊂ s2.toFinset :=
    lt_of_le_of_ne h_sub h_ne
  have h_lt := Finset.card_lt_card h_ssubset
  rw [List.toFinset_card_of_nodup hnd1, List.toFinset_card_of_nodup hnd2] at h_lt
  omega

/-- If body_satisfied holds on a subset, it holds on the superset.
    body_satisfied checks that every body literal is in the current set,
    so enlarging the set preserves satisfaction. -/
theorem bodySatisfied_mono (r : Rule) (s₁ s₂ : List Literal)
    (hsub : ∀ x, x ∈ s₁ → x ∈ s₂) (h : r.bodySatisfied s₁ = true) :
    r.bodySatisfied s₂ = true := by
  simp only [Rule.bodySatisfied] at h ⊢
  rw [List.all_eq_true] at h ⊢
  intro x hx
  have h1 := h x hx
  have hmem : x ∈ s₁ := List.mem_of_elem_eq_true h1
  have hmem2 : x ∈ s₂ := hsub x hmem
  exact List.elem_eq_true_of_mem hmem2

/-- The initial seed for deltaClose is Nodup. -/
theorem deltaClose_init_nodup (t : Theory) :
    (t.facts.map (·.head)).dedup.Nodup := List.nodup_dedup _

end Properties
