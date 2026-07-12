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
     so the derived default fuel (`t.allLiterals.length + 1`) always
     suffices — no theory-size precondition is needed.

  Note: The fixpoint stability and convergence theorems require `current.Nodup`
  (and for convergence, `current ⊆ allLiterals`). These are invariants of the
  go loops, since each step applies `dedup`. Without these hypotheses the theorems
  admit counterexamples when `current` contains duplicates.
-/
import SpindleLean.Reason
import SpindleLean.Properties.Util
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
    right; simp only [List.mem_filterMap] at h; obtain ⟨r, hrmem, hcond⟩ := h
    split at hcond
    · simp only [Option.some.injEq] at hcond; exact ⟨r, hrmem, hcond⟩
    · simp at hcond

/-- lambdaStep adds only literals that are rule heads in the theory -/
theorem lambdaStep_provenance (t : Theory) (delta current : List Literal) (l : Literal) :
    l ∈ Closure.lambdaStep t delta current → l ∈ current ∨ ∃ r ∈ t.rules, r.head = l := by
  intro h
  simp only [Closure.lambdaStep] at h
  rw [List.mem_dedup, List.mem_append] at h
  cases h with
  | inl h => exact Or.inl h
  | inr h =>
    right; simp only [List.mem_filterMap] at h; obtain ⟨r, hrmem, hcond⟩ := h
    split at hcond
    · simp only [Option.some.injEq] at hcond; exact ⟨r, hrmem, hcond⟩
    · simp at hcond

-- ═══════════════════════════════════════════════════════════════
-- Private infrastructure
-- ═══════════════════════════════════════════════════════════════

private lemma list_union_eq_append {α : Type*} [DecidableEq α] (l m : List α)
    (hl : l.Nodup) (hdisj : ∀ x ∈ l, x ∉ m) : l ∪ m = l ++ m := by
  simp only [List.union_def]
  induction l with
  | nil => simp
  | cons a t ih =>
    simp only [List.foldr]
    rw [ih (List.nodup_cons.mp hl).2 (fun x hx => hdisj x (List.mem_cons.mpr (Or.inr hx)))]
    have : a ∉ t ++ m := by
      rw [List.mem_append]; push_neg
      exact ⟨(List.nodup_cons.mp hl).1, hdisj a (List.mem_cons.mpr (Or.inl rfl))⟩
    rw [List.insert_of_not_mem this, List.cons_append]

private lemma nodup_subset_length_le {α : Type*} [DecidableEq α] (l m : List α)
    (hl : l.Nodup) (hm : m.Nodup) (hsub : l ⊆ m) : l.length ≤ m.length := by
  induction m generalizing l with
  | nil => simp only [List.subset_nil] at hsub; subst hsub; simp
  | cons a t ihm =>
    by_cases hal : a ∈ l
    · have := ihm (l.erase a) (hl.erase a) (List.nodup_cons.mp hm).2 (by
        intro x hx; cases List.mem_cons.mp (hsub (List.erase_subset hx)) with
        | inl h => subst h; exact absurd hx hl.not_mem_erase
        | inr h => exact h)
      have := List.length_erase_of_mem hal; simp [List.length_cons]; omega
    · have := ihm l hl (List.nodup_cons.mp hm).2 (by
        intro x hx; cases List.mem_cons.mp (hsub hx) with
        | inl h => subst h; exact absurd hx hal
        | inr h => exact h)
      simp [List.length_cons]; omega

private lemma step_fixpoint_pigeonhole {c target newLits : List Literal}
    (hnd : c.Nodup) (htnd : target.Nodup)
    (hsub : ∀ x ∈ c, x ∈ target) (hge : target.length ≤ c.length)
    (hnew_sub : ∀ x ∈ newLits, x ∈ target) (hnew_disj : ∀ x ∈ newLits, x ∉ c) :
    newLits = [] := by
  by_contra hne
  obtain ⟨x, hxmem⟩ := List.exists_mem_of_ne_nil newLits hne
  have hxnot := hnew_disj x hxmem
  have hxall := hnew_sub x hxmem
  have hcsub : c ⊆ target.erase x := by
    intro y hy
    have hyne : y ≠ x := fun heq => hxnot (heq ▸ hy)
    exact (List.mem_erase_of_ne hyne).2 (hsub y hy)
  have := nodup_subset_length_le c (target.erase x) hnd (htnd.erase x) hcsub
  have := List.length_erase_of_mem hxall
  have := List.length_pos_of_mem hxall
  omega

