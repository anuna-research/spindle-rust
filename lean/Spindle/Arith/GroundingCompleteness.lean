/-
  Spindle Grounding Completeness

  Proves that Rule.groundInstances is complete: every ground instance of a rule
  obtained by substituting domain values for variables is present in the output.
  This ensures no applicable rules are missed during grounding.
-/
import Spindle.Arith.GroundRule

namespace Spindle.Arith

/-! ## Substitution.set / lookup interaction -/

/-- Looking up the variable just set always returns the new value. -/
theorem Substitution.set_lookup_eq (σ : Substitution) (v : String) (t : Term) :
    (σ.set v t).lookup v = some t := by
  simp only [Substitution.set, Substitution.lookup, List.lookup, beq_self_eq_true]

/-- Filtering out pairs with key `v` preserves lookup of a different key `w`. -/
private theorem lookup_filter_bne (pairs : List (String × Term)) (v w : String)
    (hne : w ≠ v) :
    (pairs.filter (fun p => p.1 != v)).lookup w = pairs.lookup w := by
  induction pairs with
  | nil => rfl
  | cons p ps ih =>
    simp only [List.filter]
    by_cases hpv : p.1 = v
    · -- p.1 = v, filtered out; w ≠ p.1, so not a lookup hit
      have hfilt : (p.1 != v) = false := by simp [bne, BEq.beq, hpv]
      have hwp : (w == p.1) = false := beq_eq_false_iff_ne.mpr (by rw [hpv]; exact hne)
      rw [hfilt]; simp only []; rw [ih]
      simp only [List.lookup, hwp]
    · -- p.1 ≠ v, kept by filter
      have hfilt : (p.1 != v) = true := by simp [bne, BEq.beq, hpv]
      rw [hfilt]; simp only []
      simp only [List.lookup]
      split
      · rfl
      · exact ih

/-- Looking up a different variable after set returns the original binding. -/
theorem Substitution.set_lookup_ne (σ : Substitution) (v w : String) (t : Term)
    (hne : w ≠ v) :
    (σ.set v t).lookup w = σ.lookup w := by
  have hwv : (w == v) = false := beq_eq_false_iff_ne.mpr hne
  simp only [Substitution.set, Substitution.lookup, List.lookup, hwv]
  exact lookup_filter_bne σ.map v w hne

/-! ## applyTerm depends only on variable lookups -/

/-- Two substitutions agreeing on a variable produce the same applyTerm. -/
theorem Substitution.applyTerm_eq_of_lookup (σ₁ σ₂ : Substitution) (v : String)
    (h : σ₁.lookup v = σ₂.lookup v) :
    σ₁.applyTerm (.variable v) = σ₂.applyTerm (.variable v) := by
  simp only [Substitution.applyTerm, h]

/-- Two substitutions agreeing on all variables produce the same applyTerms. -/
theorem Substitution.applyTerms_eq_of_agree (σ₁ σ₂ : Substitution) (ts : List Term)
    (h : ∀ v, v ∈ ts.filterMap (fun t => match t with
      | .variable v => some v | _ => none) → σ₁.lookup v = σ₂.lookup v) :
    σ₁.applyTerms ts = σ₂.applyTerms ts := by
  simp only [Substitution.applyTerms]
  induction ts with
  | nil => rfl
  | cons t ts ih =>
    simp only [List.map_cons, List.cons.injEq]
    constructor
    · cases t with
      | «variable» v =>
        apply Substitution.applyTerm_eq_of_lookup
        apply h; simp only [List.filterMap_cons]; exact .head _
      | symbol _ => rfl
      | integer _ => rfl
      | decimal _ _ => rfl
      | finiteFloat _ => rfl
    · apply ih; intro v hv; apply h
      cases t with
      | «variable» w => simp only [List.filterMap_cons] at hv ⊢; exact .tail _ hv
      | symbol _ => simp only [List.filterMap_cons] at hv ⊢; exact hv
      | integer _ => simp only [List.filterMap_cons] at hv ⊢; exact hv
      | decimal _ _ => simp only [List.filterMap_cons] at hv ⊢; exact hv
      | finiteFloat _ => simp only [List.filterMap_cons] at hv ⊢; exact hv

/-- Two substitutions agreeing on literal variables produce the same result. -/
theorem Literal.applySubst_eq_of_agree (σ₁ σ₂ : Substitution) (l : Literal)
    (h : ∀ v, v ∈ l.vars → σ₁.lookup v = σ₂.lookup v) :
    l.applySubst σ₁ = l.applySubst σ₂ := by
  simp only [Literal.applySubst]
  congr 1
  exact Substitution.applyTerms_eq_of_agree σ₁ σ₂ l.args (by
    simp only [Literal.vars] at h; exact h)

/-- Helper: body map equality from variable agreement. -/
private theorem body_map_eq_of_agree (σ₁ σ₂ : Substitution) (body : List Literal)
    (h : ∀ v, v ∈ body.flatMap Literal.vars → σ₁.lookup v = σ₂.lookup v) :
    body.map (Literal.applySubst σ₁) = body.map (Literal.applySubst σ₂) := by
  induction body with
  | nil => rfl
  | cons l ls ih =>
    simp only [List.map_cons, List.cons.injEq]
    constructor
    · apply Literal.applySubst_eq_of_agree
      intro v hv; apply h
      simp only [List.flatMap_cons, List.mem_append]; exact Or.inl hv
    · apply ih
      intro v hv; apply h
      simp only [List.flatMap_cons, List.mem_append]; exact Or.inr hv

/-- Two substitutions agreeing on rule variables produce the same result. -/
theorem Rule.applySubst_eq_of_agree (σ₁ σ₂ : Substitution) (r : Rule)
    (h : ∀ v, v ∈ r.vars → σ₁.lookup v = σ₂.lookup v) :
    r.applySubst σ₁ = r.applySubst σ₂ := by
  simp only [Rule.applySubst, Rule.mk.injEq]
  constructor
  · apply Literal.applySubst_eq_of_agree
    intro v hv; apply h
    simp only [Rule.vars, List.mem_append]; exact Or.inl hv
  · apply body_map_eq_of_agree
    intro v hv; apply h
    simp only [Rule.vars, List.mem_append]; exact Or.inr hv

/-! ## eraseDups membership lemmas -/

/-- Membership in eraseDups implies membership in the original list. -/
private theorem mem_of_mem_eraseDups {l : List String} {a : String}
    (h : a ∈ l.eraseDups) : a ∈ l :=
  match l with
  | [] => by simp [List.eraseDups] at h
  | x :: xs => by
    rw [List.eraseDups_cons] at h
    cases h with
    | head => exact .head _
    | tail _ h =>
      have hfilt := @mem_of_mem_eraseDups (xs.filter (fun b => !b == x)) a h
      exact .tail _ (List.mem_filter.mp hfilt).1
termination_by l.length
decreasing_by
  simp_wf
  have : (xs.filter (fun b => !b == x)).length ≤ xs.length := List.length_filter_le _ _
  omega

/-- Membership in the original list implies membership in eraseDups. -/
private theorem mem_eraseDups_of_mem {l : List String} {a : String}
    (h : a ∈ l) : a ∈ l.eraseDups :=
  match l with
  | [] => absurd h nofun
  | x :: xs => by
    rw [List.eraseDups_cons]
    cases h with
    | head => exact .head _
    | tail _ hmem =>
      by_cases hax : a = x
      · rw [hax]; exact .head _
      · exact .tail _ (mem_eraseDups_of_mem (List.mem_filter.mpr
          ⟨hmem, by simp [BEq.beq, hax]⟩))
termination_by l.length
decreasing_by
  simp_wf
  have : (xs.filter (fun b => !b == x)).length ≤ xs.length := List.length_filter_le _ _
  omega

/-! ## allSubstitutions completeness -/

/-- allSubstitutions contains a substitution matching any assignment of
    variables to domain values. -/
theorem allSubstitutions_complete (vars : List String) (dom : Domain)
    (f : String → Term) (hf : ∀ v, v ∈ vars → f v ∈ dom) :
    ∃ σ ∈ allSubstitutions vars dom,
      ∀ v, v ∈ vars → σ.lookup v = some (f v) := by
  induction vars with
  | nil =>
    exact ⟨Substitution.empty, by simp [allSubstitutions],
           fun _ h => absurd h nofun⟩
  | cons w ws ih =>
    have hf_ws : ∀ v, v ∈ ws → f v ∈ dom :=
      fun v hv => hf v (.tail _ hv)
    obtain ⟨σ', hσ'_mem, hσ'_agree⟩ := ih hf_ws
    have hfw : f w ∈ dom := hf w (.head _)
    refine ⟨σ'.set w (f w), ?_, ?_⟩
    · -- Membership: σ'.set w (f w) ∈ allSubstitutions (w :: ws) dom
      simp only [allSubstitutions, List.mem_flatMap, List.mem_map]
      exact ⟨f w, hfw, σ', hσ'_mem, rfl⟩
    · -- Agreement on w :: ws
      intro v hv
      cases hv with
      | head => exact Substitution.set_lookup_eq σ' w (f w)
      | tail _ hmem =>
        by_cases hvw : v = w
        · rw [hvw]; exact Substitution.set_lookup_eq σ' w (f w)
        · rw [Substitution.set_lookup_ne σ' w v (f w) hvw]
          exact hσ'_agree v hmem

/-! ## Main completeness theorem -/

/-- **Grounding completeness**: if there exists a function `f` mapping each
    variable of `r` to a term in `dom`, and a substitution `σ` agreeing with
    `f` on all variables of `r`, then `r.applySubst σ ∈ r.groundInstances dom`.

    This ensures that no relevant ground instance is missed by the grounding
    algorithm. -/
theorem Rule.groundInstances_complete (r : Rule) (dom : Domain)
    (σ : Substitution) (f : String → Term)
    (hf_dom : ∀ v, v ∈ r.vars → f v ∈ dom)
    (hf_agree : ∀ v, v ∈ r.vars → σ.lookup v = some (f v)) :
    r.applySubst σ ∈ r.groundInstances dom := by
  simp only [Rule.groundInstances]
  have hf_dedup : ∀ v, v ∈ r.vars.eraseDups → f v ∈ dom :=
    fun v hv => hf_dom v (mem_of_mem_eraseDups hv)
  obtain ⟨σ', hσ'_mem, hσ'_agree⟩ :=
    allSubstitutions_complete (r.vars.eraseDups) dom f hf_dedup
  suffices r.applySubst σ = r.applySubst σ' by
    rw [this]; exact List.mem_map_of_mem hσ'_mem
  apply Rule.applySubst_eq_of_agree
  intro v hv
  have hv_dedup : v ∈ r.vars.eraseDups := mem_eraseDups_of_mem hv
  rw [hf_agree v hv, hσ'_agree v hv_dedup]

end Spindle.Arith