private lemma rule_head_in_allLiterals (t : Theory) (r : Rule) (h : r ∈ t.rules) :
    r.head ∈ t.allLiterals :=
  List.subset_dedup _ (List.mem_append.mpr (Or.inl (List.mem_map.mpr ⟨r, h, rfl⟩)))

-- ═══════════════════════════════════════════════════════════════
-- Delta closure
-- ═══════════════════════════════════════════════════════════════

private lemma deltaStep_nodup (t : Theory) (c : List Literal) :
    (Closure.deltaStep t c).Nodup := by unfold Closure.deltaStep; exact List.nodup_dedup _

private lemma deltaStep_newLits_disjoint (t : Theory) (c : List Literal) :
    ∀ x ∈ (t.rules.filterMap fun r =>
      if r.isDefinite && r.bodySatisfied c && !c.contains r.head then some r.head else none),
    x ∉ c := by
  intro x hx; simp only [List.mem_filterMap] at hx; obtain ⟨r, _, hcond⟩ := hx
  split at hcond
  · simp only [Option.some.injEq] at hcond; subst hcond; rename_i h
    simp only [Bool.and_eq_true, Bool.not_eq_true', List.contains_eq_any_beq] at h
    intro hmem; simp [List.any_eq_true.mpr ⟨r.head, hmem, beq_self_eq_true r.head⟩] at h
  · simp at hcond

private lemma deltaStep_structure (t : Theory) (c : List Literal) (hnd : c.Nodup) :
    Closure.deltaStep t c = c ++ (t.rules.filterMap fun r =>
      if r.isDefinite && r.bodySatisfied c && !c.contains r.head then some r.head else none).dedup := by
  unfold Closure.deltaStep; rw [List.dedup_append]
  convert list_union_eq_append c _ hnd
    (fun x hx habs => deltaStep_newLits_disjoint t c x (List.dedup_subset _ habs) hx)

private lemma deltaStep_sub_all (t : Theory) (c : List Literal) (hnd : c.Nodup)
    (hsub : ∀ x ∈ c, x ∈ t.allLiterals) :
    ∀ x ∈ Closure.deltaStep t c, x ∈ t.allLiterals := by
  intro x hx; rw [deltaStep_structure t c hnd, List.mem_append] at hx
  cases hx with
  | inl h => exact hsub x h
  | inr h =>
    have := List.dedup_subset _ h; simp only [List.mem_filterMap] at this
    obtain ⟨r, hrmem, hcond⟩ := this; split at hcond
    · simp only [Option.some.injEq] at hcond; subst hcond; exact rule_head_in_allLiterals t r hrmem
    · simp at hcond

private lemma deltaStep_strict (t : Theory) (c : List Literal) (hnd : c.Nodup)
    (h : ¬ ((Closure.deltaStep t c).length == c.length) = true) :
    c.length < (Closure.deltaStep t c).length := by
  rw [deltaStep_structure t c hnd] at h ⊢
  simp only [List.length_append, beq_iff_eq] at h ⊢; omega

private lemma deltaStep_fixpoint_when_full (t : Theory) (c : List Literal) (hnd : c.Nodup)
    (hsub : ∀ x ∈ c, x ∈ t.allLiterals) (hge : t.allLiterals.length ≤ c.length) :
    ((Closure.deltaStep t c).length == c.length) = true := by
  rw [deltaStep_structure t c hnd, beq_iff_eq, List.length_append]
  suffices h : (t.rules.filterMap fun r =>
      if r.isDefinite && r.bodySatisfied c && !c.contains r.head then some r.head else none).dedup = [] by
    rw [h]; simp
  apply step_fixpoint_pigeonhole hnd (List.nodup_dedup _) hsub hge
  · intro x hx
    have := List.dedup_subset _ hx; simp only [List.mem_filterMap] at this
    obtain ⟨r, hrmem, hcond⟩ := this; split at hcond
    · simp only [Option.some.injEq] at hcond; subst hcond; exact rule_head_in_allLiterals t r hrmem
    · simp at hcond
  · intro x hx; exact deltaStep_newLits_disjoint t c x (List.dedup_subset _ hx)

-- ═══════════════════════════════════════════════════════════════
-- Fixpoint detection is correct
-- ═══════════════════════════════════════════════════════════════

theorem fixpoint_stable_delta (t : Theory) (current : List Literal)
    (hnodup : current.Nodup)
    (hfix : (Closure.deltaStep t current).length = current.length) :
    Closure.deltaStep t current = current := by
  rw [deltaStep_structure t current hnodup] at hfix ⊢
  have : (t.rules.filterMap fun r =>
      if r.isDefinite && r.bodySatisfied current && !current.contains r.head then
        some r.head else none).dedup.length = 0 := by
    rw [List.length_append] at hfix; omega
  rw [List.eq_nil_of_length_eq_zero this, List.append_nil]

-- ═══════════════════════════════════════════════════════════════
-- Delta convergence bound
-- ═══════════════════════════════════════════════════════════════

theorem deltaClose_converges_bound (t : Theory) (current : List Literal)
    (fuel : Nat) (hfuel : t.allLiterals.length ≤ fuel)
    (hnodup : current.Nodup) (hsub : ∀ x ∈ current, x ∈ t.allLiterals) :
    Closure.deltaClose.go t current fuel =
    Closure.deltaClose.go t current (fuel + 1) := by
  suffices h : ∀ (c : List Literal) (f : Nat),
      c.Nodup → (∀ x ∈ c, x ∈ t.allLiterals) → t.allLiterals.length - c.length ≤ f →
      Closure.deltaClose.go t c f = Closure.deltaClose.go t c (f + 1) from
    h current fuel hnodup hsub (by omega)
  intro c f hnd hsc hf
  induction f generalizing c with
  | zero =>
    simp only [Closure.deltaClose.go]
    simp [deltaStep_fixpoint_when_full t c hnd hsc (by omega)]
  | succ n ih =>
    simp only [Closure.deltaClose.go]; split; · rfl
    rename_i hnotfix
    exact ih (Closure.deltaStep t c) (deltaStep_nodup t c) (deltaStep_sub_all t c hnd hsc)
      (by have := deltaStep_strict t c hnd hnotfix
          have := nodup_subset_length_le _ _ (deltaStep_nodup t c) (List.nodup_dedup _)
            (deltaStep_sub_all t c hnd hsc)
          omega)

-- ═══════════════════════════════════════════════════════════════
-- Lambda closure
-- ═══════════════════════════════════════════════════════════════

private lemma lambdaStep_nodup (t : Theory) (d c : List Literal) :
    (Closure.lambdaStep t d c).Nodup := by unfold Closure.lambdaStep; exact List.nodup_dedup _

private lemma lambdaStep_newLits_disjoint (t : Theory) (d c : List Literal) :
    ∀ x ∈ (t.rules.filterMap fun r =>
      if r.isProductive && r.bodySatisfied c && !c.contains r.head && !d.contains r.head.complement
      then some r.head else none), x ∉ c := by
  intro x hx; simp only [List.mem_filterMap] at hx; obtain ⟨r, _, hcond⟩ := hx
  split at hcond
  · simp only [Option.some.injEq] at hcond; subst hcond; rename_i h
    simp only [Bool.and_eq_true, Bool.not_eq_true', List.contains_eq_any_beq] at h
    obtain ⟨⟨⟨_, _⟩, hnotc⟩, _⟩ := h
    intro hmem; simp [List.any_eq_true.mpr ⟨r.head, hmem, beq_self_eq_true r.head⟩] at hnotc
  · simp at hcond

private lemma lambdaStep_structure (t : Theory) (d c : List Literal) (hnd : c.Nodup) :
    Closure.lambdaStep t d c = c ++ (t.rules.filterMap fun r =>
      if r.isProductive && r.bodySatisfied c && !c.contains r.head && !d.contains r.head.complement
      then some r.head else none).dedup := by
  unfold Closure.lambdaStep; rw [List.dedup_append]
  convert list_union_eq_append c _ hnd
    (fun x hx habs => lambdaStep_newLits_disjoint t d c x (List.dedup_subset _ habs) hx)

private lemma lambdaStep_sub_all (t : Theory) (d c : List Literal) (hnd : c.Nodup)
    (hsub : ∀ x ∈ c, x ∈ t.allLiterals) :
    ∀ x ∈ Closure.lambdaStep t d c, x ∈ t.allLiterals := by
  intro x hx; rw [lambdaStep_structure t d c hnd, List.mem_append] at hx
  cases hx with
  | inl h => exact hsub x h
  | inr h =>
    have := List.dedup_subset _ h; simp only [List.mem_filterMap] at this
    obtain ⟨r, hrmem, hcond⟩ := this; split at hcond
    · simp only [Option.some.injEq] at hcond; subst hcond; exact rule_head_in_allLiterals t r hrmem
    · simp at hcond

private lemma lambdaStep_strict (t : Theory) (d c : List Literal) (hnd : c.Nodup)
    (h : ¬ ((Closure.lambdaStep t d c).length == c.length) = true) :
    c.length < (Closure.lambdaStep t d c).length := by
  rw [lambdaStep_structure t d c hnd] at h ⊢
  simp only [List.length_append, beq_iff_eq] at h ⊢; omega

private lemma lambdaStep_fixpoint_when_full (t : Theory) (d c : List Literal) (hnd : c.Nodup)
    (hsub : ∀ x ∈ c, x ∈ t.allLiterals) (hge : t.allLiterals.length ≤ c.length) :
    ((Closure.lambdaStep t d c).length == c.length) = true := by
  rw [lambdaStep_structure t d c hnd, beq_iff_eq, List.length_append]
  suffices h : (t.rules.filterMap fun r =>
      if r.isProductive && r.bodySatisfied c && !c.contains r.head && !d.contains r.head.complement
      then some r.head else none).dedup = [] by rw [h]; simp
  apply step_fixpoint_pigeonhole hnd (List.nodup_dedup _) hsub hge
  · intro x hx; have := List.dedup_subset _ hx; simp only [List.mem_filterMap] at this
    obtain ⟨r, hrmem, hcond⟩ := this; split at hcond
    · simp only [Option.some.injEq] at hcond; subst hcond; exact rule_head_in_allLiterals t r hrmem
    · simp at hcond
  · intro x hx; exact lambdaStep_newLits_disjoint t d c x (List.dedup_subset _ hx)

theorem lambdaClose_converges_bound (t : Theory) (delta current : List Literal)
    (fuel : Nat) (hfuel : t.allLiterals.length ≤ fuel)
    (hnodup : current.Nodup) (hsub : ∀ x ∈ current, x ∈ t.allLiterals) :
    Closure.lambdaClose.go t delta current fuel =
    Closure.lambdaClose.go t delta current (fuel + 1) := by
  suffices h : ∀ (c : List Literal) (f : Nat),
      c.Nodup → (∀ x ∈ c, x ∈ t.allLiterals) → t.allLiterals.length - c.length ≤ f →
      Closure.lambdaClose.go t delta c f = Closure.lambdaClose.go t delta c (f + 1) from
    h current fuel hnodup hsub (by omega)
  intro c f hnd hsc hf
  induction f generalizing c with
  | zero =>
    simp only [Closure.lambdaClose.go]
    simp [lambdaStep_fixpoint_when_full t delta c hnd hsc (by omega)]
  | succ n ih =>
    simp only [Closure.lambdaClose.go]; split; · rfl
    rename_i hnotfix
    exact ih (Closure.lambdaStep t delta c) (lambdaStep_nodup t delta c)
      (lambdaStep_sub_all t delta c hnd hsc)
      (by have := lambdaStep_strict t delta c hnd hnotfix
          have := nodup_subset_length_le _ _ (lambdaStep_nodup t delta c) (List.nodup_dedup _)
            (lambdaStep_sub_all t delta c hnd hsc)
          omega)

-- ═══════════════════════════════════════════════════════════════
-- Partial closure
-- ═══════════════════════════════════════════════════════════════

private lemma partialStep_nodup (t : Theory) (d l c : List Literal) :
    (Closure.partialStep t d l c).Nodup := by unfold Closure.partialStep; exact List.nodup_dedup _

private lemma partialStep_newLits_disjoint (t : Theory) (d l c : List Literal) :
    ∀ x ∈ (l.filter fun lit => !c.contains lit && Closure.canProve t lit d l c), x ∉ c := by
  intro x hx
  simp only [List.mem_filter, Bool.and_eq_true, Bool.not_eq_true', List.contains_eq_any_beq] at hx
  obtain ⟨_, hnotc, _⟩ := hx
  intro hmem; simp [List.any_eq_true.mpr ⟨x, hmem, beq_self_eq_true x⟩] at hnotc

private lemma partialStep_structure (t : Theory) (d l c : List Literal) (hnd : c.Nodup) :
    Closure.partialStep t d l c = c ++
      (l.filter fun lit => !c.contains lit && Closure.canProve t lit d l c).dedup := by
  unfold Closure.partialStep; rw [List.dedup_append]
  convert list_union_eq_append c _ hnd
    (fun x hx habs => partialStep_newLits_disjoint t d l c x (List.dedup_subset _ habs) hx)

private lemma partialStep_sub_all (t : Theory) (d l c : List Literal) (hnd : c.Nodup)
    (hsc : ∀ x ∈ c, x ∈ t.allLiterals) (hsl : ∀ x ∈ l, x ∈ t.allLiterals) :
    ∀ x ∈ Closure.partialStep t d l c, x ∈ t.allLiterals := by
  intro x hx; rw [partialStep_structure t d l c hnd, List.mem_append] at hx
  cases hx with
  | inl h => exact hsc x h
  | inr h => have := List.dedup_subset _ h; simp only [List.mem_filter] at this; exact hsl x this.1

private lemma partialStep_strict (t : Theory) (d l c : List Literal) (hnd : c.Nodup)
    (h : ¬ ((Closure.partialStep t d l c).length == c.length) = true) :
    c.length < (Closure.partialStep t d l c).length := by
  rw [partialStep_structure t d l c hnd] at h ⊢
  simp only [List.length_append, beq_iff_eq] at h ⊢; omega

private lemma partialStep_fixpoint_when_full (t : Theory) (d l c : List Literal) (hnd : c.Nodup)
    (hsc : ∀ x ∈ c, x ∈ t.allLiterals) (hsl : ∀ x ∈ l, x ∈ t.allLiterals)
    (hge : t.allLiterals.length ≤ c.length) :
    ((Closure.partialStep t d l c).length == c.length) = true := by
  rw [partialStep_structure t d l c hnd, beq_iff_eq, List.length_append]
  suffices h : (l.filter fun lit =>
      !c.contains lit && Closure.canProve t lit d l c).dedup = [] by rw [h]; simp
  apply step_fixpoint_pigeonhole hnd (List.nodup_dedup _) hsc hge
  · intro x hx; have := List.dedup_subset _ hx
    simp only [List.mem_filter] at this; exact hsl x this.1
  · intro x hx; exact partialStep_newLits_disjoint t d l c x (List.dedup_subset _ hx)

theorem partialClose_converges_bound (t : Theory) (delta lambda current : List Literal)
    (fuel : Nat) (hfuel : t.allLiterals.length ≤ fuel)
    (hnodup : current.Nodup)
    (hsc : ∀ x ∈ current, x ∈ t.allLiterals)
    (hsl : ∀ x ∈ lambda, x ∈ t.allLiterals) :
    Closure.partialClose.go t delta lambda current fuel =
    Closure.partialClose.go t delta lambda current (fuel + 1) := by
  suffices h : ∀ (c : List Literal) (f : Nat),
      c.Nodup → (∀ x ∈ c, x ∈ t.allLiterals) →
      t.allLiterals.length - c.length ≤ f →
      Closure.partialClose.go t delta lambda c f =
      Closure.partialClose.go t delta lambda c (f + 1) from
    h current fuel hnodup hsc (by omega)
  intro c f hnd hsc' hf
  induction f generalizing c with
  | zero =>
    simp only [Closure.partialClose.go]
    simp [partialStep_fixpoint_when_full t delta lambda c hnd hsc' hsl (by omega)]
  | succ n ih =>
    simp only [Closure.partialClose.go]; split; · rfl
    rename_i hnotfix
    exact ih (Closure.partialStep t delta lambda c) (partialStep_nodup t delta lambda c)
      (partialStep_sub_all t delta lambda c hnd hsc' hsl)
      (by have := partialStep_strict t delta lambda c hnd hnotfix
          have := nodup_subset_length_le _ _ (partialStep_nodup t delta lambda c) (List.nodup_dedup _)
            (partialStep_sub_all t delta lambda c hnd hsc' hsl)
          omega)

-- ═══════════════════════════════════════════════════════════════
-- Fuel independence
-- ═══════════════════════════════════════════════════════════════

private lemma deltaClose_init_sub_all (t : Theory) :
    ∀ x ∈ (t.facts.map (·.head)).dedup, x ∈ t.allLiterals := by
  intro x hx
  have := List.dedup_subset _ hx
  simp only [List.mem_map, Theory.facts, List.mem_filter] at this
  obtain ⟨r, ⟨hrmem, _⟩, hrfl⟩ := this; subst hrfl
  exact rule_head_in_allLiterals t r hrmem

private lemma delta_go_step_eq (t : Theory) (c : List Literal) (f : Nat)
    (hnd : c.Nodup) (hsc : ∀ x ∈ c, x ∈ t.allLiterals)
    (hf : t.allLiterals.length - c.length ≤ f) :
    Closure.deltaClose.go t c f = Closure.deltaClose.go t c (f + 1) := by
  induction f generalizing c with
  | zero =>
    simp only [Closure.deltaClose.go]
    simp [deltaStep_fixpoint_when_full t c hnd hsc (by omega)]
  | succ n ih =>
    simp only [Closure.deltaClose.go]; split; · rfl
    rename_i hnotfix
    exact ih (Closure.deltaStep t c) (deltaStep_nodup t c) (deltaStep_sub_all t c hnd hsc)
      (by have := deltaStep_strict t c hnd hnotfix
          have := nodup_subset_length_le _ _ (deltaStep_nodup t c) (List.nodup_dedup _)
            (deltaStep_sub_all t c hnd hsc)
          omega)

private lemma deltaClose_go_fuel_add (t : Theory) (c : List Literal) (n : Nat)
    (hnd : c.Nodup) (hsc : ∀ x ∈ c, x ∈ t.allLiterals)
    (hf : t.allLiterals.length - c.length ≤ n) :
    ∀ k, Closure.deltaClose.go t c n = Closure.deltaClose.go t c (n + k) := by
  intro k; induction k with
  | zero => simp
  | succ k ih =>
    rw [ih, Nat.add_succ]
    exact delta_go_step_eq t c (n + k) hnd hsc (by omega)

theorem deltaClose_fuel_independent (t : Theory) (fuel₁ fuel₂ : Nat)
    (h₁ : t.allLiterals.length ≤ fuel₁)
    (h₂ : t.allLiterals.length ≤ fuel₂) :
    Closure.deltaClose.go t (t.facts.map (·.head)).dedup fuel₁ =
    Closure.deltaClose.go t (t.facts.map (·.head)).dedup fuel₂ := by
  let init := (t.facts.map (·.head)).dedup
  let n := t.allLiterals.length - init.length
  have hnd := deltaClose_init_nodup t
  have hsc := deltaClose_init_sub_all t
  have hle : init.length ≤ t.allLiterals.length :=
    nodup_subset_length_le init t.allLiterals hnd (List.nodup_dedup _) hsc
  have hn1 : n ≤ fuel₁ := by omega
  have hn2 : n ≤ fuel₂ := by omega
  have ⟨k1, hk1⟩ := Nat.le.dest hn1
  have ⟨k2, hk2⟩ := Nat.le.dest hn2
  calc Closure.deltaClose.go t init fuel₁
      = Closure.deltaClose.go t init (n + k1) := by rw [← hk1]
    _ = Closure.deltaClose.go t init n :=
        (deltaClose_go_fuel_add t init n hnd hsc (Nat.le_refl _) k1).symm
    _ = Closure.deltaClose.go t init (n + k2) :=
        deltaClose_go_fuel_add t init n hnd hsc (Nat.le_refl _) k2
    _ = Closure.deltaClose.go t init fuel₂ := by rw [← hk2]

end Properties
